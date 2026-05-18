//! The step timeline — the v1 substrate (`docs/PROJECT.md` §5).
//!
//! Everything is a view on a selected step. F0 models the spine:
//! each captured request/response pair is a step. Auth headers are
//! redacted on capture (a token must never reach the timeline or a
//! `--save` file). Serde-serializable for `--json` and snapshot tests.

use std::borrow::Cow;
use std::io::Read;

use flate2::read::{MultiGzDecoder, ZlibDecoder};
use serde::{Deserialize, Serialize};

use crate::adapter::{Assembled, Provider};

/// Header names whose value is replaced with `REDACTED` on capture.
const SENSITIVE: &[&str] = &["authorization", "x-api-key", "api-key", "cookie"];

fn redact(name: &str, value: &str) -> String {
    if SENSITIVE.iter().any(|s| name.eq_ignore_ascii_case(s)) {
        "REDACTED".to_string()
    } else {
        value.to_string()
    }
}

/// Ceiling on a *decompressed* request body. The compressed wire body is
/// already bounded by the proxy (`proxy::MAX_BODY`, 64 MiB), but gzip can
/// expand adversarially — a `ctx open` of a hostile saved session must
/// not OOM. Over-limit ⇒ treat as undecodable (keep the raw bytes ⇒
/// honest Layer-2), never silently truncate a body into the parser.
const MAX_DECOMPRESSED: usize = 64 * 1024 * 1024;

/// Transport `Content-Encoding`s decoded at the capture boundary.
/// `deflate` maps to zlib (RFC 1950) — the de-facto meaning servers/
/// clients use; a true raw-RFC1951 body has no header disambiguation
/// and no magic, so it stays honest-Layer-2 (no real request-body case
/// observed). gzip is the only encoding seen on a real LLM request in
/// the wild; zstd/br/deflate support is defensive (D-009 follow-up).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Codec {
    Gzip,
    Zlib,
    Zstd,
    Brotli,
}

/// `Content-Encoding` value → codec. A single token only: a chained
/// encoding (`gzip, br`) or `identity`/unknown ⇒ `None` ⇒ raw kept
/// (honest Layer-2 — never a half-decoded prompt to the parser).
fn header_codec(headers: &[(String, String)]) -> Option<Codec> {
    let v = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-encoding"))?
        .1
        .trim();
    match v.to_ascii_lowercase().as_str() {
        "gzip" | "x-gzip" => Some(Codec::Gzip),
        "deflate" => Some(Codec::Zlib),
        "zstd" => Some(Codec::Zstd),
        "br" => Some(Codec::Brotli),
        _ => None,
    }
}

/// Content sniff for the header-independent path (`store.rs` does not
/// persist headers; the gzip-by-magic case is the D-009 root cause).
/// Only formats with an unambiguous prefix: gzip `1f 8b`, zstd
/// `28 b5 2f fd`, zlib (`CM=8` + the RFC 1950 `%31` check). Brotli and
/// raw-deflate have no magic ⇒ header-only ⇒ otherwise honest-Layer-2.
fn magic_codec(body: &[u8]) -> Option<Codec> {
    match body {
        [0x1f, 0x8b, ..] => Some(Codec::Gzip),
        [0x28, 0xb5, 0x2f, 0xfd, ..] => Some(Codec::Zstd),
        // RFC 1950: low nibble of byte0 = 8 (deflate), and the 16-bit
        // big-endian (byte0,byte1) is a multiple of 31.
        [b0, b1, ..]
            if b0 & 0x0f == 0x08
                && b0 & 0x80 == 0
                && ((u16::from(*b0) << 8) | u16::from(*b1)) % 31 == 0 =>
        {
            Some(Codec::Zlib)
        }
        _ => None,
    }
}

/// Read `r` to end, bounded by `limit`. `take(limit + 1)`: a stream that
/// yields more than `limit` is rejected **wholesale** (raw kept) — a
/// truncated/partial prompt must never reach the structured parser
/// (that would be a fabricated decomposition). Total, panic-free.
fn bounded<R: Read>(mut r: R, body: &[u8], limit: usize) -> Cow<'_, [u8]> {
    let mut out = Vec::new();
    let cap = (limit as u64).saturating_add(1);
    match r.by_ref().take(cap).read_to_end(&mut out) {
        Ok(_) if out.len() <= limit => Cow::Owned(out),
        _ => Cow::Borrowed(body),
    }
}

/// Decompress a captured request body, bounded by `limit`. Codec is the
/// `Content-Encoding` header first (the only signal for brotli/
/// raw-deflate, present on the live `ctx run` path), then a content
/// magic sniff (header-independent — survives the unheadered post-hoc
/// path). Total and panic-free: no codec / corrupt / truncated /
/// over-limit / fallible-init all return `body` unchanged (genuine
/// opaque ⇒ legitimate Layer-2). A valid-JSON body matches no codec, so
/// clean captures are byte-identical (F0/F2/F3 unaffected) — this is
/// the D-009 fix generalized beyond gzip.
fn decode_with_limit<'b>(
    headers: &[(String, String)],
    body: &'b [u8],
    limit: usize,
) -> Cow<'b, [u8]> {
    let Some(codec) = header_codec(headers).or_else(|| magic_codec(body)) else {
        return Cow::Borrowed(body);
    };
    match codec {
        Codec::Gzip => bounded(MultiGzDecoder::new(body), body, limit),
        Codec::Zlib => bounded(ZlibDecoder::new(body), body, limit),
        Codec::Brotli => bounded(
            brotli_decompressor::Decompressor::new(body, 4096),
            body,
            limit,
        ),
        Codec::Zstd => match ruzstd::decoding::StreamingDecoder::new(body) {
            Ok(dec) => bounded(dec, body, limit),
            Err(_) => Cow::Borrowed(body),
        },
    }
}

/// Production decode at the capture boundary (`MAX_DECOMPRESSED` cap).
fn decode_request_body<'b>(headers: &[(String, String)], body: &'b [u8]) -> Cow<'b, [u8]> {
    decode_with_limit(headers, body, MAX_DECOMPRESSED)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    /// Verbatim assembled wire prompt, exactly as the agent sent it
    /// (UTF-8 lossy only for rendering; bytes are preserved on save).
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// One step of the timeline. F0: a prompt-assembled request and its
/// response. (Tool-call / tool-result first-class rows = v1.x,
/// `docs/PROJECT.md` §6 — not built in F0.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub index: usize,
    pub provider: Option<Provider>,
    pub request: CapturedRequest,
    pub response: Option<CapturedResponse>,
    pub assembled: Option<Assembled>,
    pub prompt_tokens: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timeline {
    pub steps: Vec<Step>,
}

impl Timeline {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a captured request. Headers are redacted here so a secret
    /// can never enter the timeline or a saved session.
    pub fn record_request(
        &mut self,
        method: &str,
        path: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> usize {
        // Decode a compressed wire body BEFORE the lossy String — a
        // non-UTF-8 (gzip) body would otherwise be mangled to U+FFFD,
        // blinding F1 and destroying the bytes irrecoverably (D-009).
        // Clean bodies have no gzip magic ⇒ borrowed unchanged ⇒
        // byte-identical capture (F0/F2/F3 zero-regression).
        let decoded = decode_request_body(headers, body);
        let body = String::from_utf8_lossy(&decoded).into_owned();
        let provider = Provider::detect(path, headers);
        let assembled = match provider {
            Some(p) => crate::adapter::parse(p, body.as_bytes()).ok(),
            None => None,
        };
        let prompt_tokens = assembled.as_ref().map_or_else(
            || crate::tokenizer::count(&body),
            |a| {
                crate::tokenizer::count(a.system.as_deref().unwrap_or(""))
                    + a.messages
                        .iter()
                        .map(|m| crate::tokenizer::count(&m.text))
                        .sum::<usize>()
                    + a.tools.iter().map(|t| t.schema_tokens).sum::<usize>()
            },
        );
        let index = self.steps.len();
        self.steps.push(Step {
            index,
            provider,
            request: CapturedRequest {
                method: method.to_string(),
                path: path.to_string(),
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), redact(k, v)))
                    .collect(),
                body,
            },
            response: None,
            assembled,
            prompt_tokens,
        });
        index
    }

    /// Attach the response to a previously recorded step.
    pub fn record_response(
        &mut self,
        index: usize,
        status: u16,
        headers: &[(String, String)],
        body: &[u8],
    ) {
        if let Some(step) = self.steps.get_mut(index) {
            step.response = Some(CapturedResponse {
                status,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), redact(k, v)))
                    .collect(),
                body: String::from_utf8_lossy(body).into_owned(),
            });
        }
    }

    #[must_use]
    pub fn total_prompt_tokens(&self) -> usize {
        self.steps.iter().map(|s| s.prompt_tokens).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOH: &[(String, String)] = &[];

    fn hdr(enc: &str) -> Vec<(String, String)> {
        vec![("Content-Encoding".to_string(), enc.to_string())]
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }

    fn zlib(bytes: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn decode_passthrough_when_no_codec() {
        // A valid-JSON body matches no codec (header or magic) ⇒ borrowed,
        // identical ⇒ F0/F2/F3 byte-identical (the zero-regression rule).
        let json = br#"{"model":"m","messages":[]}"#;
        let got = decode_request_body(NOH, json);
        assert!(matches!(got, Cow::Borrowed(_)), "clean body untouched");
        assert_eq!(&*got, json);
        // An unknown / chained / identity Content-Encoding ⇒ raw (never a
        // half-decoded prompt). A `br` header on non-brotli ⇒ raw, no panic.
        for enc in ["identity", "gzip, br", "snappy", ""] {
            assert_eq!(&*decode_request_body(&hdr(enc), json), json, "enc={enc:?}");
        }
        assert_eq!(&*decode_request_body(&hdr("br"), json), json);
    }

    #[test]
    fn decode_roundtrips_gzip_and_zlib_by_magic() {
        // gzip (1f 8b) and zlib (CM=8 + %31) are header-independent —
        // recovered by content alone (the unheadered post-hoc path).
        let payload = br#"{"model":"gpt","messages":[{"role":"user","content":"hello there now"}]}"#;
        assert_eq!(&*decode_request_body(NOH, &gzip(payload)), payload);
        assert_eq!(&*decode_request_body(NOH, &zlib(payload)), payload);
    }

    #[test]
    fn decode_roundtrips_real_zstd_fixture_by_magic() {
        // Real python-zstandard artifact (magic 28 b5 2f fd).
        let want = include_bytes!("../tests/fixtures/sample_payload.json");
        let zst = include_bytes!("../tests/fixtures/sample_zstd.bin");
        assert_eq!(&zst[..4], &[0x28, 0xb5, 0x2f, 0xfd], "fixture must be real zstd");
        assert_eq!(&*decode_request_body(NOH, zst), &want[..]);
    }

    #[test]
    fn decode_roundtrips_real_brotli_fixture_via_header_only() {
        // Brotli has NO magic ⇒ ONLY the Content-Encoding header can
        // trigger it (proves the header-primary path; magic alone fails).
        let want = include_bytes!("../tests/fixtures/sample_payload.json");
        let br = include_bytes!("../tests/fixtures/sample_brotli.bin");
        assert_eq!(&*decode_request_body(&hdr("br"), br), &want[..], "br via header");
        assert!(
            matches!(decode_request_body(NOH, br), Cow::Borrowed(_)),
            "brotli without the header is correctly NOT sniffed (honest Layer-2)"
        );
    }

    #[test]
    fn magic_codec_pins_each_prefix() {
        assert_eq!(magic_codec(&[0x1f, 0x8b, 0, 0]), Some(Codec::Gzip));
        assert_eq!(magic_codec(&[0x28, 0xb5, 0x2f, 0xfd]), Some(Codec::Zstd));
        // zlib: 0x78 0x9c → (0x789c % 31 == 0), CM=8 ⇒ Zlib.
        assert_eq!(magic_codec(&[0x78, 0x9c, 1, 2]), Some(Codec::Zlib));
        // CM=8 but checksum NOT a multiple of 31 ⇒ not zlib (pins %31).
        assert_eq!(magic_codec(&[0x78, 0x9d, 1, 2]), None);
        // low nibble != 8 ⇒ not zlib even if %31 holds.
        assert_eq!(magic_codec(b"{\"a\"}"), None);
        assert_eq!(magic_codec(&[]), None);
    }

    #[test]
    fn header_codec_maps_single_tokens_only() {
        for (t, c) in [
            ("gzip", Codec::Gzip),
            ("x-gzip", Codec::Gzip),
            ("DEFLATE", Codec::Zlib),
            ("zstd", Codec::Zstd),
            (" Br ", Codec::Brotli),
        ] {
            assert_eq!(header_codec(&hdr(t)), Some(c), "token {t:?}");
        }
        for t in ["identity", "gzip, br", "snappy", ""] {
            assert_eq!(header_codec(&hdr(t)), None, "token {t:?} must be None");
        }
        assert_eq!(header_codec(NOH), None);
    }

    #[test]
    fn decode_keeps_raw_on_corrupt_or_truncated() {
        // Codec-detected but garbage ⇒ undecodable ⇒ raw kept (honest
        // Layer-2), never a panic, never a partial body to the parser.
        let mut bad = gzip(b"some real body bytes here");
        bad.truncate(bad.len() / 2);
        assert_eq!(&*decode_request_body(NOH, &bad), bad.as_slice());
        assert_eq!(&*decode_request_body(&hdr("zstd"), b"\x28\xb5\x2f\xfdnope"), b"\x28\xb5\x2f\xfdnope");
        let fake = [0x1f, 0x8b, 0x00, 0x01, 0x02];
        assert_eq!(&*decode_request_body(NOH, &fake), &fake);
    }

    #[test]
    fn max_decompressed_is_the_documented_ceiling() {
        // Pins the decompression-bomb bound (kills the `*`→`+`
        // arithmetic mutants); a regression here is an OOM-bound change,
        // not a nit. `black_box` keeps it a real runtime check (not a
        // const-folded assertion) — same discipline as `proxy::MAX_BODY`.
        let actual = std::hint::black_box(MAX_DECOMPRESSED);
        assert_eq!(actual, 64 * 1024 * 1024);
    }

    #[test]
    fn decode_tiny_inputs_are_borrowed_unchanged_no_panic() {
        // Sub-magic-length / partial inputs must be returned untouched
        // without panicking (slice patterns, no index OOB).
        for b in [&b""[..], &b"{"[..], &[0x1f][..], &[0x1f, 0x8b][..], &[0x28, 0xb5][..]] {
            assert_eq!(&*decode_request_body(NOH, b), b, "tiny {b:?} verbatim");
        }
    }

    #[test]
    fn decode_rejects_over_limit_decompression_wholesale() {
        // A body that decompresses past the limit is rejected ENTIRELY
        // (raw kept) — never truncated into the parser (a truncated
        // prompt would be a fabricated decomposition). Pins the
        // attacker-`ctx open` decompression-bomb bound.
        let big = vec![b'a'; 4096];
        let gz = gzip(&big);
        assert_eq!(decode_with_limit(NOH, &gz, 64), Cow::Borrowed(gz.as_slice()));
        assert_eq!(&*decode_with_limit(NOH, &gz, 1 << 20), big.as_slice());
    }

    #[test]
    fn redacts_secrets_on_capture() {
        let mut t = Timeline::new();
        let h = vec![
            ("authorization".to_string(), "Bearer sk-secret".to_string()),
            ("content-type".to_string(), "application/json".to_string()),
        ];
        let i = t.record_request("POST", "/v1/messages", &h, b"{\"messages\":[]}");
        let got = &t.steps[i].request.headers;
        assert_eq!(got[0].1, "REDACTED");
        assert_eq!(got[1].1, "application/json");
    }

    #[test]
    fn detects_provider_and_counts_tokens() {
        let mut t = Timeline::new();
        let body =
            br#"{"model":"m","system":"hello world","messages":[{"role":"user","content":"hi"}]}"#;
        t.record_request("POST", "/v1/messages", &[], body);
        assert_eq!(t.steps[0].provider, Some(Provider::Anthropic));
        assert!(t.steps[0].prompt_tokens > 0);
        assert_eq!(t.total_prompt_tokens(), t.steps[0].prompt_tokens);
    }

    #[test]
    fn response_attaches_to_step() {
        let mut t = Timeline::new();
        let i = t.record_request("POST", "/v1/messages", &[], b"{}");
        t.record_response(i, 200, &[], b"ok");
        assert_eq!(t.steps[i].response.as_ref().unwrap().status, 200);
    }
}
