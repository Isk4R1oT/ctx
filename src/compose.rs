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
    ]
    .into_iter()
    .flatten()
    .collect()
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
}
