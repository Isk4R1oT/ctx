//! F0 EXIT criterion: timeline + opt-in-SQLite are snapshot-tested.
//! These pin the public contracts (the JSON shape `--json`/CI consumes,
//! the on-disk schema, the doc-11 plain-mode record format) so a silent
//! change to any is caught.

use ctx::color::ColorMode;
use ctx::compose::compose;
use ctx::render::Renderer;
use ctx::store::SCHEMA;
use ctx::timeline::Timeline;

/// Multi-step fixture exercising every F1 indictment (deterministic).
fn f1_fixture() -> Timeline {
    let mut t = Timeline::new();
    let body = br#"{"model":"m","system":"You are a careful, terse assistant for snapshot tests.","messages":[{"role":"user","content":"a reasonably long first user question for the snapshot"}],"tools":[{"name":"search","input_schema":{"type":"object"}},{"name":"calc","input_schema":{"type":"object"}}]}"#;
    let i = t.record_request("POST", "/v1/messages", &[], body);
    t.record_response(
        i,
        200,
        &[],
        br#"{"content":[{"type":"tool_use","name":"search"}]}"#,
    );
    let body2 = br#"{"model":"m","system":"You are a careful, terse assistant for snapshot tests.","messages":[{"role":"user","content":"a reasonably long first user question for the snapshot"},{"role":"assistant","content":"an assistant turn of reasonable length here"},{"role":"user","content":"a reasonably long first user question for the snapshot"}],"tools":[{"name":"search","input_schema":{"type":"object"}},{"name":"calc","input_schema":{"type":"object"}}]}"#;
    let j = t.record_request("POST", "/v1/messages", &[], body2);
    t.record_response(
        j,
        200,
        &[],
        br#"{"content":[{"type":"text","text":"done"}]}"#,
    );
    t
}

fn fixture() -> Timeline {
    let mut t = Timeline::new();
    let i = t.record_request(
        "POST",
        "/v1/messages",
        &[
            ("content-type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), "sk-should-be-redacted".to_string()),
        ],
        br#"{"model":"claude-3","system":"be terse","messages":[{"role":"user","content":"ping"}],"tools":[{"name":"search","input_schema":{"type":"object"}}]}"#,
    );
    t.record_response(i, 200, &[], br#"{"id":"msg_1"}"#);
    let j = t.record_request(
        "POST",
        "/v1/chat/completions",
        &[],
        br#"{"model":"gpt-4o","messages":[{"role":"system","content":"sys"},{"role":"user","content":"ping"}]}"#,
    );
    t.record_response(j, 200, &[], br#"{"id":"chatcmpl-1"}"#);
    t
}

#[test]
fn timeline_json_contract() {
    insta::assert_json_snapshot!(fixture());
}

#[test]
fn sqlite_schema_contract() {
    insta::assert_snapshot!(SCHEMA);
}

#[test]
fn plain_mode_record_format() {
    let mut buf = Vec::new();
    Renderer::new(ColorMode::None)
        .summary(&mut buf, &fixture())
        .unwrap();
    insta::assert_snapshot!(String::from_utf8(buf).unwrap());
}

// --- F1 composition + waste indictment contracts -----------------------

#[test]
fn f1_composition_json_contract() {
    // The `tl["composition"]` half of the RunReport (D-005).
    insta::assert_json_snapshot!(compose(&f1_fixture(), true));
}

#[test]
fn f1_composition_plain_grep_clean() {
    let mut buf = Vec::new();
    Renderer::new(ColorMode::None)
        .composition(&mut buf, &compose(&f1_fixture(), false))
        .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(!s.contains('\u{1b}'), "plain mode emits zero escapes");
    insta::assert_snapshot!(s);
}

#[test]
fn report_json_keeps_steps_top_level_d005() {
    // Pin the D-005 contract at the boundary it actually ships through
    // (report_json), not just the Composition shape: a serde-flatten
    // regression must fail here, not in a downstream consumer.
    let mut buf = Vec::new();
    let tl = f1_fixture();
    let comp = compose(&tl, false);
    Renderer::new(ColorMode::None)
        .report_json(&mut buf, &tl, &comp)
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
    assert!(v["steps"].is_array(), "F0 `steps` must stay top-level");
    assert!(v["composition"].is_object(), "F1 data additive");
    let obj = v.as_object().unwrap();
    assert_eq!(obj.len(), 2, "exactly two top-level keys");
    assert!(obj.contains_key("steps") && obj.contains_key("composition"));
    assert_eq!(v["steps"][0]["request"]["path"], "/v1/messages");
}

#[test]
fn f1_composition_tty_render() {
    let mut buf = Vec::new();
    Renderer::new(ColorMode::Truecolor)
        .composition(&mut buf, &compose(&f1_fixture(), true))
        .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains('\u{23FA}'), "TTY render carries the ⏺ glyph");
    assert!(!s.chars().any(|c| c as u32 >= 0x1_F000), "no emoji");
    insta::assert_snapshot!(s);
}
