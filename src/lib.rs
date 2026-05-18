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
pub mod compose;
pub mod diff;
pub mod proxy;
pub mod registry;
pub mod render;
pub mod run;
pub mod store;
pub mod timeline;
pub mod tokenizer;
pub mod view;
pub mod window;

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
    // stdout color resolved once at startup (doc-11 §1.3). stderr is
    // resolved independently when a stderr renderer exists (spinner is
    // post-F0); not computed speculatively here.
    let stdout_mode = color::for_stream(flag, &env, Stream::Stdout);
    // Decoration & the interactive pager gate on real TTY-ness, resolved
    // once — independent of palette, so `--color=always | pipe` stays
    // byte-exact and never enters raw mode.
    let stdout_tty = color::is_terminal(Stream::Stdout);
    let stdin_tty = color::is_terminal(Stream::Stdin);
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
            // F1 is the headline (D-005). --json keeps `steps` top-level.
            let comp = compose::compose(&outcome.timeline, cli.deep);
            if cli.json {
                renderer.report_json(&mut out, &outcome.timeline, &comp)?;
            } else {
                renderer.composition(&mut out, &comp)?;
            }
            out.flush()?;
            Ok(outcome.exit_code)
        }
        Some(Cmd::Open { path }) => {
            let timeline = store::load(&path)?;
            let comp = compose::compose(&timeline, cli.deep);
            if cli.json {
                renderer.report_json(&mut out, &timeline, &comp)?;
            } else {
                renderer.composition(&mut out, &comp)?;
            }
            out.flush()?;
            Ok(0)
        }
        Some(Cmd::View { path, step, tui }) => {
            let timeline = store::load(&path)?;
            let sel = view::select(&timeline, step)?;
            if cli.json {
                view::json(&mut out, &sel)?;
            } else if view::should_page(tui, stdout_tty, stdin_tty) {
                view::pager(&sel)?;
            } else {
                view::oneshot(&mut out, stdout_tty, stdout_mode, &sel)?;
            }
            out.flush()?;
            Ok(0)
        }
        Some(Cmd::Diff { path, from, to }) => {
            let timeline = store::load(&path)?;
            let d = diff::diff(&timeline, from, to)?;
            if cli.json {
                diff::json(&mut out, &d)?;
            } else {
                render::diff_block(&mut out, stdout_mode, &d)?;
            }
            out.flush()?;
            Ok(0)
        }
    }
}
