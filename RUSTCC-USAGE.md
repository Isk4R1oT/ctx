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

## Step B — fix F1 (adapter.rs lenient null-tolerant parse) + Step C — no-regression + harden

- **Brief-deviation, recorded honestly (not silent, per §1 PROJECT.md
  canonical-on-conflict):** §3-B says "fix compose.rs", but the
  TDD-probe + rust-review proved the root cause is `adapter.rs`
  (`#[serde(default)]` ≠ explicit `null`). `compose.rs` already
  decomposes BOTH shapes provider-agnostically (its OpenAI
  decomposition test passes pre-fix). Fixing `adapter.rs` is the
  minimal change that upholds D-001 (adapter = the single
  wire-normalization layer), §0 "do NOT re-architect", and yields the
  least regression surface (compose/F1/F0/F2/F3 untouched). Recorded
  here, in D-007, and the commit.
- Change: `null_or_missing_as_default` serde helper +
  `#[serde(default, deserialize_with=…)]` on `AnthropicReq.messages/
  tools` + `OpenAiReq.messages/tools` (twin symmetry — same
  `parse(...).ok()` blind-spot code path).
- Artifacts (real, this session):
  - `rustcc digest --from check`: **green** (cargo check + clippy clean)
    after the edit; commit-gate clippy/fmt pass (NOT bypassed).
  - `cargo nextest`: **92/92** — `f1_not_blind_on_realworld_openai_
    tools_null` now PASSES (red→green); all Anthropic F1, F1 snapshots,
    F0/F2/F3 (wire_capture f2/f3 + snapshots) PASS = zero regression.
  - `cargo test --doc`: ok (0 doctests).
  - `/rust-review` ×2 (`rust-reviewer` + `runtime-soundness`):
    **SHIP, 0 findings** — serde matrix empirically replicated
    (missing→[], null→[], value→value, malformed→Err preserved);
    recursion-bounded, panic-free; faithfulness strictly ≥ prior;
    pure-measurement intact; Anthropic twin = intended symmetry not
    creep; no consequential findings to apply.
  - `/rust-harden`: `cargo deny` advisories+bans+licenses+sources ok;
    `cargo machete` clean; **`cargo mutants --in-diff` (F1-fix
    surface): 1 found / 1 caught / 0 missed** (Law 11 — the changed
    code's mutant is killed by the step-A/adapter tests).
- B EXIT (step-A test passes; F1 correct on BOTH shapes) ✓.
  C EXIT (no regression; review SHIP 0-high; harden 0-missed; all
  green via the loop) ✓ — committed atomically (B+C: the fix commit is
  the §2-gated unit; a separate empty C commit would be meaningless).

## Step D — durable closure: D-007 + dual-provider F1 enforcement test

- Change: `docs/DECISIONS.md` D-007 (root cause = Anthropic-only +
  clean-only fixtures; the adapter fix; the binding standing rule
  "every F1 test/fixture exercises BOTH v1 provider shapes incl.
  null/omitted variants") + `compose::tests::
  f1_must_not_be_blind_on_any_v1_provider_shape` (a 5-row table:
  Anthropic clean, Anthropic tools:null, OpenAI clean, OpenAI
  tools:null+tool_choice:null, OpenAI content-arrays+tool_calls;
  asserts F1 never blind on any v1 shape).
- Artifacts (real, this session):
  - `rustcc digest --from check`: green (compiles, clippy clean).
  - `cargo nextest`: **93/93** — the new enforcement test PASSES;
    full suite green (F0/F1/F2/F3 zero-regression).
  - `/rust-review` (`rust-reviewer`): **SHIP, 0 findings** — and it
    *empirically proved the guard is load-bearing*: reverting the B/C
    `adapter.rs` fix makes the enforcement test FAIL naming BOTH
    regressed shapes (Anthropic null AND OpenAI null), so it durably
    guards the whole D-007 class, not just the step-A case. Zero
    production code changed (byte-proven; additions strictly inside
    `#[cfg(test)] mod tests`).
  - `/rust-harden`: `cargo mutants --in-diff` ⇒ **"No mutants to
    filter"** — the step-D src delta is test-only; cargo-mutants never
    mutates `#[cfg(test)]`, so there is genuinely no production logic
    to mutate (vacuously 0-missed, recorded truthfully — the
    production fix was already mutation-verified at B/C: 1 found / 1
    caught / 0 missed). `cargo deny` ok; `cargo machete` clean.
- D EXIT (D-007 written; enforcement test green; durable class guard) ✓.

## F1-FIX-2 — REAL fix: defensive Value-walk parse + Layer-2 (D-007 was insufficient)

- Trigger: a real OpenRouter run on D-007 HEAD STILL showed F1 blind →
  D-007's serde-null fix was mis-targeted. Root cause = rigid typed
  parse rejects valid-JSON real-client shapes (whack-a-mole class).
- Change: `src/adapter.rs` (typed structs + helper deleted; `parse` →
  `serde_json::Value` defensive walk, cannot Err on valid JSON) +
  `src/compose.rs` (Layer-2: bytes ⇒ never blind).
- Artifacts (real, this session, nothing claimed):
  - `rustcc digest`: green (cargo check + clippy 0/0).
  - `cargo nextest`: **94/94** on a CLEAN tree; **F1 snapshots
    byte-identical** ⇒ zero F1 regression (parity); F0/F2/F3 green.
  - `/rust-review` rust-reviewer: **SHIP, 0 high/med** — parity
    field-mapped + snapshot-proven; class closed by construction;
    pure-measurement intact.
  - `/rust-review` runtime-soundness: **DID NOT COMPLETE** (subagent
    hit usage cap) AND violated its read-only contract (left an
    untracked `tests/_zz_hostile_probe.rs` that broke the mutation
    baseline). Detected via the *failed* mutants run, NOT self-report.
    Signals harvested (hostile-shape probes corroborate the fix; `p3`
    was a wrong non-contract assertion; `p5/p8` = pre-existing
    tokenizer-huge-input slowness, D-006-style follow-up). File
    removed; tree clean. Soundness **substituted** by tool-grounded
    checks (no unsafe/async; `pct` checked_div; `.get()`-only
    determinism; serde_json `remaining_depth:128`) — recorded as a
    substitution, NOT an independent SHIP.
  - `/rust-harden` mutants: FIRST run = **vacuous** (`cargo test`
    timed out on the stray probe → "no mutants were tested"); I
    refused that as a false green. After cleanup + true green
    baseline: **Found 15 mutants · baseline ok · 15 tested: 13
    caught, 2 unviable, 0 missed** (Law 11, genuine). `cargo deny`
    ok; `cargo machete` clean.
- Honest status: real fix; class closed by construction grounded in
  F2's verified valid-JSON observation; NOT re-confirmed by a fresh
  live OpenRouter hit (key revoked). D-008 records all of the above.
