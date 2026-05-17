//! F3 — per-step context diff.
//!
//! Line-level diff of two captured assembled prompts (the verbatim wire
//! bodies): what was added / removed / retained between step A and B,
//! with the token delta. PURE MEASUREMENT (line counts + tokenizer
//! sums) — no scoring/judgment (evalint KILLED). Default pair is the
//! canonical "step N vs N-1".

use std::time::Duration;

use serde::Serialize;
use similar::{ChangeTag, TextDiff};

/// Bound the Myers diff: `similar` documents that two large, maximally
/// distinct sequences spin without progress unless a deadline is set
/// (`ctx diff` reads an attacker-influenceable saved `SQLite`). On
/// timeout `similar` returns a correct *approximation* (never errors or
/// panics), so all counts/JSON stay valid and deterministic per input.
const DIFF_DEADLINE: Duration = Duration::from_secs(2);

use crate::timeline::Timeline;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Add,
    Remove,
    Keep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffLine {
    pub op: Op,
    /// One line of the assembled prompt (newline stripped).
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepDiff {
    pub from: usize,
    pub to: usize,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub retained_lines: usize,
    pub added_tokens: usize,
    pub removed_tokens: usize,
    pub lines: Vec<DiffLine>,
}

/// Resolve the pair to diff. Default: the last two steps (`N-1` vs `N`).
///
/// # Errors
/// [`crate::Error::Config`] if there are fewer than 2 steps or an index
/// is out of range.
pub fn select_pair(
    timeline: &Timeline,
    from: Option<usize>,
    to: Option<usize>,
) -> crate::Result<(usize, usize)> {
    let n = timeline.steps.len();
    if n < 2 {
        return Err(crate::Error::Config(format!(
            "need >=2 captured steps to diff, have {n}"
        )));
    }
    let to = to.unwrap_or(n - 1);
    let from = from.unwrap_or(to.saturating_sub(1));
    for (label, idx) in [("from", from), ("to", to)] {
        if idx >= n {
            return Err(crate::Error::Config(format!(
                "{label} step {idx} out of range (0..{n})"
            )));
        }
    }
    if from > to {
        return Err(crate::Error::Config(format!(
            "from step {from} is after to step {to} — diff goes earlier -> later"
        )));
    }
    Ok((from, to))
}

/// Diff the verbatim assembled body of step `from` vs step `to`.
///
/// # Errors
/// [`crate::Error::Config`] via [`select_pair`] validation.
pub fn diff(
    timeline: &Timeline,
    from: Option<usize>,
    to: Option<usize>,
) -> crate::Result<StepDiff> {
    let (from, to) = select_pair(timeline, from, to)?;
    let old = &timeline.steps[from].request.body;
    let new = &timeline.steps[to].request.body;
    let td = TextDiff::configure()
        .timeout(DIFF_DEADLINE)
        .diff_lines(old.as_str(), new.as_str());

    let mut lines = Vec::new();
    let (mut added, mut removed, mut retained) = (0usize, 0usize, 0usize);
    let (mut add_tok, mut rem_tok) = (0usize, 0usize);
    for change in td.iter_all_changes() {
        // Strip the line terminator (LF, then CRLF's trailing CR) so a
        // record is one clean line and CRLF wire bodies don't smear.
        let raw = change.value();
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        let text = text.strip_suffix('\r').unwrap_or(text);
        let op = match change.tag() {
            ChangeTag::Insert => {
                added += 1;
                add_tok = add_tok.saturating_add(crate::tokenizer::count(text));
                Op::Add
            }
            ChangeTag::Delete => {
                removed += 1;
                rem_tok = rem_tok.saturating_add(crate::tokenizer::count(text));
                Op::Remove
            }
            ChangeTag::Equal => {
                retained += 1;
                Op::Keep
            }
        };
        lines.push(DiffLine {
            op,
            text: text.to_string(),
        });
    }
    Ok(StepDiff {
        from,
        to,
        added_lines: added,
        removed_lines: removed,
        retained_lines: retained,
        added_tokens: add_tok,
        removed_tokens: rem_tok,
        lines,
    })
}

/// `--json`: the stable diff contract.
///
/// # Errors
/// Propagates serialization or write errors.
pub fn json(w: &mut impl std::io::Write, d: &StepDiff) -> crate::Result<()> {
    writeln!(w, "{}", serde_json::to_string_pretty(d)?)?;
    Ok(())
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
            b"{\n  \"system\": \"be terse\",\n  \"messages\": [\n    \"first\"\n  ]\n}",
        );
        t.record_request(
            "POST",
            "/v1/messages",
            &[],
            b"{\n  \"system\": \"be terse\",\n  \"messages\": [\n    \"first\",\n    \"second\"\n  ]\n}",
        );
        t
    }

    #[test]
    fn select_pair_defaults_to_last_two() {
        assert_eq!(select_pair(&tl(), None, None).unwrap(), (0, 1));
    }

    #[test]
    fn select_pair_errors_when_too_few_or_oob() {
        let mut one = Timeline::new();
        one.record_request("POST", "/v1/messages", &[], b"{}");
        assert!(matches!(
            select_pair(&one, None, None),
            Err(crate::Error::Config(_))
        ));
        assert!(matches!(
            select_pair(&tl(), Some(0), Some(9)),
            Err(crate::Error::Config(_))
        ));
        // Explicit reversed pair is rejected (pins the from>to guard).
        let err = select_pair(&tl(), Some(1), Some(0)).unwrap_err();
        match err {
            crate::Error::Config(m) => assert!(m.contains("after to step")),
            other => panic!("expected Config, got {other:?}"),
        }
        // Equal explicit pair is allowed (from == to, not from > to).
        assert_eq!(select_pair(&tl(), Some(1), Some(1)).unwrap(), (1, 1));
    }

    #[test]
    fn crlf_line_terminator_is_stripped_from_records() {
        let mut t = Timeline::new();
        t.record_request("POST", "/v1/messages", &[], b"a\r\nb\r\n");
        t.record_request("POST", "/v1/messages", &[], b"a\r\nc\r\n");
        let d = diff(&t, None, None).unwrap();
        for l in &d.lines {
            assert!(
                !l.text.contains('\r') && !l.text.contains('\n'),
                "record must be one clean line, got {:?}",
                l.text
            );
        }
        assert!(d.lines.iter().any(|l| l.op == Op::Keep && l.text == "a"));
    }

    #[test]
    fn diff_counts_added_removed_retained_and_tokens() {
        let d = diff(&tl(), None, None).unwrap();
        assert_eq!(d.from, 0);
        assert_eq!(d.to, 1);
        // step 1 added the `"second"` line and changed the `"first"`
        // line (it gained a trailing comma) → 2 added, 1 removed.
        assert_eq!(d.added_lines, 2);
        assert_eq!(d.removed_lines, 1);
        assert!(d.retained_lines >= 4);
        assert!(d.added_tokens > d.removed_tokens);
        // every emitted line carries a real op + text (pure record).
        assert!(d
            .lines
            .iter()
            .any(|l| l.op == Op::Add && l.text.contains("second")));
        assert!(d.lines.iter().any(|l| l.op == Op::Keep));
    }

    #[test]
    fn identical_steps_diff_is_all_retained() {
        let mut t = Timeline::new();
        t.record_request("POST", "/v1/messages", &[], b"{\"a\":1}\n{\"b\":2}");
        t.record_request("POST", "/v1/messages", &[], b"{\"a\":1}\n{\"b\":2}");
        let d = diff(&t, None, None).unwrap();
        assert_eq!(d.added_lines, 0);
        assert_eq!(d.removed_lines, 0);
        assert_eq!(d.added_tokens, 0);
        assert!(d.retained_lines > 0);
    }

    #[test]
    fn json_is_valid_contract() {
        let d = diff(&tl(), None, None).unwrap();
        let mut buf = Vec::new();
        json(&mut buf, &d).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["from"], 0);
        assert_eq!(v["to"], 1);
        assert_eq!(v["lines"][0]["op"], "keep");
        assert!(v["added_tokens"].as_u64().unwrap() > 0);
    }
}
