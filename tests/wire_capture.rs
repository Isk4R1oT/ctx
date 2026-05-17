//! F0 EXIT criterion: a real agent run via `ctx run --` captures the
//! verbatim assembled wire prompt for BOTH providers.
//!
//! The "agent" here is plain `curl` — deliberately. `ctx` reads the
//! wire, so a raw HTTP client is the strongest possible proof of zero
//! SDK/framework coupling (the moat, `docs/PROJECT.md` §3). Two
//! `wiremock` servers stand in for the real Anthropic / `OpenAI`
//! upstreams; `CTX_UPSTREAM_*` points `ctx` at them (no network, no
//! keys, hermetic).

use serde_json::Value;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ANTHROPIC_BODY: &str =
    r#"{"model":"claude-3","system":"be terse","messages":[{"role":"user","content":"ping"}]}"#;
const OPENAI_BODY: &str = r#"{"model":"gpt-4o","messages":[{"role":"system","content":"sys"},{"role":"user","content":"ping"}]}"#;

#[tokio::test]
async fn captures_verbatim_wire_prompt_for_both_providers() {
    // Stand-in upstreams.
    let anthropic = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"a"})))
        .mount(&anthropic)
        .await;

    let openai = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"o"})))
        .mount(&openai)
        .await;

    // The child agent: raw curl, one Anthropic-shaped + one
    // OpenAI-shaped request, through the proxy `ctx` injects.
    let script = format!(
        "curl -s -o /dev/null -X POST \"$ANTHROPIC_BASE_URL/v1/messages\" \
            -H 'content-type: application/json' -H 'x-api-key: secret' -d '{ANTHROPIC_BODY}'; \
         curl -s -o /dev/null -X POST \"$OPENAI_BASE_URL/chat/completions\" \
            -H 'content-type: application/json' -H 'authorization: Bearer secret' -d '{OPENAI_BODY}'"
    );

    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["run", "--json", "--", "sh", "-c", &script])
        .env("CTX_UPSTREAM_ANTHROPIC", anthropic.uri())
        .env("CTX_UPSTREAM_OPENAI", openai.uri())
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx binary");

    assert!(
        out.status.success(),
        "ctx exited non-zero; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let tl: Value = serde_json::from_slice(&out.stdout).expect("ctx --json must emit valid JSON");
    let steps = tl["steps"].as_array().expect("steps array");
    assert_eq!(steps.len(), 2, "expected one step per provider request");

    // Anthropic step: verbatim body + correct detection + redaction.
    assert_eq!(steps[0]["provider"], "anthropic");
    assert_eq!(steps[0]["request"]["path"], "/v1/messages");
    assert_eq!(
        steps[0]["request"]["body"].as_str().unwrap(),
        ANTHROPIC_BODY,
        "the assembled wire prompt must be captured byte-for-byte"
    );
    assert_eq!(steps[0]["response"]["status"], 200);
    let a_hdrs = steps[0]["request"]["headers"].as_array().unwrap();
    assert!(
        a_hdrs
            .iter()
            .any(|h| h[0] == "x-api-key" && h[1] == "REDACTED"),
        "secrets must be redacted before the timeline"
    );

    // OpenAI-compat step: verbatim body + correct detection.
    assert_eq!(steps[1]["provider"], "open_ai_compat");
    assert_eq!(steps[1]["request"]["path"], "/v1/chat/completions");
    assert_eq!(
        steps[1]["request"]["body"].as_str().unwrap(),
        OPENAI_BODY,
        "the assembled wire prompt must be captured byte-for-byte"
    );
    assert!(steps[1]["prompt_tokens"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn save_then_open_roundtrips_through_the_binary() {
    let anthropic = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"a"})))
        .mount(&anthropic)
        .await;

    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("session.sqlite");
    let script = format!(
        "curl -s -o /dev/null -X POST \"$ANTHROPIC_BASE_URL/v1/messages\" \
            -H 'content-type: application/json' -d '{ANTHROPIC_BODY}'"
    );

    let run = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args([
            "run",
            "--save",
            db.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            &script,
        ])
        .env("CTX_UPSTREAM_ANTHROPIC", anthropic.uri())
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx run --save");
    assert!(run.status.success());
    assert!(db.exists(), "--save must write the opt-in SQLite file");

    let open = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["--json", "open", db.to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx open");
    assert!(open.status.success());

    let tl: Value = serde_json::from_slice(&open.stdout).expect("ctx open --json valid JSON");
    let steps = tl["steps"].as_array().expect("steps");
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0]["request"]["body"].as_str().unwrap(),
        ANTHROPIC_BODY
    );
    assert_eq!(steps[0]["provider"], "anthropic");
}

#[tokio::test]
async fn bare_invocation_renders_the_banner() {
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .arg("--color")
        .arg("always")
        .output()
        .await
        .expect("spawn ctx");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("ctx"), "banner names the tool");
    assert!(s.contains("ctx run -- "), "banner shows the primary verb");
    // Strongest family rule: no emoji, ever.
    assert!(!s.chars().any(|c| c as u32 >= 0x1_F000));
}

#[tokio::test]
async fn run_requires_a_child_command() {
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["run"])
        .output()
        .await
        .expect("spawn ctx binary");
    // clap rejects a missing required `-- <CMD>` with a non-zero exit.
    assert!(!out.status.success());
}
