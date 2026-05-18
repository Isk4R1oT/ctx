# ctx — decisions log (reconciliation of record)

> Project-scoped, authoritative. `docs/PROJECT.md` is the CANONICAL spec; this file
> records reconciliations between PROJECT.md and the line-level docs so that **no
> contradiction remains of record** within the `ctx` project. Append-only; newest
> decision wins and says so. Do not relitigate `../../docs/08-decision-log.md`.

---

## D-001 — Canonical CLI surface = wire-proxy; static-scan verbs are STALE (2026-05-17)

### Status: LOCKED. Resolves the gate-R doc contradiction. Outcome: COHERENT (no HALT).

### The contradiction

| Source | Model it implies | Verbs / persistence |
|---|---|---|
| `docs/PROJECT.md` §4/§5/§6 (**CANONICAL**) | transparent **local reverse-proxy** at the LLM-API boundary; X-rays the *actually assembled wire prompt* | `ctx run -- <cmd>` (sets `*_BASE_URL`); ephemeral by default; **opt-in** local SQLite |
| `docs/RESEARCH.md` lines 75/90/94 | same wire-proxy model | explicitly **rejects** "static repo scan — can't see the *actually assembled* prompt"; "always-on persistent store — instant death" |
| `../../docs/09-roadmap.md` §1 | static **reconstruction** "for a repo/agent (CLAUDE.md + imports + skills + MCP schemas + memory)" | `ctx scan` / `ctx diff <gitref>` / `ctx lint`; "logs each measurement to SQLite" (DB-by-default) |
| `../../docs/00-overview.md` "Anchor" | "reconstruct the actually-assembled agent context" | same static-scan framing; "SQLite log" by default |
| `../../docs/11-cli-design-system.md` §8 | worked **rendering** example | example strings `ctx scan`, `./ctx.db`, `ctx init`, `reconstruct`, `ctx explain --turn 2` |

### Resolution (the goal's directive: keep the wire-proxy MECHANISM canonical; normalize verbs)

**Mechanism — CANONICAL, not negotiable.** `ctx` is a transparent local reverse-proxy at
the LLM-API boundary. It captures the *actually assembled wire prompt* of the child
process. Static repository reconstruction (reading CLAUDE.md/imports/skills/MCP
schemas/memory off disk) is **REFUTED and never built** — PROJECT.md §3 ("the API
boundary can't faithfully reconstruct a framework graph"), §9 (graph/skeleton IDE is a
non-goal), §10 (hypothesis 1 graph half = REFUTED), RESEARCH.md line 90 (static scan is
the *rejected* alternative), and the binding goal invariant "graph/skeleton
reconstruction NEVER built (REFUTED)".

**Canonical CLI verb namespace (v1):**

- `ctx run -- <command...>` — **PRIMARY entrypoint.** Runs the child agent process behind
  the transparent proxy (sets provider `*_BASE_URL`), captures the wire timeline. Default
  human output = the F1 composition + waste headline at the final step. *(F0 delivers the
  capture + timeline data model + the tokenizer label; F1/F2/F3 are goal 2/2.)*
- Global flags (PROJECT.md §4 + doc 11 §1.3): `--json` (everywhere — CI citizen),
  `--deep` (drill-down), `--color <auto|always|never>`, plain auto on non-TTY/`NO_COLOR`,
  `--save <FILE>` (opt-in SQLite; **ephemeral by default**), `--open <FILE>` (inspect a
  previously saved session post-hoc).
- v1 post-hoc views (named here for surface coherence; built in goal 2/2): **F1** = default
  headline; **F2** = verbatim assembled-context pager (TUI + one-shot) on a selected step;
  **F3** = `ctx diff <stepA> <stepB>` — per-step / cross-run **wire** diff.

**Verbs REJECTED and why (the stale static-scan surface):**

| Stale verb (09 §1 / 00 / 11 §8) | Disposition | Authority |
|---|---|---|
| `ctx scan` (static repo reconstruction) | **REJECTED.** Its intent (composition + waste number) is the *default output of `ctx run --`* over the captured wire timeline. | PROJECT.md §3/§9/§10; RESEARCH.md L90; goal invariant |
| `ctx lint` (standalone budget/anti-pattern linter) | **REJECTED as a verb.** The indictment is folded into the F1 headline ("pure measurement + indictment", PROJECT.md §6 F1). A standalone prompt/budget linter is an *explicit kill* (09 "Explicit kills" — "too small to be a product"). The indictment *ruleset* survives only as an extensibility seam (PROJECT.md §8), never a top-level command. | PROJECT.md §6/§8; 09 kills |
| `ctx init` + `./ctx.db` (default project DB) | **REJECTED.** DB-as-product is the kill-zone. Persistence is opt-in `--save` only. The doc-11 §8 error string ``run `ctx init` first`` is therefore **stale** and must NOT be implemented; the canonical "no captured session" error references `ctx run --` / `--open`. | PROJECT.md §3 discipline rule, §9; RESEARCH.md L94 |
| `ctx diff <gitref>` (git-ref static drift) | **REJECTED.** Replaced by per-step / cross-run **wire** diff (F3 / v1.x), not a git-ref repo scan. | PROJECT.md §6 F3 / v1.x |
| `ctx explain --turn N` (doc-11 §8 "Next" line) | **REJECTED as a verb.** Canonical drill-down is `--deep` + step selection on the timeline. | PROJECT.md §4/§5 |

**Doc 11 binding status (clarified, not weakened):** every *aesthetic* rule in
`11-cli-design-system.md` remains **MANDATORY and mechanism-agnostic** — §1 palette &
terracotta `#d97757`, §1.3 ColorMode precedence (`--color` > `NO_COLOR` >
`CLICOLOR_FORCE` > `CLICOLOR` > per-stream `IsTerminal` > `COLORTERM`), §2 layout/box,
§3 `⏺`/`⎿`/`·` glyph grammar, §4 spinner (frames `· ✢ ✳ ✶ ✻ ✽`, stderr, erased on
done), §5 diff/list/prompt, §6 tone (sentence case, no emoji, terse), §7 checklist.
Only §8's *worked-example verbs/strings* are stale; §8 illustrates **rendering**, not the
command surface. The canonical wire-proxy verbs above are rendered with the doc-11
grammar unchanged.

**Line-level docs (09 §1, 00 "Anchor") are SUPERSEDED, not edited.** They are
historical line-level roadmap text; `../../CLAUDE.md` states "everything before the
2026-05-16 pivot is superseded" and PROJECT.md is "the single detailed project doc".
Editing them is out of scope for this project and risks relitigating settled line-level
decisions (08). The contradiction is removed **of record** by this entry; within the
`ctx` project, PROJECT.md + RESEARCH.md + this file are internally consistent.

### Gate-R verdict: **COHERENT.** No HALT. The reconciliation is recorded; no
contradiction remains of record. F0 proceeds against the wire-proxy mechanism only.

---

## D-002 — rust-cc compiler-truth loop is the mandated build discipline (2026-05-17)

All Rust in this project is developed via the `rust@rust-cc` plugin loop
(`/Users/igor/Projects/rust-cc/COMPILER-TRUTH.md`, 12 laws): write → `rustcc gate`
(`cargo check`) → read the ranked digest → fix one root-cause class → re-check → repeat
until green; never commit over red (Law 5 commit-gate); no memorized crate APIs (Law 2 —
verify via `context7`/rust-analyzer/docs.rs); `/rust-review` + `/rust-harden` before any
commit of code. The PostToolUse digest is the source of truth; a "green" is only ever a
real `cargo`/`nextest` result, never a claim.

---

## D-003 — Open, escalated: "zero code today" vs "build F0 to cargo-green" (2026-05-17)

The binding goal text contains an **internal contradiction not resolvable without a
consequential one-way-door guess**, so it is escalated to the user (per the goal's own
"unresolved contradiction => HALT & report — no proceed/fabricate/weaken" and "no silent
pick"; and the global rule "uncertain + consequential → ask first"):

- Goal title: *"Zero code today (only docs + .claude/settings.json)."*
- Goal EXECUTE / F0 EXIT: *"a real agent run via `ctx run --` captures the verbatim
  assembled wire prompt for BOTH providers; timeline + opt-in-sqlite snapshot-tested;
  tokenizer prints its ±N% label; cargo check/clippy/nextest green via the loop;
  /rust-harden = go."*
- RUST-CC MANDATE: *"`/rust-new <name> --bin` (green)"* — scaffolding is code.

"Zero code today" cannot coexist with "F0 built to cargo-green + two-provider wire
capture + snapshot tests + `/rust-harden` go + `/rust-new` scaffold". Gate R (this
file) is satisfied **docs-only** under either reading and is done. The F0 scope-for-today
decision is put to the user before any `.rs` file is created.

### Resolution (2026-05-17, user decision via AskUserQuestion): **BUILD F0 NOW.**

"Zero code today" is treated as a **stale clause** carried from the prior design-only
project status (`../../CLAUDE.md`: "Status: design only. ZERO code written … Next phase
= implementation"). The binding intent for this session is the detailed F0 EXIT
criteria: execute R→F0 via the rust-cc compiler-truth loop to **real** `cargo`-green.
All other HARD INVARIANTS remain in force unchanged. No contradiction remains open.

---

## D-004 — F0 honest disclosed limit: response is buffered, not streamed (2026-05-17)

### Status: DISCLOSED (intellectual honesty is the signal — no overclaiming).

The F0 proxy captures the **request** (the assembled wire prompt) byte-for-byte — this
is the F0 EXIT criterion and is proven by `tests/wire_capture.rs` for both providers.
The **upstream response** is currently *buffered in full* before being returned to the
child, not streamed-through chunk-by-chunk. Consequences, stated plainly:

- A real agent using `stream=true` SSE will receive the full body at once rather than
  incrementally. Request capture (the F0 deliverable) is unaffected and exact.
- This is a deliberate F0 scope cut for correctness/convergence, **not** a hidden
  defect. SSE pass-through with a streaming tee is a tracked **v1.x** item
  (`docs/PROJECT.md` §6 v1.x / §8 renderer-agnostic seams), not an F0 claim.

Rationale: PROJECT.md §6 lists F1/F2/F3 as the views on F0; streaming fidelity is an
orthogonal transport concern. Claiming streaming now would be the kind of overclaim the
project explicitly rejects ("knowing what NOT to build, with proof, is the signal").

---

## D-005 — F1 is `ctx run`'s default headline WITHOUT regressing the F0 `--json` timeline contract (2026-05-17)

### Status: LOCKED. Goal-2/2 design reconciliation (no silent pick).

The fork: D-001 + PROJECT.md §6 say **F1 (composition + waste) is the default headline of
`ctx run`**. But F0's `tests/wire_capture.rs` pins `ctx run --json` to the raw timeline
(`tl["steps"][..]`), and the HARD INVARIANT forbids regressing R/F0.

Resolution (zero F0 regression, D-001 honored):
- **`ctx run` / `ctx open` human output → the F1 headline** (composition + waste,
  pure-measurement; `--deep` for contestable detail). F0's timeline *summary* was an
  explicit placeholder; F1 taking the headline is the roadmap, not a regression — the F0
  *mechanism* (wire capture, step-timeline model, opt-in SQLite, ±N% tokenizer, the
  `steps[]` data) is unchanged and still fully tested.
- **`--json` emits `RunReport { #[serde(flatten)] timeline, composition }`** → JSON is
  `{"steps":[...], "composition":{...}}`. `tl["steps"]` stays top-level, so every F0
  `wire_capture`/snapshot assertion remains green **untouched**; F1's structured data is
  additive at `tl["composition"]`.
- `render::summary` (the F0 timeline plain render) is **kept intact** (its snapshot stays
  valid); F1 is a new `render` path the CLI default now calls.
- F2 = `ctx view <step>` (verbatim pager, one-shot + TUI). F3 = `ctx diff <a> <b>`
  (per-step wire diff). Both are **new subcommands** — zero F0 surface change.

This keeps R/F0 bit-for-bit intact while delivering F1/F2/F3 exactly per D-001/§6.

---

## D-006 — Honest follow-up: `timeline.rs` token-sum overflow twin (2026-05-17)

Independent review (F1 harden) found `compose.rs` summed tokens with raw `.sum()`/`+`;
fixed to saturating end-to-end (F1 in scope). The **same pattern pre-exists in
`src/timeline.rs` `record_request` (`prompt_tokens`)** — F0 code, frozen and already
harden-cleared. Per the HARD INVARIANT "do NOT regress R/F0 / do not re-scope", F0 is
left untouched here; this is **recorded, not hidden** (intellectual-honesty principle):
on a pathological unbounded `ctx open` session the F0 `prompt_tokens` could wrap in
release / panic in debug exactly like the F1 site did. Tracked as a follow-up to apply
the same `sat_sum` discipline to `timeline.rs` in a dedicated F0-maintenance change
(its own rust-cc loop + re-harden), not folded silently into F1.

---

## D-007 — F1 dual-provider parse leniency; standing "both shapes" rule (2026-05-17)

### Status: LOCKED. Closes the F1-FIX (F1-FIX-BRIEF). Does not relitigate D-001..D-006.

**Defect (TDD-proven, not assumed).** F1 composition+waste printed
`composition no captured prompt · 0 tokens · 0 findings` on an
OpenAI-compatible body **despite a full F0 capture** (provider correctly
`open_ai_compat`; `view`/`diff`/`open --json` all showed the bytes).
Root cause: `AnthropicReq`/`OpenAiReq.messages|tools` used
`#[serde(default)]`, which substitutes for a **missing** key but **not**
an explicit `null`. Real OpenAI/agent clients emit `"tools": null` /
`"messages": null` on a no-tool turn ⇒ `serde_json::from_slice` `Err`
⇒ `adapter::parse` `Err` ⇒ `timeline::record_request` `.ok()` ⇒
`assembled = None` ⇒ `compose` finds no focus ⇒ F1 blind. F0/F2/F3 use
the verbatim `request.body`, so they were unaffected — F1 alone was
silently broken on half the declared v1 surface (PROJECT.md §4/§5:
Anthropic **+ OpenAI-compatible**).

**Why CI missed it.** Every F1 fixture (compose unit tests + snapshots)
was Anthropic-shape and used *clean* (omitted-key) bodies — the
explicit-`null` real-client shape was never exercised on **either**
provider.

**Fix.** `adapter.rs`: `null_or_missing_as_default` serde helper
(`Option::<T>::deserialize → unwrap_or_default`) applied with
`#[serde(default, deserialize_with = …)]` to the four optional Vec
fields of **both** provider request shapes. Pure parsing leniency:
missing/`null` → empty; a present array is byte-identical; a
structurally malformed body still returns `Error::Adapter` (verified).
`compose.rs` is unchanged — it was already provider-agnostic over
`step.assembled`; F1 stays PURE MEASUREMENT.

**Brief-deviation, recorded (not silent).** F1-FIX-BRIEF §3-B said
"fix `compose.rs`". The empirically-proven root cause is `adapter.rs`.
Per the brief's own §1 ("on conflict, `PROJECT.md` is canonical") +
D-001 (the provider-adapter trait is the single wire→canonical
normalization layer) + §0 ("do NOT re-architect"), the minimal,
non-duplicating, least-regression fix is in `adapter.rs`. Fixing
`compose.rs` to re-parse would duplicate the adapter and violate D-001.

### Standing rule (binding going forward)

> **Every F1 test and fixture MUST exercise BOTH v1 provider wire
> shapes** (Anthropic Messages *and* OpenAI-compatible
> chat.completions), **including the real-world `null`/omitted optional
> variants** — never Anthropic-only, never clean-only. Enforced by
> `compose::tests::f1_must_not_be_blind_on_any_v1_provider_shape`
> (a table over both providers × shape variants); a new provider shape
> adds a row. This makes the "F1 blind on a provider shape" class a
> hard CI failure forever.

---

## D-008 — F1 real fix: defensive Value-walk parse; D-007 mechanism superseded (2026-05-17)

### Status: LOCKED. Supersedes the *mechanism* of D-007 (not its standing rule). Does not relitigate D-001..D-006.

**Why D-007 was insufficient.** D-007 made `OpenAiReq`/`AnthropicReq`
`messages`/`tools` tolerate explicit `null`. A **real OpenRouter run on
the D-007 HEAD still showed F1 `composition no captured prompt`** while
F0/F2/F3 (raw bytes) worked — the rigid `#[derive(Deserialize)]` structs
reject *many* real-client valid-JSON shapes, not just `null`. Chasing
field-by-field is whack-a-mole; two prior fixes were confidently wrong
because their TDD fixtures were author-invented, not real captured wire.

**Root cause (real).** `adapter::parse` deserialized into rigid typed
structs; ANY shape mismatch on a *valid-JSON* body → `serde_json`
`Err` → `timeline::record_request().ok()` swallows it → `assembled =
None` → `compose` no focus → F1 blind. F0/F2/F3 use the verbatim
`request.body`, so only F1 was hit.

**Fix.** (1) `adapter.rs`: delete all typed request structs +
`null_or_missing_as_default`; `parse` now `serde_json::from_slice::
<Value>` then a defensive walk (`messages_of/tools_of/role_of/
content_text`, reused `flatten_content/tool_tokens`). It **cannot Err
on any valid JSON**; every field read is missing/null/wrong-type
tolerant. Verified byte-identical `Assembled` for every well-formed
Anthropic+OpenAI body (F1 snapshots unchanged; rust-reviewer
field-mapped parity; only *malformed* inputs differ, improvement-only).
(2) `compose.rs` Layer-2: if nothing parsed structurally but a step
captured bytes, emit one `raw-body (structured parse failed; counted
verbatim)` component — **bytes captured ⇒ F1 never blind**. Pure
measurement (count + factual label). Class closed by construction.

**Grounding & honest caveat.** `ctx view` (F2) *verified* the real
OpenRouter body is valid JSON ⇒ the Value-walk provably yields an
`Assembled` for it. **Not** re-confirmed by a fresh live OpenRouter
hit (key revoked, correctly); the claim rests on that verified-F2 fact
+ a by-construction proof + the Layer-2 fallback — materially stronger
than the prior synthetic-fixture greens, but stated as such.

**Standing rule (binding, extends D-007).** D-007's "every F1
test/fixture exercises BOTH provider shapes" is RETAINED. Added: **a
TDD test proving a wire-parse defect MUST use a verbatim real `--save`
capture, never an author's approximation** — the two false greens this
session were exactly that failure.

**Pre-existing follow-up (recorded, NOT scope-crept, D-006-style).**
`tokenizer::count` on a multi-MB body (a crafted `ctx open` SQLite) is
pathologically slow (surfaced by a hostile probe; *old code had the
identical exposure*; F0 live capture is bounded by `MAX_BODY` 64 MiB).
Out of F1-FIX scope; tracked for a dedicated bounded-tokenization
change, not silently folded in.

**Process integrity incident (recorded honestly).** The
`runtime-soundness` review subagent hit the usage cap mid-run and, in
violation of its read-only contract, left an untracked
`tests/_zz_hostile_probe.rs` that broke the mutation baseline. Detected
via the *failed* mutants run (not self-report), signals harvested
(hostile-shape cases corroborate the fix; one assertion was a wrong
non-contract expectation — parity with old behavior is correct), file
removed, tree clean. Independent runtime-soundness verdict was NOT
obtained; substituted by tool-grounded checks (no unsafe/async; `pct`
`checked_div` div-by-zero-safe; `.get()`-only determinism; serde_json
`remaining_depth:128`) — recorded as a substitution, not claimed as an
independent SHIP.

## D-009 — F1 blindness root cause: capture-boundary lossy UTF-8 destroys compressed wire bodies; NOT a parser bug (2026-05-17)

### Status: LOCKED (diagnosis only — zero production code changed). Supersedes the *mechanism* of D-007 **and D-008** (their standing TDD/real-capture rules RETAINED). Does not relitigate D-001..D-006.

**D-008 was insufficient (third confirmed false green).** D-008 closed
the class "by construction" on the premise that `ctx view` (F2)
*verified the real OpenRouter body is valid JSON*. That premise was a
forbidden constructive proof on an *uncompressed* body; it never tested
a real compressed-request client. F1-FIX3-BRIEF §0 reproduced the
symptom; this entry pins the true mechanism on a **verbatim real `ctx
run --save` capture** (not an approximation).

**Root cause (evidence-verified, on real captures, HEAD `f866dac`).**
F1 goes blind — `compose` Layer-2 `raw-body (structured parse failed;
counted verbatim)`, 0 findings — for any real client that sends a
**compressed request body** (`Content-Encoding: gzip` demonstrated;
httpx / many agent stacks / corporate & OpenRouter proxies do this).
It is **NOT** in `adapter::parse` (where D-007 serde-null and D-008
Value-walk both wrongly patched — that code never sees real bytes).
The bug is the **F0 capture boundary**: `src/timeline.rs:73`
`String::from_utf8_lossy(body)` runs on the raw wire bytes *before*
provider-detect / parse / persist. A non-UTF-8 (compressed) body has
every non-UTF-8 byte rewritten to U+FFFD at capture, so (a)
`serde_json::from_slice` cannot parse it and (b) the original bytes
are **destroyed at capture and unrecoverable post-hoc** — the saved
BLOB is the mangled string (`src/store.rs:90`), and request headers
(the `Content-Encoding` signal) are **not persisted** at all
(`store.rs` step schema; `load()` replays with `&[]` headers).

**Evidence (real `ctx --save` captures, this session).**
- gzip capture stored request body begins `1F EF BF BD 08 00 …`;
  real gzip magic is `1F 8B 08 00` — the `8B` byte is rewritten to
  `EF BF BD` (UTF-8 for U+FFFD `REPLACEMENT CHARACTER`). Provider
  WAS detected (`open_ai_compat`, path `/v1/chat/completions`) ⇒ this
  is a parse/destruction failure (H2), **not** a detect failure (H1).
- plain capture stored body begins `7B 22 6D 6F` (`{"mo`) and Layer-1
  decomposes it correctly (system/tool-schemas/history + indictment).
- Real-capture matrix on `f866dac`: plain-200 ✅, `stream:true` ✅,
  `tools:null` ✅, 2-turn ✅ (4 indictments), **`Content-Encoding:
  gzip` ❌ Layer-2 / 0 findings** — single isolated variable.
- `ctx open gzip.db` reproduces it post-hoc (`assembled=None`),
  confirming the destruction is persisted, not transient.

**Scope correction (flagged, NOT silently scope-crept — D-006 style).**
F1-FIX3-BRIEF §0/§1 frame the defect as "Layer-1 cannot decompose a
real **valid-JSON** body" and mandate a **F1-only** fix. The evidence
refutes that framing: the captured bytes are genuinely **not valid
JSON** (destroyed gzip), so Layer-2 firing is *correct given the
destroyed input* — the real fault is upstream, at the **shared F0
capture/persistence path** (`timeline::record_request` +
`store.rs`), which `agentlock`/`guard` also build on (caliper
CLAUDE.md). A correct fix is therefore **not F1-only** and touches a
one-way-door design question (does ctx persist decoded bytes, or raw
bytes + the encoding?). Halted for that decision before any code —
per the brief’s own "record honest deviations, not silently" + the
standing rule that a wire-parse defect is pinned on a real capture.

**Recommended fix (proposed; not yet implemented).** At the capture
boundary, on the original `&[u8]` *before* the lossy String: if
`Content-Encoding` (live) or a magic-byte sniff (`1F 8B` gzip;
post-hoc, header-independent — required because headers are not
persisted) indicates compression, decompress; use the decompressed
bytes for detect/parse **and persist them** so `ctx open` round-trips.
Decompression failure ⇒ keep raw ⇒ legitimate Layer-2 (genuinely
opaque). F2/F3 on *clean* bodies stay byte-identical (UTF-8 lossy is
identity on valid JSON) ⇒ no existing-test regression; F2 on
compressed bodies improves (garbage → real JSON) rather than
regressing. To be confirmed against PROJECT.md "verbatim" semantics
with the user before step B/C.

### Resolution (implemented; user decision "make it all work" = the recommended option)

Done at the F0 capture boundary (NOT F1-only — the scope correction
above was surfaced and approved). `timeline::record_request` now
gzip-decodes the original `&[u8]` (RFC1952 magic `1f 8b`, slice-pattern
guard — no index/`<`/`||` to silently flip; header-independent so saved
sessions stay decodable) BEFORE `from_utf8_lossy`, bounded
`MAX_DECOMPRESSED` 64 MiB (corrupt/truncated/over-limit ⇒ raw kept ⇒
honest Layer-2; total, panic-free; pure deterministic — F1 stays pure
measurement). `flate2` pure-Rust backend ⇒ single static binary kept;
`cargo deny`/`machete` ok. Decoded bytes are persisted ⇒ `ctx open` of
a newly-saved compressed session round-trips. F2/F3 on *clean* bodies
are byte-identical (no `1f 8b` ⇒ `Cow::Borrowed`); F2 on compressed
captures improves (garbage → real JSON) — no existing-test regression
(101/101).

**Bounded honest claim (per F1-FIX3-BRIEF §1 — stated, not over).** F1
Layer-1 decomposes a verbatim real `Content-Encoding: gzip` OpenAI/
OpenRouter body, proven (a) by a committed real-wire-bytes fixture
test that FAILS on `f866dac` and PASSES post-fix via Layer-1 (not
Layer-2, not synthetic, not a constructive proof), AND (b) by a FRESH
LIVE OpenRouter run through the fixed binary (gzip decomposes; plain
unchanged; `ctx open` round-trips) — i.e. stronger than the §1 floor.
NOT claimed: non-gzip transport encodings. **Scoped limits (tracked,
not silent):** `Content-Encoding: deflate`/`zstd`/`br` are NOT decoded
(no real case demonstrated) ⇒ they remain honest Layer-2, a deliberate
follow-up, not a hidden gap. Pre-fix saved sessions captured by the old
mangling code are unrecoverable (their bytes were destroyed at write
time) — not a regression of new behavior.

**Gates (real, non-vacuous).** `just check` clippy `-D warnings` 0;
`just test` 101/101 + doctests; `cargo deny`/`machete` ok; cargo-mutants
`--in-diff` on the touched surface = **22 caught, 0 missed** on a REAL
green baseline (`Unmutated baseline ok 18s build + 6s test` — explicitly
not the vacuous/timed-out baseline the prior integrity incident
required rejecting). `/rust-review` skill not invocable in-thread →
substituted by a tool-grounded invariant self-review (SHIP, 0 high),
**deliberately NOT delegated to a subagent** (prior D-008 incident: a
review subagent hit a usage cap and left a stray file that corrupted
the mutation baseline) — recorded as a substitution, not an independent
SHIP.

### D-009 follow-up — multi-codec decode (zstd/brotli/zlib), user-requested defensive extension

The D-009 gzip fix was generalized at the same F0 capture boundary to
also decode `Content-Encoding: zstd | br | deflate(=zlib)`, on Igor's
explicit "decode the others too, just in case" request after web
research. Design: header-primary (`Content-Encoding`, the ONLY signal
for brotli/raw-deflate — they have no magic — present on the live
`ctx run` path) + magic-secondary (gzip `1f8b`, zstd `28b52ffd`, zlib
`CM=8` & the RFC1950 `%31` check — survives the header-less post-hoc
path). One shared bounded reader applies the SAME `MAX_DECOMPRESSED`
reject-wholesale bound to every codec; single token only; chained /
identity / unknown / fallible-init / corrupt / over-limit ⇒ raw kept
(honest Layer-2); total, panic-free. Pure-Rust deps (single static
binary preserved): `ruzstd` 0.8.3, `brotli-decompressor` 5.0.0, flate2
`ZlibDecoder`. Clean JSON matches no codec ⇒ `Cow::Borrowed` ⇒ F0/F2/F3
byte-identical (zero-regression unchanged).

**Honest real-world caveat (recorded, not buried).** Request-body
compression is *uncommon* — `Content-Encoding` is mostly a response
mechanism; gzip-on-request is the only case observed in the wild on a
real LLM client. zstd/br/deflate-on-request support is defensive, not
demonstrated-necessary.

**Bounded honest claim (extends §1).** F1 Layer-1 decomposes a real
`Content-Encoding: gzip | zstd | br | deflate` body — proven by
per-codec round-trip unit tests, real python-zstandard/brotli fixtures
(`tests/fixtures/sample_{zstd,brotli}.bin`), AND a fresh real
end-to-end run of each through `ctx run` (httpx client → F1 decomposes,
not Layer-2; `ctx open` round-trips). NOT claimed: chained encodings,
or a true raw-RFC1951-deflate body sent WITHOUT a `Content-Encoding`
header (no magic, no header ⇒ stays honest Layer-2) — a deliberate,
recorded limit, no real case exists.

**Gates (real, non-vacuous).** `just check` clippy `-D warnings` 0;
`just test` 105/105 + doctests; `cargo deny`/`machete` ok (both new
deps used, advisories/licenses/bans clean); cargo-mutants `--in-diff`
on the full multi-codec surface: run 1 = **1 missed** (`(b0<<8)|b1`
`|`→`^` — a provably *equivalent* mutant: `b0<<8` zeroes the low byte
so `|`≡`^`; no test can distinguish it) ⇒ NOT accepted, eliminated by
construction via `u16::from_be_bytes` (no shift/or operator remains);
run 2 = **41 caught, 2 unviable, 0 missed** on a REAL baseline
(`Unmutated baseline ok 21s build + 5s test`). `/rust-review`
substituted by an in-thread tool-grounded self-review (pure / total /
panic-free / shared-bound / byte-identical-clean) — deliberately NOT
subagent-delegated (D-008 integrity incident). Recorded as a
substitution, not an independent SHIP.
