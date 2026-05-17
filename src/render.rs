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

/// Dimmed secondary style. Only ever called on a colored path — plain
/// (`ColorMode::None`) renders via the dedicated `*_plain` branches and
/// never paints — so this needs no mode arm.
fn dim() -> Style {
    Style::new().dimmed()
}

fn paint(style: Style, s: &str) -> String {
    format!("{}{s}{}", style.render(), style.render_reset())
}

/// F2 one-shot TTY header (doc-11 grammar) printed above the verbatim
/// body. Never called in `ColorMode::None` (that path is byte-exact).
///
/// # Errors
/// Propagates write errors.
pub fn verbatim_header(
    w: &mut impl Write,
    mode: ColorMode,
    sel: &crate::view::SelectedStep,
) -> std::io::Result<()> {
    let acc = accent(mode);
    let faint = dim();
    let prov = crate::view::provider_label(sel.provider);
    writeln!(
        w,
        "{} Verbatim {}",
        paint(acc, &ACTION.to_string()),
        paint(
            faint,
            &format!(
                "step {}/{} {SUB} {prov} {SUB} {} {}",
                sel.index, sel.steps, sel.method, sel.path
            )
        )
    )?;
    writeln!(w)?;
    Ok(())
}

// doc-11 §1.2 success/error + §5.1 diff backgrounds.
fn success(mode: ColorMode) -> Style {
    let c = match mode {
        ColorMode::Truecolor => Some(Color::Rgb(RgbColor(0x78, 0x8C, 0x5D))),
        ColorMode::Ansi256 => Some(Color::Ansi256(anstyle::Ansi256Color(107))),
        ColorMode::Ansi16 => Some(Color::Ansi(AnsiColor::Green)),
        ColorMode::None => None,
    };
    Style::new().fg_color(c)
}

fn error(mode: ColorMode) -> Style {
    let c = match mode {
        ColorMode::Truecolor => Some(Color::Rgb(RgbColor(0xBF, 0x4D, 0x43))),
        ColorMode::Ansi256 => Some(Color::Ansi256(anstyle::Ansi256Color(167))),
        ColorMode::Ansi16 => Some(Color::Ansi(AnsiColor::Red)),
        ColorMode::None => None,
    };
    Style::new().fg_color(c)
}

// Background fills only on Truecolor/Ansi256 (doc-11 §5.1); 16/None rely
// solely on the +/-/space prefixes (survives copy-paste & `| diff`).
fn diff_bg(mode: ColorMode, add: bool) -> Style {
    let c = match (mode, add) {
        (ColorMode::Truecolor, true) => Some(Color::Rgb(RgbColor(0x1D, 0x33, 0x20))),
        (ColorMode::Truecolor, false) => Some(Color::Rgb(RgbColor(0x3A, 0x1D, 0x1D))),
        (ColorMode::Ansi256, true) => Some(Color::Ansi256(anstyle::Ansi256Color(22))),
        (ColorMode::Ansi256, false) => Some(Color::Ansi256(anstyle::Ansi256Color(52))),
        _ => None,
    };
    Style::new().bg_color(c)
}

/// F3 per-step context diff (doc-11 §5.1). `ColorMode::None` ⇒ a
/// grep-clean unified record per line (one line, `+ `/`- `/`  ` prefix,
/// zero escapes); a TTY adds success/error color + (Truecolor/256) bg.
///
/// # Errors
/// Propagates write errors.
pub fn diff_block(
    w: &mut impl Write,
    mode: ColorMode,
    d: &crate::diff::StepDiff,
) -> std::io::Result<()> {
    use crate::diff::Op;
    let label = crate::tokenizer::ACCURACY_LABEL;

    if mode == ColorMode::None {
        writeln!(w, "> diff step {} -> {}", d.from, d.to)?;
        for l in &d.lines {
            let sign = match l.op {
                Op::Add => '+',
                Op::Remove => '-',
                Op::Keep => ' ',
            };
            writeln!(w, "{sign} {}", l.text)?;
        }
        writeln!(
            w,
            "summary: +{}/-{} lines, +{}/-{} tokens ({label})",
            d.added_lines, d.removed_lines, d.added_tokens, d.removed_tokens
        )?;
        return Ok(());
    }

    let acc = accent(mode);
    let faint = dim();
    writeln!(
        w,
        "{} Diff {}",
        paint(acc, &ACTION.to_string()),
        paint(faint, &format!("step {} -> {}", d.from, d.to))
    )?;
    for l in &d.lines {
        let (sign, fg) = match l.op {
            Op::Add => ('+', success(mode)),
            Op::Remove => ('-', error(mode)),
            Op::Keep => (' ', faint),
        };
        // One combined style so the bg fill actually spans the row
        // (doc-11 §5.1: added/removed lines carry diff_*_bg).
        let bg = match l.op {
            Op::Add => diff_bg(mode, true),
            Op::Remove => diff_bg(mode, false),
            Op::Keep => Style::new(),
        };
        let style = fg.bg_color(bg.get_bg_color());
        writeln!(w, "{}", paint(style, &format!("{sign} {}", l.text)))?;
    }
    writeln!(w)?;
    writeln!(w, "{}", paint(acc, "Summary"))?;
    writeln!(
        w,
        "  {}",
        paint(
            faint,
            &format!(
                "+{}/-{} lines {SUB} +{}/-{} tokens {SUB} {label}",
                d.added_lines, d.removed_lines, d.added_tokens, d.removed_tokens
            )
        )
    )?;
    Ok(())
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
        let d = dim();
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
        let d = dim();
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

    /// `--json` for `run`/`open`: `{ "steps": [...], "composition": {...} }`
    /// — `steps` stays top-level (F0 contract preserved, D-005), F1 data
    /// additive at `composition`.
    ///
    /// # Errors
    /// Propagates serialization or write errors.
    pub fn report_json(
        &self,
        w: &mut impl Write,
        t: &Timeline,
        c: &crate::compose::Composition,
    ) -> crate::Result<()> {
        #[derive(serde::Serialize)]
        struct RunReport<'a> {
            #[serde(flatten)]
            timeline: &'a Timeline,
            composition: &'a crate::compose::Composition,
        }
        let s = serde_json::to_string_pretty(&RunReport {
            timeline: t,
            composition: c,
        })?;
        writeln!(w, "{s}")?;
        Ok(())
    }

    /// The F1 headline — composition + waste indictment (PROJECT.md §6).
    /// Pure measurement; `--deep` per-tool detail is in `c.tools_deep`.
    /// `ColorMode::None` ⇒ one grep-clean record per line (doc 11 §2.4).
    ///
    /// # Errors
    /// Propagates any write error to the supplied stream.
    pub fn composition(
        &self,
        out: &mut impl Write,
        comp: &crate::compose::Composition,
    ) -> std::io::Result<()> {
        if self.mode == ColorMode::None {
            Self::composition_plain(out, comp)
        } else {
            self.composition_tty(out, comp)
        }
    }

    fn composition_plain(
        out: &mut impl Write,
        comp: &crate::compose::Composition,
    ) -> std::io::Result<()> {
        match comp.focus_step {
            Some(ix) => writeln!(out, "> composition step {ix} {} tokens", comp.total_tokens)?,
            None => writeln!(out, "> composition no captured prompt")?,
        }
        for part in &comp.components {
            writeln!(
                out,
                "component {} {} {}%",
                part.label, part.tokens, part.pct
            )?;
        }
        for tool in &comp.tools_deep {
            writeln!(out, "tool {} {} {}%", tool.label, tool.tokens, tool.pct)?;
        }
        for ind in &comp.indictments {
            writeln!(
                out,
                "waste {} wasted_tokens={} {}",
                ind.code, ind.wasted_tokens, ind.detail
            )?;
        }
        writeln!(
            out,
            "summary: {} tokens, {} finding(s) ({})",
            comp.total_tokens,
            comp.indictments.len(),
            crate::tokenizer::ACCURACY_LABEL
        )
    }

    fn composition_tty(
        &self,
        out: &mut impl Write,
        comp: &crate::compose::Composition,
    ) -> std::io::Result<()> {
        let acc = accent(self.mode);
        let faint = dim();
        let head = match comp.focus_step {
            Some(ix) => format!("step {ix} {SUB} {} tokens", comp.total_tokens),
            None => "no captured prompt".to_string(),
        };
        writeln!(
            out,
            "{} Composition {}",
            paint(acc, &ACTION.to_string()),
            paint(faint, &head)
        )?;
        for part in &comp.components {
            writeln!(
                out,
                "  {} {}",
                paint(faint, &RESULT.to_string()),
                paint(
                    faint,
                    &format!(
                        "{} {SUB} {}% {SUB} {} tok",
                        part.label, part.pct, part.tokens
                    )
                )
            )?;
        }
        for tool in &comp.tools_deep {
            writeln!(
                out,
                "     {} {}",
                paint(faint, &SUB.to_string()),
                paint(
                    faint,
                    &format!(
                        "{} {SUB} {}% {SUB} {} tok",
                        tool.label, tool.pct, tool.tokens
                    )
                )
            )?;
        }
        if !comp.indictments.is_empty() {
            writeln!(out)?;
            writeln!(out, "{}", paint(acc, "Waste"))?;
            for ind in &comp.indictments {
                writeln!(
                    out,
                    "  {} {}",
                    paint(faint, &RESULT.to_string()),
                    paint(
                        faint,
                        &format!(
                            "{} {SUB} ~{} tok {SUB} {}",
                            ind.code, ind.wasted_tokens, ind.detail
                        )
                    )
                )?;
            }
        }
        writeln!(out)?;
        writeln!(out, "{}", paint(acc, "Summary"))?;
        writeln!(
            out,
            "  {}",
            paint(
                faint,
                &format!(
                    "{} tokens {SUB} {} finding(s) {SUB} {}",
                    comp.total_tokens,
                    comp.indictments.len(),
                    crate::tokenizer::ACCURACY_LABEL
                )
            )
        )
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
