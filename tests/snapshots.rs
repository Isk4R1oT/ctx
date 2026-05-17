//! F0 EXIT criterion: timeline + opt-in-SQLite are snapshot-tested.
//! These pin the public contracts (the JSON shape `--json`/CI consumes,
//! the on-disk schema, the doc-11 plain-mode record format) so a silent
//! change to any is caught.

use ctx::color::ColorMode;
use ctx::render::Renderer;
use ctx::store::SCHEMA;
use ctx::timeline::Timeline;

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
