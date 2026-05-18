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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Composition {
    /// The step whose breakdown is shown (last step with a parsed prompt).
    pub focus_step: Option<usize>,
    pub total_tokens: usize,
    pub components: Vec<Component>,
    /// Per-tool token detail — only populated when `--deep`.
    pub tools_deep: Vec<Component>,
    pub indictments: Vec<Indictment>,
}

fn pct(part: usize, total: usize) -> u32 {
    part.saturating_mul(100)
        .checked_div(total)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0)
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
    }
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
    let replayed: Vec<(&&str, &Vec<&crate::timeline::Step>)> =
        by_body.iter().filter(|(_, steps)| steps.len() >= 2).collect();
    if replayed.is_empty() {
        return None;
    }
    let distinct = replayed.len();
    // Total re-billed copies across all replayed bodies (each body's
    // `len() - 1` extra copies) and the matching token cost.
    let extra_copies = sat_sum(replayed.iter().map(|(_, steps)| steps.len() - 1));
    let wasted = sat_sum(replayed.iter().map(|(body, steps)| {
        replay_wasted(crate::tokenizer::count(body), steps.len() - 1)
    }));
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
            format!(r#"{{"model":"{model}","system":"{}"{big_tail}"#, sys_fill.repeat(400))
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
        assert!(w.wasted_tokens > 0, "must report the re-sent broken-tail cost");

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
        assert!(!has_code(&one, "cache-prefix-break"), "one step cannot break a cache prefix");

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
}
