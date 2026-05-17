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

        /// Everything after `--` is the child command, verbatim.
        #[arg(last = true, required = true, value_name = "CMD")]
        command: Vec<String>,
    },

    /// Inspect a previously `--save`d session (post-hoc).
    Open {
        #[arg(value_name = "FILE")]
        path: PathBuf,
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
}
