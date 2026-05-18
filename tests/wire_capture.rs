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
use wiremock::matchers::{header, method, path};
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
        // A real OpenAI-compatible upstream base carries its own `/v1`
        // (D-017: ctx forwards base + verbatim path; no synthetic `/v1`).
        .env("CTX_UPSTREAM_OPENAI", format!("{}/v1", openai.uri()))
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
    // ctx injects the proxy ROOT (no synthetic `/v1`, D-017) ⇒ the
    // captured path is exactly what the client sent.
    assert_eq!(steps[1]["request"]["path"], "/chat/completions");
    assert_eq!(
        steps[1]["request"]["body"].as_str().unwrap(),
        OPENAI_BODY,
        "the assembled wire prompt must be captured byte-for-byte"
    );
    assert!(steps[1]["prompt_tokens"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn graceful_shutdown_loses_no_steps_under_back_to_back_load() {
    // Regression for the abort-vs-try_unwrap race: every request a
    // multi-call agent makes must survive shutdown with its response.
    let anthropic = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"a"})))
        .mount(&anthropic)
        .await;

    let n = 5;
    let script = format!(
        "for i in $(seq 1 {n}); do \
           curl -s -o /dev/null -X POST \"$ANTHROPIC_BASE_URL/v1/messages\" \
             -H 'content-type: application/json' -d '{ANTHROPIC_BODY}'; \
         done"
    );

    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["run", "--json", "--", "sh", "-c", &script])
        .env("CTX_UPSTREAM_ANTHROPIC", anthropic.uri())
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx");
    assert!(out.status.success());

    let tl: Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let steps = tl["steps"].as_array().expect("steps");
    assert_eq!(steps.len(), n, "no step may be lost across shutdown");
    for (i, s) in steps.iter().enumerate() {
        assert_eq!(
            s["response"]["status"], 200,
            "step {i} must keep its recorded response (drain, not abort)"
        );
        assert_eq!(s["request"]["body"].as_str().unwrap(), ANTHROPIC_BODY);
    }
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
async fn f2_view_is_byte_exact_vs_captured_wire_bytes() {
    // F2 EXIT: `ctx view` one-shot (piped) emits the assembled prompt
    // byte-for-byte — pipe it and it equals what the agent sent.
    let anthropic = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"a"})))
        .mount(&anthropic)
        .await;

    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("s.sqlite");
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
        .expect("spawn ctx run");
    assert!(run.status.success());

    let view = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["view", db.to_str().unwrap(), "--step", "0"])
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx view");
    assert!(view.status.success());
    // Byte-exact: stdout IS the captured wire prompt, nothing added.
    assert_eq!(
        String::from_utf8_lossy(&view.stdout),
        ANTHROPIC_BODY,
        "ctx view (piped) must emit the verbatim wire bytes exactly"
    );
    assert!(!view.stdout.contains(&0x1b), "plain view: zero escapes");
}

#[tokio::test]
async fn f1_headline_works_on_a_real_openai_chat_completions_run() {
    // F1-FIX step A (integration repro, the layer the defect was proven
    // on): a real `ctx run` whose child posts an OpenAI-compatible
    // chat.completions body. F0 captures it (provider open_ai_compat);
    // the F1 headline MUST decompose it, not print "no captured prompt".
    let openai = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": "ok"}}]
        })))
        .mount(&openai)
        .await;

    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("oai.sqlite");
    // A realistic OpenAI body: system + user + assistant history +
    // tools[] (function shape) + the usual extra fields a real client
    // sends. Single-quoted in sh, so no embedded single quotes.
    let body = r#"{"model":"openai/gpt-4o","messages":[{"role":"system","content":"You are terse."},{"role":"user","content":"the first question that is reasonably long here"},{"role":"assistant","content":"a prior assistant turn of some length"},{"role":"user","content":"the first question that is reasonably long here"}],"tools":[{"type":"function","function":{"name":"search","description":"web","parameters":{"type":"object","properties":{"q":{"type":"string"}},"required":["q"]}}},{"type":"function","function":{"name":"calc","description":"math","parameters":{"type":"object"}}}],"temperature":0.2,"stream":true}"#;
    let script = format!(
        "curl -s -o /dev/null -X POST \"$OPENAI_BASE_URL/chat/completions\" \
            -H 'content-type: application/json' -H 'authorization: Bearer sk-x' -d '{body}'; \
         curl -s -o /dev/null -X POST \"$OPENAI_BASE_URL/chat/completions\" \
            -H 'content-type: application/json' -H 'authorization: Bearer sk-x' -d '{body}'"
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
        // A real OpenAI-compatible upstream base carries its own `/v1`
        // (D-017: ctx forwards base + verbatim path; no synthetic `/v1`).
        .env("CTX_UPSTREAM_OPENAI", format!("{}/v1", openai.uri()))
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx run");
    assert!(run.status.success());

    // F1 headline JSON (D-005 RunReport): composition must be populated.
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["--json", "open", db.to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx open --json");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(
        v["steps"][0]["provider"], "open_ai_compat",
        "F0 classified it"
    );
    let comp = &v["composition"];
    assert!(
        !comp["focus_step"].is_null(),
        "F1 must find a focus step on the OpenAI shape, got composition={comp}"
    );
    assert!(
        comp["total_tokens"].as_u64().unwrap() > 0,
        "F1 must not report 0 tokens on a real OpenAI capture, got {comp}"
    );

    // F1 headline plain text must NOT say "no captured prompt".
    let plain = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["open", db.to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx open");
    let s = String::from_utf8_lossy(&plain.stdout);
    assert!(
        !s.contains("no captured prompt"),
        "F1 headline is blind on the OpenAI shape: {s}"
    );
    assert!(s.contains("component system "), "system component present");
    assert!(
        s.contains("component tool-schemas "),
        "tool-schemas present"
    );
}

#[tokio::test]
async fn f3_diff_is_correct_on_a_real_multi_step_run() {
    // F3 EXIT: a real 2-step run; `ctx diff` reports the added lines
    // and the positive token delta of the grown second prompt.
    let anthropic = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"a"})))
        .mount(&anthropic)
        .await;

    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("s.sqlite");
    // Turn 2's prompt carries an extra appended message (grown history).
    let small = r#"{"model":"m","messages":[{"role":"user","content":"first"}]}"#;
    let grown = r#"{"model":"m","messages":[{"role":"user","content":"first"},{"role":"assistant","content":"reply"},{"role":"user","content":"second turn question"}]}"#;
    let script = format!(
        "curl -s -o /dev/null -X POST \"$ANTHROPIC_BASE_URL/v1/messages\" -d '{small}'; \
         curl -s -o /dev/null -X POST \"$ANTHROPIC_BASE_URL/v1/messages\" -d '{grown}'"
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
        .expect("spawn ctx run");
    assert!(run.status.success());

    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["--json", "diff", db.to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx diff");
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("ctx diff --json");
    assert_eq!(v["from"], 0);
    assert_eq!(v["to"], 1);
    assert!(
        v["added_tokens"].as_u64().unwrap() > v["removed_tokens"].as_u64().unwrap(),
        "the grown turn-2 prompt must add more tokens than it removes"
    );
    assert!(v["added_lines"].as_u64().unwrap() >= 1);
    let lines = v["lines"].as_array().unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l["op"] == "add"
                && l["text"].as_str().unwrap().contains("second turn question")),
        "the diff must show the appended message as an addition"
    );

    // Plain mode is grep-clean: zero escapes, one record per line.
    let plain = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["diff", db.to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx diff plain");
    assert!(plain.status.success());
    assert!(!plain.stdout.contains(&0x1b), "plain diff: zero escapes");
    let s = String::from_utf8_lossy(&plain.stdout);
    assert!(s.lines().next().unwrap().starts_with("> diff step 0 -> 1"));
    assert!(s.contains("summary: +"));
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
    // Pin the actual banner render so a regression to plain text is
    // caught: rounded box corner + the doc-11 spark glyph, with color
    // escapes present (--color always over a pipe).
    assert!(s.contains('\u{256D}'), "rounded box top-left corner");
    assert!(s.contains('\u{273B}'), "doc-11 banner spark glyph");
    assert!(s.contains('\u{1b}'), "--color always must emit ANSI");
    // Strongest family rule: no emoji, ever.
    assert!(!s.chars().any(|c| c as u32 >= 0x1_F000));
}

#[tokio::test]
async fn forwards_normal_headers_and_strips_hop_by_hop() {
    // Kills the `delete !` mutants in proxy::forward: a normal request
    // header must reach the upstream (mock only matches if it does), and
    // an upstream response header must be re-emitted to the child.
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("content-type", "application/json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-ctx-probe", "present")
                .set_body_json(serde_json::json!({"id":"a"})),
        )
        .mount(&upstream)
        .await;

    let dir = tempfile::tempdir().expect("tempdir");
    let hdr = dir.path().join("resp_headers.txt");
    let script = format!(
        "curl -s -o /dev/null -D \"$CTX_HDR\" -X POST \"$ANTHROPIC_BASE_URL/v1/messages\" \
            -H 'content-type: application/json' -H 'x-api-key: secret' -d '{ANTHROPIC_BODY}'"
    );

    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["run", "--json", "--", "sh", "-c", &script])
        .env("CTX_UPSTREAM_ANTHROPIC", upstream.uri())
        .env("CTX_HDR", &hdr)
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx");
    assert!(out.status.success());

    // content-type forwarded ⇒ mock matched ⇒ 200 (kills strip of
    // request headers at proxy.rs `if !is_stripped` request loop).
    let tl: Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(tl["steps"][0]["response"]["status"], 200);

    // The upstream response header was re-emitted to the child (kills
    // strip of response headers at the proxy.rs response loop).
    let dumped = std::fs::read_to_string(&hdr).expect("curl wrote response headers");
    assert!(
        dumped.to_ascii_lowercase().contains("x-ctx-probe: present"),
        "expected upstream response header forwarded to child; got:\n{dumped}"
    );
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

// P1 / D-017 — root-cause fix: the upstream base PATH must be preserved
// verbatim. A subpath upstream (OpenRouter `…/api/v1`, Azure
// `…/openai/deployments/…`, any sub-path gateway) MUST forward
// correctly with NO `CTX_UPSTREAM`/client `/api/v1` hack. On HEAD
// `origin_of` strips the path → forward misses → upstream != 200.
// MUST fail pre-fix, pass post-fix. `#[ignore]` keeps the commit-gate
// green at the red commit (un-ignored at the P1 fix commit).
#[tokio::test]
async fn forwards_verbatim_to_a_subpath_upstream() {
    // An OpenRouter-shaped upstream: the real base carries `/api/v1`.
    let openrouter = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"or"})))
        .mount(&openrouter)
        .await;

    // The client posts the OpenAI-SDK-conventional path relative to the
    // proxy base ctx injects — no `/api/v1` knowledge in the client.
    let script = format!(
        "curl -s -o /dev/null -X POST \"$OPENAI_BASE_URL/chat/completions\" \
            -H 'content-type: application/json' -H 'authorization: Bearer secret' -d '{OPENAI_BODY}'"
    );

    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["run", "--json", "--", "sh", "-c", &script])
        // The real upstream base WITH its `/api/v1` sub-path.
        .env("CTX_UPSTREAM_OPENAI", format!("{}/api/v1", openrouter.uri()))
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx binary");

    assert!(
        out.status.success(),
        "ctx exited non-zero; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tl: Value = serde_json::from_slice(&out.stdout).expect("ctx --json valid JSON");
    let steps = tl["steps"].as_array().expect("steps array");
    assert_eq!(steps.len(), 1, "one captured request");
    assert_eq!(steps[0]["provider"], "open_ai_compat");
    // ctx injects the proxy ROOT (no synthetic /v1) ⇒ the captured path
    // is exactly what the client sent.
    assert_eq!(steps[0]["request"]["path"], "/chat/completions");
    assert_eq!(
        steps[0]["response"]["status"], 200,
        "the subpath upstream `/api/v1/chat/completions` MUST be hit verbatim (no path stripping)"
    );
}

// P2/D-017 — pins `execute()`'s `openai_explicit` polarity (kills the
// `delete !` mutant): an EXPLICIT upstream MUST beat the key-prefix
// registry even when the bearer key (`sk-or-…`) would otherwise
// resolve to OpenRouter. Hermetic: the correct path hits the local
// wiremock; the mutant would route to real openrouter.ai and miss it
// (wiremock `.expect(1)` fails the test deterministically).
#[tokio::test]
async fn explicit_upstream_beats_the_key_registry() {
    let explicit = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"x"})))
        .expect(1)
        .mount(&explicit)
        .await;

    // Bearer key whose prefix WOULD infer OpenRouter — explicit must win.
    let script = format!(
        "curl -s -o /dev/null -X POST \"$OPENAI_BASE_URL/chat/completions\" \
            -H 'content-type: application/json' \
            -H 'authorization: Bearer sk-or-v1-localtest' -d '{OPENAI_BODY}'"
    );
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args(["run", "--json", "--", "sh", "-c", &script])
        .env("CTX_UPSTREAM_OPENAI", format!("{}/v1", explicit.uri()))
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx binary");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let tl: Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let steps = tl["steps"].as_array().expect("steps");
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0]["response"]["status"], 200,
        "explicit upstream must be used verbatim — NOT the sk-or- registry inference"
    );
    // `explicit` MockServer `.expect(1)` is verified on drop here.
}

// P3/D-017 — `--to <url>` is the most-explicit upstream (a discoverable
// CLI flag, the D-001 amend): no env at all, beats the key registry.
// On HEAD clap rejects `--to` (unknown arg) ⇒ non-zero ⇒ red.
// `#[ignore]` keeps the gate green at the red commit; un-ignored at
// the P3 impl commit.
#[tokio::test]
#[ignore = "P3/D-017 red: --to flag not yet added; un-ignored at the P3 impl commit"]
async fn to_flag_is_the_explicit_upstream() {
    let up = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id":"t"})))
        .expect(1)
        .mount(&up)
        .await;

    // A `sk-or-` key that WOULD infer OpenRouter — `--to` must win,
    // with ZERO upstream env set.
    let script = format!(
        "curl -s -o /dev/null -X POST \"$OPENAI_BASE_URL/chat/completions\" \
            -H 'content-type: application/json' \
            -H 'authorization: Bearer sk-or-v1-localtest' -d '{OPENAI_BODY}'"
    );
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_ctx"))
        .args([
            "run",
            "--to",
            &format!("{}/v1", up.uri()),
            "--json",
            "--",
            "sh",
            "-c",
            &script,
        ])
        .env("NO_COLOR", "1")
        .output()
        .await
        .expect("spawn ctx");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let tl: Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let steps = tl["steps"].as_array().expect("steps");
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0]["response"]["status"], 200,
        "--to must be the verbatim explicit upstream (beats env + key registry)"
    );
    // `up` `.expect(1)` verified on drop ⇒ the request really hit it.
}
