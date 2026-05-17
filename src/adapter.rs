//! Provider adapters — wire-layer normalization to a canonical model.
//!
//! `ctx` reads the **wire**, so `LangGraph` / Pydantic AI / raw SDK are
//! identical here (the moat, `docs/PROJECT.md` §3). v1 providers:
//! Anthropic Messages + `OpenAI`-compatible Chat Completions. Parsing is
//! lenient (unknown fields ignored) — the verbatim bytes are kept
//! separately by the timeline; this is the *structured view*.

use serde::{Deserialize, Serialize};

/// Accept a missing key, an explicit `null`, OR the value — all mapped
/// to `T::default()` for the first two. `#[serde(default)]` alone only
/// covers the *missing* case; real OpenAI/agent clients send explicit
/// `"tools": null` / `"messages": null` on a no-op turn, which would
/// otherwise fail the whole parse and blind F1 (the wire is lenient by
/// contract — `docs/PROJECT.md` §3/§8; D-001). Pure parsing leniency,
/// no semantics added.
fn null_or_missing_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

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

/// The canonical assembled-prompt view, normalized across providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assembled {
    pub provider: Provider,
    pub model: Option<String>,
    pub system: Option<String>,
    pub messages: Vec<WireMessage>,
    pub tools: Vec<WireTool>,
}

// --- Lenient wire shapes (only the fields we read) --------------------

#[derive(Debug, Deserialize)]
struct AnthropicReq {
    model: Option<String>,
    #[serde(default)]
    system: serde_json::Value,
    #[serde(default, deserialize_with = "null_or_missing_as_default")]
    messages: Vec<RawMessage>,
    #[serde(default, deserialize_with = "null_or_missing_as_default")]
    tools: Vec<AnthropicTool>,
}

#[derive(Debug, Deserialize)]
struct AnthropicTool {
    name: String,
    #[serde(default)]
    input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenAiReq {
    model: Option<String>,
    #[serde(default, deserialize_with = "null_or_missing_as_default")]
    messages: Vec<RawMessage>,
    #[serde(default, deserialize_with = "null_or_missing_as_default")]
    tools: Vec<OpenAiTool>,
}

#[derive(Debug, Deserialize)]
struct OpenAiTool {
    #[serde(default)]
    function: OpenAiFn,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiFn {
    #[serde(default)]
    name: String,
    #[serde(default)]
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(default)]
    role: String,
    #[serde(default)]
    content: serde_json::Value,
}

/// Flatten provider content (string, or array of typed blocks) to text,
/// without losing characters that matter for the verbatim view.
fn flatten_content(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(|it| {
                it.get("text")
                    .and_then(serde_json::Value::as_str)
                    .map_or_else(|| it.to_string(), ToOwned::to_owned)
            })
            .collect::<String>(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn tool_tokens(schema: &serde_json::Value, name: &str) -> usize {
    let body = if schema.is_null() {
        String::new()
    } else {
        schema.to_string()
    };
    crate::tokenizer::count(name) + crate::tokenizer::count(&body)
}

/// Parse a captured request body into the canonical `Assembled` view.
///
/// # Errors
/// Returns [`crate::Error::Adapter`] if the body is not valid JSON for
/// the detected provider shape.
pub fn parse(provider: Provider, body: &[u8]) -> crate::Result<Assembled> {
    match provider {
        Provider::Anthropic => {
            let r: AnthropicReq = serde_json::from_slice(body)
                .map_err(|e| crate::Error::Adapter(format!("anthropic body: {e}")))?;
            let system = match &r.system {
                serde_json::Value::Null => None,
                other => Some(flatten_content(other)),
            };
            Ok(Assembled {
                provider,
                model: r.model,
                system,
                messages: r
                    .messages
                    .iter()
                    .map(|m| WireMessage {
                        role: m.role.clone(),
                        text: flatten_content(&m.content),
                    })
                    .collect(),
                tools: r
                    .tools
                    .iter()
                    .map(|t| WireTool {
                        name: t.name.clone(),
                        schema_tokens: tool_tokens(&t.input_schema, &t.name),
                    })
                    .collect(),
            })
        }
        Provider::OpenAiCompat => {
            let r: OpenAiReq = serde_json::from_slice(body)
                .map_err(|e| crate::Error::Adapter(format!("openai body: {e}")))?;
            let mut system = None;
            let mut messages = Vec::new();
            for m in &r.messages {
                let text = flatten_content(&m.content);
                if m.role == "system" && system.is_none() {
                    system = Some(text);
                } else {
                    messages.push(WireMessage {
                        role: m.role.clone(),
                        text,
                    });
                }
            }
            Ok(Assembled {
                provider,
                model: r.model,
                system,
                messages,
                tools: r
                    .tools
                    .iter()
                    .map(|t| WireTool {
                        name: t.function.name.clone(),
                        schema_tokens: tool_tokens(&t.function.parameters, &t.function.name),
                    })
                    .collect(),
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
