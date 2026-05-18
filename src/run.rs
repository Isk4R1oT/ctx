//! `ctx run -- <cmd>` ergonomics — the primary entrypoint (D-001).
//!
//! Binds an ephemeral loopback proxy, points the child's provider base
//! URLs at it (`*_BASE_URL`), runs the child to completion, then hands
//! back the captured timeline + the child's exit code. The user's
//! original upstream is honored (so a custom OpenAI-compatible endpoint
//! still works); `CTX_UPSTREAM_*` overrides it (used by tests).

use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::proxy::{router, ProxyState};
use crate::timeline::Timeline;

#[derive(Debug)]
pub struct RunOutcome {
    pub timeline: Timeline,
    pub exit_code: i32,
}

/// The full upstream base — `scheme://host[:port][/path]` with any
/// trailing `/` trimmed, **path preserved** (D-017 root-cause fix).
/// `origin_of` used to strip the path, breaking every provider whose
/// base carries one (`OpenRouter` `/api/v1`, Azure
/// `/openai/deployments/…`, sub-path gateways). Unparseable / empty /
/// `null`-origin ⇒ the fallback (never a silently-wrong upstream).
fn base_of(raw: &str, fallback: &str) -> String {
    let candidate = if raw.is_empty() { fallback } else { raw };
    match reqwest::Url::parse(candidate) {
        Ok(u) if u.origin().ascii_serialization() != "null" => {
            candidate.trim_end_matches('/').to_string()
        }
        _ => fallback.to_string(),
    }
}

fn env_or(keys: &[&str]) -> String {
    for k in keys {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    String::new()
}

/// Run `command` behind the transparent proxy and capture its timeline.
///
/// # Errors
/// [`crate::Error::Config`] if the command is empty,
/// [`crate::Error::Bind`] if the loopback listener cannot bind, or
/// [`crate::Error::Spawn`] if the child cannot be launched.
pub async fn execute(command: &[String]) -> crate::Result<RunOutcome> {
    let Some((program, args)) = command.split_first() else {
        return Err(crate::Error::Config(
            "nothing to run — usage: ctx run -- <command>".to_string(),
        ));
    };

    let anthropic_base = base_of(
        &env_or(&["CTX_UPSTREAM_ANTHROPIC", "ANTHROPIC_BASE_URL"]),
        "https://api.anthropic.com",
    );
    // Default carries `/v1`: ctx now injects the proxy ROOT (no
    // synthetic `/v1`), so the upstream base must hold the provider's
    // real path itself (D-017).
    let openai_raw = env_or(&["CTX_UPSTREAM_OPENAI", "OPENAI_BASE_URL", "OPENAI_API_BASE"]);
    // Explicit user upstream ⇒ authoritative; absent ⇒ the per-request
    // key-prefix registry may infer it (P2/D-017).
    let openai_explicit = !openai_raw.is_empty();
    let openai_base = base_of(&openai_raw, "https://api.openai.com/v1");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(crate::Error::Bind)?;
    let port = listener.local_addr().map_err(crate::Error::Bind)?.port();

    let timeline = Arc::new(Mutex::new(Timeline::new()));
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(crate::Error::Upstream)?;
    let state = ProxyState {
        client,
        anthropic_base,
        openai_base,
        openai_explicit,
        timeline: Arc::clone(&timeline),
    };
    let app = router(state);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut server = tokio::spawn(async move {
        let shutdown = async move {
            // Signalled once the child exits; a dropped sender (early
            // error path) also triggers shutdown.
            shutdown_rx.await.ok();
        };
        if let Err(err) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
        {
            eprintln!("ctx: proxy server stopped: {err}");
        }
    });

    // Inject the proxy ROOT (no synthetic `/v1`): the client builds its
    // own provider path verbatim and ctx forwards
    // `resolved_base + request_path` unchanged (D-017). This is what
    // makes OpenRouter/Azure/sub-path bases work with zero hacks.
    let proxy_base = format!("http://127.0.0.1:{port}");
    let status_res = tokio::process::Command::new(program)
        .args(args)
        .env("ANTHROPIC_BASE_URL", &proxy_base)
        .env("OPENAI_BASE_URL", &proxy_base)
        .env("OPENAI_API_BASE", &proxy_base)
        .status()
        .await;

    // Child is done. Stop accepting and DRAIN in-flight `forward()`
    // futures (each finishes its `record_response` — `forward` is NOT
    // cancel-safe), then join the task so its `Arc<Mutex<Timeline>>`
    // clone is dropped before we take the timeline back. This closes
    // the abort-vs-`try_unwrap` race that would otherwise silently
    // empty/truncate the capture under a multi-turn agent.
    shutdown_tx.send(()).ok();
    match tokio::time::timeout(std::time::Duration::from_secs(10), &mut server).await {
        Ok(_joined) => {}
        Err(_elapsed) => {
            // A handler exceeded the drain budget; abort as a last
            // resort, then join so the task (and its Arc) is gone.
            server.abort();
            (&mut server).await.ok();
        }
    }

    // The task is joined → we are the sole owner. The `Err` arm is not
    // a data-loss fallback: it clones the real, now-uncontended
    // timeline (never an empty default).
    let timeline = match Arc::try_unwrap(timeline) {
        Ok(m) => m.into_inner(),
        Err(arc) => arc.lock().await.clone(),
    };

    let status = status_res.map_err(|source| crate::Error::Spawn {
        cmd: program.clone(),
        source,
    })?;

    Ok(RunOutcome {
        timeline,
        exit_code: status.code().unwrap_or(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_command_is_config_error() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let err = rt.block_on(execute(&[])).unwrap_err();
        assert!(matches!(err, crate::Error::Config(_)));
    }

    #[test]
    fn base_of_preserves_full_path() {
        // Path PRESERVED (the D-017 root-cause fix — was stripped).
        assert_eq!(
            base_of("https://api.anthropic.com/v1", "x"),
            "https://api.anthropic.com/v1"
        );
        // Trailing slash trimmed; sub-path kept (OpenRouter shape).
        assert_eq!(
            base_of("https://openrouter.ai/api/v1/", "f"),
            "https://openrouter.ai/api/v1"
        );
        // Empty ⇒ fallback (returned verbatim, path intact).
        assert_eq!(
            base_of("", "https://api.openai.com/v1"),
            "https://api.openai.com/v1"
        );
        // Unparseable ⇒ fallback (never a silently-wrong upstream).
        assert_eq!(base_of("not a url", "https://fb"), "https://fb");
        // Parses OK but has an OPAQUE (`null`) origin (data:/file:/…) ⇒
        // fallback, never accepted as an upstream. Pins the
        // `!= "null"` guard (a `true`-guard mutant would wrongly
        // forward to `data:…`).
        assert_eq!(base_of("data:text/plain,x", "https://fb"), "https://fb");
        assert_eq!(base_of("file:///etc/x", "https://fb"), "https://fb");
    }
}
