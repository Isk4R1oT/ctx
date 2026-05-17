# CLAUDE.md — ctx

Essentials for anyone (incl. Claude) working in this repo. Full detail: `docs/PROJECT.md`. Line context: `../docs/00-overview.md` + `../docs/09-roadmap.md`. **CLI aesthetic: follows `../docs/11-cli-design-system.md` (Claude Code house style) — mandatory.** Research & rationale: `docs/RESEARCH.md`.

## What this is

`ctx` = **`htop` / `EXPLAIN ANALYZE` for LLM prompts.** A local-first, zero-config, single static **Rust** binary that sits as a transparent **proxy at the LLM-API boundary** and X-rays the *actually assembled* prompt of any agent pipeline (LangGraph / Pydantic AI / raw SDK — identical on the wire) — composition, waste, per-step diff, verbatim text — with **no SDK, no server, no account, no cloud, no DB-as-product**.

## The one rule that decides everything

> **Needs a server or an account → kill-zone (you become a worse Langfuse). One binary against a captured prompt producing an indictment → the moat.**
> Hosted dashboard / persistent-analytics-product / cloud / accounts = instant death. Be `htop`, never Datadog.

## The moat (only the intersection)

(b) per-step **context diff** ∩ (e) in-prompt **composition + waste indictment** ∩ **zero-config wire capture with LLM semantics**. Each alone is weak; the intersection is empty across the entire $400M-funded field because their architecture (SDK→DB→dashboard→account) is the opposite. `mitmproxy ∩ tokenizer ∩ context-budget linter`.

## Locked decisions

- **Rust**; transparent local reverse-proxy (`ctx run -- <cmd>` or `*_BASE_URL`); providers v1 = Anthropic + OpenAI-compatible.
- Tokenizer: local approximation, **honestly labeled ±N%**, offline (never call an API to count).
- Headline = **pure measurement** (no contestable judgments — evalint lesson) + **`--deep`** drill-down.
- Persistence: ephemeral by default; **opt-in** local SQLite.
- Output: CLI + interactive TUI; **`--json`** everywhere (CI citizen).

## v1 scope (LOCKED)

F0 foundation (proxy + adapter + tokenizer + timeline model + opt-in SQLite + `ctx run`) — sequential, blocks all. Then **parallel**: F1 composition+waste indictment (headline) · F2 verbatim per-step text pager · F3 per-step context diff. Organizing model = the **step timeline**; everything is a view on a selected step.

## Do NOT build

Trace-tree product, session-replay IDE, graph/skeleton IDE (LangGraph Studio owns it), evals/scoring, prompt registry, cost/APM dashboards, routing/caching, any persistent-store-as-product or hosted UI. See `docs/PROJECT.md` §9.

## Extensibility seams (grow without touching core)

Provider-adapter trait · tokenizer registry · component-classifier interface · versioned indictment ruleset (ruff-style) · renderer trait · storage trait (never hosted). See §8.

## Status

Spec/roadmap complete (this repo). Name `ctx` provisional — collision-check + rename is the first **pre-publish** gate (`docs/PROJECT.md` §11), not a pre-build gate. F0 implementation in progress (see `docs/DECISIONS.md`).

## Rust (rust-cc plugin active)
- The compiler is the oracle: after Rust edits, `cargo check` is run by a
  gate; fix the digest's root-cause class first (one class per iteration).
- Errors → `thiserror` (lib) / `anyhow` (bin). Never `unwrap()`/`expect()`
  outside tests; propagate with `?`.
- Never `.clone()` to silence a borrow error — fix ownership.
- Verify crate APIs (context7 / rust-analyzer), never from memory.
- Don't commit a red build; not done until `cargo check` is green.
- Run `just check` / `just test`. See `../../rust-cc/COMPILER-TRUTH.md` and `docs/DECISIONS.md` (D-001 canonical CLI surface).
