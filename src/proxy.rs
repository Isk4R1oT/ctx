//! The transparent local reverse-proxy — the canonical mechanism
//! (`docs/PROJECT.md` §4; D-001). It binds an ephemeral loopback port,
//! captures the **verbatim** request bytes (the assembled wire prompt),
//! forwards to the real upstream, and records the response.
//!
//! F0 honest limit (recorded in PROJECT.md non-goals discipline): the
//! upstream response is buffered, not streamed-through. Request capture
//! — the F0 EXIT criterion — is byte-exact regardless. SSE pass-through
//! is a tracked v1.x item, not an F0 claim (no overclaiming).

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::Response;
use axum::Router;
use tokio::sync::Mutex;

use crate::adapter::Provider;
use crate::timeline::Timeline;

/// Hop-by-hop / length headers that must not be copied verbatim onto a
/// re-issued request or response (the new transport owns them).
const STRIP: &[&str] = &[
    "host",
    "content-length",
    "connection",
    "transfer-encoding",
    "accept-encoding",
    "keep-alive",
    "proxy-authorization",
    "te",
    "trailer",
    "upgrade",
];

/// Deliberate ceiling on a single captured request body. A transparent
/// proxy must not OOM on a misbehaving agent; an overflow is surfaced as
/// an error, never silently truncated. (Assembled LLM prompts are well
/// under this; raised if a real workload ever needs it.)
const MAX_BODY: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct ProxyState {
    pub client: reqwest::Client,
    /// Origin (`scheme://host[:port]`) of the real Anthropic upstream.
    pub anthropic_origin: String,
    /// Origin of the real OpenAI-compatible upstream.
    pub openai_origin: String,
    pub timeline: Arc<Mutex<Timeline>>,
}

/// Build the proxy router. A single fallback handles every method and
/// path (verified axum 0.8 API: `Router::new().fallback`).
pub fn router(state: ProxyState) -> Router {
    Router::new().fallback(proxy).with_state(state)
}

fn header_pairs(headers: &axum::http::HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect()
}

fn is_stripped(name: &str) -> bool {
    STRIP.iter().any(|s| name.eq_ignore_ascii_case(s))
}

async fn proxy(State(state): State<ProxyState>, req: Request) -> Response {
    match forward(state, req).await {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("ctx: proxy error: {e}");
            let body = format!("ctx proxy error: {e}");
            Response::builder()
                .status(502)
                .body(Body::from(body))
                .unwrap_or_else(|build_err| {
                    eprintln!("ctx: failed to build 502 response: {build_err}");
                    Response::new(Body::empty())
                })
        }
    }
}

/// Forward one captured request to the real upstream.
///
/// NOT cancel-safe: a cancel between `send()` and `record_response`
/// would drop the response capture and leave the upstream's (possibly
/// billed) work unacknowledged. The server is therefore shut down by
/// **draining** in-flight calls (`with_graceful_shutdown`), never by
/// `abort()` — see `run::execute`.
async fn forward(state: ProxyState, req: Request) -> crate::Result<Response> {
    let (parts, body) = req.into_parts();
    let method = parts.method.clone();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map_or_else(|| parts.uri.path().to_string(), ToString::to_string);
    let path = parts.uri.path().to_string();
    let req_headers = header_pairs(&parts.headers);

    let body_bytes = axum::body::to_bytes(body, MAX_BODY).await.map_err(|e| {
        crate::Error::Adapter(format!("reading request body (max {MAX_BODY}B): {e}"))
    })?;

    // Capture BEFORE forwarding — the F0 EXIT criterion.
    let provider = Provider::detect(&path, &req_headers);
    let idx = {
        let mut tl = state.timeline.lock().await;
        tl.record_request(method.as_str(), &path, &req_headers, &body_bytes)
    };

    let origin = match provider {
        Some(Provider::Anthropic) => &state.anthropic_origin,
        Some(Provider::OpenAiCompat) | None => &state.openai_origin,
    };
    let url = format!("{}{}", origin.trim_end_matches('/'), path_and_query);

    let fwd_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|e| crate::Error::Adapter(format!("method: {e}")))?;
    let mut builder = state.client.request(fwd_method, &url);
    for (k, v) in &req_headers {
        if !is_stripped(k) {
            builder = builder.header(k, v);
        }
    }
    let upstream = builder.body(body_bytes.to_vec()).send().await?;

    let status = upstream.status();
    let resp_headers: Vec<(String, String)> = upstream
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    let resp_bytes = upstream.bytes().await?;

    {
        let mut tl = state.timeline.lock().await;
        tl.record_response(idx, status.as_u16(), &resp_headers, &resp_bytes);
    }

    let mut out = Response::builder().status(status.as_u16());
    for (k, v) in &resp_headers {
        if !is_stripped(k) {
            out = out.header(k, v);
        }
    }
    out.body(Body::from(resp_bytes.to_vec()))
        .map_err(|e| crate::Error::Adapter(format!("building response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_hop_by_hop_headers() {
        assert!(is_stripped("Host"));
        assert!(is_stripped("content-length"));
        assert!(!is_stripped("content-type"));
        assert!(!is_stripped("x-api-key"));
    }
}
