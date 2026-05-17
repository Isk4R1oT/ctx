//! F2 — verbatim assembled context at any step.
//!
//! The captured `request.body` *is* the byte-exact assembled wire
//! prompt (F0 stored it verbatim). F2 surfaces it: one-shot (the
//! pipe-exact path) + an interactive `ratatui` pager (doc-11 rounded
//! border). Plain/`--json`/non-TTY never enters the TUI.

use std::io::Write;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::text::Text;
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::{Frame, Terminal};

use crate::adapter::Provider;
use crate::color::ColorMode;
use crate::timeline::Timeline;

/// One selected step's verbatim assembled prompt + its wire metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SelectedStep {
    #[serde(rename = "step")]
    pub index: usize,
    pub steps: usize,
    pub provider: Option<Provider>,
    pub method: String,
    pub path: String,
    /// The assembled wire prompt, byte-for-byte as the agent sent it.
    pub body: String,
}

/// Select a step (default: the last). Pure; errors are explicit.
///
/// # Errors
/// [`crate::Error::Config`] if there are no steps or `step` is out of range.
pub fn select(timeline: &Timeline, step: Option<usize>) -> crate::Result<SelectedStep> {
    let total = timeline.steps.len();
    if total == 0 {
        return Err(crate::Error::Config(
            "no captured steps — run `ctx run -- <cmd> --save <file>` first".to_string(),
        ));
    }
    let idx = step.unwrap_or(total - 1);
    let s = timeline
        .steps
        .get(idx)
        .ok_or_else(|| crate::Error::Config(format!("step {idx} out of range (0..{total})")))?;
    Ok(SelectedStep {
        index: idx,
        steps: total,
        provider: s.provider,
        method: s.request.method.clone(),
        path: s.request.path.clone(),
        body: s.request.body.clone(),
    })
}

/// Single source of the provider label (shared with `render`'s F2
/// header so the mapping can't silently diverge).
#[must_use]
pub(crate) fn provider_label(p: Option<Provider>) -> &'static str {
    match p {
        Some(Provider::Anthropic) => "anthropic",
        Some(Provider::OpenAiCompat) => "openai-compat",
        None => "unknown",
    }
}

/// One-shot output. `decorate` is the caller's resolved **stdout
/// TTY-ness** (NOT `ColorMode`): a pipe is byte-exact even under
/// `--color=always`, so `ctx view … | sha256` always equals the
/// captured prompt (bounded only by F0's UTF-8 storage). On a TTY the
/// doc-11 header is added (colored per `mode`); the body stays verbatim.
///
/// # Errors
/// Propagates write errors.
pub fn oneshot(
    w: &mut impl Write,
    decorate: bool,
    mode: ColorMode,
    sel: &SelectedStep,
) -> std::io::Result<()> {
    if !decorate {
        // The pipe contract: nothing but the assembled bytes.
        return w.write_all(sel.body.as_bytes());
    }
    crate::render::verbatim_header(w, mode, sel)?;
    w.write_all(sel.body.as_bytes())?;
    if !sel.body.ends_with('\n') {
        writeln!(w)?;
    }
    Ok(())
}

/// `--json`: metadata + the verbatim body as a JSON string.
///
/// # Errors
/// Propagates serialization or write errors.
pub fn json(w: &mut impl Write, sel: &SelectedStep) -> crate::Result<()> {
    writeln!(w, "{}", serde_json::to_string_pretty(sel)?)?;
    Ok(())
}

/// Pure pager frame — used by both the live loop and the snapshot test
/// (rendered into a `TestBackend` buffer). Doc-11: rounded border.
pub fn draw_pager(frame: &mut Frame, sel: &SelectedStep, scroll: u16) {
    let title = format!(
        " ctx · verbatim · step {}/{} · {} · {} ",
        sel.index,
        sel.steps,
        provider_label(sel.provider),
        sel.path
    );
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .title_bottom(" q quit · ↑/↓ scroll · PgUp/PgDn page ");
    let body = Paragraph::new(Text::raw(&sel.body))
        .block(block)
        .scroll((scroll, 0));
    frame.render_widget(body, frame.area());
}

/// What a key does to the pager. Pure — the entire pager *logic* lives
/// here so it is exhaustively unit-/mutation-testable; `pager` itself is
/// only the untestable PTY glue (excluded in `mutants.toml`, like main).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyOutcome {
    Quit,
    Scroll(u16),
    Ignore,
}

pub(crate) fn key_action(code: KeyCode, kind: KeyEventKind, scroll: u16, lines: u16) -> KeyOutcome {
    if kind != KeyEventKind::Press {
        return KeyOutcome::Ignore;
    }
    match code {
        KeyCode::Char('q') | KeyCode::Esc => KeyOutcome::Quit,
        KeyCode::Down => KeyOutcome::Scroll(scroll.saturating_add(1).min(lines)),
        KeyCode::Up => KeyOutcome::Scroll(scroll.saturating_sub(1)),
        KeyCode::PageDown => KeyOutcome::Scroll(scroll.saturating_add(20).min(lines)),
        KeyCode::PageUp => KeyOutcome::Scroll(scroll.saturating_sub(20)),
        KeyCode::Home => KeyOutcome::Scroll(0),
        KeyCode::End => KeyOutcome::Scroll(lines),
        _ => KeyOutcome::Ignore,
    }
}

/// The View dispatch gate (pure, so the MED-1 fix is mutation-pinned,
/// not just inspected): the raw-mode pager is entered **only** when the
/// user asked for it AND both streams are real TTYs. A piped stdout
/// would corrupt/panic; a non-TTY stdin would wedge `event::read()`.
#[must_use]
pub(crate) fn should_page(tui: bool, stdout_tty: bool, stdin_tty: bool) -> bool {
    tui && stdout_tty && stdin_tty
}

/// Interactive pager. Only reached when stdout AND stdin are real TTYs
/// (resolved once at startup); plain/`--json`/pipe use `oneshot`/`json`.
/// Pure glue: ratatui owns terminal setup/restore (verified) and all
/// key logic is delegated to the tested `key_action`.
///
/// # Errors
/// Propagates terminal or event I/O errors.
pub fn pager(sel: &SelectedStep) -> crate::Result<()> {
    let lines = u16::try_from(sel.body.lines().count()).unwrap_or(u16::MAX);
    ratatui::run(|terminal| -> std::io::Result<()> {
        let mut scroll: u16 = 0;
        loop {
            terminal.draw(|f| draw_pager(f, sel, scroll))?;
            if let Event::Key(k) = event::read()? {
                match key_action(k.code, k.kind, scroll, lines) {
                    KeyOutcome::Quit => break,
                    KeyOutcome::Scroll(s) => scroll = s,
                    KeyOutcome::Ignore => {}
                }
            }
        }
        Ok(())
    })
    .map_err(crate::Error::Io)
}

/// Render `draw_pager` into an in-memory buffer (no live terminal) and
/// return it as text — the deterministic snapshot surface.
///
/// # Errors
/// Propagates the `TestBackend` draw error.
pub fn render_to_string(sel: &SelectedStep, w: u16, h: u16) -> crate::Result<String> {
    // `TestBackend` is infallible (`Infallible` error) — map via Display
    // so this stays correct regardless of the backend's error type.
    let backend = ratatui::backend::TestBackend::new(w, h);
    let mut terminal =
        Terminal::new(backend).map_err(|e| crate::Error::Config(format!("test terminal: {e}")))?;
    terminal
        .draw(|f| draw_pager(f, sel, 0))
        .map_err(|e| crate::Error::Config(format!("test draw: {e}")))?;
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for row in 0..buf.area.height {
        let mut line = String::new();
        for col in 0..buf.area.width {
            if let Some(cell) = buf.cell((col, row)) {
                line.push_str(cell.symbol());
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tl() -> Timeline {
        let mut t = Timeline::new();
        t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","messages":[{"role":"user","content":"hello"}]}"#,
        );
        t.record_request(
            "POST",
            "/v1/chat/completions",
            &[],
            br#"{"model":"gpt","messages":[{"role":"user","content":"second"}]}"#,
        );
        t
    }

    #[test]
    fn select_defaults_to_last_step() {
        let s = select(&tl(), None).unwrap();
        assert_eq!(s.index, 1);
        assert_eq!(s.steps, 2);
        assert_eq!(s.provider, Some(Provider::OpenAiCompat));
        assert!(s.body.contains("second"));
    }

    #[test]
    fn select_specific_step() {
        let s = select(&tl(), Some(0)).unwrap();
        assert_eq!(s.index, 0);
        assert_eq!(s.provider, Some(Provider::Anthropic));
    }

    #[test]
    fn select_errors_out_of_range_and_empty() {
        assert!(matches!(
            select(&tl(), Some(9)),
            Err(crate::Error::Config(_))
        ));
        assert!(matches!(
            select(&Timeline::new(), None),
            Err(crate::Error::Config(_))
        ));
    }

    #[test]
    fn oneshot_not_decorated_is_byte_exact_even_with_color() {
        let sel = select(&tl(), Some(0)).unwrap();
        // decorate=false (piped) ⇒ exactly the wire bytes, regardless of
        // ColorMode (the --color=always | pipe contract).
        for mode in [ColorMode::None, ColorMode::Truecolor] {
            let mut buf = Vec::new();
            oneshot(&mut buf, false, mode, &sel).unwrap();
            assert_eq!(buf, sel.body.as_bytes(), "byte-exact under {mode:?}");
            assert!(!buf.contains(&0x1b));
        }
    }

    #[test]
    fn oneshot_decorated_adds_header_and_exactly_one_trailing_newline() {
        let sel = select(&tl(), Some(0)).unwrap();
        assert!(!sel.body.ends_with('\n'), "fixture body has no trailing nl");
        let mut buf = Vec::new();
        oneshot(&mut buf, true, ColorMode::Truecolor, &sel).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains('\u{23FA}'), "doc-11 action glyph");
        assert!(!s.chars().any(|c| c as u32 >= 0x1_F000), "no emoji");
        // Body verbatim, then exactly one synthesized newline (pins the
        // `!ends_with` branch — a deleted `!` would drop the newline).
        assert!(s.ends_with(&format!("{}\n", sel.body)));
        assert!(!s.ends_with(&format!("{}\n\n", sel.body)));
    }

    #[test]
    fn oneshot_decorated_does_not_double_newline_when_body_ends_nl() {
        let mut t = Timeline::new();
        t.record_request("POST", "/v1/messages", &[], b"{\"a\":1}\n");
        let sel = select(&t, Some(0)).unwrap();
        assert!(sel.body.ends_with('\n'));
        let mut buf = Vec::new();
        oneshot(&mut buf, true, ColorMode::Truecolor, &sel).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.ends_with(&sel.body), "no synthetic extra newline");
        assert!(!s.ends_with("\n\n"));
    }

    #[test]
    fn key_action_covers_every_binding() {
        use KeyOutcome::{Ignore, Quit, Scroll};
        let press = KeyEventKind::Press;
        assert_eq!(key_action(KeyCode::Char('q'), press, 5, 9), Quit);
        assert_eq!(key_action(KeyCode::Esc, press, 5, 9), Quit);
        assert_eq!(key_action(KeyCode::Down, press, 5, 9), Scroll(6));
        assert_eq!(key_action(KeyCode::Down, press, 9, 9), Scroll(9)); // clamp
        assert_eq!(key_action(KeyCode::Up, press, 5, 9), Scroll(4));
        assert_eq!(key_action(KeyCode::Up, press, 0, 9), Scroll(0)); // saturate
        assert_eq!(key_action(KeyCode::PageDown, press, 0, 9), Scroll(9));
        assert_eq!(key_action(KeyCode::PageUp, press, 5, 9), Scroll(0));
        assert_eq!(key_action(KeyCode::Home, press, 7, 9), Scroll(0));
        assert_eq!(key_action(KeyCode::End, press, 0, 9), Scroll(9));
        assert_eq!(key_action(KeyCode::Char('z'), press, 5, 9), Ignore);
        // Non-Press events are ignored (pins the kind != Press guard).
        assert_eq!(
            key_action(KeyCode::Char('q'), KeyEventKind::Release, 5, 9),
            Ignore
        );
        assert_eq!(
            key_action(KeyCode::Down, KeyEventKind::Repeat, 5, 9),
            Ignore
        );
    }

    #[test]
    fn should_page_only_when_tui_and_both_streams_are_ttys() {
        // Exhaustive truth table — pins the MED-1 gate fix so an
        // `&&`->`||` regression is caught (not just code-inspected).
        let table = [
            (false, false, false, false),
            (false, false, true, false),
            (false, true, false, false),
            (false, true, true, false),
            (true, false, false, false),
            (true, false, true, false),
            (true, true, false, false),
            (true, true, true, true),
        ];
        for (tui, so, si, want) in table {
            assert_eq!(
                should_page(tui, so, si),
                want,
                "should_page({tui},{so},{si})"
            );
        }
    }

    #[test]
    fn json_roundtrips_metadata_and_body() {
        let sel = select(&tl(), Some(1)).unwrap();
        let mut buf = Vec::new();
        json(&mut buf, &sel).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["step"], 1);
        assert_eq!(v["provider"], "open_ai_compat");
        assert_eq!(v["body"].as_str().unwrap(), sel.body);
    }

    #[test]
    fn pager_frame_renders_rounded_border_and_body() {
        let sel = select(&tl(), Some(0)).unwrap();
        let s = render_to_string(&sel, 60, 8).unwrap();
        assert!(s.contains('\u{256D}'), "rounded top-left corner");
        assert!(s.contains("verbatim"), "title present");
        assert!(s.contains("hello"), "body rendered");
        assert!(!s.chars().any(|c| c as u32 >= 0x1_F000), "no emoji");
    }
}
