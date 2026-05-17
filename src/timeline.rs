//! The step timeline — the v1 substrate (`docs/PROJECT.md` §5).
//!
//! Everything is a view on a selected step. F0 models the spine:
//! each captured request/response pair is a step. Auth headers are
//! redacted on capture (a token must never reach the timeline or a
//! `--save` file). Serde-serializable for `--json` and snapshot tests.

use std::borrow::Cow;
use std::io::Read;

use flate2::read::MultiGzDecoder;
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

/// Decompress a captured request body when it is a gzip stream, bounded
/// by `limit`. The trigger is the RFC 1952 magic (`1f 8b`), **not** the
/// `Content-Encoding` header — request headers are not persisted
/// (`store.rs`), so a saved session must stay decodable by content
/// alone (this is the D-009 root cause: real clients gzip the request;
/// the pre-fix `from_utf8_lossy` destroyed those bytes before parse).
/// Total and panic-free: not-gzip / corrupt / truncated / over-limit all
/// return `body` unchanged (genuine opaque ⇒ legitimate Layer-2). A
/// valid-JSON body never starts with `1f 8b`, so clean captures are
/// byte-identical (F0/F2/F3 unaffected).
fn decode_with_limit(body: &[u8], limit: usize) -> Cow<'_, [u8]> {
    if body.len() < 2 || body[0] != 0x1f || body[1] != 0x8b {
        return Cow::Borrowed(body);
    }
    let mut out = Vec::new();
    // `take(limit + 1)`: if the stream yields more than `limit` the body
    // is rejected wholesale — a truncated/partial prompt must never reach
    // the structured parser (that would be a fabricated decomposition).
    let cap = (limit as u64).saturating_add(1);
    let mut bounded = MultiGzDecoder::new(body).take(cap);
    match bounded.read_to_end(&mut out) {
        Ok(_) if out.len() <= limit => Cow::Owned(out),
        _ => Cow::Borrowed(body),
    }
}

/// Production decode at the capture boundary (`MAX_DECOMPRESSED` cap).
fn decode_request_body(body: &[u8]) -> Cow<'_, [u8]> {
    decode_with_limit(body, MAX_DECOMPRESSED)
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
        let decoded = decode_request_body(body);
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

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(bytes).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn decode_passthrough_when_not_gzip() {
        // A valid-JSON body never starts with 1f 8b ⇒ borrowed, identical
        // ⇒ F0/F2/F3 byte-identical (the zero-regression guarantee).
        let json = br#"{"model":"m","messages":[]}"#;
        let got = decode_request_body(json);
        assert!(matches!(got, Cow::Borrowed(_)), "clean body must be untouched");
        assert_eq!(&*got, json);
    }

    #[test]
    fn decode_roundtrips_real_gzip() {
        let payload = br#"{"model":"gpt","messages":[{"role":"user","content":"hello there"}]}"#;
        let gz = gzip(payload);
        let got = decode_request_body(&gz);
        assert_eq!(&*got, payload, "gzip body must decompress to the original");
    }

    #[test]
    fn decode_keeps_raw_on_corrupt_or_truncated_gzip() {
        // gzip magic but garbage after ⇒ undecodable ⇒ raw kept (honest
        // Layer-2), never a panic, never a partial body to the parser.
        let mut bad = gzip(b"some real body bytes");
        bad.truncate(bad.len() / 2);
        let got = decode_request_body(&bad);
        assert_eq!(&*got, bad.as_slice(), "corrupt gzip ⇒ raw bytes kept");
        let fake = [0x1f, 0x8b, 0x00, 0x01, 0x02];
        assert_eq!(&*decode_request_body(&fake), &fake);
    }

    #[test]
    fn decode_rejects_over_limit_decompression_wholesale() {
        // A body that decompresses past the limit is rejected ENTIRELY
        // (raw kept) — never truncated into the parser (a truncated
        // prompt would be a fabricated decomposition). Pins the
        // attacker-`ctx open` decompression-bomb bound.
        let big = vec![b'a'; 4096];
        let gz = gzip(&big);
        assert_eq!(decode_with_limit(&gz, 64), Cow::Borrowed(gz.as_slice()));
        assert_eq!(&*decode_with_limit(&gz, 1 << 20), big.as_slice());
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
