# ctx — project document

> Provisional codename `ctx` (almost certainly taken on crates.io / npm / GitHub — **name-collision check + rename is the first pre-publish gate**, see §11). Part of the **caliper** line (see `../../docs/09-roadmap.md`). This is the single detailed project doc; `../CLAUDE.md` holds the essentials.

---

## 1. One line

**`ctx` = `htop` / `EXPLAIN ANALYZE` for LLM prompts.** A local-first, zero-config, single static binary that sits as a transparent proxy at the LLM-API boundary and X-rays the *actually assembled* prompt of any agent pipeline (LangGraph / Pydantic AI / raw SDK / anything) — composition, waste, per-step diff, verbatim text — with **no SDK, no server, no account, no cloud, no DB-as-product**.

## 2. The problem (evidence, not vibes)

Frameworks silently mutate the request between your code and the model. The single most consistently expressed, oldest-unresolved pain in the space:

- *"Provide visibility into the final prompt as sent to the model"* — LangChain issue #912, filed years ago, **closed *not planned***; recurs verbatim in LlamaIndex #7628; is the entire premise of newer tools (vLLora Debug Mode: *"the full request payload your application is about to send — not what you assume it's sending"*).
- Hamel Husain's whole methodology = "look at your data" (manually read 20–50 raw traces). Glassbrain: *"You cannot debug what you cannot see … a critical instruction might get effectively forgotten by step eight."*
- Token-bloat corpus is overwhelming: GitHub MCP server alone ≈ 42k tokens of tool defs; tool-selection accuracy 95%→71% purely from bloat; engineers want *"which servers/skills/tools are loaded and what they cost before running a single prompt."*
- arXiv 2503.06745 (38 practitioners): ~76–80% rank agentic-flow understanding their #1 need; 79–80% cite non-determinism; 77% struggle root-cause; 60% say current tools don't meet their needs.

## 3. The defensible moat (why this isn't the funded bloodbath)

The agent-observability field is a $400M+ funded bloodbath (Langfuse/ClickHouse, LangSmith, Phoenix, AgentOps, Helicone, Logfire, Braintrust). Competitive teardown verdict: **not one incumbent does the thing `ctx` does**, and they structurally *can't* without abandoning their business model (SDK → DB → dashboard → account → seats).

Moat = the **intersection of three things, empty across the entire funded field**:
- **(b) per-step context DIFF** — what was added / removed / mutated between step N-1 and N (and run A vs run B). Everyone has prompt-*version* diff; **nobody** has *context-delta-per-step*.
- **(e) in-prompt composition + waste indictment** — decompose one assembled prompt into system / tool-schemas / history / RAG / memory / skills with % + absolute tokens, and *indict the waste* ("22 of 31 tools never called this run; system prompt duplicated 3×; this RAG chunk repeats in turns 2,4,5; history never pruned"). Everyone shows total token *counts*; nobody decomposes + indicts.
- **zero-config wire capture with LLM semantics** — `mitmproxy ∩ tokenizer ∩ context-budget linter`. mitmproxy proves no-SDK wire capture is accepted but has zero LLM semantics; Helicone proves the proxy delivery is accepted but spends it on routing/cost, not anatomy. Framework-agnostic *because* it reads the wire (LangGraph/PydanticAI/raw SDK are identical on the wire).

Each capability **alone is weak** (full text alone: everyone has it; mitmproxy has it). Defensibility lives **only** in the intersection. The OTEL GenAI spec itself defaults prompt-text capture **OFF** — the standard structurally defers exactly the data `ctx` centers on. That is the tailwind.

### THE discipline rule (non-negotiable)
> **If it needs a server to run or an account to use → kill-zone (you become a worse Langfuse). If it runs as one binary against a captured prompt and produces an indictment → the moat.** The day `ctx` grows a hosted dashboard / persistent analytics product / cloud / accounts, it is dead. Be `htop`/`EXPLAIN ANALYZE`, never Datadog.

## 4. Locked decisions

| Decision | Value | Why |
|---|---|---|
| Language | **Rust** | single static binary, zero-bootstrap-paradox, calibration-league signal (uv/ripgrep/Astral) |
| Mechanism | transparent **local reverse-proxy** at provider base URL (`ctx run -- <cmd>` sets env, or manual `*_BASE_URL`) | framework- & language-agnostic; the line's shared substrate (`agentlock`/`guard` reuse it) |
| Providers v1 | Anthropic + OpenAI-compatible | default backends of LangGraph / Pydantic AI; rest via adapter (§8) |
| Tokenizer | local approximation (e.g. o200k-class), **honestly labeled ±N%**, offline | zero-config/offline is core; never call an API to count |
| Headline | **pure measurement** (facts that survive adversarial reading) + **`--deep`** drill-down | evalint lesson — no contestable judgments ("model will ignore X") |
| Persistence | ephemeral by default; **opt-in** local SQLite (Willison pattern) | a DB-as-product = Langfuse's moat = kill-zone |
| Output | CLI one-shot + interactive **TUI**; **`--json`** everywhere | dashboard-fatigue evidence vindicates local-first CLI; JSON makes it a CI citizen |
| Never | server, account, cloud, hosted UI, eval scoring, prompt registry, APM charts | §9 |

## 5. Organizing model — the step timeline

Everything is a **view on a selected step** of a captured timeline:
`prompt assembled → tool call(args) → tool result → model decision → next prompt …`
The timeline is the spine (hypothesis #2, confirmed). The composition X-ray (#2/§6.2), the diff (§6.3), the verbatim text (§6.1) are *views attached to a selected step*. This data model is v1 substrate, not a feature.

## 6. Feature roadmap (phased, ranked by convergent evidence)

Priority order from research: verbatim-text (#1) ≈ composition+waste (#1-tie) > per-step diff (#3) > timeline spine (the model) > replay (v2, not in original hypotheses) > cross-run diff/CI.

### v1 — foundation + 3 parallel core views (LOCKED)
- **F0 Foundation (sequential, blocks everything):** wire-capture proxy · Anthropic + OpenAI-compat adapter · tokenizer · timeline data model · opt-in SQLite schema · `ctx run -- <cmd>` ergonomics.
- **F1 Composition + waste indictment** *(headline)* — decompose assembled prompt by source (system / each tool schema / each MCP / RAG / history / memory / skills); % + abs tokens; indict: unused-loaded-tools, verbatim-duplicated blocks, repeated RAG chunks across turns, never-pruned history, preamble re-payment count. **Form:** `/context`-style breakdown table/bar; `--deep` per-source detail. Subsumes the "loaded skillset" half of hypothesis 1.
- **F2 Verbatim assembled context at any step** — exact bytes, faithfully rendered (system/user/assistant/tool blocks distinguished), collapsible. **Form:** interactive TUI pager + one-shot.
- **F3 Per-step context diff** — step N vs N-1: added (green) / removed (red) / retained, text + token delta. **Form:** side-by-side TUI.
- F1/F2/F3 are **independent views on F0** → buildable in **parallel** once F0 lands.

### v1.x — the CI/scale extensions
- **Cross-run diff** (run A vs run B context delta) + **`--json` diff for CI** (deploy-gating; the *acted-upon* form of hypothesis 3). *Depends on F3.*
- **Tool-call arg/result fidelity** as first-class timeline rows (malformed-JSON / wrong-tool is a top failure class). *Depends on F0.*
- **First-divergence pinpointing** between two runs (Hamel: "first failure, not downstream"). *Depends on cross-run diff.*
- More provider adapters; tokenizer registry expansion. *Always-parallel (§8).*

### v2 — the replay primitive
- **Replay / re-run a logged request** (optionally edited) and diff the new response vs recorded — directly attacks the #1-cited problem (non-determinism, 80%). v1 SQLite log already captures the replay literature's defined "minimum event set". *Depends on F0 SQLite + proxy re-issue.*

### Frontier / extensible (grows with the field)
- Community **indictment-rule ecosystem** (like ruff lint rules) — versioned ruleset.
- Optional **minimal local web view** *only* for the two things that genuinely benefit from pixels (large side-by-side diff, treemap) — local, ephemeral, never a server product. Strictly optional, late.
- New providers / tokenizers / framework component-classifiers via adapters — perpetual.

## 7. Dependency & parallelization map

```
F0 Foundation (proxy+adapter+tokenizer+timeline+sqlite)   [SEQUENTIAL — blocks all]
        │
        ├── F1 composition+waste  ┐
        ├── F2 verbatim pager     ├─ [PARALLEL — independent views on F0]
        └── F3 per-step diff      ┘
                 │
                 ├── cross-run diff ──► first-divergence      [SEQUENTIAL chain]
                 │        └── --json/CI
                 ├── tool-call fidelity rows                  [parallel after F0]
                 └── v2 replay/re-run (needs F0 sqlite)       [after F0]

Always-parallel, never blocked: provider adapters · tokenizer registry ·
indictment rules · renderers  (these ARE the extensibility seams, §8)
```

## 8. Extensibility architecture (why it grows as the sphere grows)

Stable core + thin seams so new reality slots in without touching the core:
- **Provider adapter trait** — Anthropic, OpenAI-compat, Google, Bedrock, … (wire-layer normalization → canonical request model).
- **Tokenizer registry** — pluggable per model family; honest ±% label per tokenizer.
- **Component classifier interface** — how to segment a canonical payload into semantic components (system/tools/history/RAG/memory/skills); rule-based, extensible as framework conventions evolve.
- **Indictment ruleset** — versioned, community-extensible (ruff-style): duplicate-block, dead-tool, unpruned-history, preamble-repay, rag-repeat, …
- **Renderer trait** — TUI / JSON / (later optional minimal web), behind one interface.
- **Storage trait** — default ephemeral; opt-in local SQLite; **never** a hosted DB.

This is the design that lets `ctx` absorb new frameworks/providers/visual needs forever **without** drifting into the platform kill-zone.

## 9. Non-goals / 🚩 red-flags (DO NOT build — incumbents own these or it's the kill-zone)

- ❌ Trace trees / nested-span timelines as a product (LangSmith/Phoenix/Langfuse own it)
- ❌ Session replay/time-travel *UI* (AgentOps owns it) — note: `ctx` replay (§6 v2) is request re-issue+diff, not a replay IDE
- ❌ Graph/skeleton **IDE** (LangGraph Studio owns it, free, framework-native — and the API boundary can't faithfully reconstruct a framework graph). Hypothesis 1's graph half: **not built.**
- ❌ Evals / scoring / LLM-as-judge / datasets / CI eval gates (Braintrust/Phoenix)
- ❌ Prompt management / versioning / registry / playground (solved 5×)
- ❌ Cost dashboards / analytics-over-time / alerting / SLOs / multi-user / RBAC / retention
- ❌ Routing / caching / gateway features (Helicone)
- 🚩 **Any persistent-store-as-product, hosted UI, account, or cloud → instant death.** (Discipline rule, §3.)

## 10. Hypotheses verdict (for the record)

| User hypothesis | Verdict | Disposition |
|---|---|---|
| 4 — full context text per step | **Strongest / foundational** | v1 F2 |
| 3 — per-step add/remove diff | **Confirmed; broaden** | v1 F3 + v1.x cross-run/CI |
| 2 — log of actions/decisions | **Confirmed; it's the spine** | v1 timeline model (§5) |
| 1 — agent skeleton | **Split: graph REFUTED, skillset CONFIRMED** | graph: not built (§9); skillset: folded into v1 F1 |
| (+) per-source token attribution | user didn't name; **highest-confidence** | v1 F1 (headline) |
| (+) replay/re-run | user didn't name; **high-leverage** | v2 |

## 11. Open items

- **NAME** — `ctx` provisional & near-certainly taken. Pre-publish gate: collision-check crates.io / npm / GitHub; pick a distinct name. Candidates to vet: `ctxray`, `promptscope`, `lensllm`, `assembled`, `whatissent`, `tldraw-of-prompts`(no), `xri`(x-ray-it). Decide before `git init`/publish, not before building.
- Exact `--deep` interaction model (TUI keybindings) — design during F2.
- Tokenizer accuracy target & the honest ±% wording — fix during F0.

## 12. Evidence

Full research memos & sources: `../../docs/` (esp. `07-harsh-reality-demand-and-mechanism.md`, `09-roadmap.md`) and the two ctx-specific research memos summarized into this doc (demand/form + competitive teardown, May 2026). Key primary sources: LangChain #912, LlamaIndex #7628, Pydantic AI #4137, Claude Code #10164, OpenAI Codex #14642, arXiv 2503.06745, Hamel Husain evals-faq, vLLora debug-mode, OTEL GenAI semconv (content off-by-default).
