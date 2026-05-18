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

## D-010 — C1: cache-prefix-break, a new pure-measurement F1 indictment (2026-05-18)

### Status: LOCKED. Additive (the PROJECT.md §8 "versioned indictment ruleset" seam). Does not relitigate D-001..D-009; evalint stays KILLED.

**Why.** Prompt-prefix caching (Anthropic `cache_control`, OpenAI
automatic prefix cache) is a large real cost lever, and the *cause* of
a cache miss is a request-prefix byte divergence. Every incumbent reads
the response `usage` object (a billed, post-hoc outcome); none surface
the byte-level cause, because their SDK→DB→dashboard architecture has
no verbatim per-step request timeline. `ctx` already holds both bodies
byte-exact — this deepens the exact locked moat (composition+waste ∩
per-step diff ∩ zero-config wire capture), not drifts from it.
Corroborated by the independent research artifact
`docs/CONTEXT-SIGNALS-RESEARCH.md` (ranked C1 highest).

**Rule (pure measurement).** `indict_cache_prefix_break`: over
consecutive requests with the SAME (provider, model), measure the
common byte **prefix** and common byte **suffix** of the verbatim wire
bodies (char-boundary safe); fire only when the prompt is non-trivial
(`>= CACHE_MIN_PROMPT_TOKENS` 256), a large identical suffix proves the
same continuing context (`>= CACHE_MIN_SHARED_SUFFIX_TOKENS` 64), AND
the cacheable prefix is < half the prompt. `wasted_tokens` = the
re-sent tokens past the break. Strictly counts/bytes/tokenizer-sums/
integer compares — NO prediction of whether the provider will cache,
NO judge (evalint KILLED). Provider-specific cache mechanics (Anthropic
breakpoint placement, OpenAI's 1024-token minimum) are documentation /
`--deep` context, never the headline.

**Honest limits (recorded, not buried).** It measures byte-prefix
identity across consecutive same-(provider,model) requests — NOT a
guarantee the provider will/won't cache. Tokenizer is the offline ±N%
approximation. Request-only; needs >=2 same-namespace steps. The
shared-suffix gate biases to true-positives: a real break where the
suffix ALSO changed is a deliberate honest false-negative (under-claims
rather than over-claims). Unknown/changed provider or model ⇒ different
namespace ⇒ never flagged.

**Gates (real, non-vacuous).** TDD red-first (failing compose.rs test
proven on `19247ac` predecessor before impl). Compiler-truth fixed
E0689. `just check` clippy `-D warnings` 0; `just test` **109/109**;
`cargo deny`/`machete` ok (no new deps). cargo-mutants `--in-diff` on
the C1 surface: run 1 = **8 missed** on a REAL baseline (`ok 28s build
+ 7s test`) — NOT accepted; the decision was extracted into the pure
`cache_break_wasted` helper with exact-boundary unit pins (the approx
tokenizer cannot reach these boundaries through `compose()`), the
compound `&&` split into sequential guards, the provider/model gate
collapsed to one `(provider,model)`-key match (removed the `||`), and
worst-tracking moved to std `min_by_key` (removed the hand `<`). Run 2
= **30 caught, 3 unviable, 0 missed** on a REAL baseline (`ok 27s build
+ 7s test`). Real e2e: a real 2-turn `ctx run` — a volatile session-id
prepended to the system prompt ⇒ fires (`wasted=796, ~21/~817 tok
prefix`); an identical stable prefix ⇒ correctly SILENT. `/rust-review`
substituted by in-thread tool-grounded self-review (pure / total /
char-safe / true-positive-gated) — deliberately NOT subagent-delegated
(D-008 incident). Recorded as a substitution, not an independent SHIP.

## D-011 — C2: component-drift, a new pure-measurement F1 indictment (2026-05-18)

### Status: LOCKED. Additive (the PROJECT.md §8 "versioned indictment ruleset" seam). Does not relitigate D-001..D-010; evalint stays KILLED.

**Why.** A framework silently mutating a component the engineer believes
is stable — the `system` block, or a tool's schema — between turns of one
live run is the #1 cause of both cache invalidation and a "stable"
instruction changing under the engineer's feet. It is the OPPOSITE of
`preamble-repay` (D-?, F1): that counts an *identical* component re-paid;
C2 catches a same-NAMED component whose bytes/size CHANGE mid-session.
Every incumbent has prompt-*version* diff (a registry concept: v1 vs v2
of a managed prompt); none has "the same logical component changed bytes
between step 3 and step 4 of one live run", because that needs the
per-step verbatim wire timeline their SDK→DB→dashboard architecture does
not have. Deepens the locked moat (per-step diff ∩ waste indictment ∩
wire capture); ranked HIGH by `docs/CONTEXT-SIGNALS-RESEARCH.md` §(c) C2,
whose definition / "pure-measurement?" / caveat this entry obeys exactly.

**Rule (pure measurement).** `indict_component_drift`: walk steps in
order; per component key (`system`, and each tool keyed by `tool:<name>`)
compare its current fingerprint to its previous appearance. `system`
fingerprints on its **exact bytes** (byte change ⇒ event; the byte delta
is exact, the token delta is the labeled ±N% estimate). Each tool
fingerprints on the **size the canonical `Assembled` view exposes**
(`schema_tokens`). Emit one `component-drift` indictment iff >=1 same-
named component mutated, listing each drifted component pinned to the
FIRST step index where it changed (deterministic `BTreeMap` ordering) and
the summed token delta. The per-appearance decision is isolated in the
pure `drift_delta(prev,cur)` helper — ONE `==` + ONE saturating
abs-difference — with an exact-boundary unit table (the approximate
tokenizer cannot reach those values through `compose()`; the D-010
technique applied preemptively). Strictly counts/bytes/tokenizer-sums/
integer compares — NO prediction, NO "the model will forget X" (evalint
KILLED; the lost-in-the-middle framing is the §(d) excluded class).

**Honest limits (recorded, not buried).**
- Tool drift is detected at the **tokenizer-size granularity** the
  canonical `Assembled` view exposes, NOT at byte granularity:
  `WireTool` carries `name` + `schema_tokens` only, not the raw schema
  bytes. Re-deriving schema bytes in `compose.rs` would duplicate the
  adapter (D-007/D-008) or require an `Assembled` shape change (the
  goal's explicit HALT condition — a struct `agentlock`/`guard` share).
  So a tool-schema mutation that leaves `schema_tokens` unchanged is a
  **deliberate honest false-negative** (the D-010 true-positive-bias
  discipline: under-claims rather than over-claims). `system` is exact
  (full bytes are in `Assembled`).
- The token figures ride the offline ±N% tokenizer; the `system`
  byte-change detection itself is exact.
- A renamed tool is a different key ⇒ remove+add, NEVER a drift event
  (drift requires the SAME name to change). Asserted by a test and
  stated verbatim in the `detail` string — not inferred (per §(c) C2
  caveat).
- Request-only; needs >=2 appearances of the same-named component.

**No `Assembled` shape change (the HALT guard held).** C2 reads only
`a.system` and `a.tools` — fields the canonical view already exposes.
Zero new dependencies (stdlib `BTreeMap` + the existing tokenizer). The
existing F1 snapshots are untouched: the snapshot fixture sends a
byte-identical system + identical tools across both steps, so C2 is
correctly SILENT there (it is `preamble-repay`'s opposite) — verified, no
snapshot regeneration.

**Indictment surface.** `code = "component-drift"`; one-line `detail`:
`"<N> same-named component(s) mutated mid-session: <key>@step <ix>, … (~<T> tok changed; a renamed tool reads as remove+add, not drift)"`;
`wasted_tokens` = saturating sum of the per-event absolute token deltas
(matches the field's "per-rule measured waste", non-partition note).

**Gates (real, non-vacuous).** TDD red-first: the failing compose.rs
behavioral test was proven RED on worktree HEAD `0a07bd4` (panic "a
mutated system block across steps MUST be indicted") while the other 16
compose tests + the full 109 suite stayed green, recorded in
RUSTCC-USAGE.md, `#[ignore]`-with-reason at the test commit `719cc81`,
un-ignored in the impl commit `e87c0ff`. `just check` clippy `-D
warnings` 0; `just test` **111/111, 0 skipped** + doctests (+2 vs the
109 baseline = the un-ignored behavioral test + the `drift_delta`
boundary table; zero regression — purely additive). `cargo deny`
advisories/bans/licenses/sources ok; `cargo machete` no unused deps
(no new deps). cargo-mutants `--in-diff` on the C2 surface, **two
independent REAL non-vacuous baselines**: run 1 baseline `ok 71s build +
29s test` → 10 mutants → **8 caught, 2 unviable, 0 MISSED**; run 2
baseline `ok 41s build + 15s test` → **8 caught, 2 unviable, 0 MISSED**
(stable). The 2 unviable are non-compiling `Default::default()`
substitutions (correctly unviable, not equivalent-and-missed). 0 missed
on pass 1 *by construction* — the decision was isolated in `drift_delta`
from the start, so unlike D-010 no 2-pass restructuring was needed; no
equivalent mutant to eliminate. Real e2e (green != works): a real 2-turn
python httpx client through `ctx run --save` (dummy `Authorization:
Bearer test`, natural `/v1/chat/completions`, upstream 404 ignored —
request captured before forward, $0, no real key) — MODE=drift (turn 2
mutates the system prompt under the same provider+model) ⇒ FIRES
(`component-drift wasted_tokens=1 … system@step 1 (~1 tok changed …)`),
and round-trips post-hoc through `ctx open`; MODE=stable (byte-identical
system on turn 2) ⇒ correctly SILENT (only `preamble-repay`/
`unused-loaded-tools` fire), also post-hoc. `/rust-review` not invocable
in-thread ⇒ substituted by an in-thread tool-grounded self-review (pure
/ total / panic-free / no `Assembled` change / deterministic / renamed-
tool=remove+add) — deliberately NOT subagent-delegated (D-008 integrity
incident). Recorded as a substitution, not an independent SHIP. No false
green.
## D-012 — C3: context-window headroom & growth-rate slope, a new pure-measurement signal (2026-05-18)

### Status: LOCKED. Additive (the PROJECT.md §8 static-registry seam, alongside the F1 headline). Does not relitigate D-001..D-011; evalint stays KILLED.

**Why.** Frameworks silently grow the assembled context turn over turn;
the single most-cited agent pain is not seeing *what is actually in the
window* and *how fast it is filling*. Every incumbent shows per-call
token *totals*; none plots the assembled-context growth slope vs the
model's window from the **wire**, because their SDK→DB→dashboard
architecture does not model the prompt as a per-step series of assembled
bytes. `ctx` already holds that series (the F0 timeline `prompt_tokens`).
Ranked **MED** by the independent research artifact
`docs/CONTEXT-SIGNALS-RESEARCH.md` §(c) C3 — *MED not for the code
(trivial) but for the discipline risk*: a "headroom / turns remaining"
framing is one wording slip away from a quality *prediction* (evalint,
KILLED). This entry records the constraint that keeps it pure.

**The signal (pure measurement).** Per the focus step (the last
structurally-parsed step, same as the F1 headline focus):
- **Window fraction** — `prompt_tokens` (the existing F0-computed ±N%
  estimate) as an integer percent of the model's context window. The
  window is a **static offline registry** (`src/window.rs`, the §8
  seam), keyed by a case-insensitive longest-substring match on the
  wire model id, carrying its own honest label `WINDOW_LABEL`
  ("offline static window table, approximate (never calls an API)") —
  the C3 analogue of the tokenizer's `±N%`. No API call (zero-config
  core).
- **Growth slope** — the measured mean `(last − first) / (turns − 1)`
  tokens/turn over the **same-(provider, model)** turns (a different
  model is a different budget; its turns are not in the series). Signed
  `i64` (a shrinking session is a real negative slope, never clamped).
- **Headline = the fraction + the slope ONLY.** Integer-only
  (`used_pct`, `slope_tokens_per_turn`), snapshot-stable, no float —
  mirrors the existing `pct` integer discipline; additive
  `Composition.headroom: Option<Headroom>` preserves the D-005 `--json`
  contract (`steps` stays top-level — verified at `report_json`).

**The DISCIPLINE constraint that keeps it pure (the reason this is a
signal, not evalint).** Any "turns remaining" figure is a *projection*,
which is contestable. It is therefore:
- **`--deep`-ONLY** (absent from the headline and from non-`--deep`
  `--json`; gated in `compose`, the renderer never decides), and
- **worded as neutral arithmetic** — verbatim: *"at the observed mean
  rate (~S tok/turn over N turn(s)), ~K more turn(s) before the W-tok
  window is reached (neutral arithmetic projection, not a prediction)"*.
  It NEVER says "you will overflow", "the model will truncate", "you
  will run out" — a test (`c3_projection_is_deep_only_and_neutrally_
  worded` + the snapshot guards) makes that banned-phrasing class a hard
  CI failure forever. The extrapolation is pure division on the labeled
  estimates, not a claim about fate (that would be evalint — EXCLUDED,
  CONTEXT-SIGNALS-RESEARCH.md §(d)).

**Honest limits (recorded, not buried).**
- The window table is a **maintained approximation** — context-window
  sizes drift per model release (`[INFER]`, research §(e)); labeled like
  the ±N% tokenizer, never sold as exact.
- An **unknown wire model id ⇒ NO window claim** (skipped honestly,
  `headroom: None` — never a guessed size/fraction). A session with
  **< 2 same-namespace turns ⇒ no measurable slope ⇒ no C3 claim** at
  all. The two gates are independent (a known model with one turn is
  still silent).
- The token figures ride the existing offline ±N% tokenizer (byte→token
  is an approximation); the slope is exact integer arithmetic on that
  approximate series. Request-only; round-trips post-hoc (`ctx open`).
- The slope is a coarse `(last − first)/(turns − 1)` mean, not a fitted
  regression — deliberately the simplest honest arithmetic (research
  §(c) "least-effort"), no smoothing, no trend claim.

**Gates (real, non-vacuous).** TDD red-first: the `c3_*` compose tests
**fail to compile on the worktree HEAD** (`no field 'headroom' on
Composition`, 7×) with the rest of the suite green (109 nextest + 91
lib) — proven before impl. Compiler-truth loop (`just check`,
clippy `-D warnings` incl. pedantic): the digest named
*too-many-lines* / *u128-as-u32-truncate* / *useless-conversion*; fixed
root-cause-first (extracted `headroom_tty`; replaced the lossy test
cast with the exact `pct()` check). `just test` = **129 nextest + 100
lib** green + doctests; `cargo deny`/`machete` ok (no new deps —
`window.rs` is pure data + std). cargo-mutants `--in-diff` on the C3
surface, REAL baselines (explicitly not vacuous/timed-out):
- **Run 1** — baseline `ok 25s build + 8s test`, 28 mutants → **1
  MISSED**: `replace && with || in headroom` (the `provider && model`
  series filter; all prior fixtures used a single namespace so `&&`≡
  `||`). **NOT accepted.** Eliminated **BY CONSTRUCTION** (the proven
  D-010 technique): the namespace match became a single `(provider,
  model)` **tuple equality** via the pure `step_namespace` helper — no
  `&&` operator remains to widen into a namespace-crossing `||` — plus
  a discriminating mixed-namespace fixture (a foreign `gpt-4o` step
  wedged between focus Anthropic turns; pins `turns == 2`, not 3) and
  exact-value `step_namespace` pins.
- **Run 2** — baseline `ok 31s build + 14s test`, 29 mutants → **25
  caught, 4 unviable, 0 MISSED**. The pure helpers `slope_per_turn` /
  `turns_until_window` carry deterministic exact-boundary unit tables
  (the approximate tokenizer cannot reach those boundaries through
  `compose`); every `window.rs` table entry is exact-value pinned.

Real e2e (green ≠ works): a real python-httpx **multi-turn growing**
conversation through `ctx run --save /tmp/c3.db` (DUMMY
`Authorization: Bearer test`, natural `/v1/chat/completions` — upstream
404s, request captured BEFORE forward, $0, no real key). `ctx open`
headline shows `window gpt-4o 561/128000 tok 0% slope 146 tok/turn over
4 turn(s) (...approximate...)` and **no** projection; `ctx --deep open`
adds exactly `window-projection at the observed mean rate (~146
tok/turn over 4 turn(s)), ~872 more turn(s) before the 128000-tok
window is reached (neutral arithmetic projection, not a prediction)`.
A single-turn known-model session and an unknown-model session each
emit **zero** window claims even with `--deep`. `--json`/`--deep
--json` confirm `steps` top-level + integer-only `headroom`.
`/rust-review`/`/rust-harden` skills not invocable in-thread →
substituted by the tool-grounded compiler-truth loop + the 2-pass real
cargo-mutants evidence above; deliberately NOT subagent-delegated
(D-008 integrity incident). Recorded as a substitution, not an
independent SHIP. evalint stays KILLED — no kill-zone, no graph, no
hosted anything; no false green.

## D-013 — C6: request-replayed, a new pure-measurement F1 indictment (2026-05-18)

### Status: LOCKED. Additive (the PROJECT.md §8 "versioned indictment ruleset" seam). Does not relitigate D-001..D-010; evalint stays KILLED; graph stays REFUTED.

**Why.** A retry after a 429/5xx, or an idempotent re-issue, is emitted
by an HTTP-client/framework layer *below* the user's code — the engineer
usually cannot see that the SAME assembled prompt was re-sent verbatim,
nor the real, re-billed token cost of it. Every incumbent treats retry
storms as a *tracing* concern (count retry spans); none asserts "this
exact assembled prompt was re-sent byte-for-byte, here is the duplicated
token cost" from wire byte-equality, because their SDK→DB→dashboard
architecture has no verbatim cross-step request timeline. `ctx` already
holds every request body byte-exact (F0) and buffers responses (F0,
D-004) — C6 is a hash-equality pass over data it already has, deepening
the locked moat (composition+waste ∩ per-step diff ∩ zero-config wire
capture), not drifting from it. Spec'd and ranked by the independent
research artifact `docs/CONTEXT-SIGNALS-RESEARCH.md` §(c) C6.

**Rule (pure measurement).** `indict_request_replayed`: group steps by
the **verbatim request body** (non-empty only — an empty body carries no
re-billed prompt cost, mirroring the block rules' `MIN_BLOCK_BYTES`
discipline); a body occurring in `>= 2` steps is a replay. Reports the
total re-billed copies (`Σ occurrences − 1`), the count of distinct
replayed bodies, the duplicated token weight (`Σ tokenizer::count(body)
× (occurrences − 1)`), and — from F0's ALREADY-BUFFERED response — the
status of the *first occurrence of the most-replayed body* (the attempt
that was retried; e.g. "first replayed attempt returned 529"), or an
honest "status not captured" when no response was buffered (never a
fabricated status). Strictly full-body byte-equality + counts +
tokenizer sums + the buffered status — NO judge, NO score, NO prediction
(evalint KILLED).

**The `guard`-boundary (honest, load-bearing).** C6 BORDERS `guard`'s
territory (the deterministic loop/cost circuit-breaker). `ctx` only
**REPORTS the fact** — it MUST NEVER throttle, rate-limit, circuit-break,
de-duplicate, or otherwise intervene/remediate. That is `guard` and is
**EXCLUDED here by construction** (CONTEXT-SIGNALS-RESEARCH §c/§d; the
function returns an `Option<Indictment>` and has no side effect, no
network, no mutation of the timeline). This boundary is the reason C6 is
request-only for the core fact and a pure measurement, not an action.

**Honest limits (recorded, not buried).** (1) Whole-body **exact**
byte-equality only — a retry that changed even one byte (e.g. an
idempotency-key header is not in the body, but a per-attempt nonce in the
body would be) is a deliberate honest false-negative (under-claims rather
than over-claims; near-duplicate is contestable and EXCLUDED per
research §d). (2) Token figures ride the offline ±N% tokenizer (labelled
in the summary line); the byte-equality and counts are exact. (3)
Request-only; needs >=2 steps. (4) The status annotation is the
most-replayed body's *first* occurrence only (one representative status,
not a per-attempt list — kept minimal and pure; `--deep` per-attempt
status is a possible future seam, not built). (5) When the upstream is
unreachable the proxy synthesizes a 502 and does **not** `record_response`
(F0 forward-error path), so the annotation is the honest "status not
captured" — observed and accepted in the real e2e, not hidden. (6)
Streaming-vs-buffered does not affect C6 (request-side; D-004).

**Scope (HALT condition checked, NOT triggered).** C6 reads only
`step.request.body` / `step.response` / (no `Assembled` needed). It does
**not** touch `adapter.rs::Assembled` or any shared F0 struct — no
re-architecture, no F0/R regression (snapshots use two *different*
bodies ⇒ C6 correctly silent there; all pre-existing snapshots
unchanged). The HALT-if-it-needs-`Assembled` invariant was explicitly
verified false before implementation.

**Gates (real, non-vacuous).** TDD red-first: three `compose.rs` tests
written and proven RED on worktree HEAD `410c790`
(`request_replayed_*` → 2 failed / 1 control-pass) with the rest of the
suite green (**109/109**), committed before the feature. `just check`
clippy `-D warnings` **0**; `just test` **113/113** + doctests;
`cargo deny`/`machete` ok (**no new deps** — reuses `tokenizer` + std).
cargo-mutants `--in-diff` on the C6 surface, **two independent passes,
both 0 missed on a REAL baseline**: pass 1 = **11 caught, 2 unviable,
0 missed** (`Unmutated baseline ok 31s build + 8s test`); pass 2 =
**11 caught, 2 unviable, 0 missed** (fresh `Unmutated baseline ok 40s
build + 14s test`). 0-missed was reached **by construction** (proactive
D-010 technique: the cost decision isolated in the pure
`replay_wasted` helper with an exact-boundary unit table; std
`filter(len()>=2)`/`max_by_key`/`sat_sum` instead of hand
`<`/`||`/compound booleans; `!is_empty` + `wasted==0` guards) — NOT by
post-hoc test patching; the 2 unviable are the non-compiling
`Default::default()` return arms (genuinely unviable, not skipped).
Real e2e through the `ctx` binary (real python httpx clients, no real
key, DUMMY `Authorization: Bearer test`, natural
`/v1/chat/completions`, $0): same body POSTed twice ⇒ **fires**
(`request-replayed wasted_tokens=60`); two different bodies ⇒ correctly
**SILENT**; `ctx open`/`--json` round-trip the indictment post-hoc; a
local 529 upstream ⇒ the buffered-status path reports "first replayed
attempt returned 529" (the research §c shape). `/rust-review` +
`/rust-harden` not invocable in-thread → substituted by an in-thread
tool-grounded self-review (pure / total / panic-free / saturating /
byte-equality-only / zero `guard`-style intervention) — deliberately
NOT subagent-delegated (D-008 review-subagent integrity incident).
Recorded as a substitution, not an independent SHIP.
