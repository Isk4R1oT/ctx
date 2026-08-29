//! CLI surface — the canonical wire-proxy verbs (D-001). `ctx run --
//! <cmd>` is primary; `--json`/`--deep`/`--color`/`--save`/`--open` are
//! the locked flags. Static-scan verbs (`scan`/`lint`/`init`) are
//! rejected of record and deliberately absent.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::color::ColorFlag;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

impl From<ColorChoice> for ColorFlag {
    fn from(c: ColorChoice) -> Self {
        match c {
            ColorChoice::Auto => ColorFlag::Auto,
            ColorChoice::Always => ColorFlag::Always,
            ColorChoice::Never => ColorFlag::Never,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "ctx",
    version,
    about = "htop / EXPLAIN ANALYZE for LLM prompts (wire-proxy)",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Color mode (precedence still yields to `NO_COLOR` on `auto`).
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto, global = true)]
    pub color: ColorChoice,

    /// Emit machine-readable JSON instead of the human summary.
    #[arg(long, global = true)]
    pub json: bool,

    /// Reserved for the F2/F3 per-step drill-down (goal 2/2).
    #[arg(long, global = true)]
    pub deep: bool,

    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Run an agent command behind the transparent proxy and X-ray it.
    Run {
        /// Opt-in: persist the captured session to a local `SQLite` file.
        #[arg(long, value_name = "FILE")]
        save: Option<PathBuf>,

        /// Explicit upstream base URL (full, path preserved). The
        /// most-explicit source — beats env and key inference (D-017).
        #[arg(long, value_name = "URL", conflicts_with = "provider")]
        to: Option<String>,

        /// Known-provider shortcut: openai | anthropic | openrouter |
        /// groq | google (resolves to that provider's base, D-017).
        #[arg(long, value_name = "NAME")]
        provider: Option<String>,

        /// Everything after `--` is the child command, verbatim.
        #[arg(last = true, required = true, value_name = "CMD")]
        command: Vec<String>,
    },

    /// Inspect a previously `--save`d session (post-hoc).
    Open {
        #[arg(value_name = "FILE")]
        path: PathBuf,
    },

    /// F2 — show the verbatim assembled prompt of one captured step.
    View {
        #[arg(value_name = "FILE")]
        path: PathBuf,

        /// Step index (default: the last captured step).
        #[arg(long)]
        step: Option<usize>,

        /// Open the interactive pager (ignored when piped / `--json`).
        #[arg(long)]
        tui: bool,

        /// F4 — split the context into semantic zones instead of dumping
        /// the raw wire body.
        #[arg(long)]
        zones: bool,
    },

    /// F3 — per-step context diff (default: step N vs N-1).
    Diff {
        #[arg(value_name = "FILE")]
        path: PathBuf,

        /// Earlier step (default: `to - 1`).
        #[arg(long)]
        from: Option<usize>,

        /// Later step (default: the last captured step).
        #[arg(long)]
        to: Option<usize>,
    },
}

/// Parse the process arguments.
#[must_use]
pub fn parse() -> Cli {
    Cli::parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn run_captures_trailing_command_verbatim() {
        let c = Cli::try_parse_from(["ctx", "run", "--", "python", "-c", "x"]).unwrap();
        match c.cmd {
            Some(Cmd::Run { command, .. }) => {
                assert_eq!(command, vec!["python", "-c", "x"]);
            }
            _ => panic!("expected run"),
        }
    }

    #[test]
    fn color_flag_maps() {
        let c = Cli::try_parse_from(["ctx", "--color", "never", "open", "x.db"]).unwrap();
        assert_eq!(ColorFlag::from(c.color), ColorFlag::Never);
    }

    #[test]
    fn bare_invocation_has_no_subcommand() {
        let c = Cli::try_parse_from(["ctx"]).unwrap();
        assert!(c.cmd.is_none());
    }

    #[test]
    fn run_parses_to_and_provider_flags() {
        let c = Cli::try_parse_from(["ctx", "run", "--to", "https://gw/v1", "--", "sh"]).unwrap();
        match c.cmd {
            Some(Cmd::Run { to, provider, .. }) => {
                assert_eq!(to.as_deref(), Some("https://gw/v1"));
                assert_eq!(provider, None);
            }
            _ => panic!("expected run"),
        }
        let c = Cli::try_parse_from(["ctx", "run", "--provider", "openrouter", "--", "sh"]).unwrap();
        match c.cmd {
            Some(Cmd::Run { to, provider, .. }) => {
                assert_eq!(provider.as_deref(), Some("openrouter"));
                assert_eq!(to, None);
            }
            _ => panic!("expected run"),
        }
    }

    #[test]
    fn run_to_and_provider_are_mutually_exclusive() {
        // One explicit source only — clap must reject both together.
        let e = Cli::try_parse_from([
            "ctx", "run", "--to", "https://x", "--provider", "openai", "--", "sh",
        ]);
        assert!(e.is_err(), "--to and --provider must conflict");
    }
}
