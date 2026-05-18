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

---

## F1-FIX3 — step A (diagnosis only; rust-cc loop NOT engaged: zero `.rs` changed)

- **rust-cc artifact:** none, and that is correct for step A. The
  brief §3-A mandates "zero production code changed yet"; no `cargo`
  edit/check/fix was run, no PostToolUse `rustcc` digest produced —
  the compiler-truth loop engages at step C (the fix), not for a
  read-only diagnosis. A missing artifact here is NOT a bypass.
- **Method:** static path trace (`proxy.rs`→`timeline.rs`→
  `adapter.rs`→`compose.rs`) proved the Layer-2 string requires
  `assembled==None` for all steps ⇒ exactly H1 (detect→None) or H2
  (`from_slice`→Err); H3 (parsed-but-empty) eliminated by code.
- **Real captures (NOT synthetic, per brief §1):** built `ctx` at
  `f866dac`; ran 5 genuine `ctx run --save` captures of a real httpx
  agent client against OpenRouter (authorized limited key; real 200
  on the plain run, cost $0.0000231; variants 403/404 — request
  still captured BEFORE forward). plain/stream/tools-null/2-turn =
  Layer-1 ✅; `Content-Encoding: gzip` = Layer-2 ❌ (reproduces §0).
- **Evidence pinned in D-009:** stored gzip body `1F EF BF BD 08…`
  (`8B`→`EF BF BD`=U+FFFD) vs plain `7B 22 6D 6F`=`{"mo`; provider
  detected ⇒ H2. Root cause = `timeline.rs:73` lossy String at the
  F0 capture boundary, NOT `adapter::parse` (D-007/D-008 misfire).
- **HALT (honest, per §4 “record deviations, not silently”).** The
  verified mechanism contradicts brief §0/§1 (defect is NOT a
  valid-JSON Layer-1 failure; fix is NOT F1-only — it is the shared
  F0 capture/persistence path). Surfaced to the user for the
  scope/one-way-door decision BEFORE step B/C. No code, no fixture,
  no green claimed.

---

## F1-FIX3 — step B (failing test on a VERBATIM REAL gzip capture; commit-gate green)

- **Real fixture (NOT synthetic, brief §1):**
  `tests/fixtures/real_openai_gzip_request.bin` = 446 bytes, the exact
  wire bytes a real httpx OpenAI-shaped client put on the wire with
  `Content-Encoding: gzip`, recorded by a real listener. Magic `1F 8B
  08 00` (UNMANGLED) — vs the ctx-saved copy `1F EFBFBD 08` (already
  destroyed by the bug). Decompresses to 965 B valid JSON
  (system+user+assistant+user, tools lookup_complexity/run_benchmark).
- **rust-cc loop artifact:** Edit on `src/compose.rs` → PostToolUse
  `rustcc` digest clean; `just check` (`cargo clippy --all-targets
  --all-features -D warnings`) = **Finished, 0 warnings**.
- **Defect PROVEN on the real body (pre-fix, f866dac):**
  `cargo nextest run --run-ignored only -E test(f1_decomposes_real_
  gzip_openai_capture)` → **FAIL**:
  `F1 fell back to Layer-2 raw-body on a real gzip capture:
  [Component { label: "raw-body (structured parse failed; counted
  verbatim)", tokens: 342, pct: 100 }]` (src/compose.rs:707). Exact
  §0 symptom, through the real `record_request`→`compose` path.
- **Existing suite green (commit-gate honored):** `just test` →
  `94 tests run: 94 passed, 1 skipped` + doctests ok. The new test is
  `#[ignore]`d (reason string documents why) so the gate is NOT
  bypassed; the pre-fix failure is demonstrated above, un-ignored in
  step C. No `--no-verify`, no `RUSTCC_GATES=off`.

---

## F1-FIX3 — step C (fix at the F0 capture boundary; rust-cc loop, real e2e verified)

- **Dep (rust-cc deps discipline):** context7 was UNAVAILABLE (HTTP
  403); per the project oracle ("the compiler is the oracle") the
  `flate2` API was verified by the compiler-truth loop instead of
  docs — recorded as an honest substitution, not a silent skip.
  Added `flate2 = { version = "1.0.35", default-features = false,
  features = ["rust_backend"] }` — pure-Rust (miniz_oxide), no system
  zlib ⇒ single static binary preserved; `cargo machete` (step D)
  confirms it is used.
- **Fix (root cause, D-009):** `src/timeline.rs` — decode a gzip wire
  body (RFC1952 magic `1f 8b`, header-independent so saved sessions
  stay decodable) on the original `&[u8]` BEFORE `from_utf8_lossy`,
  bounded by `MAX_DECOMPRESSED` (64 MiB; over-limit/corrupt/truncated
  ⇒ raw kept ⇒ honest Layer-2, never a panic, never a partial body
  to the parser). NOT in `adapter::parse` (where D-007/D-008 wrongly
  patched). Pure deterministic; no judge; F1 stays pure measurement.
- **Compiler-truth loop (real, not claimed):** Edit→PostToolUse
  `rustcc` digest; `just check` first RED — `error[E0716]` temporary
  dropped while borrowed in a new test; fix = the compiler-suggested
  `let` binding (1 trivial lifetime case, < borrow-surgeon threshold).
  Re-run `just check` = GREEN (`clippy --all-targets --all-features
  -D warnings`, 0 warnings — also validates the flate2 API).
- **Suite:** `just test` = **99 tests run: 99 passed, 0 skipped** +
  doctests ok. Was 94; +1 (step-B test now UN-IGNORED and PASSING
  via Layer-1 on the verbatim real gzip body) +4 decode unit tests
  (passthrough / roundtrip / corrupt→raw / over-limit→raw). All F0/
  F2/F3 (snapshots, wire_capture, f2 byte-exact, f3 diff, save/open
  roundtrip) GREEN ⇒ byte-identical, zero-regression.
- **REAL end-to-end (green ≠ works — verified on live OpenRouter):**
  fixed binary on a real `Content-Encoding: gzip` client → F1 now
  `system 16 · tool-schemas 53 · history 31 · unused-loaded-tools`
  (was `raw-body … 0 findings`). `ctx open` of the saved gzip db =
  identical (persistence round-trips decoded). Real PLAIN run
  unchanged (real 200) ⇒ no regression. Spend ≈ $0.00005 total.

---

## F1-FIX3 — step D (no-regression + REAL harden + honest record)

- **Mutation (rust-cc harden, real baseline — the brief's anti-vacuous
  rule enforced):** cargo-mutants 27.0.0, `--in-diff` on the touched
  surface, `--test-tool nextest`.
  - Run 1: baseline `ok 20s build + 5s test` (REAL, not vacuous),
    28 mutants → **6 MISSED** (all my new code: `MAX_DECOMPRESSED`
    `*`→`+` ×2 unpinned; `decode_with_limit` `len()<2 || …` `<`/`||`
    ×4). NOT accepted as a pass — per the brief, missed mutants on the
    touched surface ⇒ fix, don't ship.
  - Killed properly: guard rewritten to `matches!(body.get(..2),
    Some([0x1f,0x8b]))` (removes the index/`<`/`||` operators *by
    construction* — also OOB-safe); added a `black_box`
    `MAX_DECOMPRESSED` ceiling-pin test (same pattern the repo already
    uses for `proxy::MAX_BODY`) + a tiny-input pass-through test.
  - Run 2: baseline `ok 18s build + 6s test` (REAL), 22 mutants →
    **22 caught, 0 MISSED**. Genuine green.
- **No-regression:** `just test` **101/101, 0 skipped** + doctests
  (was 99; +2 mutant-killing tests). F0/F2/F3 (snapshots, wire_capture,
  f2 byte-exact, f3 diff, save/open roundtrip) all green ⇒
  byte-identical. `just check` clippy `-D warnings` = 0.
- **Harden:** `cargo deny check` = advisories/bans/licenses/sources ok;
  `cargo machete` = no unused deps (flate2 used). `cargo semver-checks`
  noise (compares vs an unrelated crates.io crate also named `ctx`) —
  guarded by the justfile `|| true`, not a real finding (publish=false,
  name provisional per PROJECT.md §11).
- **/rust-review:** skill not invocable in-thread; substituted by a
  tool-grounded invariant self-review vs brief §4 (no unsafe/unwrap/
  expect in prod; total/panic-free; pure deterministic; F1 pure
  measurement; F0/F2/F3 byte-identical) → SHIP, 0 high. Deliberately
  NOT delegated to a subagent (D-008 incident: review subagent hit a
  usage cap, left a stray file, corrupted the mutation baseline).
  Recorded as a substitution, NOT an independent SHIP.
- **Honest status:** REAL fix at the F0 capture boundary; bounded claim
  per §1 recorded in D-009 Resolution (gzip proven on a verbatim real
  capture + a fresh live OpenRouter run; deflate/zstd/br = tracked
  honest-Layer-2 follow-up, not a silent gap; pre-fix saved sessions
  unrecoverable). No false green; every gate tool-verified on a real
  baseline.

---

## D-009 follow-up — multi-codec decode (zstd/brotli/zlib), user-requested defensive extension

- **Web research first (user asked):** request-body `Content-Encoding`
  is uncommon (mostly a *response* mechanism); gzip-on-request is the
  only one seen real, zstd/br/deflate is defensive. Pure-Rust decoders
  chosen ⇒ single static binary preserved: flate2 `ZlibDecoder`
  (already a dep), `ruzstd` 0.8.3, `brotli-decompressor` 5.0.0.
- **Compiler-as-oracle (context7 still 403; APIs verified by the
  loop + vendored crate source, not memory):** `just check` caught
  E0106 (decode_with_limit lifetime — output borrows `body` only, not
  `headers`; named `'b`), E0433 (`ruzstd::StreamingDecoder` wrong path
  → resolved to `ruzstd::decoding::StreamingDecoder::new(READ) ->
  Result` from the vendored 0.8.3 source `decoding/mod.rs`), and a
  clippy `elidable_lifetime_names` on `bounded`. Each fixed per the
  compiler's own guidance; re-run GREEN.
- **Design:** header-primary (`Content-Encoding`: gzip/x-gzip,
  deflate→zlib, zstd, br — the ONLY signal for brotli/raw-deflate,
  present on the live path) + magic-secondary (gzip 1f8b, zstd
  28b52ffd, zlib CM=8 & %31 — survives the unheadered post-hoc path).
  One shared `bounded()` reader enforces the SAME MAX_DECOMPRESSED
  reject-wholesale bound across every codec. Single-token only;
  chained/identity/unknown/fallible-init ⇒ raw (honest Layer-2).
  Total, panic-free. Clean JSON matches no codec ⇒ Cow::Borrowed ⇒
  F0/F2/F3 byte-identical.
- **Suite:** `just check` clippy -D warnings 0; `just test` **105/105,
  0 skipped** (+ per-codec round-trip, header-only brotli, magic_codec
  /header_codec pins, corrupt/tiny/over-limit; step-B real-gzip still
  PASS; F0/F2/F3 green). Fixtures `sample_{zstd,brotli}.bin` +
  `sample_payload.json` are REAL python-zstandard/brotli artifacts.
- **REAL end-to-end (green ≠ works):** real httpx client sending
  `Content-Encoding: zstd|br|deflate` through `ctx run` → F1 decomposes
  (`system·tool-schemas·history` + unused-tools indictment), NOT
  Layer-2; `ctx open` round-trips each. $0 (natural /v1 path 404s
  upstream; request captured+decoded before forward).

---

## D-009 follow-up — multi-codec HARDEN (0 missed on a real baseline)

- cargo-mutants `--in-diff` on the full multi-codec timeline.rs surface:
  - Run 1: baseline `ok 22s build + 5s test` (REAL), 46 mutants →
    **1 MISSED** `timeline.rs:80 replace | with ^` in `magic_codec`.
    Analyzed, NOT papered over: it is a provably EQUIVALENT mutant
    (`(b0<<8)|b1` ≡ `(b0<<8)^b1` because `b0<<8` zeroes the low byte ⇒
    disjoint bits ⇒ no input distinguishes them; no test can kill it).
    Per the step-D technique, eliminated BY CONSTRUCTION:
    `u16::from_be_bytes([*b0,*b1])` (no shift/or operator left to flip).
  - Run 2: baseline `ok 21s build + 5s test` (REAL), 43 mutants →
    **41 caught, 2 unviable, 0 MISSED**. `just check`/`just test`
    105/105 green after the change (`magic_codec` test still passes —
    `0x789c % 31 == 0` unchanged).
- `cargo deny` advisories/bans/licenses/sources ok; `cargo machete`
  no unused deps (ruzstd + brotli-decompressor used). Pure-Rust ⇒
  single static binary intact.
- Honest status: REAL fix; equivalent-mutant handled honestly (not
  accepted, not faked, removed structurally); bounded claim + recorded
  limits in D-009 follow-up. No false green.

---

## C1 / D-010 — cache-prefix-break indictment (new pure-measurement F1 rule)

- TDD: failing compose.rs test FIRST (`cache_prefix_break_fires_only_
  on_early_break_with_large_shared_suffix`) — proved RED (105 others
  green) before any impl.
- Compiler-truth loop: `just check` caught **E0689** (ambiguous numeric
  `n` in `common_suffix_len`) ⇒ fixed per the compiler's own guidance
  (`let mut n: usize = 0`). Re-run `just check` clippy `-D warnings` 0.
- `just test` **106/106, 0 skipped** + doctests — all F0/F1/F2/F3 +
  decode + step-B green ⇒ purely additive, zero regression.
- REAL e2e (green ≠ works): a real httpx 2-turn run through `ctx run`
  — MODE=break (volatile session-id prepended to the system prompt) ⇒
  `cache-prefix-break wasted=796, ~21 of ~817 tok shared as prefix`;
  MODE=healthy (identical stable prefix) ⇒ rule correctly SILENT (only
  the expected preamble-repay/repeated-block fire). $0 (natural /v1
  path 404s; requests captured before forward — C1 is request-only).
- Pure measurement only: byte prefix/suffix (char-boundary safe) +
  tokenizer sums + integer compares; per provider+model; no prediction
  of provider cache behaviour (evalint stays KILLED). Harden/mutants
  in the next commit.

---

## C1 / D-010 — HARDEN (0 missed on a real baseline) + honest record

- cargo-mutants `--in-diff` on the C1 surface:
  - Run 1: baseline `ok 28s build + 7s test` (REAL), 37 mutants →
    **8 MISSED** in `indict_cache_prefix_break` (`||`→`&&`; `<`→`==`/
    `<=` boundary; compound `&&`; `<` worst-tracking). NOT accepted.
    Fixed BY CONSTRUCTION + exact pins: decision extracted to a pure
    `cache_break_wasted(prefix,suffix,total)` with sequential guards
    and a deterministic exact-boundary unit table (the approx tokenizer
    cannot hit those boundaries via compose()); provider+model gate
    collapsed to one `(provider,model)`-key match (no `||`); worst pair
    via std `min_by_key` (no hand `<`).
  - Run 2: baseline `ok 27s build + 7s test` (REAL), 33 mutants →
    **30 caught, 3 unviable, 0 MISSED**.
- `just check` 0; `just test` **109/109**; `cargo deny`/`machete` ok
  (C1 added no deps). Behavior preserved across the refactor (the C1
  fires/silent e2e + pin tests stay green).
- PROJECT.md §6 indictment list updated (versioned-ruleset seam);
  D-010 records the rule, the bounded honest claim, the honest limits
  (byte-prefix identity ≠ a provider-cache guarantee; ±N% tokenizer;
  shared-suffix gate is a deliberate true-positive bias / honest
  false-negative), and that evalint stays KILLED. No false green.

---

## C2 / D-011 — component-drift indictment (new pure-measurement F1 rule)

- **TDD red-first (COMPILER-TRUTH Law 1/11).** Failing compose.rs
  behavioral test written FIRST. Proven RED on worktree HEAD `0a07bd4`:
  `cargo nextest run compose::` → `FAIL … component_drift_fires_only_
  when_a_same_named_component_mutates`, panic `a mutated system block
  across steps MUST be indicted`, while the other **16 compose tests
  PASS** and the full 109 suite stays green (only this new test red).
  `#[ignore]`-with-reason at test commit `719cc81` (commit-gate +
  suite stay green: clippy 0, 109/109 1-skipped); un-ignored in impl
  commit `e87c0ff`.
- **Compiler-truth loop (Law 1/3/4/5).** PostToolUse `rustcc gate` =
  the signal after every `.rs` edit. Red-test edit → `cargo clippy
  --all-targets -- -D warnings` GREEN (the test only uses public
  `compose`/`has_code`, so it compiles and fails at *runtime* — the
  clean TDD red). Impl edit (`drift_delta` + `indict_component_drift`
  + `indict()` wiring + un-ignore + boundary table) → `just check`
  clippy `-D warnings` **0** first pass (no error class to fix — the
  helper was isolated by construction). No hand-rolled cargo; the loop
  was the oracle throughout. No borrow/lifetime/trait/move class arose
  (read-only walk over `&Timeline`, no `.clone()`-to-silence, Law 10).
- **`just test` 111/111, 0 skipped** + doctests (was 109; +2 = the
  un-ignored behavioral test + `component_drift_decision_exact_
  boundaries`). Tests ASSERT real values (Law 11): names the drifted
  component, the step index, a non-zero token delta, and pins the
  renamed-tool=remove+add / single-step / stable-is-silent negatives.
- **`just harden`.** `cargo deny` advisories/bans/licenses/sources
  **ok**; `cargo machete` **no unused deps** — C2 added ZERO new deps
  (stdlib `BTreeMap` + existing tokenizer). `cargo semver-checks` flags
  the pre-existing crates.io `ctx` NAME COLLISION (PROJECT.md §11
  pre-publish rename gate) — non-fatal in the justfile (`|| true`),
  unrelated to C2.
- **Mutation-hardening — 2 independent REAL non-vacuous baselines
  (Law 11).** `cargo mutants --in-diff /tmp/c2.diff --test-tool
  nextest --timeout 120`:
  - Run 1: baseline `ok 71s build + 29s test` (REAL — not vacuous,
    not timed-out), 10 mutants → **8 caught, 2 unviable, 0 MISSED**.
  - Run 2: baseline `ok 41s build + 15s test` (REAL), 10 mutants →
    **8 caught, 2 unviable, 0 MISSED** (stable across passes).
  - 0 missed on pass 1 **by construction**: the decision was isolated
    in the pure `drift_delta(prev,cur)` helper (ONE `==` + ONE
    saturating abs-diff) with an exact-boundary unit table FROM THE
    START (the D-010 technique applied preemptively) — so unlike D-010
    (8 missed → restructure) no 2-pass restructuring was needed and
    there is no equivalent mutant to eliminate. The 2 unviable are
    non-compiling `Some(Default::default())` / `vec![Default::
    default()]` substitutions (`Indictment`/`Vec` have no `Default`
    there) — correctly unviable, NOT equivalent-and-missed.
- **Real e2e (green ≠ works).** `cargo build` → `target/debug/ctx`.
  A real 2-turn python `httpx` client via `ctx run --save` (dummy
  `Authorization: Bearer test`, natural `/v1/chat/completions`;
  upstream 404/conn-refused IGNORED — F0 captures the request BEFORE
  forward; $0, no real key anywhere):
  - **MODE=drift** (turn 2 mutates the system prompt, same
    provider+model) ⇒ FIRES: `waste component-drift wasted_tokens=1
    1 same-named component(s) mutated mid-session: system@step 1
    (~1 tok changed; a renamed tool reads as remove+add, not drift)`.
    Round-trips post-hoc via `ctx open /tmp/c2_drift.db` (same line).
  - **MODE=stable** (byte-identical system on turn 2) ⇒ correctly
    SILENT: only `preamble-repay`/`unused-loaded-tools` (C2 is
    preamble-repay's OPPOSITE). Also silent post-hoc via `ctx open`.
- **Pure measurement / evalint KILLED.** Only `==`/`!=`, `max/min`,
  `saturating_sub`, the existing labeled ±N% tokenizer, `sat_sum`. No
  judge, no prediction, no "model will forget X". No `Assembled` shape
  change (reads only the existing `system` + named `tools`). `/rust-
  review` not invocable in-thread ⇒ in-thread tool-grounded self-
  review (pure / total / panic-free / deterministic `BTreeMap` /
  no-`Assembled`-change / renamed=remove+add), deliberately NOT
  subagent-delegated (D-008 incident). Substitution, not an
  independent SHIP. No false green. Honest limit recorded in D-011:
  tool drift is at `schema_tokens` granularity (a same-token-count
  schema mutation is a deliberate honest false-negative — `Assembled`
  carries no per-tool raw bytes and changing it is the goal HALT
  condition); `system` drift is byte-exact.

---

## (relocated) C6/D-013 — was mis-filed in docs/RUSTCC-USAGE.md during parallel dev

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

---


---

## C3 / D-012 — context-window headroom & growth-rate slope (new pure-measurement signal)

- TDD red-first: the `c3_*` compose tests were written FIRST and proven
  to **fail to compile on the worktree HEAD** (`error[E0609]: no field
  'headroom' on type 'Composition'`, ×7) with the rest of the suite
  green (109 nextest + 91 lib) — recorded before any impl.
- New module `src/window.rs`: a static offline context-window registry
  (PROJECT.md §8 seam), pure data + a pure longest-substring lookup,
  honest `WINDOW_LABEL` (the C3 analogue of the tokenizer ±N% label).
  Every table entry exact-value pinned; unknown id ⇒ `None` (no guess).
- Compiler-truth loop (`just check`, clippy `-D warnings` incl.
  pedantic) — the PostToolUse `rustcc gate` ran `cargo check` after
  each `.rs` edit; the digest named, fixed root-cause-first one class
  at a time: `too_many_lines` (extracted `render::headroom_tty`),
  `cast u128→u32 may truncate` + `useless_conversion` (replaced a lossy
  test cast with the exact `pct()` equality). Re-checked green.
- `just test` after impl: **129 nextest + 100 lib** + doctests, 0
  skipped — purely additive, zero regression (all F0/F1/F2/F3 + decode
  + C1 stay green; the only existing-snapshot change is the additive
  `"headroom": null` on the unknown-model `f1_fixture`, manually
  applied — `steps` stays top-level, D-005 intact).
- REAL e2e (green ≠ works): a real python-httpx **multi-turn growing**
  conversation through `ctx run --save /tmp/c3.db` (DUMMY
  `Authorization: Bearer test`; natural `/v1/chat/completions` 404s
  upstream; request captured BEFORE forward; $0; no real key).
  - `ctx open` headline: `window gpt-4o 561/128000 tok 0% slope 146
    tok/turn over 4 turn(s) (...approximate...)` — fraction + slope,
    **no projection**.
  - `ctx --deep open`: adds exactly `window-projection at the observed
    mean rate (~146 tok/turn over 4 turn(s)), ~872 more turn(s) before
    the 128000-tok window is reached (neutral arithmetic projection,
    not a prediction)` — neutral, never "will overflow/truncate".
  - A single-turn known-model session AND an unknown-model
    (`llama-3-70b`) session each emit **zero** window claims even with
    `--deep` (both gates honest & independent).
  - `--json` keeps `steps` top-level; `headroom` integer-only
    (`used_pct`/`slope`), `projection` null without `--deep`.
- cargo-mutants `--in-diff` on the C3 surface, REAL non-vacuous
  baselines:
  - Run 1: baseline `ok 25s build + 8s test`, 28 mutants → **1
    MISSED** (`replace && with || in headroom` — the `provider &&
    model` series filter; single-namespace fixtures made `&&`≡`||`).
    NOT accepted. Eliminated **BY CONSTRUCTION**: the namespace match
    is now a single `(provider, model)` **tuple equality** via the
    pure `step_namespace` helper (no `&&` operator remains to widen
    into a namespace-crossing `||`) + a discriminating mixed-namespace
    fixture (foreign `gpt-4o` step wedged between focus Anthropic
    turns; pins `turns == 2`) + exact `step_namespace` pins.
  - Run 2: baseline `ok 31s build + 14s test`, 29 mutants → **25
    caught, 4 unviable, 0 MISSED**.
- Pure measurement only: fraction = `floor(used*100/window)` (the
  `pct` discipline), slope = exact integer `(last−first)/(turns−1)` on
  the labeled ±N% series; the projection is pure division, `--deep`-
  only, neutrally worded. No prediction of fate (evalint stays
  KILLED). `cargo deny`/`machete` ok (no new deps; `window.rs` = pure
  data + std). `/rust-review`/`/rust-harden` not invocable in-thread →
  substituted by the tool-grounded compiler-truth loop + 2-pass real
  cargo-mutants; deliberately NOT subagent-delegated (D-008 incident).
  Recorded as a substitution, not an independent SHIP. No false green.

---

## C4 / D-014 — `param-drift` (sampling/decoding parameter drift; + an ADDITIVE `Assembled` field)

- **TDD red-first.** The `compose.rs` behavioral test
  `param_drift_fires_only_when_a_tracked_field_value_changes` was
  written FIRST and proven **RED on worktree HEAD `dc895aa`**: a
  `cargo nextest run` of just that test panicked at `src/compose.rs`
  ("a changed sampling field across same-(provider,model) turns MUST be
  indicted") while the full **135** suite stayed green (proven by a
  prior full `cargo nextest run` = `135 passed, 0 skipped`). An earlier
  draft that also referenced a not-yet-existing `param_change` helper
  was a *compile*-fail RED (`error[E0425]: cannot find function
  param_change`, ×5) — the boundary table was moved to the impl commit
  (it cannot be `#[ignore]`d), leaving a clean runtime-RED behavioral
  test that compiles ⇒ `#[ignore]`-with-reason keeps the commit-gate +
  suite green at the test commit `bb06a4c`; un-ignored at the impl
  commit `8661dd0` (the D-010/D-013 red-first discipline; C3 had to
  bundle test+impl because its RED was a compile-fail — C4 avoided that
  by structuring the RED as a runtime panic, the C2 pattern).
- **ADDITIVE `Assembled` change (the wave-1 HALT condition, lifted for
  C4 additively only).** `adapter.rs` gains
  `Assembled.sampling: Vec<(String,String)>` appended after `tools` +
  `pub const SAMPLING_FIELDS` + a pure `sampling_of(&Value)` (present-
  only; `null` ≡ absent; one `for` over the shared slice — no `||`
  chain). Code-read verified `Assembled` is constructed in only the two
  `adapter::parse` arms ⇒ contained; `cargo check --all-targets` after
  the field add = **0 errors** (no other literal construction site).
- **Compiler-truth loop.** The PostToolUse `rustcc gate` ran `cargo
  check`/clippy after each `.rs` edit. The digest named ONE root-cause
  class: clippy `doc_markdown` "item in documentation is missing
  backticks" — an un-backticked `OpenAI` in the new `SAMPLING_FIELDS`
  doc comment (`src/adapter.rs:62`). Fixed root-cause-first (backticked
  the `OpenAI` reference), re-ran `just check` (clippy `-D warnings`
  incl. pedantic) = **0 errors / 0 warnings**. No hand-rolled cargo;
  commit-gate never bypassed (no `--no-verify`, no `RUSTCC_GATES=off`).
- **`just test` after impl: 138 nextest + doctests, 0 skipped** — 135
  baseline **+3** (the un-ignored behavioral test + the `param_change`
  exact-boundary table + the `SAMPLING_FIELDS` `black_box` pin). Purely
  additive, zero regression: all F0/F1/F2/F3 + decode + C1/C2/C3/C6
  green. The ONLY existing-snapshot change is the additive
  `"sampling": []` on the param-less `fixture` in
  `timeline_json_contract.snap` (the `Assembled` serializer); `cargo
  insta` is not installed, so the `.snap.new` was diffed
  (`diff` showed EXACTLY two `+ "sampling": []` lines, every other byte
  identical, `steps` top-level) and applied manually — the C3
  manually-applied `"headroom": null` precedent; D-005 intact. `grep
  -l param-drift tests/snapshots` is **empty** (C4 correctly SILENT on
  every existing fixture).
- **`cargo deny`/`machete` ok — NO new deps** (`serde_json::Value` +
  std `BTreeMap` only).
- **cargo-mutants `--in-diff` on the C4 diff (`dc895aa..HEAD`, src
  only), TWO independent REAL non-vacuous baselines:**
  - Pass 1 — `Unmutated baseline ok 22s build + 6s test`, 23 mutants →
    **20 caught, 3 unviable, 0 MISSED**.
  - Pass 2 — `Unmutated baseline ok 18s build + 6s test`, 23 mutants →
    **20 caught, 3 unviable, 0 MISSED** (stable).
  - 0-missed **by construction** (the proactive D-010/D-011/D-013
    technique, no 2-pass restructuring): the per-pair decision isolated
    in pure `param_change` with an exact-boundary unit table; the
    namespace a single `step_namespace` `(provider,model)` tuple
    equality (no `&&`/`||` to widen); `SAMPLING_FIELDS` `black_box`-
    pinned. The 3 unviable are the non-compiling `Default::default()`
    return arms (genuinely unviable, not equivalent-and-missed).
- **REAL e2e (green ≠ works).** `cargo build` green; two real
  python-httpx clients through `./target/debug/ctx run --save` (DUMMY
  `Authorization: Bearer test`; natural `/v1/chat/completions`;
  upstream connection-refused/404 — request captured BEFORE forward;
  $0; no real key).
  - MODE=drift (turn 2 `temperature` 0.2→0.9, same `gpt-4o`) ⇒
    **FIRES**: `waste param-drift wasted_tokens=0 1 sampling/decoding
    field(s) changed mid-session (same provider+model):
    temperature@step 1 (0.2->0.9) (a reported determinism-surface
    fact, not a non-determinism claim)`; round-trips post-hoc via
    `ctx open` AND `ctx --json open`.
  - MODE=stable (sampling params byte-stable, only the user message
    changes) ⇒ correctly **SILENT** (0 `param-drift`; 0 findings).
  - `ctx --json open` (drift): top-level keys exactly
    `['composition','steps']`, `steps` array top-level,
    `steps[0].assembled.sampling ==
    [["temperature","0.2"],["top_p","1"],["max_tokens","512"]]` →
    `steps[1]` `temperature` `"0.9"`, the `param-drift` indictment
    round-trips with `wasted_tokens:0` (D-005 intact, additive only).
- **Pure measurement only.** Value (in)equality (canonical JSON value
  strings) + named field + step index; `wasted_tokens` hard-0 (a param
  change re-bills no prompt tokens — never a fabricated cost). The
  `detail` ends "(a reported determinism-surface fact, not a
  non-determinism claim)" — the `agentlock` *attribution* is EXCLUDED
  by construction (evalint stays KILLED; graph stays REFUTED; no
  kill-zone, no lockfile, no intervention).
- **`/rust-review`/`/rust-harden` NOT invocable in-thread** (not in
  this environment's skill registry) → substituted by an in-thread
  tool-grounded self-review (`grep`/`cargo` verified: no unsafe / no
  async / no unwrap / no clone-to-silence / no `&&`|`||` in the
  namespace gate / `wasted_tokens` hard-0 / detail is a reported FACT /
  SILENT on every existing snapshot) + the 2-pass real cargo-mutants
  evidence above; deliberately NOT subagent-delegated (the D-008
  review-subagent integrity incident). Recorded as a substitution, not
  an independent SHIP. No false green.

---

## C4 / D-014 — integration verification (worktree-isolation breach, independently re-verified)

Process anomaly recorded honestly (not silently): the C4 agent's
worktree isolation did NOT hold — its 4 commits (bb06a4c/8661dd0/
8fe95de/17ceff9) landed directly on `main`, no `worktree-agent-*`
branch (the 2nd isolation breach this session; C3 earlier leaked
uncommitted files). This bypassed the planned independent integration
gate, so it was re-verified from scratch on `main@17ceff9`, NOT trusted
from the agent self-report:

- `git merge-base --is-ancestor dc895aa HEAD` ⇒ wave-1 merge is an
  intact ancestor (no history loss); main = dc895aa + exactly C4's 4
  linear commits; tree clean.
- adapter.rs change verified ADDITIVE-only (zero `-` deletions on any
  existing `pub`/struct line) — `Assembled.sampling` is a new field,
  the wave-1 HALT condition (no shape break) holds; snapshot delta is
  exactly the additive `"sampling": []` (+ benign insta metadata),
  D-005 `--json` contract intact.
- `just test` 138/138 + doctests; `just check` clippy `-D warnings` 0;
  `cargo deny`/`machete` ok (no new deps).
- cargo-mutants `--in-diff` on C4's delta: REAL baseline
  `ok 21s build + 6s test` (non-vacuous), 23 mutants → 20 caught,
  3 unviable, **0 missed**.
- Purity confirmed by read: pure `param_change`/`step_namespace`,
  `wasted_tokens: 0` hard-zero, the agentlock non-determinism
  attribution EXCLUDED in-code, evalint stays KILLED.

Conclusion: C4 is correct and honestly green ON MAIN despite the
isolation breach — accepted. `.gitignore` now excludes
`/.claude/worktrees/` (added by C4; legitimate hygiene).
