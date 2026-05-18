//! Provider adapters — wire-layer normalization to a canonical model.
//!
//! `ctx` reads the **wire**, so `LangGraph` / Pydantic AI / raw SDK are
//! identical here (the moat, `docs/PROJECT.md` §3). v1 providers:
//! Anthropic Messages + `OpenAI`-compatible Chat Completions. Parsing is
//! lenient (unknown fields ignored) — the verbatim bytes are kept
//! separately by the timeline; this is the *structured view*.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Anthropic,
    OpenAiCompat,
}

impl Provider {
    /// Detect the provider from the request path and headers — the only
    /// signals available on the wire.
    #[must_use]
    pub fn detect(path: &str, headers: &[(String, String)]) -> Option<Self> {
        if path.contains("/messages") {
            return Some(Provider::Anthropic);
        }
        if path.contains("/chat/completions") || path.contains("/responses") {
            return Some(Provider::OpenAiCompat);
        }
        let has = |name: &str| headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name));
        if has("x-api-key") || has("anthropic-version") {
            return Some(Provider::Anthropic);
        }
        None
    }
}

/// One message as it sits on the wire (role + flattened text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireMessage {
    pub role: String,
    pub text: String,
}

/// A tool/function schema loaded into the prompt (named, with its own
/// token cost — the F1 indictment will use this; F0 just models it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireTool {
    pub name: String,
    pub schema_tokens: usize,
}

/// Sampling/decoding fields tracked for C4 `param-drift` (D-014). These
/// are the request fields a framework can silently set/override and that
/// define the **determinism surface** of a turn. Surfacing them is a
/// pure FACT — `ctx` reports the value, never attributes non-determinism
/// (that is `agentlock`'s scoped framing; EXCLUDED here, evalint KILLED).
/// A single shared, ordered slice: the per-field extraction is a `for`
/// over this list (no `||` chain for a mutant to widen) and the result
/// is deterministically ordered (slice order == declaration order).
/// Covers both wire shapes — `stop_sequences` is Anthropic's spelling of
/// the `OpenAI` `stop` field; both are tracked verbatim (no
/// canonicalization that could mask a real change).
pub const SAMPLING_FIELDS: &[&str] = &[
    "temperature",
    "top_p",
    "top_k",
    "max_tokens",
    "max_completion_tokens",
    "stop",
    "stop_sequences",
    "presence_penalty",
    "frequency_penalty",
    "seed",
    "response_format",
    "tool_choice",
];

/// Extract the tracked sampling fields PRESENT in the wire body, as
/// `(field, canonical_json_value_string)` pairs in `SAMPLING_FIELDS`
/// order. A field **absent** from the body is **not** in the vec
/// (`absent ≠ a value` — the C4 spec caveat: a present→absent transition
/// is NOT a value change). The value is `Value::to_string()` (canonical
/// JSON) so `0.2` vs `0.2` compares equal and `["X"]` vs `["Y"]` differs
/// — pure verbatim value equality, no judgment. `null` is treated as
/// absence (a client sending `"temperature": null` declared no value).
fn sampling_of(v: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for &field in SAMPLING_FIELDS {
        match v.get(field) {
            None | Some(Value::Null) => {}
            Some(val) => out.push((field.to_string(), val.to_string())),
        }
    }
    out
}

/// C5 (D-015) — content-block `type` values that carry a non-text
/// (multimodal / file) payload, across BOTH wire shapes. Anthropic uses
/// `image` / `document`; `OpenAI` uses `image_url` / `input_audio` /
/// `file`. A single shared, ordered slice so the detection is a pure
/// membership test (no `||` chain for a mutant to widen) — a content
/// block whose `type` is in this set is attributed as non-text weight,
/// everything else (incl. `text`) is left to flow into `history` exactly
/// as before (ZERO history/F0 regression — C5 is purely additive).
pub const NON_TEXT_KINDS: &[&str] = &[
    "image",
    "image_url",
    "input_audio",
    "audio",
    "document",
    "file",
];

/// C5 (D-015) — one non-text content block detected on the wire. The
/// byte figure is the **EXACT wire byte length** of that block's JSON
/// (`Value::to_string().len()`) — the real weight those bytes add to the
/// request body. base64 length is NOT decoded (no base64 crate, no
/// guess): the wire bytes ARE the cost. STRICTLY PURE MEASUREMENT —
/// kind label + exact byte length; NO media token estimate (the weakest
/// tokenizer regime — omitted entirely to stay strictly pure, per the
/// C5 spec / `CONTEXT-SIGNALS-RESEARCH` §c).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonTextPart {
    /// The content block `type` verbatim (e.g. `image_url`, `image`,
    /// `input_audio`, `file`) — a member of `NON_TEXT_KINDS`.
    pub kind: String,
    /// EXACT wire byte length of this content block's JSON.
    pub bytes: usize,
}

/// C5 (D-015) — the non-text (multimodal/file) content blocks PRESENT in
/// the wire body, in message-then-block order. A block is non-text iff
/// its `type` is in `NON_TEXT_KINDS`; its `bytes` is the EXACT byte
/// length of that block's wire JSON (base64 NOT decoded — the wire bytes
/// are the real cost; no new dep). Absent (text-only) ⇒ empty vec (the
/// common case — every existing fixture stays silent, like C4's
/// `sampling`). Pure extraction, no judgment (evalint KILLED).
fn non_text_of(messages: &[Value]) -> Vec<NonTextPart> {
    let mut out = Vec::new();
    for m in messages {
        let Some(blocks) = m.get("content").and_then(Value::as_array) else {
            continue;
        };
        for b in blocks {
            let Some(kind) = b.get("type").and_then(Value::as_str) else {
                continue;
            };
            if NON_TEXT_KINDS.contains(&kind) {
                out.push(NonTextPart {
                    kind: kind.to_string(),
                    bytes: b.to_string().len(),
                });
            }
        }
    }
    out
}

/// The canonical assembled-prompt view, normalized across providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assembled {
    pub provider: Provider,
    pub model: Option<String>,
    pub system: Option<String>,
    pub messages: Vec<WireMessage>,
    pub tools: Vec<WireTool>,
    /// C4 (D-014) — tracked sampling/decoding fields PRESENT on the wire,
    /// as `(field, canonical-json-value)` pairs in `SAMPLING_FIELDS`
    /// order. ADDITIVE (D-014, mirrors C3's additive `Composition`
    /// field): a new key, never removing/reordering an existing one;
    /// serialization stays backward-compatible. Empty when the body
    /// carries none (the common case — `f1_fixture` etc. stay silent).
    pub sampling: Vec<(String, String)>,
    /// C5 (D-015) — non-text (multimodal/file) content blocks PRESENT on
    /// the wire, in message-then-block order, each with its EXACT wire
    /// byte length. ADDITIVE (mirrors C4's `sampling` exactly): a new
    /// field appended after `sampling`, never removing/reordering an
    /// existing one; serialization stays backward-compatible. Empty when
    /// the body is text-only (the common case — every existing fixture
    /// stays silent, like `sampling`).
    pub non_text: Vec<NonTextPart>,
}

// --- Defensive wire extraction ----------------------------------------
//
// We parse to `serde_json::Value` and walk it, never into rigid typed
// structs. Real OpenAI/agent clients send shapes a `#[derive]` struct
// rejects (a tool without `function`, `tools: null`, a non-string field
// somewhere, …) — that made `from_slice::<TypedReq>` return `Err`,
// which `record_request().ok()` swallowed, blinding F1 while F0/F2/F3
// (raw bytes) kept working. A `Value` walk **cannot fail on any valid
// JSON**: every field is read defensively (missing/null/wrong-type ⇒
// that field is simply absent). This closes the whole "real client
// shape breaks F1" class by construction (D-008). Pure extraction —
// no scoring/judgment (F1 stays pure measurement; evalint KILLED).

/// Flatten provider content (string, or array of typed blocks) to text,
/// without losing characters that matter for the verbatim view.
fn flatten_content(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|it| {
                it.get("text")
                    .and_then(Value::as_str)
                    .map_or_else(|| it.to_string(), ToOwned::to_owned)
            })
            .collect::<String>(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn tool_tokens(schema: &Value, name: &str) -> usize {
    let body = if schema.is_null() {
        String::new()
    } else {
        schema.to_string()
    };
    crate::tokenizer::count(name) + crate::tokenizer::count(&body)
}

fn messages_of(v: &Value) -> &[Value] {
    v.get("messages")
        .and_then(Value::as_array)
        .map_or(&[], |a| a.as_slice())
}

fn tools_of(v: &Value) -> &[Value] {
    v.get("tools")
        .and_then(Value::as_array)
        .map_or(&[], |a| a.as_slice())
}

fn role_of(m: &Value) -> String {
    m.get("role")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn content_text(m: &Value) -> String {
    flatten_content(m.get("content").unwrap_or(&Value::Null))
}

/// Parse a captured request body into the canonical `Assembled` view.
///
/// Defensive: succeeds for **any valid JSON** (the verbatim bytes are
/// kept separately by the timeline — this is the lenient *structured*
/// view, `docs/PROJECT.md` §3/§8; D-001/D-008).
///
/// # Errors
/// [`crate::Error::Adapter`] only if `body` is not valid JSON at all.
pub fn parse(provider: Provider, body: &[u8]) -> crate::Result<Assembled> {
    let v: Value = serde_json::from_slice(body)
        .map_err(|e| crate::Error::Adapter(format!("{provider:?} body not JSON: {e}")))?;
    let model = v
        .get("model")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    match provider {
        Provider::Anthropic => {
            let system = match v.get("system") {
                None | Some(Value::Null) => None,
                Some(other) => Some(flatten_content(other)),
            };
            let messages = messages_of(&v)
                .iter()
                .map(|m| WireMessage {
                    role: role_of(m),
                    text: content_text(m),
                })
                .collect();
            let tools = tools_of(&v)
                .iter()
                .filter_map(|t| {
                    let name = t.get("name").and_then(Value::as_str)?;
                    Some(WireTool {
                        name: name.to_string(),
                        schema_tokens: tool_tokens(
                            t.get("input_schema").unwrap_or(&Value::Null),
                            name,
                        ),
                    })
                })
                .collect();
            Ok(Assembled {
                provider,
                model,
                system,
                messages,
                tools,
                sampling: sampling_of(&v),
                non_text: non_text_of(messages_of(&v)),
            })
        }
        Provider::OpenAiCompat => {
            let mut system = None;
            let mut messages = Vec::new();
            for m in messages_of(&v) {
                let role = role_of(m);
                let text = content_text(m);
                if role == "system" && system.is_none() {
                    system = Some(text);
                } else {
                    messages.push(WireMessage { role, text });
                }
            }
            let tools = tools_of(&v)
                .iter()
                .filter_map(|t| {
                    let f = t.get("function")?;
                    let name = f.get("name").and_then(Value::as_str)?;
                    Some(WireTool {
                        name: name.to_string(),
                        schema_tokens: tool_tokens(
                            f.get("parameters").unwrap_or(&Value::Null),
                            name,
                        ),
                    })
                })
                .collect();
            Ok(Assembled {
                provider,
                model,
                system,
                messages,
                tools,
                sampling: sampling_of(&v),
                non_text: non_text_of(messages_of(&v)),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_anthropic_by_path() {
        assert_eq!(
            Provider::detect("/v1/messages", &[]),
            Some(Provider::Anthropic)
        );
    }

    #[test]
    fn detects_openai_by_path() {
        assert_eq!(
            Provider::detect("/v1/chat/completions", &[]),
            Some(Provider::OpenAiCompat)
        );
    }

    #[test]
    fn detects_anthropic_by_header() {
        let h = vec![("X-Api-Key".to_string(), "sk".to_string())];
        assert_eq!(
            Provider::detect("/v1/unknown", &h),
            Some(Provider::Anthropic)
        );
    }

    #[test]
    fn parses_anthropic_system_and_tools() {
        let body = br#"{"model":"claude-x","system":"be terse","messages":[{"role":"user","content":"hi"}],"tools":[{"name":"search","input_schema":{"type":"object"}}]}"#;
        let a = parse(Provider::Anthropic, body).unwrap();
        assert_eq!(a.model.as_deref(), Some("claude-x"));
        assert_eq!(a.system.as_deref(), Some("be terse"));
        assert_eq!(a.messages.len(), 1);
        assert_eq!(a.tools[0].name, "search");
        assert!(a.tools[0].schema_tokens > 0);
    }

    #[test]
    fn parses_openai_system_split_out() {
        let body = br#"{"model":"gpt","messages":[{"role":"system","content":"sys"},{"role":"user","content":"q"}]}"#;
        let a = parse(Provider::OpenAiCompat, body).unwrap();
        assert_eq!(a.system.as_deref(), Some("sys"));
        assert_eq!(a.messages.len(), 1);
        assert_eq!(a.messages[0].role, "user");
    }

    #[test]
    fn flattens_block_content() {
        let body = br#"{"messages":[{"role":"user","content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]}]}"#;
        let a = parse(Provider::OpenAiCompat, body).unwrap();
        assert_eq!(a.messages[0].text, "ab");
    }

    #[test]
    fn bad_json_is_adapter_error() {
        assert!(parse(Provider::Anthropic, b"not json").is_err());
    }
}
