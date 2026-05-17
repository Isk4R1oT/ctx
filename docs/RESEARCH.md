# ctx — Research & Rationale

> Why this project exists, what the evidence actually says, who the competition is, why each decision was made — and exactly where the claims are soft. The rigor is the point.

---

## TL;DR

- **The pain:** frameworks silently mutate the request between your code and the model; "show me the prompt *as actually sent*" is the single most consistently expressed, oldest-unresolved ask in the agent-dev space — filed years ago as LangChain #912, **closed *not planned***, and still recurring verbatim across LlamaIndex, Pydantic AI, Claude Code and Codex.
- **The wedge:** a local-first, zero-config, single static Rust binary that sits as a transparent proxy at the LLM-API boundary and X-rays the *actually assembled* prompt of any pipeline (LangGraph / Pydantic AI / raw SDK — identical on the wire). `htop` / `EXPLAIN ANALYZE` for prompts, not another dashboard.
- **The moat:** the agent-observability field is a **$400M+ funded bloodbath** (Langfuse/ClickHouse, LangSmith, Phoenix, AgentOps, Helicone, Logfire, Braintrust) — and *not one incumbent does what `ctx` does*, because their architecture (SDK → DB → dashboard → account → seats) structurally can't. Defensibility lives only in the **intersection** of (b) per-step context diff ∩ (e) in-prompt composition + waste indictment ∩ zero-config wire capture with LLM semantics. Each capability *alone* is weak; the intersection is empty across the whole funded field.
- **The tailwind:** the OTEL GenAI semantic-convention spec defaults prompt-text capture **OFF** — the industry standard structurally defers exactly the data `ctx` centers on.
- **The key honest limit:** the headline metric depends on a **local approximation tokenizer** that is offline by design and therefore only honest if labeled **±N%**; and the demand corpus, while strong, leans on issue-tracker recurrence and practitioner surveys, not a controlled study (every soft claim is flagged in §6).
- **The discipline:** the day `ctx` grows a hosted dashboard / persistent-analytics product / cloud / accounts, it is dead. Be `htop`, never Datadog. This rule is non-negotiable and is what keeps it *out* of the bloodbath rather than a worse entrant in it.
- **The verdict:** a defensible, ahead-of-time, single-purpose artifact that closes a demand-proven universal pain on an axis no funded incumbent can take without abandoning its business model — and whose own spec carries explicit "why this might fail" lines. Top-0.01% as calibration, not as a slogan.

---

## The problem & why it matters

Frameworks silently mutate the request between your code and the model. You write what you *think* the agent sends; the wire carries something else — re-payments of the preamble, tool schemas you never invoke, RAG chunks duplicated across turns, history that is never pruned. The user pain here is not a vibe; it is the **single most consistently expressed, oldest-unresolved pain in the space**, evidenced across independent surfaces:

- **The canonical ask, filed and refused:** *"Provide visibility into the final prompt as sent to the model"* — **LangChain issue #912**, filed years ago, **closed *not planned***. The same request **recurs verbatim in LlamaIndex #7628**, and is the *entire premise* of newer tools — **vLLora Debug Mode**: *"the full request payload your application is about to send — not what you assume it's sending."* The exact phrasing recurring across unrelated projects over years is the strongest possible signal that the pain is real and unmet.
- **The methodology of the people who are good at this:** Hamel Husain's whole approach = "look at your data" (manually read 20–50 raw traces). **Glassbrain**: *"You cannot debug what you cannot see … a critical instruction might get effectively forgotten by step eight."*
- **Token-bloat is overwhelming and quantified:** the **GitHub MCP server alone ≈ 42k tokens of tool definitions**; tool-selection accuracy drops **95% → 71% purely from bloat**; engineers explicitly want *"which servers/skills/tools are loaded and what they cost before running a single prompt."*
- **Practitioner study:** **arXiv 2503.06745** (38 practitioners): **~76–80%** rank agentic-flow understanding their **#1 need**; **79–80%** cite non-determinism; **77%** struggle with root-cause; **60%** say current tools don't meet their needs.

Named primary sources (verbatim, as recorded in PROJECT.md §12 and CLAUDE.md):

- **LangChain #912** (the canonical, refused ask)
- **LlamaIndex #7628** (verbatim recurrence)
- **Pydantic AI #4137**
- **Claude Code #10164**
- **OpenAI Codex #14642**
- **arXiv 2503.06745** (38-practitioner study)
- **Hamel Husain — evals-faq** ("look at your data")
- **vLLora debug-mode** ("not what you assume it's sending")
- **OTEL GenAI semconv** — prompt/response content capture **off-by-default** (the standard structurally defers exactly this data; this is the tailwind)

Roadmap-level demand framing for the anchor (from `09-roadmap.md` §1): context-window waste/opacity is **universal** — instruction-budget research; **MCP schemas burn 40–55k tokens pre-message**; **~20% cost inflation**. Verdict in the roadmap matrix: **FIRST** (holistic context attribution; explicitly *not* the saturated MCP-proxy lane), measurable axis = *tokens-of-waste surfaced per repo; drift over commits*, risk level **Low**.

Why it matters: this is not a niche annoyance. It is *every* agent developer, on *every* framework, paying for tokens they cannot see, debugging behavior they cannot reconstruct, against an industry standard (OTEL GenAI) that defaults the relevant data off. The pain is broad, acute, old, and structurally unaddressed by the incumbents — the rare combination that justifies a sharp single-purpose tool.

---

## What the research found

### Demand evidence (breadth × acuteness × lack-of-solution)

The keystone demand memo (`07-harsh-reality-demand-and-mechanism.md`) is deliberately brutal and re-ranks pains by *breadth × acuteness × lack-of-solution*. Verbatim numbers:

- **Stack Overflow 2025 Survey:** **66%** — "AI solutions almost right but not quite" (the #1 frustration); **45%** — "debugging AI-generated code more time-consuming than writing it"; **84%** use AI but **46%** don't trust output; positive sentiment **70%+ → 60%**.
- **Empirical SO agent-challenges study** (3,191 posts 2021–25, **arXiv 2510.25423**), ranked: install/dependency conflicts **20.9%** (broad, shallow); embeddings/vector stores **17.3%**; prompt/output engineering **17.0%**; orchestration **13.0%**; runtime/integration ops **12.2%**; RAG engineering **9.8%** (hardest: median **87.4h** vs **11.7h** for deps); robustness/reliability/evaluation **9.7%** (RAGAS in only **0.6%** of posts).
- **Agent-reliability** (**arXiv 2602.16666** + others): low outcome consistency across **14 models / 3 providers**; non-determinism persists at temp=0; *"replaying the request never reproduces the failure"*; agents looping *"burning $50/minute"*; **68%** use blunt step limits because there is no loop-detection infra.

`ctx`'s pain — context-window waste/opacity — sits in the broad, universal band: it is the visible substrate beneath "prompt/output engineering" (17.0%), token bloat, and the #1-cited agentic-flow-understanding need (~76–80% in arXiv 2503.06745). It is the *low-risk anchor* precisely because it is universal and read-only, not because it is the single highest-frequency SO tag.

### The competitive teardown (who exists, where each is wrong / siloed / non-universal)

The agent-observability field is a **$400M+ funded bloodbath**. The teardown verdict is blunt: **not one incumbent does the thing `ctx` does, and they structurally can't without abandoning their business model** (SDK → DB → dashboard → account → seats).

- **Langfuse / ClickHouse, LangSmith, Phoenix, AgentOps, Helicone, Logfire, Braintrust** — own *live tracing*, prompt-*version* diff, cost dashboards, eval scoring. They are architecturally the **opposite** of `ctx`: server-resident, account-gated, DB-as-product. Their moat (the persistent store) is precisely `ctx`'s kill-zone.
- **mitmproxy** — proves no-SDK wire capture is *accepted*, but has **zero LLM semantics** (no tokenizer, no composition, no indictment).
- **Helicone** — proves the proxy delivery is *accepted*, but spends it on **routing/cost**, not prompt anatomy.
- **LangGraph Studio** — owns the graph/skeleton IDE: free, framework-native. (And the API boundary *cannot* faithfully reconstruct a framework graph — so `ctx` explicitly does **not** build this; hypothesis 1's graph half is **refuted/not built**.)

Prior-art sweep context (`05-prior-art-uniqueness.md`, Space 2 — agent transcript analysis / observability CLIs): small OSS entries exist — `eunomia-bpf/agentsight` (~322★, eBPF infra-level), `GoogleCloudPlatform/BigQuery-Agent-Analytics-SDK` (~25★), `masonc15/codex-transcript-viewer` (~12★), `dkondo/agent-tackle-box` (~49★, LangGraph/LangChain debugger), `WhitzardAgent/agentir` (~0★), `kroq86/flow-xray` (~1★). **Verdict: PARTIAL / contested** — *LangSmith, Langfuse, AgentOps, Helicone, and Phoenix own live tracing.* None is the framework-agnostic, zero-config, wire-level composition+waste indictment `ctx` is. (Caveat from that doc: star counts approximate, via GitHub API 2026-05-16; some entries inferred; a manual topic scan still owed before naming — see §6.)

### The uniqueness verdict — the moat is an *intersection*, not a feature

The moat is the **intersection of three things, empty across the entire funded field**:

- **(b) per-step context DIFF** — what was added / removed / mutated between step N-1 and N (and run A vs run B). Everyone has prompt-*version* diff; **nobody** has *context-delta-per-step*.
- **(e) in-prompt composition + waste indictment** — decompose one assembled prompt into system / tool-schemas / history / RAG / memory / skills with % + absolute tokens, and *indict the waste* ("22 of 31 tools never called this run; system prompt duplicated 3×; this RAG chunk repeats in turns 2,4,5; history never pruned"). Everyone shows total token *counts*; nobody decomposes + indicts.
- **zero-config wire capture with LLM semantics** — `mitmproxy ∩ tokenizer ∩ context-budget linter`. Framework-agnostic *because* it reads the wire (LangGraph / Pydantic AI / raw SDK are identical on the wire).

Each capability **alone is weak** (full prompt text alone: everyone has it; mitmproxy has it). Defensibility lives **only** in the intersection — and that intersection is empty across the $400M-funded field because their SDK→DB→dashboard→account architecture is the structural opposite. The **OTEL GenAI spec defaulting prompt-text capture OFF** is the standing tailwind: the standard itself defers exactly the data `ctx` centers on.

Hypotheses verdict for the record (PROJECT.md §10): #4 full context text per step — **strongest / foundational** (→ v1 F2); #3 per-step add/remove diff — **confirmed; broaden** (→ v1 F3 + cross-run/CI); #2 log of actions/decisions — **confirmed; it's the spine** (→ v1 timeline model); #1 agent skeleton — **split: graph REFUTED, skillset CONFIRMED** (graph not built; skillset → F1); per-source token attribution — user didn't name it, **highest-confidence** (→ v1 F1 headline); replay/re-run — user didn't name it, **high-leverage** (→ v2).

---

## Why these decisions

Every locked decision below carries the reasoning *and* the rejected alternative. Source: PROJECT.md §4 + §3 (the discipline rule), `09-roadmap.md`, `06-better-than-big-brother.md` (the North Star), `08-decision-log.md`.

| Decision | Locked value | Why | Alternative rejected |
|---|---|---|---|
| **Language** | **Rust** | Single static binary, zero bootstrap-paradox (you can't profile a Python agent with a tool that needs the same broken Python env), calibration-league signal (uv/ripgrep/Astral lineage). | Python/Node CLI — reintroduces the dependency-hell the tool exists to expose; weaker single-binary story; off the "infra not toy" calibration. |
| **Mechanism** | Transparent **local reverse-proxy** at the provider base URL (`ctx run -- <cmd>` sets env, or manual `*_BASE_URL`) | Framework- *and* language-agnostic (the wire is identical for LangGraph / Pydantic AI / raw SDK); it is the **line's shared substrate** — `agentlock`/`guard` reuse the same interceptor. | SDK/middleware integration — framework-specific, N integrations to maintain, and it's the incumbents' exact lock-in model. Static repo scan — can't see the *actually assembled* prompt. |
| **Providers v1** | **Anthropic + OpenAI-compatible** | The default backends of LangGraph / Pydantic AI; everything else slots in via the provider-adapter trait (§8). | "All providers day 1" — scope creep; the adapter seam makes it unnecessary. |
| **Tokenizer** | **Local approximation** (e.g. o200k-class), **honestly labeled ±N%**, offline | Zero-config / offline is core identity; you must never call an API to count tokens (defeats local-first, adds latency, leaks the prompt). | API-exact token counts — violates offline/zero-config; routes the user's prompt to a vendor just to count it. |
| **Headline posture** | **Pure measurement** (facts that survive adversarial reading) + **`--deep`** drill-down | The evalint lesson: no contestable judgments ("the model *will* ignore X"). A headline that survives an adversarial reader is the only defensible one. `--deep` carries the opinionated detail. | Predictive/judgment headline ("this prompt will fail") — the exact snake-oil smell that killed evalint; anti-signal to expert audience. |
| **Persistence** | **Ephemeral by default; opt-in local SQLite** (Willison pattern) | A DB-as-product *is* Langfuse's moat and the kill-zone. Ephemeral keeps `ctx` an `htop`, not a Datadog. Opt-in SQLite enables drift-over-commits + the v2 replay substrate without becoming a store-product. | Always-on persistent store — instant death (the discipline rule); becomes "a worse Langfuse." |
| **Output** | CLI one-shot + interactive **TUI**; **`--json` everywhere** | Dashboard-fatigue evidence vindicates local-first CLI; `--json` makes it a first-class CI citizen (the *acted-upon* form of the diff). | Web dashboard / hosted UI — kill-zone; the field is a venture bloodbath in exactly that lane. |
| **Scope (Never)** | No server, account, cloud, hosted UI, eval scoring, prompt registry, APM charts; no trace-tree product, no session-replay IDE, no graph/skeleton IDE | Each of these is either owned by a funded incumbent or *is* the kill-zone. LangGraph Studio owns the graph IDE (free, framework-native); Braintrust/Phoenix own evals; prompt registry solved 5×; the API boundary can't faithfully reconstruct a framework graph anyway. | Scope expansion into observability platform — the $400M-funded bloodbath; the line's explicit kill criterion. |

**The one rule that decides everything** (PROJECT.md §3, CLAUDE.md): *"If it needs a server to run or an account to use → kill-zone (you become a worse Langfuse). If it runs as one binary against a captured prompt and produces an indictment → the moat. The day `ctx` grows a hosted dashboard / persistent-analytics product / cloud / accounts, it is dead. Be `htop`/`EXPLAIN ANALYZE`, never Datadog."*

**The North Star that gates the whole line** (`06-better-than-big-brother.md`): the master discriminator — *a reimplementation/tool is genuinely better ONLY if it wins a concrete, measurable, named dimension real users optimize for. If the only honest claim is "easier to read/understand," it is educational, not better. Hackability ≠ superiority.* `ctx`'s named axis: **tokens-of-waste surfaced per repo + drift over commits** — measurable, verifiable at second 1, and unclaimed by any incumbent. The lineage cited as proof the pattern is fundable: ripgrep, uv (10x+, **OpenAI acquired Astral Mar 2026**), ruff (~150–200x), esbuild (10–100x), vLLM PagedAttention (KV-cache waste 60–80% → <4%, up to 24x). The counterexample to avoid: nanoGPT — *genuinely better on NO dimension, educational only.*

**Why this anchor and not the earlier candidates** (`08-decision-log.md` §8): the anchor moved off `резолвер`/`рекордер` → `ctx` because `ctx` has the **best elegance score in the whole set** — read-only, zero-config, a verifiable headline number at second 1, fits every workflow with zero relearning, lower risk than bit-exact replay, *and its SQLite context log is the substrate the rest of the line composes on*. `резолвер` deferred (combinatorial-CI-farm treadmill is solo-fatal); `рекордер` was reframed honestly ("attribute drift", not "reproduce") and split into `agentlock` + `guard`. The eval-integrity direction (`evalint`) was **killed** on four independent axes (demand ≈ 0 at ~0.6% of posts; indefensible solo; wrong hiring signal; funded-bloodbath adjacent) — recorded so it is *not relitigated*.

---

## Honest limits & unverified claims

This section is the top-engineer signal: stating plainly where the case is soft. Every flag below is carried verbatim or near-verbatim from the source docs.

- **Name is almost certainly taken — unresolved.** `ctx` is a *provisional codename*, "almost certainly taken on crates.io / npm / GitHub." The collision-check + rename is the **first pre-publish gate** (PROJECT.md §11, CLAUDE.md), explicitly *not* a pre-build gate. Candidates to vet: `ctxray`, `promptscope`, `lensllm`, `assembled`, `whatissent`, `xri`. Decision owed before `git init`/publish.
- **Tokenizer is an approximation, by design.** The headline number rests on a *local approximation* tokenizer, offline, and is only honest if labeled **±N%**. The exact accuracy target and the precise ±% wording are still **open** ("fix during F0", PROJECT.md §11). The headline metric is therefore *indicative within a stated band*, not exact — and the spec demands it be labeled as such.
- **The graph half of hypothesis 1 is refuted, not deferred.** The API boundary *cannot* faithfully reconstruct a framework graph; LangGraph Studio owns that lane and is free/framework-native. `ctx` does **not** build it. Stated as a hard non-goal, not a roadmap "later."
- **Inferred / folklore flags carried from the keystone demand memo** (`07`):
  - The hiring/notice mechanism pattern ("viral-OSS-to-lab is real but a brutal base rate") is explicitly tagged **"(inference, strong)"** — and that whole §A was scoped to an *old* hire-at-Anthropic goal, now downgraded to context, not action (07 status note + decision-log §5: goal reframed — Anthropic is the *calibration standard*, not the target).
  - *">90% of the ~28M new 2025 repos never hit 100 stars"* and *"Median 100 stars in 5 days is survivorship garbage"* — base-rate folklore stated as such; the modal outcome for a new solo AI CLI is explicitly *"<50 stars, no installs, dead in 3 months."* This is the honest prior `ctx` is launched *against*, not a projection it escapes.
  - Some prior-art star counts are **approximate** (GitHub API, 2026-05-16); some entries **inferred from general knowledge**; *"No tool synthesizes eval-trust" is an inference from absence*; a **manual topic scan is still owed before any naming** (`05` caveats). A fresh prior-art sweep is explicitly **owed for the new dev-infra targets before building** (05 status note).
- **Roadmap-line numbers flagged unverified in `09`** (carried for faithfulness even though they belong to sibling tools, because the line shares the moat thesis):
  - RAG "hardest pain ~**87h**" is annotated **"(unverified/indicative; defensible: RAGGY 'half-day/change' median)"** — repeated everywhere it appears.
  - RAGAS adoption "≤1%" is annotated **"(UNVERIFIED — likely wrong; public data suggests ≥5M evals/mo. Structural-distrust thesis stands on arXiv 2504.20119 / Galileo 93%, NOT on this number)."**
- **Demand basis is recurrence + surveys, not a controlled experiment** *([INFER], stated plainly).* The strongest evidence — LangChain #912 closed *not planned*, verbatim recurrence across LlamaIndex/Pydantic AI/Claude Code/Codex, the arXiv 2503.06745 38-practitioner study, the token-bloat corpus — is *convergent issue-tracker + survey + practitioner-study* evidence. It is strong and triangulated, but it is not a randomized study of "would developers adopt `ctx`." The inference that an empty competitive intersection equals durable defensibility is *itself an inference from absence* (the same epistemic shape the prior-art doc flags) — robust given the structural argument (incumbents' business model is the opposite), but it is a structural argument, not a market proof.
- **Cross-cutting risk, acknowledged in the spec itself** (`09` + PROJECT.md §1 footnote): **vendor-native absorption** — Anthropic/OpenAI could ship native context inspection in ~12 months. Baked-in mitigation: cross-tool positioning, ship fast, sit at the framework-agnostic infra boundary, treat the tool as portfolio-complete even if later absorbed. The risk is named, not hidden.

The intellectual honesty here is deliberate: a tool whose own spec carries "why this might fail" lines, labels its headline metric ±N%, refuses to build the half it can't do faithfully, and flags its folklore as folklore — is a stronger artifact than one that overclaims.

---

## Conclusion

`ctx` is a defensible, ahead-of-time, top-tier artifact for reasons that survive an adversarial reading:

1. **It closes a demand-proven, universal pain.** Not a niche: every agent developer, every framework, paying for invisible tokens against an industry standard (OTEL GenAI) that defaults the relevant data off. The canonical ask was *filed and refused* (LangChain #912, *closed not planned*) and *recurs verbatim* across four other major projects over years — the strongest possible unmet-demand signature.

2. **Its moat is an empty intersection, not a feature.** (b) per-step context diff ∩ (e) composition + waste indictment ∩ zero-config wire capture with LLM semantics. Each alone is weak (everyone has full prompt text; mitmproxy has wire capture). The intersection is empty across a **$400M+ funded field** — and structurally must stay empty, because the incumbents' SDK→DB→dashboard→account architecture is the *opposite* of a one-binary, ephemeral, account-less tool. This is defensibility by *architecture incompatibility*, the most durable kind.

3. **It is disciplined to stay out of the bloodbath rather than enter it badly.** The one non-negotiable rule (server/account → death) is not a slogan; it is what differentiates `ctx` from "a worse Langfuse." The North Star (`06`) gates it on a *named measurable axis* — tokens-of-waste surfaced per repo + drift over commits — which it wins by construction and no incumbent can take without abandoning its business model. The lineage that proves this pattern is real and fundable (ripgrep, uv → Astral acquired by OpenAI, ruff, esbuild, vLLM) is the calibration band; nanoGPT is the explicitly-avoided counterexample.

4. **It is engineered, not just specced.** Single static Rust binary (no bootstrap paradox), wire-level mechanism that is framework- *and* language-agnostic by reading the bytes both sides agree on, an extensibility architecture (provider-adapter / tokenizer-registry / component-classifier / versioned indictment ruleset / renderer / storage seams) that lets it absorb new frameworks and providers *forever without drifting into the platform kill-zone*, and a v1 scope that is sequenced (F0 blocks; F1/F2/F3 are independent parallel views) with a v2 replay primitive whose substrate v1 already captures.

5. **It is intellectually honest where it is soft.** The name is unresolved; the headline metric is an approximation labeled ±N%; the graph half is refused outright because the boundary can't do it faithfully; the folklore is flagged as folklore; the absorption risk is named in the spec. A tool that *self-discloses its regime* is the top-0.01% signal — the same discipline that killed `evalint` on four independent axes and dropped `durable` on logically-prior axes (`08`) is the discipline that makes `ctx` the anchor.

Calibration, not slogan: "top-0.01%" here means the work survives the harshest reading of its own evidence, competition, and limits — which is exactly the test this document was written to apply to it.

---

## Sources

Every URL across the read source files, deduplicated, grouped. Named primary sources without URLs in the docs (issue numbers / memos) are listed first.

### Primary sources cited by ctx (PROJECT.md §12 / CLAUDE.md — referenced by identifier, no URL in docs)

- LangChain issue **#912** (the canonical "show the final prompt" ask, closed *not planned*)
- LlamaIndex **#7628** (verbatim recurrence)
- Pydantic AI **#4137**
- Claude Code **#10164**
- OpenAI Codex **#14642**
- Hamel Husain — evals-faq ("look at your data")
- vLLora — debug-mode ("not what you assume it's sending")
- OTEL GenAI semantic conventions (prompt/response content capture off-by-default)

### Demand / hiring-mechanism / reliability (from `07-harsh-reality-demand-and-mechanism.md`)

- https://openai.com/index/openai-to-acquire-astral/
- https://simonwillison.net/2026/mar/19/openai-acquiring-astral/
- https://huggingface.co/blog/ggml-joins-hf
- https://www.anthropic.com/news/anthropic-acquires-bun-as-claude-code-reaches-usd1b-milestone
- https://mondaymorning.substack.com/p/openclaw-and-the-acqui-hire-that
- https://alignment.anthropic.com/2025/anthropic-fellows-program-2026/
- https://www.businesstoday.in/tech-today/news/story/anthropic-ai-safety-fellowship-2026-how-to-apply-15000-funding-duration-hiring-chances-531592-2026-05-14
- https://letsdatascience.com/blog/how-to-land-a-job-at-openai-anthropic-or-google-deepmind
- https://www.sundeepteki.org/advice/how-to-get-hired-at-openai-anthropic-and-google-deepmind-in-2026
- https://stackoverflow.blog/2025/12/29/developers-remain-willing-but-reluctant-to-use-ai-the-2025-developer-survey-results-are-here/
- https://shiftmag.dev/stack-overflow-survey-2025-ai-5653/
- https://arxiv.org/html/2510.25423v1 (empirical SO agent-challenges study, 3,191 posts)
- https://arxiv.org/abs/2602.16666 (agent-reliability: 14 models / 3 providers)
- https://www.augmentcode.com/guides/debug-parallel-ai-agents
- https://temporal.io/blog/ai-reliability-is-a-decade-old-problem
- https://hackernoon.com/the-ultimate-playbook-for-getting-more-github-stars
- https://gingiris.github.io/growth-tools/blog/2026/03/30/github-stars-history-how-to-track-and-analyze-repository-growth/
- https://news.ycombinator.com/item?id=43962427
- https://www.turbodocx.com/blog/best-claude-code-skills-plugins-mcp-servers
- https://www.braintrust.dev/articles/deepeval-alternatives-2026
- https://arxiv.org/html/2502.06215v1 (LessLeak-Bench)
- https://rdi.berkeley.edu/blog/trustworthy-benchmarks-cont/

### "Genuinely better than big brother" — the North Star case studies (from `06-better-than-big-brother.md`)

- https://en.wikipedia.org/wiki/Llama.cpp
- https://mattrickard.com/lessons-from-llama-cpp
- https://burntsushi.net/ripgrep/
- https://github.com/BurntSushi/ripgrep
- https://github.com/sharkdp/fd
- https://github.com/astral-sh/ruff
- https://docs.astral.sh/ruff/
- https://toolhalla.ai/blog/openai-acquires-astral-uv-ruff-2026
- https://blog.logrocket.com/fast-javascript-bundling-with-esbuild/
- https://bun.com/
- https://blog.vllm.ai/2023/06/20/vllm.html
- https://arxiv.org/abs/2511.17593
- https://en.wikipedia.org/wiki/SQLite
- https://sqlite.org/whentouse.html
- https://scalegrid.io/blog/redis-vs-memcached/
- https://github.com/karpathy/nanoGPT
- https://github.com/karpathy/micrograd
- https://github.com/tinygrad/tinygrad
- https://github.com/modelcontextprotocol/python-sdk
- https://github.com/jlowin/fastmcp
- https://www.braintrust.dev/blog/agent-while-loop
- https://github.com/shareAI-lab/learn-claude-code
- https://www.anthropic.com/research/petri-open-source-auditing
- https://alignment.anthropic.com/2026/petri-v2/
- https://github.com/safety-research/petri
- https://github.com/UKGovernmentBEIS/inspect_ai
- https://github.com/TransformerLensOrg/TransformerLens
- https://arxiv.org/html/2511.14465v1

### Prior-art / uniqueness sweep (from `05-prior-art-uniqueness.md`)

- https://github.com/baceolus/eval_awareness
- https://github.com/microsoft/Test_Awareness_Steering
- https://github.com/tim-hua-01/steering-eval-awareness-public
- https://github.com/openai/monitorability-evals
- https://github.com/HowieHwong/Awareness-in-LLM
- https://github.com/scaleapi/propensity-evaluation
- https://github.com/uiuc-kang-lab/agentic-benchmarks
- https://uiuc-kang-lab.github.io/agentic-benchmarks/
- https://arxiv.org/html/2507.02825v1
- https://github.com/lyy1994/awesome-data-contamination
- https://github.com/microsoft/MMLU-CF
- https://github.com/xuhaoxh/infini-gram-mini
- https://github.com/princeton-pli/hal-harness
- https://github.com/UKGovernmentBEIS/inspect_evals
- https://github.com/eunomia-bpf/agentsight
- https://github.com/GoogleCloudPlatform/BigQuery-Agent-Analytics-SDK
- https://github.com/masonc15/codex-transcript-viewer
- https://github.com/dkondo/agent-tackle-box
- https://github.com/WhitzardAgent/agentir
- https://github.com/kroq86/flow-xray
- https://github.com/sanbuphy/nanoMCP
- https://github.com/sanbuphy/nanoAgent
- https://github.com/disler/aider-mcp-server
- https://github.com/karpathy/nanochat
- https://github.com/GeeeekExplorer/nano-vllm
- https://github.com/huggingface/nanoVLM
- https://github.com/gusye1234/nano-graphrag
- https://github.com/HKUDS/nanobot
- https://github.com/Nano-Collective/nanocoder
- https://github.com/jingyaogong/minimind

### Roadmap-line cross-references (from `09-roadmap.md`, flagged unverified where annotated)

- arXiv **2504.20119** + Galileo 93% (the structural-distrust evidence the RAG thesis actually stands on; the "≤1% RAGAS adoption" number is annotated UNVERIFIED — likely wrong)
- RAG "~87h" median: annotated *unverified/indicative; defensible via RAGGY "half-day/change"*

---

*This document is derived solely from `ctx/docs/PROJECT.md`, `ctx/CLAUDE.md`, and `caliper/docs/{05,06,07,08,09}`. No web research or new sources were added. Numbers, percentages, URLs, and caveats are preserved as recorded; inferences are marked.*
