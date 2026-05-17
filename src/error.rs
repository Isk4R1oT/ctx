//! F0 error type. Library code propagates `Error` via `?`; the binary
//! boundary adds `anyhow` context (COMPILER-TRUTH Law 9).

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("upstream request failed: {0}")]
    Upstream(#[from] reqwest::Error),

    #[error("persistence: {0}")]
    Persistence(#[from] rusqlite::Error),

    #[error("could not bind proxy listener on 127.0.0.1: {0}")]
    Bind(std::io::Error),

    #[error("failed to spawn child command `{cmd}`: {source}")]
    Spawn {
        cmd: String,
        #[source]
        source: std::io::Error,
    },

    #[error("adapter: {0}")]
    Adapter(String),

    #[error("config: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, Error>;
