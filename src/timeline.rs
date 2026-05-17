//! The step timeline — the v1 substrate (`docs/PROJECT.md` §5).
//!
//! Everything is a view on a selected step. F0 models the spine:
//! each captured request/response pair is a step. Auth headers are
//! redacted on capture (a token must never reach the timeline or a
//! `--save` file). Serde-serializable for `--json` and snapshot tests.

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
        let body = String::from_utf8_lossy(body).into_owned();
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
