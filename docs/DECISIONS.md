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
