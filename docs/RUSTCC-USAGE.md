# RUSTCC-USAGE.md — rust-cc compiler-truth artifacts

> Per-feature record of the rust-cc compiler-truth loop (COMPILER-TRUTH.md,
> 12 laws) with concrete artifacts. No artifact ⇒ HALT. The PostToolUse
> `rustcc` digest is the signal; `just check`/`just test` are the gates;
> `cargo mutants --in-diff` proves the tests assert (Law 11).

---

## C6 / D-013 — `request-replayed` (same-body retry-replay F1 indictment)

Isolated worktree branch `worktree-agent-a8666b7b81bf44e78`. C6 works on
`step.request.body` / `step.response` only — `adapter.rs::Assembled` was
NOT touched (HALT condition checked and not triggered; the data the rule
needs — verbatim body + buffered response status — is already on `Step`).

### Loop discipline (Laws 1–8)

- **TDD red-first (Law 11 + the goal's red-first mandate).** Three C6
  tests added to `compose.rs` BEFORE any implementation. Proven RED on
  worktree HEAD `410c790`:
  `cargo nextest run -E 'test(request_replayed)'` →
  `2 failed, 1 passed` (the two "fires" tests fail with
  *"a byte-identical re-send MUST be indicted as request-replayed"*; the
  control "silent" test passes — correctly nothing fires yet). The rest
  of the suite proven still green at the red HEAD:
  `cargo nextest run -E 'not test(request_replayed)'` → **109/109**.
  Red-first committed as `410c790` before the feature commit.
- **Law 1 (no claim without a tool) / Law 3–4 (one root class).** Every
  `.rs` edit ran the `rustcc` PostToolUse gate (`cargo check`). The C6
  implementation compiled clean on the first `cargo check` (no error
  cascade; no borrow/lifetime/trait/move family → no `borrow-checker-
  surgeon` needed, no `/rust-fix` iteration needed). Recorded artifact:
  `cargo check` → `Finished dev profile … in 13.37s`, no diagnostics.
- **Law 2 (no memorized crate APIs).** No new crates (`/rust-deps` not
  required). C6 reuses `crate::tokenizer::count`, `std::collections::
  BTreeMap`, and std iterator combinators (`filter`, `max_by_key`) — all
  already in use in `compose.rs`; APIs are compiler-verified, not
  recalled.
- **Law 5 (never commit over red) / Law 8 (not done while red).**
  `just check` (`cargo clippy --all-targets -- -D warnings`) → clean,
  0 warnings. `just test` → `cargo nextest run` **113/113** + doctests
  `ok`. Commit-gate never bypassed; two atomic commits
  (`test(C6/D-013)` red-first, `feat(C6/D-013)` impl) each on a green
  build.
- **Law 9 (no unwrap/expect in non-test code).** `indict_request_replayed`
  / `replay_wasted` use `?`-free total combinators, `map_or_else` for the
  status fallback, and `sat_sum` (saturating) for all arithmetic — no
  `unwrap`/`expect`/panic path on attacker-influenced `ctx open` bytes.
  `unwrap` appears only in `#[cfg(test)]`.
- **Law 10 (no clone to dodge borrow).** No `.clone()` added; the rule
  borrows `&timeline.steps` and keys a `BTreeMap<&str, Vec<&Step>>` by
  reference.

### Real verification (green ≠ works) — `ctx run` / `ctx open`

Real local python httpx clients (no real key; DUMMY `Authorization:
Bearer test`; natural `/v1/chat/completions` path; upstream deliberately
fails — F0 captures the request BEFORE forwarding; $0):

- **Fires (replay).** `/tmp/c6_replay_client.py` POSTs the SAME body
  twice (a real retry-after-failure shape).
  `ctx run --save /tmp/c6.db -- python3 …` →
  `waste request-replayed wasted_tokens=60 1 request body re-send(s)
  across 1 distinct body/ies, re-billed verbatim; replayed attempt
  status not captured` (upstream unreachable ⇒ no buffered response ⇒
  the honest "status not captured" fallback, NOT a fabricated status).
- **Silent (control).** `/tmp/c6_control_client.py` POSTs two DIFFERENT
  bodies. `ctx run` → only `preamble-repay`; **no `request-replayed`**
  (proves whole-body byte-equality, not similarity).
- **Post-hoc round-trip.** `ctx open /tmp/c6.db` and
  `ctx open /tmp/c6.db --json` both re-emit the `request-replayed`
  indictment (`wasted_tokens: 60`) through the SQLite save/load — F0
  contract intact, `--json` CI contract carries it.
- **Buffered-status annotation (real 529).** A local upstream returning
  529 (`/tmp/c6_upstream_529.py`) makes the proxy buffer a real
  response; the replay run then reports
  `… re-billed verbatim; first replayed attempt returned 529` —
  exactly the research §c shape ("step 5 == step 4 body; step 4
  returned 529"), sourced from F0's already-buffered response.

### Mutation-hardening (Law 11) — `cargo mutants --in-diff`

Proactive D-010 technique applied BY CONSTRUCTION before the first run:
the cost decision is isolated in the pure `replay_wasted(body_tokens,
extra_copies)` helper with an exact-boundary unit table
(`replay_wasted_exact_boundaries`) the approximate tokenizer cannot reach
through `compose()`; the replay detection uses std `filter(len() >= 2)` /
`max_by_key` / `sat_sum` (no hand-written `<`/`||`/compound boolean for a
mutant to flip); empty-body (`!is_empty`) and `wasted == 0` guards remove
degenerate cases.

- **Pass 1** (`/tmp/c6_src.diff`, `--test-tool nextest --timeout 120`):
  **Found 13 mutants; 11 caught, 2 unviable, 0 MISSED** on a REAL
  baseline — `Unmutated baseline ok 31s build + 8s test` (explicitly
  non-vacuous, not timed-out; the integrity bar the D-008 incident set).
  The 2 unviable are the `Default::default()` return-replacement arms
  (do not compile — genuinely unviable, not skipped). Caught mutants
  cover every load-bearing site: `replay_wasted → 0 / 1`,
  `indict_request_replayed → None`, `delete !` on the empty-body filter,
  `>= → <` on the replay threshold, `- → + / /` on both
  `len() - 1` sites, `== → !=` on the `wasted == 0` guard, and the
  `indict()` registration.
- **Pass 2** (confirming, fresh `mutants.out`): **13 mutants; 11
  caught, 2 unviable, 0 MISSED** on a fresh REAL baseline —
  `Unmutated baseline ok 40s build + 14s test` (independent build, not
  a reused cache; non-vacuous). Identical result to pass 1 ⇒ the C6
  test suite genuinely pins the behaviour (Law 11 satisfied; no
  equivalent-mutant hand-waving — every mutated operator is either
  caught or structurally unviable).

`/rust-review` + `/rust-harden` skills are not invocable inside this
agent thread; substituted by an in-thread tool-grounded self-review
(pure / total / panic-free / saturating / byte-equality-only / no
`guard`-style intervention) — deliberately NOT subagent-delegated (the
D-008 review-subagent integrity incident). Recorded as a substitution,
not an independent SHIP.
