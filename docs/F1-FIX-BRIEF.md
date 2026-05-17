# ctx — F1-FIX-BRIEF (binding execution contract)

> BINDING for this work item. Adds the fix sequence; does NOT restate
> the spec. On conflict, `docs/PROJECT.md` is canonical. Build on HEAD
> `8160b09` (R/F0/F1/F2/F3 DONE). Targeted defect fix — do NOT
> re-scope/re-architect; F0/F2/F3 must NOT regress.

## 0. The defect (empirically proven on a real OpenRouter run)

F0 capture + F2 (`view`) + F3 (`diff`) work provider-agnostically (raw
bytes). **F1 composition+waste only parses the Anthropic Messages wire
shape.** On an OpenAI-compatible body (`chat.completions`:
`messages:[{role:system|user|assistant}], tools:[...]`) F1 prints
`composition no captured prompt · 0 tokens · 0 findings` **despite a
full F0 capture** (verified: `step=2, capture=4`, provider
`open_ai_compat`, `ctx view`/`diff`/`open --json` all show the real
bodies; only F1 is blind). `PROJECT.md` §4/§5 declare v1 providers =
Anthropic **+ OpenAI-compatible**, so the flagship headline is silently
broken on half the supported surface (OpenAI / OpenRouter / Azure /
ollama). Root cause the prior "F1 SHIP / mutation-tested" missed:
**snapshot fixtures were Anthropic-shape ONLY** — CI never exercised
F1 on an OpenAI body.

## 1. Source of truth (binding; obey, never contradict)

- CANONICAL: `/Users/igor/Projects/caliper/ctx/docs/PROJECT.md`
- Also: `ctx/CLAUDE.md`, `ctx/docs/DECISIONS.md` (D-001..D-006),
  `ctx/docs/RESEARCH.md`,
  `/Users/igor/Projects/caliper/docs/11-cli-design-system.md` =
  MANDATORY exact CLI contract, `/Users/igor/Projects/rust-cc/COMPILER-TRUTH.md`.
- Anti-relitigate D-001..D-006: wire-proxy canonical; **evalint KILLED
  — F1 stays PURE MEASUREMENT, no scoring/judge/prediction**;
  graph/skeleton reconstruction NEVER built; build-now. Local git only;
  name provisional — never publish/rename.

## 2. rust-cc = VERIFIED BY ARTIFACT (ctx/.claude/settings.json already enables rust@rust-cc)

Develop EVERY `.rs` change in the compiler-truth loop, obeying the 12
laws in `/Users/igor/Projects/rust-cc/COMPILER-TRUTH.md`:

- the PostToolUse `rustcc` digest IS the signal acted on;
- red ⇒ `/rust-fix` (NOT a hand-fix); borrow/lifetime/trait/move ≥2 ⇒
  `borrow-checker-surgeon`;
- `/rust-deps` for crates (API-truth); `/rust-test`
  (nextest + doctests, `--cov`);
- `/rust-review` then `/rust-harden` before EVERY commit;
- commit-gate NEVER bypassed (no `--no-verify`, no `RUSTCC_GATES=off`);
- `/rust-harden` full-F1-surface mutants via `--in-diff`.

PROOF: maintain `ctx/RUSTCC-USAGE.md` — per step log the concrete
artifacts (ranked-digest excerpts that drove each fix, skills/subagents
fired, `/rust-harden` verdict, mutation result); every commit cites it.
A step with no artifact = treated as NOT used → HALT.

## 3. EXECUTE (sequential; each EXIT green before the next)

- **A. TDD — failing test FIRST.** Add a real OpenAI `chat.completions`
  fixture (system + user + assistant history + a `tools[]` schema) and
  an F1 composition test asserting it decomposes into the same source
  categories (system / tool-schemas / history) with non-zero tokens +
  the waste indictments. EXIT: the new test **FAILS on HEAD** (the
  defect is proven real, not assumed), Anthropic F1 tests still pass.
- **B. Fix F1 only.** Make `compose.rs` parse the OpenAI
  `chat.completions` shape (`messages[].role` system/user/assistant →
  system/history; `tools[]` → tool-schemas) into the SAME composition
  model + indictments as Anthropic. PURE MEASUREMENT only (counts +
  tokenizer sums; NO judge/scoring). F0 already classifies the provider
  `open_ai_compat` — branch on that; do not re-derive the provider.
  EXIT: the step-A test PASSES; F1 correct on BOTH provider shapes.
- **C. No regression + harden.** F0/F2/F3 unchanged (their tests +
  snapshots green, verbatim bytes byte-identical); full $0 suite +
  clippy 0/0 + nextest green; `/rust-review` SHIP 0-high (apply
  consequential findings); `/rust-harden` full-F1-surface mutants
  **0 missed**. EXIT: all green via the loop; `RUSTCC-USAGE.md` cites
  digests/mutation/harden; atomic commit.
- **D. Close the gap durably.** Record `DECISIONS.md` **D-007** (root
  cause = Anthropic-only fixtures; the fix; the standing rule "every
  F1 test runs on BOTH provider shapes"); add a test that enforces
  both-shape F1 coverage so CI can never regress this class again.
  EXIT: D-007 written; the enforcement test green; commit.

## 4. HARD INVARIANTS — HALT & report (no proceed/fabricate/weaken) if ANY fails

- rust-cc loop actually exercised with per-step `RUSTCC-USAGE.md`
  artifact PROOF; bypass / no artifact ⇒ HALT, never hand-roll raw
  cargo.
- The step-A test MUST fail pre-fix and pass post-fix — the fixture is
  never rigged to pass; the defect is proven real (TDD).
- F1 stays PURE MEASUREMENT (NO scoring/judge/prediction — evalint
  KILLED).
- F0/F2/F3 zero-regression — their tests pass and `view` verbatim
  bytes are byte-identical.
- tokenizer stays offline, ±N%-labeled, never an API call to count.
- graph/skeleton reconstruction NEVER built (REFUTED).
- NO emoji; CLI EXACTLY per doc 11 (terracotta `#d97757`, `⏺ ⎿ ·`,
  `NO_COLOR`/`CLICOLOR` precedence, plain grep-clean, one record/line).
- single static binary; compiler-truth REAL (cargo actually green via
  rust-cc, never claimed-green); `/rust-harden` mutants 0 missed on
  the F1 surface.
- do not relitigate D-001..D-006; never publish/rename; one atomic
  commit per step; never fabricate a green / number.

> The plan = §0–§1 docs (PROJECT.md canonical) + this brief. Execute
> §3 A→D via the verified rust-cc loop with the §4 invariants intact;
> prove the defect with a failing OpenAI-shape F1 test, fix
> `compose.rs`, zero F0/F2/F3 regression, close the dual-provider gap
> (D-007); halt honestly on any invariant failure.
