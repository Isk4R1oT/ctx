//! Output rendering — the exact contract of `docs/11-cli-design-system.md`.
//!
//! No emoji ever. Sentence case. Terracotta accent `#d97757` only on
//! the action bullet / headings. `ColorMode::None` (pipe / `NO_COLOR` /
//! non-TTY) emits zero escape codes, no box, no banner — one grep-clean
//! record per line. `ColorMode` is resolved once at startup, passed in.

use std::io::Write;

use anstyle::{AnsiColor, Color, RgbColor, Style};

use crate::color::ColorMode;
use crate::timeline::Timeline;

// Exact glyphs (doc 11 §2.1/§3.1/§2.3). No emoji.
const ACTION: char = '\u{23FA}'; // ⏺
const RESULT: char = '\u{23BF}'; // ⎿
const SUB: char = '\u{00B7}'; //   ·
const TL: char = '\u{256D}';
const TR: char = '\u{256E}';
const BL: char = '\u{2570}';
const BR: char = '\u{256F}';
const HZ: char = '\u{2500}';
const VT: char = '\u{2502}';
const SPARK: char = '\u{273B}'; // ✻

/// Semantic role → a `ColorMode`-aware [`Style`] (doc 11 §1.2).
fn accent(mode: ColorMode) -> Style {
    let c = match mode {
        ColorMode::Truecolor => Some(Color::Rgb(RgbColor(0xD9, 0x77, 0x57))),
        ColorMode::Ansi256 => Some(Color::Ansi256(anstyle::Ansi256Color(173))),
        ColorMode::Ansi16 => Some(Color::Ansi(AnsiColor::Yellow)),
        ColorMode::None => None,
    };
    Style::new().fg_color(c).bold()
}

fn dim(mode: ColorMode) -> Style {
    match mode {
        ColorMode::None => Style::new(),
        _ => Style::new().dimmed(),
    }
}

fn paint(style: Style, s: &str) -> String {
    format!("{}{s}{}", style.render(), style.render_reset())
}

/// One-shot renderer; holds the once-resolved stdout color mode.
pub struct Renderer {
    mode: ColorMode,
}

impl Renderer {
    #[must_use]
    pub fn new(stdout_mode: ColorMode) -> Self {
        Self { mode: stdout_mode }
    }

    /// The bare-invocation banner (doc 11 §2.1). Only ever shown with no
    /// subcommand and never in plain mode.
    ///
    /// # Errors
    /// Propagates any write error to the supplied stream.
    pub fn banner(&self, w: &mut impl Write) -> std::io::Result<()> {
        if self.mode == ColorMode::None {
            return Ok(());
        }
        let a = accent(self.mode);
        let d = dim(self.mode);
        let width = 58;
        let line = HZ.to_string().repeat(width);
        writeln!(w, "{TL}{line}{TR}")?;
        writeln!(w, "{VT}{}{VT}", " ".repeat(width))?;
        writeln!(
            w,
            "{VT}  {} {}  {}{}",
            paint(a, &SPARK.to_string()),
            paint(a, "ctx"),
            paint(d, "context-window x-ray at the LLM-API boundary"),
            " ".repeat(width.saturating_sub(48))
        )?;
        writeln!(w, "{VT}{}{VT}", " ".repeat(width))?;
        writeln!(w, "{BL}{line}{BR}")?;
        writeln!(w)?;
        writeln!(w, "{}", paint(d, "run:  ctx run -- <your agent command>"))?;
        Ok(())
    }

    /// The F0 captured-timeline summary. In `ColorMode::None` every line
    /// is a single stable grep-clean record (doc 11 §2.4).
    ///
    /// # Errors
    /// Propagates any write error to the supplied stream.
    pub fn summary(&self, w: &mut impl Write, t: &Timeline) -> std::io::Result<()> {
        let a = accent(self.mode);
        let d = dim(self.mode);
        let steps = t.steps.len();
        let toks = t.total_prompt_tokens();

        if self.mode == ColorMode::None {
            writeln!(w, "> captured {steps} step(s)")?;
            for s in &t.steps {
                let prov = s.provider.map_or("unknown", |p| match p {
                    crate::adapter::Provider::Anthropic => "anthropic",
                    crate::adapter::Provider::OpenAiCompat => "openai-compat",
                });
                let status = s.response.as_ref().map_or(0, |r| r.status);
                writeln!(
                    w,
                    "step {} {} {} {} status={} prompt_tokens={}",
                    s.index, prov, s.request.method, s.request.path, status, s.prompt_tokens
                )?;
            }
            writeln!(
                w,
                "summary: {steps} step(s), {toks} prompt tokens ({})",
                crate::tokenizer::ACCURACY_LABEL
            )?;
            return Ok(());
        }

        writeln!(
            w,
            "{} Captured {}",
            paint(a, &ACTION.to_string()),
            paint(d, &format!("{steps} step(s) at the wire"))
        )?;
        for s in &t.steps {
            let prov = s.provider.map_or("unknown", |p| match p {
                crate::adapter::Provider::Anthropic => "anthropic",
                crate::adapter::Provider::OpenAiCompat => "openai-compat",
            });
            let status = s.response.as_ref().map_or(0, |r| r.status);
            writeln!(
                w,
                "  {} {}",
                paint(d, &RESULT.to_string()),
                paint(
                    d,
                    &format!(
                        "step {} {SUB} {prov} {SUB} {} {} {SUB} {status} {SUB} {} tok",
                        s.index, s.request.method, s.request.path, s.prompt_tokens
                    )
                )
            )?;
        }
        writeln!(w)?;
        writeln!(w, "{}", paint(a, "Summary"))?;
        writeln!(
            w,
            "  {}",
            paint(
                d,
                &format!(
                    "{steps} step(s) {SUB} {toks} prompt tokens {SUB} {}",
                    crate::tokenizer::ACCURACY_LABEL
                )
            )
        )?;
        Ok(())
    }

    /// `--json` — the CI citizen. Stable serialization of the timeline.
    ///
    /// # Errors
    /// Propagates serialization or write errors.
    pub fn json(&self, w: &mut impl Write, t: &Timeline) -> crate::Result<()> {
        let s = serde_json::to_string_pretty(t)?;
        writeln!(w, "{s}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline() -> Timeline {
        let mut t = Timeline::new();
        let i = t.record_request(
            "POST",
            "/v1/messages",
            &[],
            b"{\"model\":\"m\",\"system\":\"s\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}",
        );
        t.record_response(i, 200, &[], b"{}");
        t
    }

    #[test]
    fn plain_mode_has_no_escape_codes() {
        let r = Renderer::new(ColorMode::None);
        let mut buf = Vec::new();
        r.summary(&mut buf, &timeline()).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(!s.contains('\u{1b}'), "plain mode must emit zero escapes");
        assert!(s.contains("step 0 anthropic POST /v1/messages status=200"));
        assert!(s.lines().count() >= 3);
    }

    #[test]
    fn plain_mode_banner_is_silent() {
        let r = Renderer::new(ColorMode::None);
        let mut buf = Vec::new();
        r.banner(&mut buf).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn json_is_valid_and_roundtrips() {
        let r = Renderer::new(ColorMode::None);
        let mut buf = Vec::new();
        r.json(&mut buf, &timeline()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["steps"][0]["request"]["path"], "/v1/messages");
    }

    #[test]
    fn tty_summary_carries_accent_and_no_emoji() {
        let r = Renderer::new(ColorMode::Truecolor);
        let mut buf = Vec::new();
        r.summary(&mut buf, &timeline()).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains('\u{23FA}'));
        // No emoji: only the sanctioned glyph set is allowed.
        assert!(!s.chars().any(|c| c as u32 >= 0x1_F000));
    }
}
