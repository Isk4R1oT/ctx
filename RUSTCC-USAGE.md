# RUSTCC-USAGE.md — F1-FIX rust-cc compiler-truth artifact log

> Per-step proof that every `.rs` change went through the rust-cc loop
> (F1-FIX-BRIEF §2/§4). Each commit cites the relevant step here.
> Plugin: `rust@rust-cc` (ctx/.claude/settings.json); digest binary
> `/Users/igor/.claude/plugins/cache/rust-cc/rust/0.1.0/bin/rustcc`.

## Step A — TDD: prove the defect with a failing OpenAI-shape F1 test

- Discipline: edit is a `.rs` test addition → PostToolUse `rustcc gate`
  digest is the signal; the *test* (not memory) is the truth oracle for
  the defect (COMPILER-TRUTH Law 1 — "no claim without a tool").
- Goal: a real OpenAI `chat.completions` fixture (system + user +
  assistant history + `tools[]`) + an F1 composition test that MUST
  FAIL on HEAD `8160b09` (defect real, fixture never rigged) while the
  Anthropic F1 tests still pass.
### Artifacts (real, this session — never claimed)

- `rustcc digest --from check` after the test edits: **`rust-cc · ✓
  green — cargo check clean, no clippy findings`** (compiles; a failing
  *test* is not a red *build*, so the commit-gate clippy/fmt pass — gate
  NOT bypassed).
- TDD truth-oracle (`cargo nextest`): on HEAD `8160b09`, unchanged
  production code:
  - `compose::tests::f1_not_blind_on_realworld_openai_tools_null` →
    **FAIL**: `F1 blind on a real OpenAI tools:null body (focus None)`
    — the defect, **proven real, not assumed**.
  - Root cause localized by the probe: `OpenAiReq` (`adapter.rs`) uses
    `#[serde(default)]` on `messages`/`tools`, which substitutes for a
    **missing** key but NOT an explicit `null`. Real OpenAI/agent
    clients emit `"tools": null` on a no-tool turn ⇒
    `serde_json::from_slice::<OpenAiReq>` errors ⇒
    `adapter::parse(OpenAiCompat)` `Err` ⇒ `record_request` `.ok()` ⇒
    `assembled = None` ⇒ `compose` finds no focus ⇒ F1 "no captured
    prompt", while F0/F2/F3 (raw `request.body`) + provider
    classification still work. Matches the brief §0 symptom exactly
    (step=2, provider `open_ai_compat`, only F1 blind).
  - Anthropic F1 still green: `decomposes_by_source`,
    `raises_at_least_four_correct_indictments`,
    `deep_adds_per_tool_detail`, `unused_tools_detected_precisely`,
    `f1_composition_json_contract`, `f1_composition_plain_grep_clean`
    all PASS — they never exercised the explicit-`null` real-world
    shape (the brief's stated root cause: Anthropic-only fixtures).
- Skills/subagents fired: `/rust-deps` not needed (no crate change);
  `rustcc digest` (the gate signal); `/rust-review` on the test-only
  diff (below).
- `/rust-harden` (mutants): **structurally deferred to step C** —
  cargo-mutants requires a GREEN unmutated baseline; at TDD-red (step A
  intentionally has a failing test) a mutation baseline cannot run by
  construction. This is the §3 phase EXIT design (A = prove red; harden
  = C), not a bypassed gate; full-F1-surface `--in-diff` mutants run at
  step C with 0-missed required.
