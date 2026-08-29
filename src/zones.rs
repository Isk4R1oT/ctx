//! F4 — the assembled context split into semantic zones.
//!
//! `view` prints the wire body byte-exact: correct, and unreadable. A
//! token count per component is readable, and answers nothing — knowing
//! that tools cost 39% does not tell you *which* text is sitting there.
//! This module answers the question the bytes hide: **which part of the
//! context is which**, over the full content, and records for every span
//! WHY it landed in its zone.
//!
//! Two confidence levels, never conflated:
//!
//! - [`Evidence::Structural`] — the wire format itself decides. A `tools[]`
//!   entry *is* a tool definition; an `OpenAI` `role: "tool"` message *is* a
//!   tool result. Correct for any well-formed body.
//! - [`Evidence::Marker`] — a textual marker inside a message named the zone.
//!   Correct only when the marker is present. A framework that injects
//!   silently is invisible here, and the renderer must say so rather than
//!   imply the map is complete.
//!
//! The structural pass alone already corrects a lie every role-based viewer
//! tells: Anthropic carries tool results **inside a `role: "user"` message**
//! (`type: "tool_result"` blocks), so reading roles literally attributes
//! machine output to the human — and the biggest zone in a long agent run
//! gets filed under "what the user said".

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::adapter::Provider;
use crate::tokenizer::count;

/// A semantic region of the assembled context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Zone {
    /// Developer-authored standing instructions.
    Instructions,
    /// Content pulled in by the harness: memory files, retrieved documents.
    Memory,
    /// Tool/function definitions loaded into the prompt.
    Tools,
    /// What the human actually typed.
    Input,
    /// The model's own prose.
    Output,
    /// The model's tool invocations.
    ToolCall,
    /// What tools returned to the model.
    ToolResult,
    /// Blocks the framework inserted that nobody typed.
    Injected,
}

impl Zone {
    /// Stable lowercase name — used by the renderer and the `--json` contract.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Instructions => "instructions",
            Self::Memory => "memory",
            Self::Tools => "tools",
            Self::Input => "input",
            Self::Output => "output",
            Self::ToolCall => "tool-call",
            Self::ToolResult => "tool-result",
            Self::Injected => "injected",
        }
    }

    /// Declaration order — the renderer groups by this, not by wire order,
    /// so the same zone never appears twice in a summary.
    pub const ALL: &'static [Self] = &[
        Self::Instructions,
        Self::Memory,
        Self::Tools,
        Self::Input,
        Self::Output,
        Self::ToolCall,
        Self::ToolResult,
        Self::Injected,
    ];
}

/// Why a span carries its zone. Kept per-span so the classification is
/// auditable: a reader can see the marker that produced a `Memory` label
/// and disagree with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Evidence {
    /// The wire format places this content here. Always correct.
    Structural,
    /// This textual marker named the zone. Correct only where present.
    Marker(String),
}

/// One contiguous region of the assembled context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub zone: Zone,
    /// Where it sits on the wire, e.g. `system`, `message 4 (user)`,
    /// `tool: lookup_invoice`.
    pub label: String,
    pub text: String,
    pub tokens: usize,
    pub evidence: Evidence,
}

/// Textual markers that reveal, inside one message, content the framework
/// inserted rather than the human typing it.
///
/// Each entry is `(marker, zone)`. A marker written as an opening XML-ish
/// tag (`<foo>`) claims text through its matching `</foo>`; any other
/// marker claims text up to the next marker or the end of the message.
///
/// The list is deliberately short. A marker earns its place only when the
/// injected block is *self-labelling* on the wire — these two are, and both
/// were read off a live captured context, not inferred from a changelog. A
/// framework that splices text in silently leaves no marker to find, and
/// guessing at one would trade a true "unknown" for a confident wrong
/// answer; the renderer states the marked count instead, so a reader can
/// see how much of the map is heuristic.
pub const MARKERS: &[(&str, Zone)] = &[
    ("<system-reminder>", Zone::Injected),
    ("# claudeMd", Zone::Memory),
];

/// Closing tag for an opening XML-ish marker, else `None`.
fn close_tag(marker: &str) -> Option<String> {
    let inner = marker.strip_prefix('<')?.strip_suffix('>')?;
    if inner.is_empty() || inner.starts_with('/') {
        return None;
    }
    Some(format!("</{inner}>"))
}

/// Split one message body on [`MARKERS`], attributing unmarked text to
/// `base`. Pure: returns spans, never mutates its input.
fn split_marked(text: &str, base: Zone, label: &str) -> Vec<Span> {
    let mut hits: Vec<(usize, &str, Zone)> = Vec::new();
    for &(marker, zone) in MARKERS {
        let mut from = 0;
        while let Some(rel) = text[from..].find(marker) {
            let at = from + rel;
            hits.push((at, marker, zone));
            from = at + marker.len();
        }
    }
    hits.sort_by_key(|&(at, _, _)| at);

    let mut spans = Vec::new();
    let mut cursor = 0;
    for (at, marker, zone) in hits {
        if at < cursor {
            continue; // already swallowed by an enclosing marked region
        }
        push_span(&mut spans, base, label, &text[cursor..at], Evidence::Structural);
        let body_from = at;
        let end = close_tag(marker)
            .and_then(|c| text[body_from..].find(&c).map(|r| body_from + r + c.len()))
            .unwrap_or(text.len());
        push_span(
            &mut spans,
            zone,
            label,
            &text[body_from..end],
            Evidence::Marker(marker.to_string()),
        );
        cursor = end;
    }
    push_span(&mut spans, base, label, &text[cursor..], Evidence::Structural);
    spans
}

/// Append a span, dropping whitespace-only regions (a marker at position 0
/// would otherwise emit an empty `Instructions` span before it).
fn push_span(out: &mut Vec<Span>, zone: Zone, label: &str, text: &str, ev: Evidence) {
    if text.trim().is_empty() {
        return;
    }
    out.push(Span {
        zone,
        label: label.to_string(),
        text: text.to_string(),
        tokens: count(text),
        evidence: ev,
    });
}

/// Flatten a content field (string, or array of typed blocks) to the text
/// a reader needs to see, keeping non-text blocks named rather than
/// silently dropped.
fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| match b.get("type").and_then(Value::as_str) {
                Some("text") => b.get("text").and_then(Value::as_str).map(String::from),
                Some(other) => Some(format!("[{other}]")),
                None => b.as_str().map(String::from),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Tool definitions, whichever wire shape carries them. The whole
/// definition object is the content: `description` is usually longer than
/// the schema, and it is what actually occupies the window.
fn tool_spans(root: &Value, out: &mut Vec<Span>) {
    let Some(tools) = root.get("tools").and_then(Value::as_array) else {
        return;
    };
    for t in tools {
        // OpenAI nests under `function`; Anthropic is flat.
        let def = t.get("function").unwrap_or(t);
        let name = def
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("(unnamed)");
        let text = serde_json::to_string_pretty(t).unwrap_or_else(|_| t.to_string());
        out.push(Span {
            zone: Zone::Tools,
            label: format!("tool: {name}"),
            tokens: count(&text),
            text,
            evidence: Evidence::Structural,
        });
    }
}

/// Anthropic keeps the system prompt in a top-level `system` field, not in
/// `messages`.
fn system_spans(root: &Value, out: &mut Vec<Span>) {
    if let Some(sys) = root.get("system") {
        let text = text_of(sys);
        out.extend(split_marked(&text, Zone::Instructions, "system"));
    }
}

/// Structural zone for a whole message, by role. `None` ⇒ the message
/// needs per-block treatment (its blocks disagree with its role).
fn zone_of_role(role: &str) -> Zone {
    match role {
        "system" | "developer" => Zone::Instructions,
        "assistant" => Zone::Output,
        "tool" | "function" => Zone::ToolResult,
        _ => Zone::Input,
    }
}

/// Blocks whose `type` overrides the message's role. This is where the
/// role-based reading breaks: Anthropic files tool results under `user`
/// and tool calls under `assistant`, so the role is not the zone.
fn zone_of_block(kind: &str) -> Option<Zone> {
    match kind {
        "tool_result" => Some(Zone::ToolResult),
        "tool_use" => Some(Zone::ToolCall),
        _ => None,
    }
}

fn message_spans(idx: usize, m: &Value, out: &mut Vec<Span>) {
    let role = m.get("role").and_then(Value::as_str).unwrap_or("user");
    let label = format!("message {idx} ({role})");
    let base = zone_of_role(role);

    // OpenAI carries the model's tool calls in a sibling field, not in content.
    if let Some(calls) = m.get("tool_calls").and_then(Value::as_array) {
        for c in calls {
            let text = serde_json::to_string_pretty(c).unwrap_or_else(|_| c.to_string());
            out.push(Span {
                zone: Zone::ToolCall,
                label: label.clone(),
                tokens: count(&text),
                text,
                evidence: Evidence::Structural,
            });
        }
    }

    match m.get("content") {
        Some(Value::Array(blocks)) => {
            for b in blocks {
                let kind = b.get("type").and_then(Value::as_str).unwrap_or("text");
                match zone_of_block(kind) {
                    Some(z) => {
                        let text =
                            serde_json::to_string_pretty(b).unwrap_or_else(|_| b.to_string());
                        out.push(Span {
                            zone: z,
                            label: label.clone(),
                            tokens: count(&text),
                            text,
                            evidence: Evidence::Structural,
                        });
                    }
                    None => out.extend(split_marked(&text_of(b), base, &label)),
                }
            }
        }
        Some(v) => out.extend(split_marked(&text_of(v), base, &label)),
        None => {}
    }
}

/// Split an assembled wire body into zoned spans, in wire order.
///
/// `provider` is accepted for symmetry with [`crate::adapter::parse`] and
/// future per-provider rules; the current split is shape-driven and handles
/// both wire formats from the body alone.
///
/// # Errors
/// Propagates a JSON parse failure of the captured body.
pub fn split(provider: Provider, body: &[u8]) -> crate::Result<Vec<Span>> {
    let _ = provider;
    let root: Value = serde_json::from_slice(body)?;
    let mut out = Vec::new();
    system_spans(&root, &mut out);
    tool_spans(&root, &mut out);
    if let Some(msgs) = root.get("messages").and_then(Value::as_array) {
        for (i, m) in msgs.iter().enumerate() {
            message_spans(i, m, &mut out);
        }
    }
    Ok(out)
}

/// Total tokens per zone, in [`Zone::ALL`] order. Zones with no content are
/// omitted — an empty zone is not a finding.
#[must_use]
pub fn totals(spans: &[Span]) -> Vec<(Zone, usize)> {
    Zone::ALL
        .iter()
        .filter_map(|&z| {
            let t: usize = spans.iter().filter(|s| s.zone == z).map(|s| s.tokens).sum();
            (t > 0).then_some((z, t))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The load-bearing structural claim: Anthropic carries tool results
    /// inside a `role: "user"` message, so a role-literal reading files
    /// machine output under the human. The label keeps the wire role so the
    /// correction stays auditable.
    #[test]
    fn anthropic_tool_result_is_not_user_input() {
        let body = br#"{"messages":[
            {"role":"user","content":"why?"},
            {"role":"user","content":[
                {"type":"tool_result","tool_use_id":"t1","content":"42"}]}]}"#;
        let spans = split(Provider::Anthropic, body).unwrap();
        let result: Vec<&Span> = spans.iter().filter(|s| s.zone == Zone::ToolResult).collect();
        assert_eq!(result.len(), 1, "the tool_result block must leave the input zone");
        assert!(result[0].label.contains("(user)"), "the wire role stays visible");
        let input: Vec<&Span> = spans.iter().filter(|s| s.zone == Zone::Input).collect();
        assert_eq!(input.len(), 1, "only the real question is input");
        assert!(input[0].text.contains("why?"));
    }

    /// The `OpenAI` shape puts the same content in a dedicated role and the
    /// model's calls in a sibling field — both must land in the same zones
    /// as their Anthropic equivalents, or the view is not provider-neutral.
    #[test]
    fn openai_shape_maps_to_the_same_zones() {
        let body = br#"{"messages":[
            {"role":"system","content":"be brief"},
            {"role":"assistant","tool_calls":[{"id":"c1","function":{"name":"f"}}]},
            {"role":"tool","content":"42"}],
            "tools":[{"type":"function","function":{"name":"f","description":"d"}}]}"#;
        let spans = split(Provider::OpenAiCompat, body).unwrap();
        let zones: Vec<Zone> = spans.iter().map(|s| s.zone).collect();
        assert!(zones.contains(&Zone::Instructions));
        assert!(zones.contains(&Zone::Tools));
        assert!(zones.contains(&Zone::ToolCall));
        assert!(zones.contains(&Zone::ToolResult));
    }

    #[test]
    fn a_paired_marker_claims_its_whole_block_and_nothing_after() {
        let text = "keep\n<system-reminder>injected</system-reminder>\ntail";
        let spans = split_marked(text, Zone::Input, "m0");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].zone, Zone::Input);
        assert_eq!(spans[1].zone, Zone::Injected);
        assert!(spans[1].text.contains("injected"));
        assert_eq!(
            spans[1].evidence,
            Evidence::Marker("<system-reminder>".to_string())
        );
        assert_eq!(spans[2].zone, Zone::Input, "text after the block is not injected");
        assert!(spans[2].text.contains("tail"));
    }

    /// An unpaired marker has no closing tag to stop at, so it claims the
    /// rest of the message — memory files are appended, never interleaved.
    #[test]
    fn an_unpaired_marker_claims_the_rest_of_the_message() {
        let spans = split_marked("hi\n# claudeMd\nrules here", Zone::Instructions, "m0");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[1].zone, Zone::Memory);
        assert!(spans[1].text.contains("rules here"));
    }

    #[test]
    fn totals_omit_empty_zones_and_sum_their_spans() {
        let body = br#"{"messages":[{"role":"user","content":"hello there"}]}"#;
        let spans = split(Provider::OpenAiCompat, body).unwrap();
        let t = totals(&spans);
        assert_eq!(t.len(), 1, "an absent zone is not a finding");
        assert_eq!(t[0].0, Zone::Input);
        assert!(t[0].1 > 0);
    }

    #[test]
    fn a_body_that_is_not_json_is_an_explicit_error() {
        assert!(split(Provider::OpenAiCompat, b"not json").is_err());
    }
}
