//! F1 — composition + waste indictment (the headline).
//!
//! Decomposes the captured assembled prompt by source and indicts waste.
//! HARD INVARIANT (PROJECT.md §6, evalint KILLED): every number here is
//! **pure measurement** — counts, byte-equality, token sums. No
//! prediction, no scoring, no judge. Contestable detail lives behind
//! `--deep`; the headline never says "the model will ignore X".
//!
//! Components are exactly what the wire exposes (system / tool schemas /
//! history). RAG/memory/skills are not separately identifiable on the
//! wire without a classifier (PROJECT.md §8 seam) — not fabricated here.

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use crate::timeline::Timeline;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Component {
    pub label: String,
    pub tokens: usize,
    /// Integer percent of `total_tokens` (floored) — no float, snapshot-stable.
    pub pct: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Indictment {
    pub code: String,
    pub detail: String,
    /// Per-rule measured waste. Rules can overlap (a block duplicated
    /// *within* a prompt is often also repeated *across* turns), so this
    /// is **not a partition** — a consumer must not sum `wasted_tokens`
    /// across indictments and call it a total. The headline never does.
    pub wasted_tokens: usize,
}

/// C3 (D-012) — context-window headroom & growth-rate slope. PURE
/// MEASUREMENT: a fraction of the model's window (the labeled offline
/// table) plus the measured tokens/turn slope (arithmetic on the
/// labeled ±N% token series). It asserts NOTHING about overflow or
/// truncation (that would be evalint — EXCLUDED). The only contestable
/// part — a *projection* of turns until the window is reached — is a
/// NEUTRAL arithmetic, `--deep`-only, worded "at the observed mean
/// rate", never as fate. `None` when the model id is not in the table
/// (no window claim) or the session has < 2 turns (no measured slope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Headroom {
    /// The wire model id this window size was resolved for.
    pub model: String,
    /// Context-window budget from the static offline table (labeled
    /// approximation — `window::WINDOW_LABEL`, like the tokenizer ±N%).
    pub window_tokens: usize,
    /// Focus step's assembled prompt tokens (the ±N% token estimate).
    pub used_tokens: usize,
    /// `used_tokens / window_tokens` as an integer percent (floored) —
    /// no float, snapshot-stable (mirrors `Component::pct`).
    pub used_pct: u32,
    /// Number of same-(provider,model) turns the slope was measured over.
    pub turns: usize,
    /// Measured mean growth of the assembled prompt, tokens/turn:
    /// `(last - first) / (turns - 1)`. Signed (a shrinking session is a
    /// real measured negative slope, never clamped to a guess).
    pub slope_tokens_per_turn: i64,
    /// `--deep`-ONLY neutral arithmetic projection ("at the observed
    /// mean rate, ~N more turn(s) before the window is reached"). `None`
    /// in the headline and whenever the slope is non-positive (a flat or
    /// shrinking session has no honest "turns remaining" arithmetic).
    pub projection: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Composition {
    /// The step whose breakdown is shown (last step with a parsed prompt).
    pub focus_step: Option<usize>,
    pub total_tokens: usize,
    pub components: Vec<Component>,
    /// Per-tool token detail — only populated when `--deep`.
    pub tools_deep: Vec<Component>,
    pub indictments: Vec<Indictment>,
    /// C3 (D-012) context-window headroom & slope. Additive (preserves
    /// the D-005 `--json` contract); `None` ⇒ no window claim / no slope.
    pub headroom: Option<Headroom>,
}

fn pct(part: usize, total: usize) -> u32 {
    part.saturating_mul(100)
        .checked_div(total)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0)
}

/// C3 (D-012) — measured mean growth of the assembled prompt across a
/// same-(provider,model) session, in tokens/turn: `(last - first) /
/// (turns - 1)`. PURE ARITHMETIC on the already-counted ±N% token
/// series. Signed `i64`: a shrinking session is a real negative slope,
/// never clamped (clamping would fabricate a measurement). `None` for
/// < 2 turns (no two points ⇒ no measurable rate). Truncating integer
/// division is intentional and snapshot-stable (no float in the
/// contract, mirrors the `pct` integer discipline). Extracted as a pure
/// helper with exact-value unit pins (the proven D-010 technique — the
/// approximate tokenizer cannot hit these boundaries through `compose`).
fn slope_per_turn(first_tok: usize, last_tok: usize, turns: usize) -> Option<i64> {
    let span = turns.checked_sub(1)?;
    if span == 0 {
        return None; // a single turn has no measurable growth rate
    }
    let first = i64::try_from(first_tok).ok()?;
    let last = i64::try_from(last_tok).ok()?;
    let span = i64::try_from(span).ok()?;
    Some((last - first) / span)
}

/// C3 (D-012) — the NEUTRAL `--deep` arithmetic projection: at the
/// measured mean rate, how many further turns of the same growth before
/// the window budget is reached. PURE ARITHMETIC, phrased as a
/// mean-rate extrapolation, NEVER asserted as fate (no "will overflow /
/// will truncate" — that is evalint, EXCLUDED). `None` unless the slope
/// is strictly positive and there is remaining budget (a flat /
/// shrinking / already-over session has no honest "turns remaining"
/// arithmetic — say nothing rather than fabricate one).
fn turns_until_window(used_tok: usize, window_tok: usize, slope: i64) -> Option<usize> {
    if slope <= 0 {
        return None; // not growing ⇒ no honest "turns remaining" figure
    }
    let slope = usize::try_from(slope).ok()?;
    let remaining = window_tok.checked_sub(used_tok)?;
    if remaining == 0 {
        return None; // already at/over the window ⇒ no positive headroom
    }
    // Ceil-free floor division: whole turns of headroom at the mean rate.
    Some(remaining / slope)
}

/// C3 (D-012) — a step's cache/budget **namespace**: `(provider,
/// model)`, or `None` when either is unknown (an unparsed/unknown step
/// is not in any namespace, never a guess). Returned as ONE tuple so the
/// `headroom` series filter is a single tuple equality — there is no
/// `provider && model` boolean for a mutant to widen into a
/// namespace-crossing `||` (the proven D-010 by-construction
/// elimination). Pure.
fn step_namespace(s: &crate::timeline::Step) -> Option<(crate::adapter::Provider, &str)> {
    Some((s.provider?, s.assembled.as_ref()?.model.as_deref()?))
}

/// Byte-length floor below which a message is trivial role-glue, not an
/// indictable block. A deterministic **byte** heuristic (not char/token),
/// shared by the two block rules.
const MIN_BLOCK_BYTES: usize = 40;

/// C1 (D-010) cache-prefix-break gates. PURE MEASUREMENT thresholds — a
/// measured fact about prefix reuse, never a prediction of provider
/// caching behaviour (evalint KILLED; provider-specific cache mechanics
/// are a `--deep`/doc caveat, not the headline). Tunable, snapshot-
/// stable (integer-only), `black_box`-pinned in tests.
///
/// Below this prompt size, prefix caching is economically irrelevant —
/// do not indict (avoids noise on small calls).
const CACHE_MIN_PROMPT_TOKENS: usize = 256;
/// A shared *suffix* this large proves the two turns are the SAME
/// continuing context (not two unrelated prompts) — the gate that makes
/// the rule true-positive-only.
const CACHE_MIN_SHARED_SUFFIX_TOKENS: usize = 64;

/// Saturating sum. Waste math runs on attacker-influenced wire bytes
/// (`ctx open` reads an unbounded saved session); it must never panic in
/// debug nor silently wrap in release — that would turn a "pure
/// measurement" into a garbage number. Matches the `pct()` discipline.
fn sat_sum(it: impl Iterator<Item = usize>) -> usize {
    it.fold(0usize, usize::saturating_add)
}

/// C5 (D-015) — the pure non-text-payload decision for ONE step: given
/// the focus step's non-text block byte sizes and the EXACT assembled
/// request-body byte length, return `(block_count, total_non_text_bytes,
/// percent_of_body)` iff there is at least one non-text block, else
/// `None`. Isolated here so the only arithmetic (the count, the
/// saturating byte sum, the integer percent) is unit-pinnable at its
/// exact boundary — no tokenizer/heuristic reaches it through
/// `compose()` (the proven D-010/D-011/D-014 by-construction technique).
///
/// STRICTLY PURE MEASUREMENT: an exact block count plus an exact byte
/// sum plus an integer percent of the EXACT body byte length. NO media
/// token estimate (the weakest tokenizer regime — omitted entirely to
/// stay strictly pure, per the C5 spec). NO judgment (a "too big" or
/// "will be ignored" verdict is evalint — EXCLUDED). The shared integer
/// `pct()` is reused (floored, no float, snapshot-stable,
/// div-by-zero-safe).
fn non_text_weight(part_bytes: &[usize], body_bytes: usize) -> Option<(usize, usize, u32)> {
    let count = part_bytes.len();
    if count == 0 {
        return None; // text-only ⇒ no non-text-payload claim (silent)
    }
    let total = sat_sum(part_bytes.iter().copied());
    Some((count, total, pct(total, body_bytes)))
}

/// C5 (D-015) — the deterministic per-kind tally string (e.g.
/// `"2 image_url, 1 file"`). Walks `NON_TEXT_KINDS` in its FIXED
/// declaration order (stable, snapshot-friendly — no `BTreeMap` re-sort)
/// and, per kind, counts the matching parts with `Iterator::count()`.
/// `count()` is used DELIBERATELY: it has no mutable arithmetic operator
/// for a mutant to flip (the proven D-010 by-construction elimination —
/// a hand `+= 1` accumulator's `+=`→`*=` is a no-op for a single block
/// and was MISSED on pass 1; `filter().count()` removes the operator
/// entirely). A kind absent from `parts` contributes nothing (the
/// `> 0` guard) — only kinds actually on the wire appear. Pure.
fn kind_tally(parts: &[crate::adapter::NonTextPart]) -> String {
    crate::adapter::NON_TEXT_KINDS
        .iter()
        .filter_map(|&kind| {
            let n = parts.iter().filter(|p| p.kind == kind).count();
            (n > 0).then(|| format!("{n} {kind}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Recursively collect tool names the model actually invoked, across
/// both wire shapes (Anthropic `tool_use` blocks, `OpenAI` `tool_calls`).
fn collect_used(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            if m.get("type").and_then(Value::as_str) == Some("tool_use") {
                if let Some(n) = m.get("name").and_then(Value::as_str) {
                    out.insert(n.to_string());
                }
            }
            if let Some(Value::Array(calls)) = m.get("tool_calls") {
                for c in calls {
                    if let Some(n) = c
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(Value::as_str)
                    {
                        out.insert(n.to_string());
                    }
                }
            }
            for val in m.values() {
                collect_used(val, out);
            }
        }
        Value::Array(a) => {
            for x in a {
                collect_used(x, out);
            }
        }
        _ => {}
    }
}

fn used_tool_names(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        collect_used(&v, &mut out);
    }
    out
}

/// Build the F1 composition for a captured timeline. `deep` adds the
/// per-tool token breakdown (still pure measurement, just finer).
#[must_use]
pub fn compose(timeline: &Timeline, deep: bool) -> Composition {
    let focus = timeline.steps.iter().rev().find(|s| s.assembled.is_some());

    let mut components = Vec::new();
    let mut tools_deep = Vec::new();
    let mut total = 0usize;
    let mut focus_step = focus.map(|s| s.index);

    if let Some(step) = focus {
        if let Some(a) = &step.assembled {
            // Saturating end-to-end: `ctx open` reads an unbounded saved
            // session, so the headline total must not panic/wrap either.
            let sys = a.system.as_deref().map_or(0, crate::tokenizer::count);
            let hist = sat_sum(a.messages.iter().map(|m| crate::tokenizer::count(&m.text)));
            let tools = sat_sum(a.tools.iter().map(|t| t.schema_tokens));
            total = sys.saturating_add(hist).saturating_add(tools);
            components.push(Component {
                label: "system".to_string(),
                tokens: sys,
                pct: pct(sys, total),
            });
            components.push(Component {
                label: "tool-schemas".to_string(),
                tokens: tools,
                pct: pct(tools, total),
            });
            components.push(Component {
                label: "history".to_string(),
                tokens: hist,
                pct: pct(hist, total),
            });
            // C5 (D-015) — a DISTINCT non-text-payload component so the
            // multimodal/file byte weight is attributed on its own row
            // instead of silently hiding inside `history`. The EXACT
            // byte/block/% facts are carried by the `non-text-payload`
            // INDICTMENT (the spec's required output string); this row's
            // `tokens` is a hard `0` ON PURPOSE — the per-image token
            // figure is the weakest tokenizer regime and is OMITTED
            // ENTIRELY to stay strictly pure (the C5 spec). A `0`-token
            // additive row never perturbs the `Σ components == total`
            // invariant (`decomposes_by_source`) — purely additive,
            // present only when the wire actually carried a non-text
            // block (text-only ⇒ absent, like C4's `sampling`).
            if !a.non_text.is_empty() {
                components.push(Component {
                    label: "non-text-payload".to_string(),
                    tokens: 0,
                    pct: 0,
                });
            }
            if deep {
                for t in &a.tools {
                    tools_deep.push(Component {
                        label: format!("tool:{}", t.name),
                        tokens: t.schema_tokens,
                        pct: pct(t.schema_tokens, total),
                    });
                }
            }
        }
    } else if let Some(step) = timeline
        .steps
        .iter()
        .rev()
        .find(|s| !s.request.body.is_empty())
    {
        // Layer 2 — graceful degradation: a body was captured but no
        // step parsed structurally (e.g. a non-JSON / compressed wire
        // body). F1 must NEVER be blind when bytes exist ("you cannot
        // debug what you cannot see"): count the verbatim body and say
        // *why* the structured view is unavailable. Pure measurement
        // (a token count + a factual label) — no judgment.
        total = crate::tokenizer::count(&step.request.body);
        components.push(Component {
            label: "raw-body (structured parse failed; counted verbatim)".to_string(),
            tokens: total,
            pct: pct(total, total),
        });
        focus_step = Some(step.index);
    }

    Composition {
        focus_step,
        total_tokens: total,
        components,
        tools_deep,
        indictments: indict(timeline),
        headroom: headroom(timeline, deep),
    }
}

/// C3 (D-012) — context-window headroom & growth-rate slope. PURE
/// MEASUREMENT. The window comes from the static offline table keyed by
/// the focus step's wire model id (`window::window_for`); an unknown id
/// ⇒ `None` (NO window claim — skipped honestly, never guessed). The
/// slope is the measured mean tokens/turn across the **same
/// (provider, model)** turns (a different model is a different budget,
/// so its turns are not in this series). The `--deep`-ONLY projection is
/// a neutral mean-rate arithmetic, never asserted as fate.
fn headroom(timeline: &Timeline, deep: bool) -> Option<Headroom> {
    // Focus = the last structurally-parsed step (same as `compose`'s
    // headline focus). C3 has no honest meaning on a Layer-2 raw body
    // (no model id, no per-turn series) — skip rather than fabricate.
    let focus = timeline
        .steps
        .iter()
        .rev()
        .find(|s| s.assembled.is_some())?;
    let provider = focus.provider?;
    let model = focus.assembled.as_ref()?.model.as_deref()?.to_owned();
    let window_tokens = crate::window::window_for(&model)?;

    // The per-turn assembled-token series, scoped to the SAME
    // (provider, model) namespace as the focus step. `prompt_tokens` is
    // the F0-computed ±N% estimate already on every step. The namespace
    // match is a single **tuple equality** (not a `provider && model`
    // boolean) so there is no `&&` operator for a mutant to flip into a
    // namespace-widening `||` (the proven D-010 by-construction
    // technique); a mixed-namespace fixture pins the polarity.
    let series: Vec<usize> = timeline
        .steps
        .iter()
        .filter(|s| step_namespace(s) == Some((provider, model.as_str())))
        .map(|s| s.prompt_tokens)
        .collect();
    let turns = series.len();
    let first = *series.first()?;
    let last = *series.last()?;
    // The measured slope needs >=2 turns; < 2 ⇒ no measurable rate ⇒
    // skip the whole C3 claim honestly (no slope, no headline).
    let slope_tokens_per_turn = slope_per_turn(first, last, turns)?;

    let used_tokens = focus.prompt_tokens;
    let used_pct = pct(used_tokens, window_tokens);

    // `--deep` ONLY: the neutral arithmetic projection. Worded as a
    // mean-rate extrapolation, NEVER as fate (no "will overflow /
    // truncate" — evalint, EXCLUDED). Absent in the headline and when
    // the slope is non-positive (no honest "turns remaining" arithmetic).
    let projection = if deep {
        turns_until_window(used_tokens, window_tokens, slope_tokens_per_turn).map(|n| {
            format!(
                "at the observed mean rate (~{slope_tokens_per_turn} tok/turn over {turns} turn(s)), ~{n} more turn(s) before the {window_tokens}-tok window is reached (neutral arithmetic projection, not a prediction)"
            )
        })
    } else {
        None
    };

    Some(Headroom {
        model,
        window_tokens,
        used_tokens,
        used_pct,
        turns,
        slope_tokens_per_turn,
        projection,
    })
}

/// All indictments — each rule is its own pure-measurement helper.
fn indict(timeline: &Timeline) -> Vec<Indictment> {
    [
        indict_unused_tools(timeline),
        indict_duplicate_blocks(timeline),
        indict_repeated_across_turns(timeline),
        indict_unpruned_history(timeline),
        indict_preamble_repay(timeline),
        indict_cache_prefix_break(timeline),
        indict_request_replayed(timeline),
        indict_component_drift(timeline),
        indict_param_drift(timeline),
        indict_non_text_payload(timeline),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Length (bytes) of the longest common leading prefix of two strings,
/// always returned on a UTF-8 char boundary (char-wise compare).
fn common_prefix_len(a: &str, b: &str) -> usize {
    let mut n = 0;
    for ((ia, ca), (_, cb)) in a.char_indices().zip(b.char_indices()) {
        if ca != cb {
            break;
        }
        n = ia + ca.len_utf8();
    }
    n
}

/// Length (bytes) of the longest common trailing suffix, char-boundary
/// safe, capped at `cap` so it can never overlap a counted prefix.
fn common_suffix_len(a: &str, b: &str, cap: usize) -> usize {
    let mut n: usize = 0;
    let mut ai = a.chars().rev();
    let mut bi = b.chars().rev();
    while let (Some(x), Some(y)) = (ai.next(), bi.next()) {
        if x != y || n.saturating_add(x.len_utf8()) > cap {
            break;
        }
        n += x.len_utf8();
    }
    n
}

/// C1 (D-010) — cache-prefix-break. Across consecutive requests to the
/// **same provider + model**, a short identical leading prefix while a
/// large identical suffix proves the *same context continues* means the
/// reusable bulk is re-sent past an early break (a volatile field,
/// reordered tools, or a regenerated system prompt pushed the stable
/// content off the cacheable prefix). The substrate is the **verbatim
/// wire body** (what a prefix cache actually keys on), not the
/// normalized view. Strictly PURE MEASUREMENT: byte-prefix/suffix
/// lengths + tokenizer sums + integer comparisons — no prediction of
/// whether the provider will cache (evalint KILLED).
/// Pure decision for ONE turn-pair: given the already-counted prefix /
/// shared-suffix / total tokens, the re-sent (uncacheable) cost iff
/// this is a genuine early break, else `None`. Sequential guards (no
/// compound boolean to flip); each threshold is unit-pinned at its
/// exact boundary — the approximate tokenizer cannot hit these
/// boundaries through `compose()`, so the decision is isolated here.
fn cache_break_wasted(prefix_tok: usize, suffix_tok: usize, total_tok: usize) -> Option<usize> {
    if total_tok < CACHE_MIN_PROMPT_TOKENS {
        return None; // prompt too small for prefix caching to matter
    }
    if suffix_tok < CACHE_MIN_SHARED_SUFFIX_TOKENS {
        return None; // not clearly the same continuing context
    }
    if prefix_tok.saturating_mul(2) >= total_tok {
        return None; // the cacheable prefix is >= half ⇒ healthy
    }
    Some(total_tok.saturating_sub(prefix_tok))
}

fn indict_cache_prefix_break(timeline: &Timeline) -> Option<Indictment> {
    // Cache namespace = (provider, model). Only a KNOWN, unchanged
    // namespace across consecutive turns can have a broken prefix; an
    // unknown or changed one is a different namespace, never a break.
    let key = |s: &crate::timeline::Step| match (
        s.provider,
        s.assembled.as_ref().and_then(|a| a.model.as_deref()),
    ) {
        (Some(p), Some(m)) => Some((p, m.to_owned())),
        _ => None,
    };
    let mut breaks: Vec<(usize, usize)> = Vec::new(); // (prefix_tok, total_tok)
    let mut wasted: Vec<usize> = Vec::new();
    for w in timeline.steps.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        match (key(a), key(b)) {
            (Some(x), Some(y)) if x == y => {}
            _ => continue,
        }
        let (pa, pb) = (a.request.body.as_str(), b.request.body.as_str());
        let cp = common_prefix_len(pa, pb);
        let cap = pa.len().min(pb.len()).saturating_sub(cp);
        let cs = common_suffix_len(pa, pb, cap);
        let total_tok = crate::tokenizer::count(pb);
        let prefix_tok = crate::tokenizer::count(pb.get(..cp).unwrap_or(""));
        let suffix_tok =
            crate::tokenizer::count(pb.get(pb.len().saturating_sub(cs)..).unwrap_or(""));
        if let Some(w) = cache_break_wasted(prefix_tok, suffix_tok, total_tok) {
            breaks.push((prefix_tok, total_tok));
            wasted.push(w);
        }
    }
    let pairs = wasted.len();
    if pairs == 0 {
        return None;
    }
    // `min_by_key` (std) — no hand-written comparison for a mutant to flip.
    let (worst_prefix, worst_total) = breaks
        .iter()
        .min_by_key(|(p, _)| *p)
        .copied()
        .unwrap_or((0, 0));
    Some(Indictment {
        code: "cache-prefix-break".to_string(),
        detail: format!(
            "{pairs} turn-pair(s): cacheable prefix broke early (~{worst_prefix} of ~{worst_total} tok shared as prefix) — tail re-sent uncacheable"
        ),
        wasted_tokens: sat_sum(wasted.into_iter()),
    })
}

/// Tools declared on the wire but never invoked in any response.
fn indict_unused_tools(timeline: &Timeline) -> Option<Indictment> {
    let mut declared_tokens = std::collections::BTreeMap::<String, usize>::new();
    for s in &timeline.steps {
        if let Some(a) = &s.assembled {
            for t in &a.tools {
                // Keep the largest schema seen for a tool (a later turn
                // may carry a bigger schema) — avoids under-counting.
                declared_tokens
                    .entry(t.name.clone())
                    .and_modify(|v| *v = (*v).max(t.schema_tokens))
                    .or_insert(t.schema_tokens);
            }
        }
    }
    let mut used = BTreeSet::new();
    for s in &timeline.steps {
        if let Some(r) = &s.response {
            used.append(&mut used_tool_names(&r.body));
        }
    }
    let unused: Vec<&String> = declared_tokens
        .keys()
        .filter(|n| !used.contains(*n))
        .collect();
    if declared_tokens.is_empty() || unused.is_empty() {
        return None;
    }
    let wasted = sat_sum(unused.iter().map(|n| declared_tokens[*n]));
    Some(Indictment {
        code: "unused-loaded-tools".to_string(),
        detail: format!(
            "{} of {} loaded tools never called this run",
            unused.len(),
            declared_tokens.len()
        ),
        wasted_tokens: wasted,
    })
}

/// The same non-trivial message text appears >1× within one prompt.
fn indict_duplicate_blocks(timeline: &Timeline) -> Option<Indictment> {
    for s in &timeline.steps {
        let Some(a) = &s.assembled else { continue };
        let mut seen = std::collections::BTreeMap::<&str, usize>::new();
        for m in &a.messages {
            if m.text.len() >= MIN_BLOCK_BYTES {
                *seen.entry(m.text.as_str()).or_insert(0) += 1;
            }
        }
        // One load-bearing `> 1` comparison: the set of distinct blocks
        // that appear more than once. `dups` is that set's size (so the
        // threshold is observable in `detail`, not a no-op on c==1).
        let duplicated: Vec<(&&str, &usize)> = seen.iter().filter(|(_, c)| **c > 1).collect();
        let dups = duplicated.len();
        if dups > 0 {
            let wasted = sat_sum(
                duplicated
                    .iter()
                    .map(|(t, c)| crate::tokenizer::count(t).saturating_mul(**c - 1)),
            );
            return Some(Indictment {
                code: "duplicate-block".to_string(),
                detail: format!(
                    "step {}: {dups} verbatim-duplicated block(s) in one prompt",
                    s.index
                ),
                wasted_tokens: wasted,
            });
        }
    }
    None
}

/// A message text sent verbatim in >=2 distinct steps (the honest,
/// wire-measurable generalization of "repeated RAG chunks across turns").
fn indict_repeated_across_turns(timeline: &Timeline) -> Option<Indictment> {
    let mut block_steps = std::collections::BTreeMap::<&str, BTreeSet<usize>>::new();
    for s in &timeline.steps {
        if let Some(a) = &s.assembled {
            for m in &a.messages {
                if m.text.len() >= MIN_BLOCK_BYTES {
                    block_steps
                        .entry(m.text.as_str())
                        .or_default()
                        .insert(s.index);
                }
            }
        }
    }
    let repeated = block_steps.values().filter(|st| st.len() >= 2).count();
    if repeated == 0 {
        return None;
    }
    let wasted = sat_sum(
        block_steps
            .iter()
            .filter(|(_, st)| st.len() >= 2)
            .map(|(t, st)| crate::tokenizer::count(t).saturating_mul(st.len() - 1)),
    );
    Some(Indictment {
        code: "repeated-block-across-turns".to_string(),
        detail: format!("{repeated} block(s) re-sent verbatim across >=2 turns"),
        wasted_tokens: wasted,
    })
}

/// History message count never decreases and strictly grows.
fn indict_unpruned_history(timeline: &Timeline) -> Option<Indictment> {
    let counts: Vec<usize> = timeline
        .steps
        .iter()
        .filter_map(|s| s.assembled.as_ref().map(|a| a.messages.len()))
        .collect();
    if counts.len() < 2 {
        return None;
    }
    let monotone = counts.windows(2).all(|w| w[1] >= w[0]);
    let grew = counts.last() > counts.first();
    if !(monotone && grew) {
        return None;
    }
    // Cumulative history tokens carried after turn 0 (re-sent every
    // turn because nothing is pruned) — a coarse but honest measure.
    let carried = sat_sum(
        timeline
            .steps
            .iter()
            .skip(1)
            .filter_map(|s| s.assembled.as_ref())
            .flat_map(|a| a.messages.iter())
            .map(|m| crate::tokenizer::count(&m.text)),
    );
    Some(Indictment {
        code: "unpruned-history".to_string(),
        detail: format!(
            "history grew {}->{} msgs over {} turns, never pruned ({carried} tok carried after turn 0)",
            counts.first().copied().unwrap_or(0),
            counts.last().copied().unwrap_or(0),
            counts.len()
        ),
        wasted_tokens: carried,
    })
}

/// An identical system prompt re-sent across >=2 steps is re-paid.
fn indict_preamble_repay(timeline: &Timeline) -> Option<Indictment> {
    let mut sys_steps = std::collections::BTreeMap::<&str, usize>::new();
    for s in &timeline.steps {
        if let Some(sys) = s.assembled.as_ref().and_then(|a| a.system.as_deref()) {
            if !sys.is_empty() {
                *sys_steps.entry(sys).or_insert(0) += 1;
            }
        }
    }
    // Sum every re-paid system variant, not just the most frequent — a
    // run alternating two system prompts re-pays both.
    let repaid: Vec<(&&str, &usize)> = sys_steps.iter().filter(|(_, c)| **c >= 2).collect();
    if repaid.is_empty() {
        return None;
    }
    let extra_payments: usize = sat_sum(repaid.iter().map(|(_, c)| *c - 1));
    let wasted = sat_sum(
        repaid
            .iter()
            .map(|(sys, c)| crate::tokenizer::count(sys).saturating_mul(*c - 1)),
    );
    Some(Indictment {
        code: "preamble-repay".to_string(),
        detail: format!(
            "system prompt re-paid: {extra_payments} extra payment(s) across {} variant(s)",
            repaid.len()
        ),
        wasted_tokens: wasted,
    })
}

/// C6 (D-013) — same-body re-send / retry-replay. PURE MEASUREMENT: the
/// re-billed token cost of a body sent verbatim more than once, given its
/// per-copy token count and how many times it occurred. `extra_copies` =
/// `occurrences - 1` is computed by the caller from a count map (no hand
/// comparison for a mutant to flip); this is the cost arithmetic only,
/// kept pure + saturating + isolated so the approximate tokenizer cannot
/// reach this boundary through `compose()` (the proven D-010 technique).
/// `0` means "no re-billed cost" (a lone send, or empty body) and the
/// caller drops it — it is NEVER an indictment.
fn replay_wasted(body_tokens: usize, extra_copies: usize) -> usize {
    body_tokens.saturating_mul(extra_copies)
}

/// C6 (D-013) — `request-replayed`. A retry after a 429/5xx or an
/// idempotent re-issue emitted by an HTTP/framework layer *below* the
/// user's code re-sends the SAME assembled prompt verbatim; only the
/// wire shows it and only a cross-step holder can detect the duplicate.
/// Whole-body sibling of `repeated-block-across-turns` (that is a message
/// block repeated across turns; this is the ENTIRE request body re-sent —
/// a distinct waste class). Strictly PURE MEASUREMENT: full-body
/// byte-equality + occurrence count + the re-billed token weight + the
/// ALREADY-BUFFERED response status of the replayed attempt (F0 buffers
/// responses; request-only for the core fact). It BORDERS `guard`'s
/// territory — `ctx` only REPORTS the fact; it NEVER throttles /
/// circuit-breaks / intervenes (that is `guard`; EXCLUDED, research §c/d).
fn indict_request_replayed(timeline: &Timeline) -> Option<Indictment> {
    // Group steps by verbatim request body. Empty bodies carry no
    // re-billed prompt cost (role-glue / a non-prompt POST) — excluded
    // by the `is_empty` filter, mirroring the MIN_BLOCK_BYTES discipline
    // of the block rules.
    let mut by_body: std::collections::BTreeMap<&str, Vec<&crate::timeline::Step>> =
        std::collections::BTreeMap::new();
    for s in &timeline.steps {
        if !s.request.body.is_empty() {
            by_body.entry(s.request.body.as_str()).or_default().push(s);
        }
    }
    // A replayed body = one that occurred in >=2 steps. `Vec::len` and
    // the std `filter` give the count; no hand-written comparison.
    let replayed: Vec<(&&str, &Vec<&crate::timeline::Step>)> = by_body
        .iter()
        .filter(|(_, steps)| steps.len() >= 2)
        .collect();
    if replayed.is_empty() {
        return None;
    }
    let distinct = replayed.len();
    // Total re-billed copies across all replayed bodies (each body's
    // `len() - 1` extra copies) and the matching token cost.
    let extra_copies = sat_sum(replayed.iter().map(|(_, steps)| steps.len() - 1));
    let wasted = sat_sum(
        replayed
            .iter()
            .map(|(body, steps)| replay_wasted(crate::tokenizer::count(body), steps.len() - 1)),
    );
    if wasted == 0 {
        return None; // no real re-billed prompt cost (degenerate body)
    }
    // Annotate the buffered status of the *replayed* attempt: the
    // most-replayed body's FIRST occurrence (the attempt that was retried
    // — research §c: "step 5 == step 4 body; step 4 returned 529"). std
    // `max_by_key` — no hand comparison for a mutant to flip.
    let worst = replayed
        .iter()
        .max_by_key(|(_, steps)| steps.len())
        .map(|(_, steps)| *steps);
    let status_note = worst
        .and_then(|steps| steps.first())
        .and_then(|first| first.response.as_ref())
        .map_or_else(
            || "; replayed attempt status not captured".to_string(),
            |r| format!("; first replayed attempt returned {}", r.status),
        );
    Some(Indictment {
        code: "request-replayed".to_string(),
        detail: format!(
            "{extra_copies} request body re-send(s) across {distinct} distinct body/ies, re-billed verbatim{status_note}"
        ),
        wasted_tokens: wasted,
    })
}

/// C2 (D-011) — the pure per-appearance decision for ONE component:
/// given the previous and current *size* of a same-named component, the
/// drift magnitude iff it changed, else `None`. ONE equality + ONE
/// absolute difference, isolated here so the only comparison/arithmetic
/// is unit-pinnable at its exact boundary — the approximate tokenizer
/// cannot reach these values through `compose()` (the D-010 technique).
/// Pure measurement: integer (in)equality, no judgment.
fn drift_delta(prev: usize, cur: usize) -> Option<usize> {
    if prev == cur {
        return None; // byte/size-identical re-payment ⇒ NOT drift
    }
    // Saturating abs-difference: waste math runs on attacker-influenced
    // wire bytes (`ctx open` reads an unbounded saved session); it must
    // never panic in debug nor wrap in release (the `sat_sum`/`pct`
    // discipline). `prev != cur` here, so the result is always >= 1.
    Some(prev.max(cur).saturating_sub(prev.min(cur)))
}

/// C2 (D-011) — component-drift. The OPPOSITE of `preamble-repay`: that
/// counts an *identical* component re-paid; this catches a same-NAMED
/// component (`system`, or a tool keyed by its name) whose bytes/size
/// silently **mutate** between two of its appearances mid-session — the
/// #1 cause of both cache invalidation and a "stable" instruction
/// changing under the engineer's feet. Strictly PURE MEASUREMENT:
/// per-component cross-step (in)equality + byte/token deltas + the step
/// index; NO prediction, NO "the model will forget X" (evalint KILLED).
///
/// Component identity is keyed by NAME. `system` fingerprints on its
/// exact bytes (so the byte delta is exact AND the token delta is the
/// labeled ±N% estimate). Each tool fingerprints on the size the
/// canonical `Assembled` view exposes (`schema_tokens`) — the byte
/// granularity is not carried by `Assembled` and is NOT re-derived here
/// (that would duplicate the adapter / require an `Assembled` shape
/// change — explicitly out of scope, D-001/D-007/D-008); a same-token-
/// count schema mutation is therefore a deliberate honest false-negative
/// (the D-010 true-positive-bias discipline). A renamed tool is a
/// different key ⇒ remove+add, never a drift event — stated, not
/// inferred (per the C2 spec caveat).
fn indict_component_drift(timeline: &Timeline) -> Option<Indictment> {
    // Per component key → its size at the previous appearance. `system`
    // also keeps the previous bytes so the byte delta is exact.
    let mut prev_tool: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut prev_sys: Option<(usize, String)> = None;
    // Distinct components that drifted, each pinned to the FIRST step
    // index where it changed (a BTreeMap ⇒ deterministic ordering).
    let mut first_drift: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut deltas: Vec<usize> = Vec::new();

    for s in &timeline.steps {
        let Some(a) = &s.assembled else { continue };
        // system — exact-byte fingerprint (only when present & non-empty,
        // matching `preamble-repay`'s "a system block exists" gate).
        if let Some(sys) = a.system.as_deref().filter(|x| !x.is_empty()) {
            let cur = crate::tokenizer::count(sys);
            if let Some((prev_tok, prev_bytes)) = prev_sys.as_ref() {
                if prev_bytes != sys {
                    if let Some(d) = drift_delta(*prev_tok, cur) {
                        first_drift.entry("system".to_string()).or_insert(s.index);
                        deltas.push(d);
                    }
                }
            }
            prev_sys = Some((cur, sys.to_string()));
        }
        // each tool, keyed by name — size fingerprint (`schema_tokens`).
        for t in &a.tools {
            let key = format!("tool:{}", t.name);
            if let Some(prev) = prev_tool.get(&key) {
                if let Some(d) = drift_delta(*prev, t.schema_tokens) {
                    first_drift.entry(key.clone()).or_insert(s.index);
                    deltas.push(d);
                }
            }
            prev_tool.insert(key, t.schema_tokens);
        }
    }

    if first_drift.is_empty() {
        return None;
    }
    // Deterministic "component@step" list (BTreeMap ⇒ sorted by key).
    let where_ = first_drift
        .iter()
        .map(|(name, ix)| format!("{name}@step {ix}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(Indictment {
        code: "component-drift".to_string(),
        detail: format!(
            "{} same-named component(s) mutated mid-session: {where_} (~{} tok changed; a renamed tool reads as remove+add, not drift)",
            first_drift.len(),
            sat_sum(deltas.iter().copied())
        ),
        wasted_tokens: sat_sum(deltas.into_iter()),
    })
}

/// C4 (D-014) — the pure per-pair decision for ONE sampling field:
/// given a field name and its canonical JSON value at the previous and
/// current appearance, the drift event `(field, old, new)` iff the value
/// changed, else `None`. ONE equality, isolated here so the only
/// comparison is unit-pinnable at its exact boundary — no tokenizer /
/// format heuristic can reach it through `compose()` (the proven
/// D-010/D-011 by-construction technique). STRICTLY PURE MEASUREMENT:
/// verbatim value (in)equality, no judgment, NEVER "this caused
/// non-determinism" (that attribution is `agentlock`'s; EXCLUDED here).
fn param_change(field: &str, prev: &str, cur: &str) -> Option<(String, String, String)> {
    if prev == cur {
        return None; // byte-identical value ⇒ NOT drift
    }
    Some((field.to_string(), prev.to_string(), cur.to_string()))
}

/// C4 (D-014) — `param-drift`. A framework often sets/overrides sampling
/// & decoding request fields (`temperature`, `top_p`, `max_tokens`,
/// `stop`, `seed`, `tool_choice`, …) invisibly between the engineer's
/// code and the model; the wire is the only ground truth and only a
/// cross-step holder can assert "field X changed at step N". Across
/// consecutive requests in the SAME `(provider, model)` namespace, emit
/// `param-drift` when any tracked field's value changes between two
/// turns, naming the field, old→new, and the step index.
///
/// STRICTLY PURE MEASUREMENT: value (in)equality + named field + step
/// index. It is a REPORTED FACT — it NEVER says "this drift caused
/// non-determinism / will change the output". That determinism
/// *attribution* belongs to `agentlock`'s scoped framing and is
/// **EXCLUDED here by construction** (CONTEXT-SIGNALS-RESEARCH §c/§d;
/// evalint KILLED). C4 does NOT build `agentlock`'s lockfile — it only
/// surfaces the determinism-surface fact `agentlock` will later consume
/// (the shared F0 substrate). The namespace is a single `(provider,
/// model)` tuple equality via `step_namespace` (the proven D-012
/// by-construction technique — no `provider && model` boolean for a
/// mutant to widen into a namespace-crossing `||`).
///
/// `absent ≠ a value` (the C4 spec caveat): a field present in one turn
/// and omitted in the next is NOT a value change — only fields present
/// in BOTH consecutive same-namespace turns are compared. `wasted_tokens`
/// is `0` here: a parameter change re-bills no prompt tokens (it is a
/// determinism fact, not a token-waste class); the headline carries the
/// field/old→new/step facts, never a fabricated cost.
fn indict_param_drift(timeline: &Timeline) -> Option<Indictment> {
    // Distinct "field@step ix (old->new)" events, first occurrence per
    // field pinned (a BTreeMap ⇒ deterministic ordering, like C2).
    let mut first_event: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for w in timeline.steps.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        // Same (provider, model) namespace only — a different model is a
        // different determinism surface (the agentlock-boundary caveat).
        // ONE tuple equality (no `&&` for a mutant to widen into `||`).
        match (step_namespace(a), step_namespace(b)) {
            (Some(x), Some(y)) if x == y => {}
            _ => continue,
        }
        let (Some(pa), Some(pb)) = (a.assembled.as_ref(), b.assembled.as_ref()) else {
            continue;
        };
        // A field is compared only when PRESENT in BOTH turns
        // (`absent ≠ a value`). The lookup is a `BTreeMap` over the
        // small fixed `SAMPLING_FIELDS` set, so the comparison is the
        // sole decision (isolated in `param_change`).
        let prev: std::collections::BTreeMap<&str, &str> = pa
            .sampling
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        for (field, cur) in &pb.sampling {
            if let Some(old) = prev.get(field.as_str()) {
                if let Some((f, o, n)) = param_change(field, old, cur) {
                    first_event
                        .entry(f.clone())
                        .or_insert_with(|| format!("{f}@step {} ({o}->{n})", b.index));
                }
            }
        }
    }
    if first_event.is_empty() {
        return None;
    }
    let events = first_event
        .values()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    Some(Indictment {
        code: "param-drift".to_string(),
        detail: format!(
            "{} sampling/decoding field(s) changed mid-session (same provider+model): {events} (a reported determinism-surface fact, not a non-determinism claim)",
            first_event.len()
        ),
        // A param change re-bills no prompt tokens — it is a determinism
        // fact, not a token-waste class. `0` (never a fabricated cost).
        wasted_tokens: 0,
    })
}

/// C5 (D-015) — `non-text-payload`. Multimodal / file content (base64
/// data URIs, `image`/`image_url`/`input_audio`/`file`/`document`
/// blocks, both wire shapes) is invisible to the SDK→DB→dashboard field
/// (`OTEL` `GenAI` content capture is OFF by default; even on, large
/// media is truncated by the exporter) — only `ctx`'s verbatim wire
/// capture holds it. Without C5 those bytes silently inflate `history`;
/// C5 attributes them as their OWN component + this indictment so the
/// engineer SEES that e.g. 38% of the assembled body is inline image
/// data.
///
/// Focus = the last structurally-parsed step (the SAME focus as the F1
/// headline / the `non-text-payload` component) — one representative
/// step, not a cross-step sum (kept minimal & pure, like C2/C4's first-
/// event discipline). The `%` is of the focus step's EXACT request-body
/// byte length (the real on-the-wire denominator), div-by-zero-safe via
/// the shared integer `pct()`.
///
/// STRICTLY PURE MEASUREMENT: an EXACT block count + an EXACT byte sum +
/// an integer percent of the EXACT body byte length. base64 is NOT
/// decoded (the wire bytes ARE the cost; no new dep). NO media token
/// estimate (the weakest tokenizer regime — omitted entirely to stay
/// strictly pure, per the C5 spec / `CONTEXT-SIGNALS-RESEARCH` §c). NO
/// judgment — it NEVER says "too big" / "will be ignored" (that is
/// evalint — KILLED, EXCLUDED §d). `wasted_tokens` is a hard `0`: a
/// non-text payload is a byte-ATTRIBUTION fact, not a token-waste class
/// (mirrors C4's hard-`0`; the headline carries the block/byte/% facts,
/// never a fabricated cost). The per-pair decision is isolated in the
/// pure `non_text_weight` helper (an exact-boundary unit table — no
/// tokenizer/heuristic can reach it through `compose()`).
fn indict_non_text_payload(timeline: &Timeline) -> Option<Indictment> {
    let focus = timeline.steps.iter().rev().find(|s| s.assembled.is_some())?;
    let parts = &focus.assembled.as_ref()?.non_text;
    let body_bytes = focus.request.body.len();
    let part_bytes: Vec<usize> = parts.iter().map(|p| p.bytes).collect();
    let (count, bytes, percent) = non_text_weight(&part_bytes, body_bytes)?;
    let kinds = kind_tally(parts);
    Some(Indictment {
        code: "non-text-payload".to_string(),
        detail: format!(
            "non-text-payload: {count} block(s) ({kinds}), ~{bytes} bytes ({percent}% of the assembled body) — exact wire bytes, base64 not decoded, no media token estimate (omitted to stay strictly pure)"
        ),
        // A byte-ATTRIBUTION fact, not a token-waste class — never a
        // fabricated cost (the C4 hard-`0` discipline, the `Indictment`
        // "not a partition" rule).
        wasted_tokens: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tl() -> Timeline {
        let mut t = Timeline::new();
        // turn 1: system + 1 msg + 2 tools
        let i = t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"You are a careful assistant. Always be terse and exact.","messages":[{"role":"user","content":"the first user question, which is reasonably long here"}],"tools":[{"name":"search","input_schema":{"type":"object"}},{"name":"calc","input_schema":{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"},"op":{"type":"string"}},"required":["a","b","op"]}}]}"#,
        );
        // model used only `search`
        t.record_response(
            i,
            200,
            &[],
            br#"{"content":[{"type":"tool_use","name":"search"}]}"#,
        );
        // turn 2: same system (re-paid), history grew, same first msg repeated
        let j = t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"You are a careful assistant. Always be terse and exact.","messages":[{"role":"user","content":"the first user question, which is reasonably long here"},{"role":"assistant","content":"a tool result block here"},{"role":"user","content":"the first user question, which is reasonably long here"}],"tools":[{"name":"search","input_schema":{"type":"object"}},{"name":"calc","input_schema":{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"},"op":{"type":"string"}},"required":["a","b","op"]}}]}"#,
        );
        t.record_response(
            j,
            200,
            &[],
            br#"{"content":[{"type":"text","text":"done"}]}"#,
        );
        t
    }

    #[test]
    fn decomposes_by_source() {
        let c = compose(&tl(), false);
        assert_eq!(c.focus_step, Some(1));
        let labels: Vec<&str> = c.components.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["system", "tool-schemas", "history"]);
        assert!(c.total_tokens > 0);
        let sum: usize = c.components.iter().map(|x| x.tokens).sum();
        assert_eq!(sum, c.total_tokens);
    }

    #[test]
    fn deep_adds_per_tool_detail() {
        assert!(compose(&tl(), false).tools_deep.is_empty());
        let d = compose(&tl(), true);
        assert_eq!(d.tools_deep.len(), 2);
        assert!(d.tools_deep.iter().any(|c| c.label == "tool:search"));
    }

    #[test]
    fn raises_at_least_four_correct_indictments() {
        let c = compose(&tl(), false);
        let codes: BTreeSet<&str> = c.indictments.iter().map(|i| i.code.as_str()).collect();
        // calc loaded, never called
        assert!(codes.contains("unused-loaded-tools"));
        // turn 2 repeats the first user msg verbatim within one prompt
        assert!(codes.contains("duplicate-block"));
        // that block also appears in step 0 and step 1
        assert!(codes.contains("repeated-block-across-turns"));
        // 1 -> 3 messages, monotone, never pruned
        assert!(codes.contains("unpruned-history"));
        // identical system in both steps
        assert!(codes.contains("preamble-repay"));
        assert!(codes.len() >= 4, "need >=4 indictments, got {codes:?}");
        // Pin a real measured value per rule (not just non-emptiness):
        // every indictment with a token cost must report a positive,
        // non-wrapped count.
        let by = |code: &str| c.indictments.iter().find(|i| i.code == code).unwrap();
        assert!(by("unused-loaded-tools").wasted_tokens > 0);
        assert!(by("duplicate-block").wasted_tokens > 0);
        assert!(by("repeated-block-across-turns").wasted_tokens > 0);
        assert!(by("unpruned-history").wasted_tokens > 0);
        assert!(by("preamble-repay").wasted_tokens > 0);
        assert!(by("preamble-repay").detail.contains("extra payment"));
    }

    #[test]
    fn empty_timeline_is_graceful() {
        let c = compose(&Timeline::new(), false);
        assert_eq!(c.focus_step, None);
        assert_eq!(c.total_tokens, 0);
        assert!(c.components.is_empty());
        assert!(c.indictments.is_empty());
    }

    #[test]
    fn unused_tools_detected_precisely() {
        let c = compose(&tl(), false);
        let u = c
            .indictments
            .iter()
            .find(|i| i.code == "unused-loaded-tools")
            .unwrap();
        assert!(u.detail.contains("1 of 2"));
        // `calc` (the bigger schema) is the unused one; `search` was
        // invoked. The waste must be *calc's* tokens, strictly larger
        // than search's — pins the `!used.contains` polarity and the
        // exact attribution (not just ">0").
        let calc_tokens = crate::tokenizer::count("calc")
            + crate::tokenizer::count(
                r#"{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"},"op":{"type":"string"}},"required":["a","b","op"]}"#,
            );
        assert_eq!(u.wasted_tokens, calc_tokens);
        let search_tokens =
            crate::tokenizer::count("search") + crate::tokenizer::count(r#"{"type":"object"}"#);
        assert!(calc_tokens > search_tokens, "fixture must be asymmetric");
    }

    // F1-FIX step A: the exact OpenAI `chat.completions` parallel of
    // `tl()` — system as messages[0]{role:system}, user/assistant
    // history, tools[] as {"type":"function","function":{...}}, an
    // OpenAI tool_calls response. NOT rigged: same provider-agnostic
    // assertions as the Anthropic F1 tests.
    fn openai_tl() -> Timeline {
        let mut t = Timeline::new();
        let i = t.record_request(
            "POST",
            "/v1/chat/completions",
            &[],
            br#"{"model":"openai/gpt-4o","messages":[{"role":"system","content":"You are a careful assistant. Always be terse and exact."},{"role":"user","content":"the first user question, which is reasonably long here"}],"tools":[{"type":"function","function":{"name":"search","description":"web","parameters":{"type":"object"}}},{"type":"function","function":{"name":"calc","description":"math","parameters":{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"},"op":{"type":"string"}},"required":["a","b","op"]}}}],"stream":true}"#,
        );
        t.record_response(
            i,
            200,
            &[],
            br#"{"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"c1","type":"function","function":{"name":"search","arguments":"{}"}}]}}]}"#,
        );
        let j = t.record_request(
            "POST",
            "/v1/chat/completions",
            &[],
            br#"{"model":"openai/gpt-4o","messages":[{"role":"system","content":"You are a careful assistant. Always be terse and exact."},{"role":"user","content":"the first user question, which is reasonably long here"},{"role":"assistant","content":"a tool result block here"},{"role":"user","content":"the first user question, which is reasonably long here"}],"tools":[{"type":"function","function":{"name":"search","description":"web","parameters":{"type":"object"}}},{"type":"function","function":{"name":"calc","description":"math","parameters":{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"},"op":{"type":"string"}},"required":["a","b","op"]}}}],"stream":true}"#,
        );
        t.record_response(
            j,
            200,
            &[],
            br#"{"choices":[{"message":{"role":"assistant","content":"done"}}]}"#,
        );
        t
    }

    #[test]
    fn f1_degrades_visibly_not_blind_on_non_json_capture() {
        // Layer 2 (D-008): a captured body that is NOT valid JSON (e.g.
        // a compressed/binary wire body) must NOT make F1 say
        // "no captured prompt · 0 tokens". Bytes exist ⇒ F1 counts them
        // verbatim and surfaces *why* the structured view is missing.
        let mut t = Timeline::new();
        t.record_request(
            "POST",
            "/v1/chat/completions",
            &[],
            b"\x1f\x8b\x08 not json at all -- gzip/binary-ish wire body",
        );
        let c = compose(&t, false);
        assert!(c.focus_step.is_some(), "must not be blind when bytes exist");
        assert!(c.total_tokens > 0, "must count the verbatim body");
        assert_eq!(c.components.len(), 1);
        assert!(
            c.components[0].label.contains("structured parse failed"),
            "must visibly state why the structured view is unavailable: {}",
            c.components[0].label
        );
    }

    #[test]
    fn f1_must_not_be_blind_on_any_v1_provider_shape() {
        // F1-FIX step D — durable class guard (DECISIONS.md D-007
        // standing rule). F1 MUST decompose every v1 provider wire
        // shape (PROJECT.md §4/§5: Anthropic + OpenAI-compatible),
        // including the real-world null/omitted optional variants. A
        // new provider/shape adds a row; this makes "F1 blind on a
        // provider shape" a hard CI failure forever.
        let cases: &[(&str, &[u8])] = &[
            (
                "anthropic: clean (system field + messages + tools)",
                br#"{"model":"claude-3","system":"You are terse and exact.","messages":[{"role":"user","content":"a reasonably long question for the guard"}],"tools":[{"name":"s","input_schema":{"type":"object"}}]}"#,
            ),
            (
                "anthropic: explicit null optionals (no tools turn)",
                br#"{"model":"claude-3","system":"You are terse and exact.","messages":[{"role":"user","content":"a reasonably long question for the guard"}],"tools":null}"#,
            ),
            (
                "openai: clean (system msg + history + tools[].function)",
                br#"{"model":"gpt-4o","messages":[{"role":"system","content":"You are terse and exact."},{"role":"user","content":"a reasonably long question for the guard"}],"tools":[{"type":"function","function":{"name":"s","parameters":{"type":"object"}}}]}"#,
            ),
            (
                "openai: real-client tools:null + tool_choice:null",
                br#"{"model":"gpt-4o","messages":[{"role":"system","content":"You are terse and exact."},{"role":"user","content":"a reasonably long question for the guard"}],"tools":null,"tool_choice":null}"#,
            ),
            (
                "openai: messages content arrays + assistant tool_calls",
                br#"{"model":"gpt-4o","messages":[{"role":"system","content":[{"type":"text","text":"You are terse and exact."}]},{"role":"user","content":[{"type":"text","text":"a reasonably long question for the guard"}]},{"role":"assistant","content":null,"tool_calls":[{"id":"c","type":"function","function":{"name":"s","arguments":"{}"}}]}],"tools":[{"type":"function","function":{"name":"s","parameters":{"type":"object"}}}]}"#,
            ),
        ];
        let mut blind = Vec::new();
        for (name, body) in cases {
            let mut t = Timeline::new();
            // Path drives provider detection (F0 already classifies it;
            // we do not re-derive — D-001 / brief §3-B).
            let path = if name.starts_with("anthropic") {
                "/v1/messages"
            } else {
                "/v1/chat/completions"
            };
            t.record_request("POST", path, &[], body);
            let c = compose(&t, false);
            let by = |l: &str| c.components.iter().find(|x| x.label == l).map(|x| x.tokens);
            if c.focus_step.is_none()
                || c.total_tokens == 0
                || by("system").unwrap_or(0) == 0
                || by("history").unwrap_or(0) == 0
            {
                blind.push(*name);
            }
        }
        assert!(
            blind.is_empty(),
            "F1 is BLIND on v1 provider shape(s) {blind:?} — D-007 violated"
        );
    }

    #[test]
    fn f1_not_blind_on_realworld_openai_tools_null() {
        // F1-FIX step A — the proven defect, as a real OpenAI body:
        // a no-tools chat.completions turn (real clients emit explicit
        // `"tools": null`, not an omitted key). On HEAD `adapter::parse`
        // errors (serde `default` ≠ null) ⇒ assembled None ⇒ F1 prints
        // "composition no captured prompt" though F0/F2/F3 see the bytes.
        // MUST fail pre-fix, pass post-fix. Not rigged: this is exactly
        // what e.g. the OpenAI SDK / agent frameworks send on a no-tool
        // call, and PROJECT.md §4/§5 declares OpenAI-compatible a v1
        // provider.
        let mut t = Timeline::new();
        t.record_request(
            "POST",
            "/v1/chat/completions",
            &[],
            br#"{"model":"openai/gpt-4o","messages":[{"role":"system","content":"You are terse."},{"role":"user","content":"a reasonably long user question for the fixture"}],"tools":null,"tool_choice":null}"#,
        );
        let c = compose(&t, false);
        assert!(
            c.focus_step.is_some(),
            "F1 blind on a real OpenAI tools:null body (focus None)"
        );
        assert!(
            c.total_tokens > 0,
            "F1 reports 0 tokens on a real OpenAI tools:null capture"
        );
        // system + user history must decompose (no tools this turn ⇒
        // tool-schemas legitimately 0; system & history are not).
        let by = |l: &str| c.components.iter().find(|x| x.label == l).map(|x| x.tokens);
        assert!(by("system").unwrap_or(0) > 0, "system must decompose");
        assert!(by("history").unwrap_or(0) > 0, "history must decompose");
    }

    #[test]
    fn f1_decomposes_openai_chat_completions_shape() {
        // The flagship headline MUST work on the OpenAI-compatible wire
        // shape (PROJECT.md §4/§5: v1 = Anthropic + OpenAI-compatible),
        // identically to the Anthropic shape — same categories, non-zero
        // tokens, same indictments.
        let c = compose(&openai_tl(), true);
        assert_eq!(c.focus_step, Some(1), "OpenAI step must be the focus");
        let labels: Vec<&str> = c.components.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["system", "tool-schemas", "history"]);
        assert!(c.total_tokens > 0, "OpenAI prompt must not be 0 tokens");
        for comp in &c.components {
            assert!(
                comp.tokens > 0,
                "component `{}` must be non-zero on the OpenAI shape",
                comp.label
            );
        }
        assert_eq!(c.tools_deep.len(), 2, "both OpenAI tools attributed");
        let codes: BTreeSet<&str> = c.indictments.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains("unused-loaded-tools"), "calc never called");
        assert!(codes.contains("duplicate-block"));
        assert!(codes.contains("repeated-block-across-turns"));
        assert!(codes.contains("unpruned-history"));
        assert!(codes.contains("preamble-repay"));
        assert!(codes.len() >= 4, "need >=4 indictments, got {codes:?}");
    }

    // F1-FIX3 step B — the defect proven on a VERBATIM REAL capture.
    // `tests/fixtures/real_openai_gzip_request.bin` is the exact bytes a
    // real httpx OpenAI-shaped client (system + user/assistant history +
    // tools[]) put on the wire with `Content-Encoding: gzip`, recorded by
    // a real listener — NOT hand-authored, and NOT the ctx-saved copy
    // (that copy is already destroyed by the very bug under test:
    // `timeline::record_request`'s `String::from_utf8_lossy` mangles the
    // compressed body before parse/persist, so D-007/D-008's parser fixes
    // never see real bytes). MUST fail on `f866dac`, pass after step C.
    // `#[ignore]` keeps the commit-gate green at step B; the pre-fix
    // failure is demonstrated and recorded in RUSTCC-USAGE.md, the test
    // is un-ignored in step C (D-009).
    #[test]
    fn f1_decomposes_real_gzip_openai_capture() {
        let gz = include_bytes!("../tests/fixtures/real_openai_gzip_request.bin");
        // Guard: the fixture must really be gzip — a future regen that
        // silently produced plain JSON would make this test vacuously
        // pass and re-hide the defect (brief §1: no assertion-weakening).
        assert_eq!(
            &gz[..2],
            &[0x1f, 0x8b],
            "fixture must be REAL gzip wire bytes (magic 1f 8b)"
        );
        let mut t = crate::timeline::Timeline::new();
        t.record_request(
            "POST",
            "/v1/chat/completions",
            &[("content-encoding".to_string(), "gzip".to_string())],
            gz,
        );
        let c = compose(&t, false);
        // F1-EXIT (brief §1): structured decomposition of the REAL body,
        // NOT the Layer-2 raw-body fallback (which on a compressed body is
        // a non-result for the flagship — 0 indictments).
        assert!(
            c.focus_step.is_some(),
            "F1 blind on a real gzip capture (focus None)"
        );
        assert!(
            !c.components
                .iter()
                .any(|x| x.label.contains("structured parse failed")),
            "F1 fell back to Layer-2 raw-body on a real gzip capture: {:?}",
            c.components
        );
        let by = |l: &str| c.components.iter().find(|x| x.label == l).map(|x| x.tokens);
        assert!(by("system").unwrap_or(0) > 0, "system must decompose");
        assert!(by("history").unwrap_or(0) > 0, "history must decompose");
        assert!(
            by("tool-schemas").unwrap_or(0) > 0,
            "tool-schemas must decompose"
        );
        // The real body declares 2 tools and calls none ⇒ the flagship
        // waste indictment must fire (proves real decomposition, not just
        // a non-empty count).
        assert!(
            c.indictments
                .iter()
                .any(|i| i.code == "unused-loaded-tools"),
            "real decomposition must indict the 2 unused tools, got {:?}",
            c.indictments.iter().map(|i| &i.code).collect::<Vec<_>>()
        );
    }

    fn step_with(system: &str, n_msgs: usize, tools: &str) -> Vec<u8> {
        let msgs: Vec<String> = (0..n_msgs)
            .map(|k| format!(r#"{{"role":"user","content":"message number {k} of this turn"}}"#))
            .collect();
        format!(
            r#"{{"model":"m","system":"{system}","messages":[{}],"tools":[{tools}]}}"#,
            msgs.join(",")
        )
        .into_bytes()
    }

    #[test]
    fn no_unused_indictment_when_every_tool_is_called() {
        let mut t = Timeline::new();
        let i = t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","messages":[{"role":"user","content":"go do the thing now please"}],"tools":[{"name":"search","input_schema":{"type":"object"}}]}"#,
        );
        t.record_response(
            i,
            200,
            &[],
            br#"{"content":[{"type":"tool_use","name":"search"}]}"#,
        );
        let c = compose(&t, false);
        let codes: Vec<&str> = c.indictments.iter().map(|x| x.code.as_str()).collect();
        assert!(
            !codes.contains(&"unused-loaded-tools"),
            "all tools used ⇒ no unused indictment, got {codes:?}"
        );
    }

    fn has_unpruned(t: &Timeline) -> bool {
        compose(t, false)
            .indictments
            .iter()
            .any(|i| i.code == "unpruned-history")
    }

    #[test]
    fn unpruned_requires_three_plus_steps_and_strict_monotone_growth() {
        // 3 steps strictly growing ⇒ indicted (pins `counts.len() < 2`:
        // a `< → >` mutant would skip 3-step runs).
        let mut grow = Timeline::new();
        for (k, n) in [1usize, 2, 3].into_iter().enumerate() {
            let i = grow.record_request("POST", "/v1/messages", &[], &step_with("S", n, ""));
            grow.record_response(i, 200, &[], format!("{{\"i\":{k}}}").as_bytes());
        }
        assert!(has_unpruned(&grow), "3-step strict growth must be indicted");

        // 2 steps, constant history ⇒ NOT indicted (pins `last > first`
        // against `>=`, and `monotone && grew` against `||`).
        let mut flat = Timeline::new();
        for _ in 0..2 {
            let i = flat.record_request("POST", "/v1/messages", &[], &step_with("S", 2, ""));
            flat.record_response(i, 200, &[], b"{}");
        }
        assert!(
            !has_unpruned(&flat),
            "constant history must not be indicted"
        );

        // Non-monotone (shrinks then grows) ⇒ NOT indicted (the other
        // half of `monotone && grew` vs `||`).
        let mut wobble = Timeline::new();
        for n in [3usize, 1, 4] {
            let i = wobble.record_request("POST", "/v1/messages", &[], &step_with("S", n, ""));
            wobble.record_response(i, 200, &[], b"{}");
        }
        assert!(
            !has_unpruned(&wobble),
            "non-monotone history must not be indicted"
        );
    }

    fn has_code(t: &Timeline, code: &str) -> bool {
        compose(t, false).indictments.iter().any(|i| i.code == code)
    }

    #[test]
    fn cache_prefix_gates_are_the_documented_constants() {
        // `black_box` keeps these real runtime checks (not const-folded)
        // — pins the `*`/literal mutants on the C1 gate constants, same
        // discipline as `MAX_DECOMPRESSED` / `proxy::MAX_BODY`.
        assert_eq!(std::hint::black_box(CACHE_MIN_PROMPT_TOKENS), 256);
        assert_eq!(std::hint::black_box(CACHE_MIN_SHARED_SUFFIX_TOKENS), 64);
    }

    #[test]
    fn cache_break_wasted_exact_boundaries() {
        // Deterministic exact-value table — the approximate tokenizer
        // cannot hit these boundaries through compose(), so the decision
        // is pinned here. Each row kills a specific mutant on the gates.
        // total < MIN ⇒ None; == MIN proceeds (kills `<`→`==`/`<=`):
        assert_eq!(cache_break_wasted(8, 64, 256), Some(248));
        assert_eq!(cache_break_wasted(8, 64, 255), None);
        // suffix < MIN ⇒ None (== MIN already proven to fire above):
        assert_eq!(cache_break_wasted(8, 63, 256), None);
        // prefix*2 vs total: exact half ⇒ healthy(None); just-under ⇒ fires:
        assert_eq!(cache_break_wasted(128, 999, 256), None); // 256>=256
        assert_eq!(cache_break_wasted(127, 999, 256), Some(129)); // 254<256
        assert_eq!(cache_break_wasted(200, 999, 256), None); // >  (kills >=→==/<=/<)
        assert_eq!(cache_break_wasted(130, 999, 256), None); // 260>=256 (kills *→+: 132<256)
        assert_eq!(cache_break_wasted(86, 999, 256), Some(170)); // 172<256 (kills *2→*3: 258>=256)
                                                                 // saturating: no panic / wrap at the extreme.
        assert_eq!(cache_break_wasted(usize::MAX, 64, 256), None);
    }

    #[test]
    fn common_prefix_and_suffix_len_are_exact_and_char_safe() {
        assert_eq!(common_prefix_len("abcXY", "abcZZ"), 3);
        assert_eq!(common_prefix_len("same", "same"), 4);
        assert_eq!(common_prefix_len("", "x"), 0);
        assert_eq!(common_prefix_len("xyz", "abc"), 0);
        // multi-byte: stop on the differing char, never mid-codepoint.
        assert_eq!(common_prefix_len("héllo", "héXlo"), 3); // 'h' + 'é'(2B)
                                                            // suffix, capped so it can never overlap the prefix region.
        assert_eq!(common_suffix_len("abcTAIL", "zzzTAIL", 4), 4);
        assert_eq!(common_suffix_len("abcTAIL", "zzzTAIL", 2), 2); // cap binds
        assert_eq!(common_suffix_len("abc", "abc", 100), 3);
        assert_eq!(common_suffix_len("abc", "xyz", 100), 0);
        assert_eq!(common_suffix_len(" café", "  fé", 10), 3); // 'f'+'é'(2B)
    }

    // C2 (D-011) — component-drift. PURE MEASUREMENT and the OPPOSITE of
    // `preamble-repay`: a same-NAMED component (the `system` block, or a
    // tool keyed by its name) whose bytes/size CHANGE between two of its
    // appearances mid-session. Hash/size equality + step index + delta;
    // NO judgment, NO "the model will forget X" (evalint KILLED). A tool
    // rename reads as remove+add (NOT a drift event — drift requires the
    // SAME name) — asserted here, stated in `detail`/`--deep`.
    #[test]
    fn component_drift_decision_exact_boundaries() {
        // The pure per-appearance decision, pinned at its exact boundary
        // so the approximate tokenizer cannot reach it through compose()
        // (the D-010 technique). prev == cur ⇒ no event; any inequality
        // ⇒ the saturating absolute magnitude (always >= 1).
        assert_eq!(drift_delta(10, 10), None); // identical ⇒ stable
        assert_eq!(drift_delta(10, 11), Some(1)); // grew by 1 (kills ==→!=)
        assert_eq!(drift_delta(11, 10), Some(1)); // shrank by 1 (abs both ways)
        assert_eq!(drift_delta(0, 0), None); // empty == empty ⇒ stable
        assert_eq!(drift_delta(0, 1), Some(1)); // appeared/grew from 0
        assert_eq!(drift_delta(5, 9), Some(4)); // exact magnitude, not a flag
                                                // saturating: no panic / wrap at the extreme either direction.
        assert_eq!(drift_delta(usize::MAX, 0), Some(usize::MAX));
        assert_eq!(drift_delta(0, usize::MAX), Some(usize::MAX));
    }

    #[test]
    fn component_drift_fires_only_when_a_same_named_component_mutates() {
        // FIRES — system mutates between step 0 and step 1 (same key
        // "system", different bytes). Tools stable.
        let mut sys_drift = Timeline::new();
        sys_drift.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"You are a careful assistant. Rule A applies.","messages":[{"role":"user","content":"a reasonably long first user question here"}]}"#,
        );
        sys_drift.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"You are a careful assistant. Rule B applies now.","messages":[{"role":"user","content":"a reasonably long second user question here"}]}"#,
        );
        assert!(
            has_code(&sys_drift, "component-drift"),
            "a mutated system block across steps MUST be indicted"
        );
        let d = compose(&sys_drift, false)
            .indictments
            .into_iter()
            .find(|i| i.code == "component-drift")
            .unwrap();
        assert!(
            d.detail.contains("system"),
            "detail must name the drifted component: {}",
            d.detail
        );
        assert!(
            d.detail.contains("step 1"),
            "detail must report the step index where it changed: {}",
            d.detail
        );
        assert!(
            d.wasted_tokens > 0,
            "a real byte change must report a non-zero token delta"
        );

        // FIRES — a tool's schema mutates while its NAME is unchanged
        // (the #1 silent cache-invalidation cause). System stable.
        let mut tool_drift = Timeline::new();
        tool_drift.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"stable system prompt of a reasonable length here","messages":[{"role":"user","content":"a reasonably long question"}],"tools":[{"name":"search","input_schema":{"type":"object"}}]}"#,
        );
        tool_drift.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"stable system prompt of a reasonable length here","messages":[{"role":"user","content":"a reasonably long question"}],"tools":[{"name":"search","input_schema":{"type":"object","properties":{"q":{"type":"string"},"k":{"type":"number"},"deep":{"type":"boolean"}},"required":["q"]}}]}"#,
        );
        assert!(
            has_code(&tool_drift, "component-drift"),
            "a same-named tool whose schema mutates MUST be indicted"
        );
        assert!(
            compose(&tool_drift, false)
                .indictments
                .iter()
                .find(|i| i.code == "component-drift")
                .unwrap()
                .detail
                .contains("tool:search"),
            "detail must name the drifted tool by its key"
        );

        // NOT FIRES — fully stable system + stable tools across 2 steps
        // (this is exactly the `preamble-repay` case: identical re-payment,
        // the OPPOSITE failure — C2 must stay silent here).
        let mut stable = Timeline::new();
        let stable_body = br#"{"model":"m","system":"You are a careful, terse assistant for the stable case.","messages":[{"role":"user","content":"a reasonably long first user question here"}],"tools":[{"name":"search","input_schema":{"type":"object"}}]}"#;
        stable.record_request("POST", "/v1/messages", &[], stable_body);
        stable.record_request("POST", "/v1/messages", &[], stable_body);
        assert!(
            !has_code(&stable, "component-drift"),
            "an identical re-paid component is preamble-repay, NOT drift"
        );

        // NOT FIRES — a renamed tool reads as remove+add, never a drift
        // event (drift requires the SAME name to change its bytes).
        let mut renamed = Timeline::new();
        renamed.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"stable system prompt of a reasonable length here","messages":[{"role":"user","content":"a reasonably long question"}],"tools":[{"name":"search_v1","input_schema":{"type":"object"}}]}"#,
        );
        renamed.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"stable system prompt of a reasonable length here","messages":[{"role":"user","content":"a reasonably long question"}],"tools":[{"name":"search_v2","input_schema":{"type":"object","properties":{"x":{"type":"string"}}}}]}"#,
        );
        assert!(
            !has_code(&renamed, "component-drift"),
            "a renamed tool is remove+add, not a same-name drift"
        );

        // NOT FIRES — a single step cannot drift (needs >=2 appearances).
        let mut one = Timeline::new();
        one.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"a single-step system prompt of reasonable length","messages":[{"role":"user","content":"only one step here"}]}"#,
        );
        assert!(
            !has_code(&one, "component-drift"),
            "one appearance cannot drift"
        );
    }

    // C4 (D-014) — param-drift. PURE MEASUREMENT: a sampling/decoding
    // field's value CHANGES between two consecutive same-(provider,model)
    // turns. Value (in)equality + named field + step index ONLY. It is a
    // REPORTED FACT — never "this drift caused non-determinism / will
    // change output" (that attribution is `agentlock`'s; EXCLUDED here,
    // evalint KILLED). `absent ≠ a value`: a field present in one turn
    // and omitted in the next is NOT a value change.
    #[test]
    fn param_change_decision_exact_boundaries() {
        // The pure per-pair decision, pinned at its exact boundary so no
        // tokenizer/format heuristic can reach it through compose() (the
        // D-010/D-011 technique). Equal value ⇒ None; any inequality ⇒
        // Some((field, old, new)) — verbatim, no judgment.
        assert_eq!(param_change("temperature", "0.2", "0.2"), None);
        assert_eq!(
            param_change("temperature", "0.2", "0.7"),
            Some((
                "temperature".to_string(),
                "0.2".to_string(),
                "0.7".to_string()
            ))
        );
        // canonical JSON value strings compared verbatim — a real change
        // is a real change (kills the ==→!= and value-swap mutants).
        assert_eq!(
            param_change("top_p", "1", "0.9"),
            Some(("top_p".to_string(), "1".to_string(), "0.9".to_string()))
        );
        // identical structured values (stop arrays) ⇒ no event.
        assert_eq!(param_change("stop", "[\"X\"]", "[\"X\"]"), None);
        assert_eq!(
            param_change("stop", "[\"X\"]", "[\"Y\"]"),
            Some((
                "stop".to_string(),
                "[\"X\"]".to_string(),
                "[\"Y\"]".to_string()
            ))
        );
        // empty == empty ⇒ no event; the field name is carried through
        // verbatim (kills a mutant that drops/duplicates a tuple slot).
        assert_eq!(param_change("seed", "", ""), None);
        assert_eq!(
            param_change("seed", "1", "2"),
            Some(("seed".to_string(), "1".to_string(), "2".to_string()))
        );
    }

    #[test]
    fn sampling_fields_are_the_documented_tracked_set() {
        // `black_box` keeps this a real runtime check (not const-folded)
        // — pins the tracked-field slice + its order so a mutant that
        // drops/reorders an entry is caught (the C1/C3 const discipline).
        let f = std::hint::black_box(crate::adapter::SAMPLING_FIELDS);
        assert_eq!(
            f,
            &[
                "temperature",
                "top_p",
                "top_k",
                "max_tokens",
                "max_completion_tokens",
                "stop",
                "stop_sequences",
                "presence_penalty",
                "frequency_penalty",
                "seed",
                "response_format",
                "tool_choice",
            ]
        );
    }

    #[test]
    fn param_drift_fires_only_when_a_tracked_field_value_changes() {
        // FIRES — temperature changes between turn 0 and turn 1 under the
        // SAME (provider, model). top_p stable.
        let mut temp_drift = Timeline::new();
        temp_drift.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-x","system":"be terse and exact in answers","messages":[{"role":"user","content":"a reasonably long first user question here"}],"temperature":0.2,"top_p":1}"#,
        );
        temp_drift.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-x","system":"be terse and exact in answers","messages":[{"role":"user","content":"a reasonably long second user question here"}],"temperature":0.9,"top_p":1}"#,
        );
        assert!(
            has_code(&temp_drift, "param-drift"),
            "a changed sampling field across same-(provider,model) turns MUST be indicted"
        );
        let d = compose(&temp_drift, false)
            .indictments
            .into_iter()
            .find(|i| i.code == "param-drift")
            .unwrap();
        assert!(
            d.detail.contains("temperature"),
            "detail must name the drifted field: {}",
            d.detail
        );
        assert!(
            d.detail.contains("0.2") && d.detail.contains("0.9"),
            "detail must report old->new: {}",
            d.detail
        );
        assert!(
            d.detail.contains("step 1"),
            "detail must report the step index where it changed: {}",
            d.detail
        );

        // NOT FIRES — every tracked field byte-stable across 2 turns
        // (only the user message changes, which is NOT a sampling param).
        let mut stable = Timeline::new();
        stable.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-x","system":"be terse","messages":[{"role":"user","content":"first stable-params question of a reasonable length"}],"temperature":0.2,"max_tokens":1024}"#,
        );
        stable.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-x","system":"be terse","messages":[{"role":"user","content":"second stable-params question of a reasonable length"}],"temperature":0.2,"max_tokens":1024}"#,
        );
        assert!(
            !has_code(&stable, "param-drift"),
            "stable sampling params across turns must NOT be indicted"
        );

        // NOT FIRES — `absent ≠ a value`: temperature present in turn 0,
        // omitted in turn 1 is NOT a value change (the spec's caveat).
        let mut absent = Timeline::new();
        absent.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-x","system":"be terse","messages":[{"role":"user","content":"a reasonably long first user question here"}],"temperature":0.2}"#,
        );
        absent.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-x","system":"be terse","messages":[{"role":"user","content":"a reasonably long second user question here"}]}"#,
        );
        assert!(
            !has_code(&absent, "param-drift"),
            "a field present then absent is NOT a value change (absent != a value)"
        );

        // NOT FIRES — a param change ACROSS a (provider,model) boundary
        // is not a same-namespace drift (the agentlock-boundary caveat:
        // a different model is a different determinism surface).
        let mut crossns = Timeline::new();
        crossns.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-x","system":"be terse","messages":[{"role":"user","content":"a reasonably long first user question here"}],"temperature":0.2}"#,
        );
        crossns.record_request(
            "POST",
            "/v1/chat/completions",
            &[],
            br#"{"model":"gpt-4o","messages":[{"role":"user","content":"a reasonably long second user question here"}],"temperature":0.9}"#,
        );
        assert!(
            !has_code(&crossns, "param-drift"),
            "a param change across a (provider,model) boundary is not same-namespace drift"
        );

        // NOT FIRES — a single step cannot drift (needs >=2 appearances
        // in one namespace).
        let mut one = Timeline::new();
        one.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-x","system":"be terse","messages":[{"role":"user","content":"only one step here"}],"temperature":0.2}"#,
        );
        assert!(
            !has_code(&one, "param-drift"),
            "one appearance cannot drift"
        );
    }

    // C1 (D-010) — cache-prefix-break. PURE MEASUREMENT: a short
    // identical leading prefix across consecutive same-provider+model
    // requests while a LARGE identical suffix proves the same context
    // continues ⇒ the reusable tail is re-sent past an early break
    // (a volatile field / reordered tools / regenerated system prompt
    // pushed the stable content off the cacheable prefix). NOT a
    // prediction — a measured fact (bytes + tokenizer sum).
    #[test]
    fn cache_prefix_break_fires_only_on_early_break_with_large_shared_suffix() {
        let big_tail = format!(
            r#","messages":[{{"role":"user","content":"{}"}}]}}"#,
            "the quick brown fox jumps over the lazy dog ".repeat(40)
        );
        let mk = |model: &str, sys_fill: &str| {
            format!(
                r#"{{"model":"{model}","system":"{}"{big_tail}"#,
                sys_fill.repeat(400)
            )
            .into_bytes()
        };

        // FIRES: tiny common prefix (system diverges almost immediately),
        // huge identical suffix (same continuing context), big prompt.
        let mut brk = Timeline::new();
        brk.record_request("POST", "/v1/messages", &[], &mk("m", "S"));
        brk.record_request("POST", "/v1/messages", &[], &mk("m", "Z"));
        assert!(
            has_code(&brk, "cache-prefix-break"),
            "early prefix break with a large shared suffix MUST be indicted"
        );
        let w = compose(&brk, false)
            .indictments
            .into_iter()
            .find(|i| i.code == "cache-prefix-break")
            .unwrap();
        assert!(
            w.wasted_tokens > 0,
            "must report the re-sent broken-tail cost"
        );

        // NOT FIRES — healthy: long identical prefix (stable system +
        // early messages), conversation only grows at the end.
        let common = format!(
            r#"{{"model":"m","system":"{}","messages":[{{"role":"user","content":"{}"#,
            "S".repeat(400),
            "x".repeat(1200)
        );
        let mut healthy = Timeline::new();
        healthy.record_request(
            "POST",
            "/v1/messages",
            &[],
            format!(r#"{common}"}}]}}"#).as_bytes(),
        );
        healthy.record_request(
            "POST",
            "/v1/messages",
            &[],
            format!(r#"{common}"}},{{"role":"user","content":"more"}}]}}"#).as_bytes(),
        );
        assert!(
            !has_code(&healthy, "cache-prefix-break"),
            "a stable prefix that only grows at the end must NOT be indicted"
        );

        // NOT FIRES — unrelated short prompts (suffix + size gates).
        let mut unrel = Timeline::new();
        unrel.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"alpha","messages":[{"role":"user","content":"q1"}]}"#,
        );
        unrel.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","system":"beta gamma","messages":[{"role":"user","content":"other"}]}"#,
        );
        assert!(
            !has_code(&unrel, "cache-prefix-break"),
            "unrelated small prompts must NOT be indicted"
        );

        // NOT FIRES — single step (needs >=2 same-provider+model).
        let mut one = Timeline::new();
        one.record_request("POST", "/v1/messages", &[], &mk("m", "S"));
        assert!(
            !has_code(&one, "cache-prefix-break"),
            "one step cannot break a cache prefix"
        );

        // NOT FIRES — different model (prefix cache is per model).
        let mut diff_model = Timeline::new();
        diff_model.record_request("POST", "/v1/messages", &[], &mk("m", "S"));
        diff_model.record_request("POST", "/v1/messages", &[], &mk("n", "Z"));
        assert!(
            !has_code(&diff_model, "cache-prefix-break"),
            "a different model is a different cache namespace, not a break"
        );
    }

    // ---- C6 (D-013) — same-body re-send / retry-replay detection -------
    // PURE MEASUREMENT: full-body byte-equality + count + the duplicated
    // (re-billed) token weight + the buffered-response status of the
    // replayed attempt. Whole-body sibling of repeated-block-across-turns.
    // It REPORTS the fact only — it must NEVER throttle / circuit-break /
    // intervene (that is `guard`; EXCLUDED here, research §c/§d).

    #[test]
    fn replay_wasted_exact_boundaries() {
        // Deterministic exact-value table for the isolated cost decision
        // (the approximate tokenizer cannot hit these boundaries through
        // compose(); the D-010 technique). Each row kills a mutant on
        // the multiply / saturation.
        assert_eq!(replay_wasted(0, 5), 0); // empty body ⇒ no cost
        assert_eq!(replay_wasted(100, 0), 0); // lone send ⇒ no extra copy
        assert_eq!(replay_wasted(7, 1), 7); // one re-bill = one copy (kills `*`→`-`: 7-1=6)
        assert_eq!(replay_wasted(7, 3), 21); // kills `*`→`+`(10) / `-`(4) / `/`(2) / `%`(1)
                                             // saturating: an attacker `ctx open` body must not panic/wrap.
        assert_eq!(replay_wasted(usize::MAX, 2), usize::MAX);
        assert_eq!(replay_wasted(usize::MAX, 0), 0);
    }

    #[test]
    fn request_replayed_fires_on_a_byte_identical_resend_with_status() {
        // A real retry shape: the SAME assembled prompt is POSTed twice
        // because attempt 1 got a 529 (overloaded). The HTTP/framework
        // layer below the user's code re-issues it verbatim; only the
        // wire shows the identical re-send.
        let body = br#"{"model":"m","system":"You are a careful assistant for the replay fixture.","messages":[{"role":"user","content":"a reasonably long user question that will be retried verbatim"}]}"#;
        let mut t = Timeline::new();
        let i = t.record_request("POST", "/v1/messages", &[], body);
        t.record_response(i, 529, &[], br#"{"type":"overloaded_error"}"#);
        let j = t.record_request("POST", "/v1/messages", &[], body);
        t.record_response(j, 200, &[], br#"{"content":[{"type":"text","text":"ok"}]}"#);

        let c = compose(&t, false);
        let ind = c
            .indictments
            .iter()
            .find(|i| i.code == "request-replayed")
            .expect("a byte-identical re-send MUST be indicted as request-replayed");
        // 2 occurrences of one body ⇒ exactly 1 re-billed copy.
        assert!(
            ind.detail.contains('1'),
            "detail must report the replay/re-billed count: {}",
            ind.detail
        );
        // The re-billed cost = exactly one extra copy of the body tokens.
        assert_eq!(
            ind.wasted_tokens,
            crate::tokenizer::count(std::str::from_utf8(body).unwrap()),
            "wasted = exactly the re-sent (re-billed) copy"
        );
        // Status annotation from the ALREADY-BUFFERED response of the
        // first (replayed) attempt — the real, re-billed retry cause.
        assert!(
            ind.detail.contains("529"),
            "must annotate the replayed attempt's buffered status: {}",
            ind.detail
        );
    }

    #[test]
    fn request_replayed_silent_on_distinct_bodies() {
        // Control: two DIFFERENT bodies (a normal multi-turn run) must
        // NOT fire — replay is whole-body byte-equality, not similarity.
        let mut t = Timeline::new();
        t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","messages":[{"role":"user","content":"the first distinct question for this run"}]}"#,
        );
        t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","messages":[{"role":"user","content":"a second, entirely different question here"}]}"#,
        );
        assert!(
            !has_code(&t, "request-replayed"),
            "two distinct bodies are a normal turn, never a replay"
        );

        // A single step cannot be a replay.
        let mut one = Timeline::new();
        one.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","messages":[{"role":"user","content":"only one request was ever sent"}]}"#,
        );
        assert!(
            !has_code(&one, "request-replayed"),
            "a lone request cannot have been replayed"
        );

        // An empty body is not an indictable replay even if repeated
        // (no real re-billed prompt cost; avoids glue/noise).
        let mut empties = Timeline::new();
        empties.record_request("POST", "/v1/messages", &[], b"");
        empties.record_request("POST", "/v1/messages", &[], b"");
        assert!(
            !has_code(&empties, "request-replayed"),
            "empty bodies carry no re-billed prompt cost"
        );
    }

    #[test]
    fn request_replayed_counts_every_re_billed_copy() {
        // 3 identical sends ⇒ 2 re-billed copies; a 4th distinct body is
        // ignored. Pins the (occurrences - 1) weight and the count.
        let dup = br#"{"model":"m","messages":[{"role":"user","content":"a verbatim body sent three separate times for the retry storm"}]}"#;
        let mut t = Timeline::new();
        for _ in 0..3 {
            let i = t.record_request("POST", "/v1/messages", &[], dup);
            t.record_response(i, 500, &[], b"{}");
        }
        t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"m","messages":[{"role":"user","content":"an unrelated final body, not part of the storm"}]}"#,
        );
        let c = compose(&t, false);
        let ind = c
            .indictments
            .iter()
            .find(|i| i.code == "request-replayed")
            .expect("3 identical sends must be indicted");
        assert_eq!(
            ind.wasted_tokens,
            crate::tokenizer::count(std::str::from_utf8(dup).unwrap()) * 2,
            "3 sends ⇒ exactly 2 re-billed copies"
        );
        assert!(
            ind.detail.contains('2'),
            "detail must report 2 re-billed copies: {}",
            ind.detail
        );
    }

    // --- C3 (D-012) context-window headroom & growth-rate slope --------
    //
    // PURE MEASUREMENT. Headline = the measured window fraction + the
    // measured tokens/turn slope only (arithmetic on the labeled ±N%
    // token series and the labeled offline window table). The
    // "turns remaining at the observed mean rate" projection is a
    // NEUTRAL arithmetic, `--deep` ONLY, never asserted as fate
    // (that would be evalint — EXCLUDED). Unknown model id ⇒ NO window
    // claim (skipped honestly, never guessed). Request-only.

    /// A growing same-(provider,model) session: 3 turns whose prompt
    /// grows by a fixed history increment each turn (the C3 slope is the
    /// per-turn growth). Model id is a real Anthropic wire id ⇒ the
    /// static window table resolves it.
    fn growing_session(model: &str) -> Timeline {
        let mut t = Timeline::new();
        let big = "a reasonably substantial user message of stable length ".repeat(40);
        for turn in 1..=3usize {
            let msgs: Vec<String> = (0..turn)
                .map(|k| format!(r#"{{"role":"user","content":"{big} #{k}"}}"#))
                .collect();
            let body = format!(
                r#"{{"model":"{model}","system":"You are a careful, terse assistant.","messages":[{}]}}"#,
                msgs.join(",")
            );
            let i = t.record_request("POST", "/v1/messages", &[], body.as_bytes());
            t.record_response(i, 200, &[], br#"{"content":[{"type":"text","text":"ok"}]}"#);
        }
        t
    }

    #[test]
    fn c3_headline_is_the_measured_fraction_and_slope_for_a_known_model() {
        let c = compose(&growing_session("claude-3-5-sonnet-20241022"), false);
        let h = c
            .headroom
            .as_ref()
            .expect("a known model + >=2 turns must yield a headroom measurement");
        // Window resolved from the static offline table (NOT guessed).
        assert_eq!(h.window_tokens, 200_000);
        assert_eq!(h.model, "claude-3-5-sonnet-20241022");
        // The fraction is the focus step's prompt tokens / window — a
        // pure integer-percent measurement (snapshot-stable, like `pct`).
        assert!(h.used_tokens > 0);
        assert!(
            h.used_pct < 100,
            "this fixture is well under the window: {}%",
            h.used_pct
        );
        // Exact integer-percent check (no lossy cast): used_pct is the
        // floored `used*100/window`, the same discipline as `pct()`.
        let expect_pct = pct(h.used_tokens, h.window_tokens);
        assert_eq!(h.used_pct, expect_pct, "fraction = floor(used*100/window)");
        // The slope is the MEASURED mean growth across the session
        // (tokens/turn). This fixture strictly grows ⇒ slope > 0.
        assert_eq!(h.turns, 3);
        assert!(
            h.slope_tokens_per_turn > 0,
            "a strictly growing session has a positive measured slope, got {}",
            h.slope_tokens_per_turn
        );
        // The neutral arithmetic projection is `--deep` ONLY — it must
        // NOT appear in the non-deep headline data.
        assert!(
            h.projection.is_none(),
            "the headroom projection must be --deep-only, not in the headline"
        );
    }

    #[test]
    fn c3_projection_is_deep_only_and_neutrally_worded() {
        let c = compose(&growing_session("claude-3-5-sonnet-20241022"), true);
        let h = c.headroom.as_ref().expect("known model + >=2 turns");
        let p = h
            .projection
            .as_ref()
            .expect("--deep must add the neutral arithmetic projection");
        // Neutral arithmetic phrasing ONLY — the research-doc discipline:
        // "at the observed mean rate", never asserted as fate.
        assert!(
            p.contains("at the observed mean rate"),
            "projection must be phrased as a neutral mean-rate arithmetic: {p:?}"
        );
        // It must NEVER assert overflow / truncation / prediction
        // (that is evalint — EXCLUDED by construction).
        for banned in [
            "will overflow",
            "will truncate",
            "you will",
            "run out",
            "exceed",
        ] {
            assert!(
                !p.to_lowercase().contains(banned),
                "projection must not assert fate ({banned:?}): {p:?}"
            );
        }
    }

    #[test]
    fn c3_unknown_model_makes_no_window_claim() {
        // Discipline rule: an unknown wire model id ⇒ NO window claim.
        // Skipped honestly (None), never a fabricated window/fraction.
        let c = compose(&growing_session("totally-unreleased-model-9000"), true);
        assert!(
            c.headroom.is_none(),
            "an unknown model must yield NO headroom (no window claim), got {:?}",
            c.headroom
        );
    }

    #[test]
    fn c3_single_turn_has_no_slope_so_no_headroom() {
        // The slope needs >=2 turns (last-first over turns). A single
        // turn cannot have a measured growth rate ⇒ skip honestly.
        let mut t = Timeline::new();
        t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-3-5-sonnet-20241022","system":"s","messages":[{"role":"user","content":"a short question here for the test"}]}"#,
        );
        let c = compose(&t, true);
        assert!(
            c.headroom.is_none(),
            "a single turn has no measured slope ⇒ no C3 headroom, got {:?}",
            c.headroom
        );
    }

    #[test]
    fn c3_step_namespace_is_exact_and_none_when_unknown() {
        // Pure namespace helper: a single (provider, model) tuple or
        // None. Pins that a step with no provider OR no model is in NO
        // namespace (so a foreign step can never enter the C3 series).
        let mut t = Timeline::new();
        // Known provider + model ⇒ Some namespace.
        t.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"model":"claude-3-5-sonnet-20241022","messages":[{"role":"user","content":"hi there friend"}]}"#,
        );
        assert_eq!(
            step_namespace(&t.steps[0]),
            Some((
                crate::adapter::Provider::Anthropic,
                "claude-3-5-sonnet-20241022"
            ))
        );
        // No provider (unrecognized path, no header) ⇒ None.
        let mut u = Timeline::new();
        u.record_request("POST", "/unknown/path", &[], br#"{"model":"gpt-4o"}"#);
        assert_eq!(u.steps[0].provider, None);
        assert_eq!(step_namespace(&u.steps[0]), None);
        // Provider known but model absent ⇒ None (no fabricated id).
        let mut v = Timeline::new();
        v.record_request(
            "POST",
            "/v1/messages",
            &[],
            br#"{"messages":[{"role":"user","content":"no model field at all here"}]}"#,
        );
        assert_eq!(
            v.steps[0].provider,
            Some(crate::adapter::Provider::Anthropic)
        );
        assert_eq!(step_namespace(&v.steps[0]), None);
    }

    #[test]
    fn c3_slope_is_scoped_to_the_focus_namespace_not_widened() {
        // Mixed-namespace timeline: interleave a DIFFERENT (provider,
        // model) between the focus model's turns. The C3 slope must be
        // measured over the focus namespace's turns ONLY — a filter that
        // widened across namespaces (an `&&`→`||`-class regression)
        // would include the foreign step and corrupt turns/slope. This
        // fixture makes that polarity observable (kills the missed
        // pass-1 mutant by behaviour, complementing the by-construction
        // tuple-equality elimination).
        let mut t = Timeline::new();
        let mk_anthropic = |n: usize| {
            let msgs: Vec<String> = (0..n)
                .map(|k| format!(r#"{{"role":"user","content":"anthropic turn body number {k} reasonably long"}}"#))
                .collect();
            format!(
                r#"{{"model":"claude-3-5-sonnet-20241022","system":"s","messages":[{}]}}"#,
                msgs.join(",")
            )
            .into_bytes()
        };
        // focus-namespace turn 1 (1 msg).
        t.record_request("POST", "/v1/messages", &[], &mk_anthropic(1));
        // a FOREIGN namespace step wedged in (different provider+model,
        // huge prompt) — must NOT enter the focus series.
        t.record_request(
            "POST",
            "/v1/chat/completions",
            &[],
            br#"{"model":"gpt-4o","messages":[{"role":"user","content":"a totally unrelated and very different openai turn that is quite long indeed"}]}"#,
        );
        // focus-namespace turn 2 (3 msgs) — the focus (last parsed of
        // the Anthropic namespace order) and the slope endpoint.
        t.record_request("POST", "/v1/messages", &[], &mk_anthropic(3));

        let h = compose(&t, false)
            .headroom
            .expect("focus = a known Anthropic model with 2 same-namespace turns");
        assert_eq!(h.model, "claude-3-5-sonnet-20241022");
        assert_eq!(h.window_tokens, 200_000);
        // EXACTLY 2 turns counted (the 2 Anthropic turns), NOT 3 — the
        // foreign gpt-4o step is excluded by the namespace filter.
        assert_eq!(
            h.turns, 2,
            "the foreign-namespace step must NOT be in the C3 series"
        );
        // Growing within the focus namespace ⇒ a positive measured slope
        // computed from the focus turns only.
        assert!(h.slope_tokens_per_turn > 0);
    }

    #[test]
    fn c3_slope_per_turn_exact_boundaries() {
        // Deterministic exact-value table — the approximate tokenizer
        // cannot hit these boundaries through compose(), so the pure
        // decision is pinned here (the proven D-010 technique). Each row
        // kills a specific arithmetic/comparison mutant.
        // < 2 turns ⇒ None (no measurable rate); == 2 ⇒ Some.
        assert_eq!(slope_per_turn(100, 100, 0), None);
        assert_eq!(slope_per_turn(100, 100, 1), None);
        assert_eq!(slope_per_turn(100, 300, 2), Some(200)); // (300-100)/1
                                                            // span = turns - 1 (kills `-`→`+` and off-by-one on the divisor):
        assert_eq!(slope_per_turn(100, 400, 4), Some(100)); // (400-100)/3
                                                            // flat ⇒ exactly 0 (kills a mutant that fabricates growth):
        assert_eq!(slope_per_turn(500, 500, 3), Some(0));
        // shrinking ⇒ a real NEGATIVE slope, never clamped to 0:
        assert_eq!(slope_per_turn(900, 300, 3), Some(-300)); // (300-900)/2
                                                             // truncating integer division is intentional & snapshot-stable:
        assert_eq!(slope_per_turn(0, 10, 3), Some(5)); // 10/2
        assert_eq!(slope_per_turn(0, 10, 4), Some(3)); // 10/3 floor
    }

    #[test]
    fn c3_turns_until_window_exact_boundaries() {
        // The NEUTRAL `--deep` projection arithmetic. slope <= 0 ⇒ None
        // (no honest "turns remaining" — kills `<=`→`<`/`==`):
        assert_eq!(turns_until_window(100, 1000, 0), None);
        assert_eq!(turns_until_window(100, 1000, -5), None);
        // remaining = window - used; whole turns at the mean rate:
        assert_eq!(turns_until_window(100, 1000, 100), Some(9)); // 900/100
        assert_eq!(turns_until_window(0, 1000, 300), Some(3)); // 1000/300 floor
                                                               // used == window ⇒ no positive headroom ⇒ None (kills the
                                                               // `remaining == 0` guard and `checked_sub` underflow):
        assert_eq!(turns_until_window(1000, 1000, 50), None);
        // used > window ⇒ checked_sub None ⇒ None (no negative figure):
        assert_eq!(turns_until_window(1500, 1000, 50), None);
    }

    #[test]
    fn c3_slope_is_signed_and_zero_when_flat() {
        // A same-size repeated prompt across turns ⇒ measured slope 0
        // (pins the arithmetic against a mutant that fabricates growth).
        let mut t = Timeline::new();
        let body = br#"{"model":"gpt-4o-2024-08-06","messages":[{"role":"system","content":"sys"},{"role":"user","content":"an identical, stable user prompt repeated each turn here"}]}"#;
        for _ in 0..3 {
            let i = t.record_request("POST", "/v1/chat/completions", &[], body);
            t.record_response(
                i,
                200,
                &[],
                br#"{"choices":[{"message":{"content":"ok"}}]}"#,
            );
        }
        let h = compose(&t, false)
            .headroom
            .expect("known model + 3 turns ⇒ headroom present");
        assert_eq!(h.window_tokens, 128_000);
        assert_eq!(
            h.slope_tokens_per_turn, 0,
            "a flat (non-growing) session has a measured slope of exactly 0"
        );
    }

    // --- C5 (D-015) non-text-payload weight attribution ----------------

    #[test]
    fn non_text_payload_fires_and_is_a_distinct_component_not_silent_history() {
        // A real OpenAI-shaped chat.completions body with a base64 image
        // content-part (a tiny 1x1 PNG data URI). The payload bytes must
        // be attributed to a DISTINCT `non-text-payload` component +
        // indictment, NOT silently inflate `history`.
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let mut img = Timeline::new();
        let body = format!(
            r#"{{"model":"gpt-4o","messages":[{{"role":"user","content":[{{"type":"text","text":"what is in this image, described at some reasonable length"}},{{"type":"image_url","image_url":{{"url":"data:image/png;base64,{png}"}}}}]}}]}}"#
        );
        img.record_request("POST", "/v1/chat/completions", &[], body.as_bytes());

        let c = compose(&img, false);
        assert!(
            c.components.iter().any(|p| p.label == "non-text-payload"),
            "inline image bytes MUST be a distinct `non-text-payload` component, not silently in history: {:?}",
            c.components.iter().map(|p| p.label.as_str()).collect::<Vec<_>>()
        );
        let nt = c
            .indictments
            .iter()
            .find(|i| i.code == "non-text-payload")
            .expect("a non-text payload MUST raise the `non-text-payload` indictment");
        assert!(
            nt.detail.contains("block") && nt.detail.contains("byte"),
            "detail must report block count + bytes: {}",
            nt.detail
        );
        assert!(
            nt.detail.contains('%'),
            "detail must report the percent of the assembled body: {}",
            nt.detail
        );
        // PURE: byte-attribution FACT, never a token-waste class.
        assert!(
            nt.wasted_tokens == 0,
            "non-text-payload is a byte-attribution FACT, not a token-waste class: wasted_tokens must be 0, got {}",
            nt.wasted_tokens
        );
        assert!(
            nt.detail.contains("base64 not decoded")
                && nt.detail.contains("no media token estimate"),
            "detail must state the strictly-pure regime (base64 not decoded, no token estimate): {}",
            nt.detail
        );

        // BOTH WIRE SHAPES + count > 1 (the D-007 dual-provider rule;
        // pins the per-kind tally COUNT through `compose()` so the
        // pass-1 `+=`→`*=` class is killed end-to-end, not only in the
        // helper unit test). Anthropic-shape, TWO image blocks in one
        // message ⇒ the detail must read "2 image".
        let mut anth = Timeline::new();
        let abody = format!(
            r#"{{"model":"claude-3-5-sonnet-20241022","messages":[{{"role":"user","content":[{{"type":"text","text":"compare these two images for me in reasonable detail please"}},{{"type":"image","source":{{"type":"base64","media_type":"image/png","data":"{png}"}}}},{{"type":"image","source":{{"type":"base64","media_type":"image/png","data":"{png}"}}}}]}}]}}"#
        );
        anth.record_request("POST", "/v1/messages", &[], abody.as_bytes());
        let ant = compose(&anth, false)
            .indictments
            .into_iter()
            .find(|i| i.code == "non-text-payload")
            .expect("Anthropic-shape image blocks MUST also raise non-text-payload");
        assert!(
            ant.detail.contains("2 image") && ant.detail.contains("2 block(s)"),
            "two Anthropic image blocks must tally as `2 image` / `2 block(s)`: {}",
            ant.detail
        );

        // CONTROL — a text-only body of comparable size must NOT raise it
        // (correctly ABSENT/zero, the C2/C4 silent-on-text discipline).
        let mut txt = Timeline::new();
        txt.record_request(
            "POST",
            "/v1/chat/completions",
            &[],
            br#"{"model":"gpt-4o","messages":[{"role":"user","content":[{"type":"text","text":"a purely textual user message of a reasonable, comparable length here for the control case"}]}]}"#,
        );
        assert!(
            !compose(&txt, false)
                .indictments
                .iter()
                .any(|i| i.code == "non-text-payload"),
            "a text-only body MUST NOT raise non-text-payload"
        );
        assert!(
            !compose(&txt, false)
                .components
                .iter()
                .any(|p| p.label == "non-text-payload"),
            "a text-only body MUST NOT add a non-text-payload component"
        );
    }

    #[test]
    fn non_text_kinds_are_the_documented_tracked_set() {
        // `black_box` keeps this a real runtime check (not const-folded)
        // — pins the tracked-kind slice + its order so a mutant that
        // drops/reorders/adds an entry is caught (the C1/C3/C4 const
        // discipline). Covers BOTH wire shapes (Anthropic image/document;
        // OpenAI image_url/input_audio/file) — the D-007 dual-provider rule.
        let k = std::hint::black_box(crate::adapter::NON_TEXT_KINDS);
        assert_eq!(
            k,
            &[
                "image",
                "image_url",
                "input_audio",
                "audio",
                "document",
                "file",
            ]
        );
    }

    #[test]
    fn non_text_weight_exact_boundaries() {
        // 0 blocks ⇒ None (text-only is silent; kills the `== 0`→`!= 0`
        // / `is_empty` mutants and a fabricated-Some return):
        assert_eq!(non_text_weight(&[], 1000), None);
        assert_eq!(non_text_weight(&[], 0), None);
        // 1 block: exact count, exact byte sum, integer floored percent
        // (kills count/sum/pct mutants — the approx tokenizer cannot
        // reach these through `compose()`):
        assert_eq!(non_text_weight(&[250], 1000), Some((1, 250, 25)));
        // multiple blocks: count is the slice len, bytes the saturating
        // sum, percent floored (380/1000 = 38%):
        assert_eq!(non_text_weight(&[200, 180], 1000), Some((2, 380, 38)));
        // body smaller than payload (a tiny envelope around a huge image)
        // ⇒ percent is capped only by the integer math, never panics:
        assert_eq!(non_text_weight(&[900], 1000), Some((1, 900, 90)));
        // body == 0 ⇒ pct() is div-by-zero-safe ⇒ 0%, still reports the
        // exact count + bytes (kills a panic/`unwrap` mutant in pct):
        assert_eq!(non_text_weight(&[42], 0), Some((1, 42, 0)));
        // a zero-byte block still counts as a block (count is structural,
        // not byte-gated — kills a `len()` → byte-filter mutant):
        assert_eq!(non_text_weight(&[0], 100), Some((1, 0, 0)));
    }

    #[test]
    fn kind_tally_counts_per_kind_in_fixed_order() {
        use crate::adapter::NonTextPart;
        let p = |k: &str| NonTextPart {
            kind: k.to_string(),
            bytes: 1,
        };
        // empty ⇒ empty string (kills a fabricated-nonempty mutant):
        assert_eq!(kind_tally(&[]), "");
        // a single block ⇒ "1 <kind>" (count == 1):
        assert_eq!(kind_tally(&[p("file")]), "1 file");
        // TWO blocks of the SAME kind ⇒ "2 image_url" — count is 2, so a
        // `+=`→`*=` mutant (1*1==1, but 1+1==2) is now KILLED, and a
        // `count()`→`0`/`1` mutant differs from 2 (the pass-1 fix is
        // pinned BY a discriminating value, not just by construction):
        assert_eq!(
            kind_tally(&[p("image_url"), p("image_url")]),
            "2 image_url"
        );
        // mixed kinds appear in the FIXED `NON_TEXT_KINDS` declaration
        // order (image, image_url, input_audio, audio, document, file),
        // NOT input order — a reorder/`BTreeMap`-resort mutant is caught:
        assert_eq!(
            kind_tally(&[p("file"), p("image"), p("image"), p("document")]),
            "2 image, 1 document, 1 file"
        );
        // a kind NOT in `NON_TEXT_KINDS` never appears (the membership
        // discipline; `non_text_of` would not have emitted it anyway):
        assert_eq!(kind_tally(&[p("text")]), "");
    }
}
