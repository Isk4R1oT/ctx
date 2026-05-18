//! F0 EXIT criterion: timeline + opt-in-SQLite are snapshot-tested.
//! These pin the public contracts (the JSON shape `--json`/CI consumes,
//! the on-disk schema, the doc-11 plain-mode record format) so a silent
//! change to any is caught.

use ctx::color::ColorMode;
use ctx::compose::compose;
use ctx::render::Renderer;
use ctx::store::SCHEMA;
use ctx::timeline::Timeline;
use ctx::view;

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

// --- C3 (D-012) context-window headroom & growth-rate slope -----------
//
// A KNOWN-model multi-turn fixture so the C3 contract is pinned: the
// JSON `headroom` object, the plain grep-clean record, and the deep
// projection are all integer-only / snapshot-stable (no float — the
// `pct`/`slope` integer discipline). The headline carries only the
// measured fraction + slope; the projection is `--deep`-only and
// neutrally worded.
fn c3_fixture() -> Timeline {
    let mut t = Timeline::new();
    // Real Anthropic wire model id ⇒ the static offline window table
    // resolves it to 200_000. History grows by a fixed block each turn.
    let block = "a stable user sentence of fixed length for the snapshot fixture ".repeat(20);
    for turn in 1..=3usize {
        let msgs: Vec<String> = (0..turn)
            .map(|k| format!(r#"{{"role":"user","content":"{block} #{k}"}}"#))
            .collect();
        let body = format!(
            r#"{{"model":"claude-3-5-sonnet-20241022","system":"You are a careful, terse assistant for snapshot tests.","messages":[{}]}}"#,
            msgs.join(",")
        );
        let i = t.record_request("POST", "/v1/messages", &[], body.as_bytes());
        t.record_response(i, 200, &[], br#"{"content":[{"type":"text","text":"ok"}]}"#);
    }
    t
}

#[test]
fn c3_headroom_json_contract() {
    // Integer-only, snapshot-stable Headroom object (no float). With
    // `--deep` the neutral projection string is present and worded
    // "at the observed mean rate" — never as fate.
    insta::assert_json_snapshot!(compose(&c3_fixture(), true).headroom);
}

#[test]
fn c3_headline_has_fraction_and_slope_but_not_projection() {
    // Headline (NOT --deep): the measured fraction + slope appear; the
    // neutral projection MUST be absent (it is --deep-only).
    let mut buf = Vec::new();
    Renderer::new(ColorMode::None)
        .composition(&mut buf, &compose(&c3_fixture(), false))
        .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(!s.contains('\u{1b}'), "plain mode emits zero escapes");
    assert!(
        s.contains("window claude-3-5-sonnet-20241022") && s.contains("slope "),
        "headline must show the measured fraction + slope: {s}"
    );
    assert!(
        !s.contains("window-projection"),
        "the projection must be --deep-only, absent from the headline: {s}"
    );
    insta::assert_snapshot!(s);
}

#[test]
fn c3_deep_adds_neutral_projection_only() {
    // --deep: the neutral mean-rate projection appears, worded
    // "at the observed mean rate", and asserts NO fate.
    let mut buf = Vec::new();
    Renderer::new(ColorMode::None)
        .composition(&mut buf, &compose(&c3_fixture(), true))
        .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(
        s.contains("window-projection") && s.contains("at the observed mean rate"),
        "--deep must add the neutral mean-rate projection: {s}"
    );
    for banned in ["will overflow", "will truncate", "you will", "run out"] {
        assert!(
            !s.to_lowercase().contains(banned),
            "projection must not assert fate ({banned}): {s}"
        );
    }
    insta::assert_snapshot!(s);
}

#[test]
fn c3_unknown_model_emits_no_window_line() {
    // The discipline rule, at the render boundary: an unknown model ⇒
    // NO window claim anywhere in the output (the f1_fixture model is
    // "m", not in the table).
    let mut buf = Vec::new();
    Renderer::new(ColorMode::None)
        .composition(&mut buf, &compose(&f1_fixture(), true))
        .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(
        !s.contains("window ") && !s.contains("window-projection"),
        "an unknown model must emit no window claim: {s}"
    );
}

#[test]
fn c3_headroom_tty_render() {
    let mut buf = Vec::new();
    Renderer::new(ColorMode::Truecolor)
        .composition(&mut buf, &compose(&c3_fixture(), true))
        .unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains('\u{23FA}'), "TTY render carries the ⏺ glyph");
    assert!(!s.chars().any(|c| c as u32 >= 0x1_F000), "no emoji");
    assert!(s.contains("Window"), "the C3 TTY section header");
    insta::assert_snapshot!(s);
}

// --- F2 verbatim-context contracts ------------------------------------

// Test-support fixture: the timeline is constructed right here with one
// step, so `select(.., Some(0))` is a genuine documented invariant.
#[allow(clippy::expect_used)]
fn f2_step() -> view::SelectedStep {
    let mut t = Timeline::new();
    t.record_request(
        "POST",
        "/v1/messages",
        &[],
        br#"{"model":"claude-3","system":"be terse","messages":[{"role":"user","content":"ping"}]}"#,
    );
    view::select(&t, Some(0)).expect("fixture: step 0 always present")
}

#[test]
fn f2_oneshot_plain_is_byte_exact() {
    let sel = f2_step();
    let mut buf = Vec::new();
    view::oneshot(&mut buf, false, ColorMode::None, &sel).unwrap();
    assert_eq!(buf, sel.body.as_bytes(), "plain = the wire bytes, exactly");
    assert!(!buf.contains(&0x1b));
    insta::assert_snapshot!(String::from_utf8(buf).unwrap());
}

#[test]
fn f2_oneshot_tty_header_render() {
    let sel = f2_step();
    let mut buf = Vec::new();
    view::oneshot(&mut buf, true, ColorMode::Truecolor, &sel).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains('\u{23FA}') && !s.chars().any(|c| c as u32 >= 0x1_F000));
    insta::assert_snapshot!(s);
}

#[test]
fn f2_json_contract() {
    let sel = f2_step();
    let mut buf = Vec::new();
    view::json(&mut buf, &sel).unwrap();
    insta::assert_snapshot!(String::from_utf8(buf).unwrap());
}

#[test]
fn f2_pager_frame_render() {
    let sel = f2_step();
    insta::assert_snapshot!(view::render_to_string(&sel, 64, 10).unwrap());
}

// --- F3 per-step context diff contracts -------------------------------

// Two-step timeline built inline ⇒ `diff(.., None, None)` is a genuine
// documented invariant (Law 9), like the F2 fixture.
#[allow(clippy::expect_used)]
fn f3_diff() -> ctx::diff::StepDiff {
    let mut t = Timeline::new();
    t.record_request(
        "POST",
        "/v1/messages",
        &[],
        b"{\n  \"system\": \"be terse\",\n  \"messages\": [ \"first\" ]\n}",
    );
    t.record_request(
        "POST",
        "/v1/messages",
        &[],
        b"{\n  \"system\": \"be terse\",\n  \"messages\": [ \"first\", \"second\" ]\n}",
    );
    ctx::diff::diff(&t, None, None).expect("fixture: 2 steps present")
}

#[test]
fn f3_diff_json_contract() {
    insta::assert_json_snapshot!(f3_diff());
}

#[test]
fn f3_diff_plain_grep_clean() {
    let mut buf = Vec::new();
    ctx::render::diff_block(&mut buf, ColorMode::None, &f3_diff()).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(!s.contains('\u{1b}'), "plain diff emits zero escapes");
    assert!(s.lines().next().unwrap().starts_with("> diff step 0 -> 1"));
    insta::assert_snapshot!(s);
}

#[test]
fn f3_diff_tty_render() {
    let mut buf = Vec::new();
    ctx::render::diff_block(&mut buf, ColorMode::Truecolor, &f3_diff()).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains('\u{23FA}'), "doc-11 ⏺ action glyph");
    assert!(!s.chars().any(|c| c as u32 >= 0x1_F000), "no emoji");
    insta::assert_snapshot!(s);
}

#[test]
fn f3_diff_ansi256_uses_the_256_bg_fills() {
    // doc-11 §5.1: on Ansi256, add/remove rows carry the 256-indexed
    // diff backgrounds (22 add / 52 remove). Pins the diff_bg Ansi256
    // arms (mutation: deleting them would drop the fills silently).
    let mut buf = Vec::new();
    ctx::render::diff_block(&mut buf, ColorMode::Ansi256, &f3_diff()).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("\u{1b}[48;5;22m"), "Ansi256 add bg = 22");
    assert!(s.contains("\u{1b}[48;5;52m"), "Ansi256 remove bg = 52");
    assert!(!s.chars().any(|c| c as u32 >= 0x1_F000), "no emoji");
}
