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

fn origin_of(raw: &str, fallback: &str) -> String {
    let candidate = if raw.is_empty() { fallback } else { raw };
    reqwest::Url::parse(candidate)
        .ok()
        .map(|u| u.origin().ascii_serialization())
        .filter(|o| o != "null")
        .unwrap_or_else(|| fallback.to_string())
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

    let anthropic_origin = origin_of(
        &env_or(&["CTX_UPSTREAM_ANTHROPIC", "ANTHROPIC_BASE_URL"]),
        "https://api.anthropic.com",
    );
    let openai_origin = origin_of(
        &env_or(&["CTX_UPSTREAM_OPENAI", "OPENAI_BASE_URL", "OPENAI_API_BASE"]),
        "https://api.openai.com",
    );

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
        anthropic_origin,
        openai_origin,
        timeline: Arc::clone(&timeline),
    };
    let app = router(state);
    let server = tokio::spawn(async move {
        // Resolves only on a real bind/accept failure; on normal
        // shutdown the task is aborted (cancelled), never resolved.
        if let Err(err) = axum::serve(listener, app).await {
            eprintln!("ctx: proxy server stopped: {err}");
        }
    });

    let proxy_base = format!("http://127.0.0.1:{port}");
    let openai_base = format!("{proxy_base}/v1");
    let status = tokio::process::Command::new(program)
        .args(args)
        .env("ANTHROPIC_BASE_URL", &proxy_base)
        .env("OPENAI_BASE_URL", &openai_base)
        .env("OPENAI_API_BASE", &openai_base)
        .status()
        .await
        .map_err(|source| crate::Error::Spawn {
            cmd: program.clone(),
            source,
        })?;

    server.abort();

    // Sole owner expected (server task aborted); clone-fallback is
    // defensive and should not occur in practice.
    let timeline = Arc::try_unwrap(timeline).map_or_else(
        |arc| {
            arc.try_lock()
                .map_or_else(|_| Timeline::default(), |g| g.clone())
        },
        Mutex::into_inner,
    );

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
    fn origin_reduces_to_scheme_host() {
        assert_eq!(
            origin_of("https://api.anthropic.com/v1", "x"),
            "https://api.anthropic.com"
        );
        assert_eq!(
            origin_of("", "https://api.openai.com"),
            "https://api.openai.com"
        );
    }
}
