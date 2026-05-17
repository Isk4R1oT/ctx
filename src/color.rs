//! Color-mode resolution — the mandatory precedence from
//! `docs/11-cli-design-system.md` §1.3. Resolved **once at startup**,
//! independently per stream. Pure core (`resolve`) is exhaustively
//! testable; `for_stream` is the thin real-env wrapper.

use std::io::IsTerminal;

/// The palette a stream may use, resolved once at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Truecolor,
    Ansi256,
    Ansi16,
    None,
}

impl ColorMode {
    #[must_use]
    pub fn enabled(self) -> bool {
        self != ColorMode::None
    }
}

/// The `--color` flag value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFlag {
    Always,
    Auto,
    Never,
}

/// Which stream we are resolving for (decided independently, §1.3 rule 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// All environment inputs, captured explicitly so the core is a pure
/// function (no hidden global reads — testable per the precedence table).
#[derive(Debug, Clone)]
pub struct Env {
    pub no_color: bool,
    pub clicolor_force: Option<String>,
    pub clicolor: Option<String>,
    pub colorterm: Option<String>,
    pub term: Option<String>,
}

impl Env {
    /// Capture the real process environment once.
    #[must_use]
    pub fn capture() -> Self {
        Self {
            no_color: std::env::var_os("NO_COLOR").is_some(),
            clicolor_force: std::env::var("CLICOLOR_FORCE").ok(),
            clicolor: std::env::var("CLICOLOR").ok(),
            colorterm: std::env::var("COLORTERM").ok(),
            term: std::env::var("TERM").ok(),
        }
    }
}

fn palette_depth(env: &Env) -> ColorMode {
    let ct = env.colorterm.as_deref().unwrap_or("");
    if ct.contains("truecolor") || ct.contains("24bit") {
        return ColorMode::Truecolor;
    }
    let term = env.term.as_deref().unwrap_or("");
    if term.contains("256color") || term.contains("-256") {
        return ColorMode::Ansi256;
    }
    if term.is_empty() && ct.is_empty() {
        // Undecidable ⇒ never assume truecolor (§1.3 rule 7).
        return ColorMode::Ansi16;
    }
    ColorMode::Ansi16
}

fn truthy(v: Option<&str>) -> bool {
    matches!(v, Some(s) if !s.is_empty() && s != "0")
}

/// Pure resolution of the §1.3 precedence. `is_tty` is the per-stream
/// `IsTerminal` result; everything else is captured `Env`.
#[must_use]
pub fn resolve(flag: ColorFlag, env: &Env, is_tty: bool) -> ColorMode {
    // 1. --color flag is absolute.
    match flag {
        ColorFlag::Never => return ColorMode::None,
        ColorFlag::Always => return palette_depth(env),
        ColorFlag::Auto => {}
    }
    // 2. NO_COLOR set (any value, even empty) ⇒ None.
    if env.no_color {
        return ColorMode::None;
    }
    // 3. CLICOLOR_FORCE set and non-zero ⇒ force color even if not a TTY.
    if truthy(env.clicolor_force.as_deref()) {
        return palette_depth(env);
    }
    // 4. CLICOLOR=0 ⇒ None.
    if matches!(env.clicolor.as_deref(), Some("0")) {
        return ColorMode::None;
    }
    // 5. Per-stream TTY check.
    if !is_tty {
        return ColorMode::None;
    }
    // 6/7. Palette depth from COLORTERM/TERM, else Ansi16.
    palette_depth(env)
}

/// Real wrapper: resolve for one stream using its actual `IsTerminal`.
#[must_use]
pub fn for_stream(flag: ColorFlag, env: &Env, stream: Stream) -> ColorMode {
    let is_tty = match stream {
        Stream::Stdout => std::io::stdout().is_terminal(),
        Stream::Stderr => std::io::stderr().is_terminal(),
    };
    resolve(flag, env, is_tty)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env {
            no_color: false,
            clicolor_force: None,
            clicolor: None,
            colorterm: Some("truecolor".into()),
            term: Some("xterm-256color".into()),
        }
    }

    #[test]
    fn never_flag_is_absolute_over_tty() {
        assert_eq!(resolve(ColorFlag::Never, &env(), true), ColorMode::None);
    }

    #[test]
    fn always_flag_beats_no_color() {
        let mut e = env();
        e.no_color = true;
        assert_eq!(resolve(ColorFlag::Always, &e, false), ColorMode::Truecolor);
    }

    #[test]
    fn no_color_beats_auto_on_tty() {
        let mut e = env();
        e.no_color = true;
        assert_eq!(resolve(ColorFlag::Auto, &e, true), ColorMode::None);
    }

    #[test]
    fn clicolor_force_overrides_non_tty() {
        let mut e = env();
        e.clicolor_force = Some("1".into());
        assert_eq!(resolve(ColorFlag::Auto, &e, false), ColorMode::Truecolor);
    }

    #[test]
    fn clicolor_zero_disables() {
        let mut e = env();
        e.clicolor = Some("0".into());
        assert_eq!(resolve(ColorFlag::Auto, &e, true), ColorMode::None);
    }

    #[test]
    fn piped_stream_is_plain() {
        assert_eq!(resolve(ColorFlag::Auto, &env(), false), ColorMode::None);
    }

    #[test]
    fn depth_falls_back_to_ansi16_when_undecidable() {
        let e = Env {
            no_color: false,
            clicolor_force: None,
            clicolor: None,
            colorterm: None,
            term: None,
        };
        assert_eq!(resolve(ColorFlag::Always, &e, true), ColorMode::Ansi16);
    }

    #[test]
    fn depth_256_from_term() {
        let e = Env {
            no_color: false,
            clicolor_force: None,
            clicolor: None,
            colorterm: None,
            term: Some("screen-256color".into()),
        };
        assert_eq!(resolve(ColorFlag::Auto, &e, true), ColorMode::Ansi256);
    }
}
