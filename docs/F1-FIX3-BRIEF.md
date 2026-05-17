# ctx — F1-FIX3-BRIEF (binding execution contract)

> BINDING for this work item. This is the THIRD attempt at the same
> defect; the prior two reported "complete" and were not. Read §1
> before anything else. On conflict, `docs/PROJECT.md` is canonical.
> Build on HEAD `f866dac`. Targeted defect fix; F0/F2/F3 must NOT
> regress. ctx is Rust — develop via the rust-cc compiler-truth loop,
> usage VERIFIED BY ARTIFACT (`ctx/.claude/settings.json` enables
> rust@rust-cc).

## 0. The defect, EMPIRICALLY VERIFIED (offline repro, $0, no key)

The exact real OpenRouter/OpenAI wire body (the one F2 `ctx view`
showed as clean valid JSON) was replayed through HEAD `f866dac`. Result:

```
> composition step 0  105 tokens
component raw-body (structured parse failed; counted verbatim) 105 100%
summary: 105 tokens, 0 finding(s)
```

So: F1-FIX (D-007, serde-null) and F1-FIX-2 (D-008, Value-walk +
"constructive proof") **did not fix the capability.** They only made
F1 stop saying "0 tokens / no captured prompt" by counting the whole
body as one opaque blob via the **Layer-2 raw-body fallback**. On a
**valid-JSON real OpenAI body, Layer-1 structured decomposition STILL
FAILS** → no system/tool-schema/history split, **0 waste indictments**.
That is NOT F1 working: F1's entire spec value (`PROJECT.md` §4/§5) is
decompose-by-source + indict waste. A token count of an opaque blob is
a non-result for the flagship.

## 1. THE CARDINAL ACCEPTANCE RULE (this is why the last two "passed" yet failed)

F1-EXIT is satisfied **only** when, on a **verbatim real captured wire
body** (an actual `ctx --save` capture of a real OpenAI/OpenRouter
request — NOT a hand-authored fixture), F1 emits the **structured
decomposition**: per-source components (system, tool-schemas, history,
…) each with a non-zero token count where present, AND ≥1 waste
indictment where applicable.

HALT-worthy, all explicitly forbidden — these are the exact cheats the
prior attempts used:

- satisfying F1-EXIT via the **Layer-2 raw-body / "structured parse
  failed" path on a VALID-JSON body** — that is FAIL, never pass.
  Layer-2 is legitimate ONLY for genuinely non-JSON bytes.
- substituting a **synthetic / hand-authored fixture** for the real
  captured body (D-008's own rule, now enforced as ACCEPTANCE, not
  just as a TDD guideline).
- declaring "complete / works / class closed by construction" on a
  **constructive argument**. "Not blind when bytes exist" is NOT
  acceptance. Only real structured decomposition of the real body is.
- weakening / relaxing the F1-EXIT assertion to make it pass.

Until a live OpenRouter run is possible (needs the user's key +
usage), the STRONGEST claim permitted is exactly: *"Layer-1 decomposes
the verbatim real captured OpenAI body in an offline test"* — stated
with that bound, never as "works on real OpenRouter". The live run
remains the ultimate acceptance and is owed (record in D-009).

## 2. Source of truth

CANONICAL `docs/PROJECT.md`; also `ctx/CLAUDE.md`, `docs/DECISIONS.md`
(D-001..D-008), `docs/RESEARCH.md`,
`/Users/igor/Projects/caliper/docs/11-cli-design-system.md` (MANDATORY
CLI), `/Users/igor/Projects/rust-cc/COMPILER-TRUTH.md`. Anti-relitigate
D-001..D-008: wire-proxy canonical; evalint KILLED (F1 = pure
deterministic measurement, no scoring/judge); graph reconstruction
never built. Local git only; never publish/rename.

## 3. EXECUTE (sequential; each EXIT green before the next; rust-cc loop, RUSTCC-USAGE.md proof per step)

- **A. Diagnose the REAL Layer-1 failure — no fix yet.** Replay the
  verbatim real OpenAI body and determine PRECISELY why the structured
  extractor yields nothing/Err on valid JSON (the prior two guessed —
  serde-null, then Value-walk — both wrong/insufficient). Identify the
  exact field / shape / code path. Record as **D-009** (real
  mechanism; supersedes D-007/D-008 mechanisms; keeps their standing
  rules). EXIT: D-009 names the verified mechanism with evidence; zero
  production code changed yet.
- **B. Failing test with the REAL body, FIRST.** Add a `ctx --save`
  capture of a real OpenAI-shaped request as a committed fixture
  (verbatim bytes), and a test asserting F1 yields the structured
  decomposition (system tok>0, tool-schema tok>0, history tok>0) and
  is NOT the raw-body fallback. EXIT: the test FAILS on `f866dac`
  (Layer-1 still broken — defect proven on the real body, not a
  synthetic), the 94 existing tests still green.
- **C. Fix Layer-1 to genuinely decompose the real body.** Layer-2
  remains ONLY for truly-non-JSON bytes; a valid-JSON body MUST go
  through Layer-1 structured decomposition. Pure deterministic
  measurement; no judge; F0 provider classification unchanged. EXIT:
  the step-B real-body test passes via Layer-1 (NOT Layer-2); F1 shows
  the decomposed components + indictments on the real body.
- **D. No-regression + REAL harden + honest record.** F0/F2/F3
  byte-identical (their tests + verbatim view bytes); 94+/94+ nextest;
  clippy 0/0; deny/machete ok; `/rust-review` SHIP 0-high; `/rust-harden`
  mutants on a **real green baseline** (a vacuous / timed-out / no-
  mutants-generated baseline is NOT a pass — reject and re-establish,
  exactly as the integrity incident required) → 0 missed on the F1
  surface. Record the bounded honest claim (per §1) in D-009 +
  RUSTCC-USAGE.md + the commit message — never overclaimed. EXIT: all
  tool-verified green; atomic commit.

## 4. HARD INVARIANTS — HALT & report (no proceed/fabricate/weaken) if ANY fails

- §1 cardinal acceptance in full: Layer-2/raw-body on valid JSON =
  FAIL; synthetic-fixture substitution = FAIL; constructive-proof-as-
  acceptance = FAIL; assertion-weakening = FAIL; claim stated with the
  exact §1 bound, never overclaimed.
- defect proven by a test that FAILS pre-fix on the verbatim REAL body
  and PASSES post-fix via Layer-1.
- rust-cc loop actually exercised with per-step RUSTCC-USAGE.md
  artifact proof; bypass / no-artifact / vacuous-mutation-baseline ⇒
  HALT (never accept a non-real green; never hand-roll raw cargo).
- F1 stays pure deterministic measurement; evalint KILLED; no judge.
- F0/F2/F3 zero-regression (tests + verbatim bytes byte-identical).
- tokenizer offline ±N%-labeled; graph/skeleton never built; NO emoji;
  CLI EXACTLY per doc 11; single static binary; compiler-truth real.
- review-subagent integrity: any subagent that hits a usage cap /
  violates read-only / leaves stray files ⇒ detect (not via self-
  report), surface, remove, re-establish the true baseline before any
  gate is trusted.
- do not relitigate D-001..D-008; never publish/rename; one atomic
  commit per step; never fabricate a green/number; record honest
  deviations, not silently.

> The plan = §0–§2 + this brief; PROJECT.md canonical. Execute §3 A→D
> with §1+§4 intact. The bug is "Layer-1 cannot decompose a real
> valid-JSON OpenAI body" — fix THAT, proven on the verbatim real
> captured body, not by a fallback or a proof. A green that is Layer-2
> on valid JSON, or rests on a constructive argument, is a FAILURE, not
> a success. Halt honestly on any breach.
