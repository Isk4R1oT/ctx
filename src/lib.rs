//! ctx — htop / EXPLAIN ANALYZE for LLM prompts.
//!
//! A local-first, zero-config, single static binary that sits as a
//! transparent reverse-proxy at the LLM-API boundary and X-rays the
//! *actually assembled* wire prompt. Canonical mechanism + non-goals:
//! `docs/PROJECT.md`; reconciled CLI surface: `docs/DECISIONS.md` D-001.

mod error;

pub mod adapter;
pub mod cli;
pub mod color;
pub mod proxy;
pub mod render;
pub mod run;
pub mod store;
pub mod timeline;
pub mod tokenizer;

pub use error::{Error, Result};

use std::io::Write;

use crate::cli::{Cli, Cmd};
use crate::color::{Env, Stream};
use crate::render::Renderer;
use crate::store::Sink;

/// Application entrypoint. Resolves color **once**, dispatches the
/// canonical verbs, returns the process exit code (the child's, for
/// `run`). Errors are surfaced with context at the binary boundary.
///
/// # Errors
/// Propagates [`Error`] from the proxy, child spawn, or persistence.
pub async fn run_app(cli: Cli) -> Result<i32> {
    let env = Env::capture();
    let flag = cli.color.into();
    let stdout_mode = color::for_stream(flag, &env, Stream::Stdout);
    let _stderr_mode = color::for_stream(flag, &env, Stream::Stderr);
    let renderer = Renderer::new(stdout_mode);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match cli.cmd {
        None => {
            renderer.banner(&mut out)?;
            Ok(0)
        }
        Some(Cmd::Run { save, command }) => {
            let outcome = run::execute(&command).await?;
            let sink = save.map_or(Sink::Ephemeral, Sink::Sqlite);
            store::persist(&outcome.timeline, &sink)?;
            if cli.json {
                renderer.json(&mut out, &outcome.timeline)?;
            } else {
                renderer.summary(&mut out, &outcome.timeline)?;
            }
            out.flush()?;
            Ok(outcome.exit_code)
        }
        Some(Cmd::Open { path }) => {
            let timeline = store::load(&path)?;
            if cli.json {
                renderer.json(&mut out, &timeline)?;
            } else {
                renderer.summary(&mut out, &timeline)?;
            }
            out.flush()?;
            Ok(0)
        }
    }
}
