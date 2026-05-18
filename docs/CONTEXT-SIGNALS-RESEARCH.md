# ctx — context-signal differentiation research

> What ELSE `ctx` can usefully measure ABOUT THE CONTEXT/PROMPT purely from the
> LLM-API wire — and what the funded SDK→DB→dashboard field structurally cannot.
> Scope-bound by PROJECT.md §3/§9, DECISIONS.md D-001..D-009, the locked moat,
> and the evalint kill (PURE MEASUREMENT only in the headline). Soft claims are
> flagged `[INFER]`; URLs in §(e).
>
> **Status (current state).** Implemented as pure-measurement F1
> signals (all 2026-05-18): C1 `cache-prefix-break` (D-010), C2
> `component-drift` (D-011), C3 context-window headroom & slope
> (D-012), C6 `request-replayed` (D-013), C4 `param-drift` (D-014 —
> the additive `Assembled.sampling` field + the determinism-surface
> fact; `agentlock`'s non-determinism *attribution* stays EXCLUDED),
> C5 `non-text-payload` (D-015 — the additive `Assembled.non_text`
> field + a DISTINCT component/indictment by EXACT wire byte weight +
> block counts; base64 NOT decoded, the per-image token estimate
> OMITTED ENTIRELY to stay strictly pure). C1/C2/C3/C4/C5/C6 are now
> all implemented. **C7 deferred by decision (2026-05-18):** ranked LOW +
> the honest `store.rs`-does-not-persist-request-headers substrate
> blocker (shared F0 surface with agentlock/guard) — not built
> rather than silently half-shipped.

---

## (a) What F0/F1/F2/F3 already cover

F0 is the wire-proxy substrate: it captures the **verbatim request body**
byte-for-byte (gzip/zlib/zstd/brotli decoded at the boundary, D-009), buffers
the response, redacts auth headers, detects provider (Anthropic Messages /
OpenAI-compat Chat Completions), and models a **step timeline** with opt-in
SQLite. F1 (headline) decomposes the *focus* step's assembled prompt into
`system` / `tool-schemas` / `history` (% + token sums) and emits five
pure-measurement indictments — `unused-loaded-tools`, `duplicate-block`,
`repeated-block-across-turns`, `unpruned-history`, `preamble-repay`. F2 is the
verbatim per-step pager; F3 is the line-level per-step context diff (added /
removed / retained lines + token deltas, default N vs N-1). Note the live
limit: the adapter extracts `model` / `system` / `messages` / `tools`, and
(C4/D-014) the tracked **sampling/decoding fields** into the canonical
`Assembled` (the additive `sampling` field — `temperature`/`top_p`/`top_k`/
`max_tokens`/`stop`/`stop_sequences`/penalties/`seed`/`response_format`/
`tool_choice`, present-only); `cache_control`, `metadata`, response-`usage`
are still captured verbatim in the body/response but **not yet surfaced** as
signals. Response headers are **not persisted** by
`store.rs` (D-009 note) — relevant to several candidates below.

---

## (b) Competitive teardown — the "what is actually in the context window" axis

Narrow axis only: can the tool see the *actual assembled prompt bytes*, *per-step
composition/waste*, *per-step drift*, and *request-side cache economics* (why a
prefix broke, not just what the provider billed). Generic tracing/cost/eval
features are out of scope by construction.

| Tool | Actual prompt bytes? | Per-step composition + waste indictment | Per-step context drift (delta) | Request-side cache economics (prefix byte-stability) | Structural reason |
|---|---|---|---|---|---|
| **Langfuse** | Only if SDK instrumented & content-capture on (OTEL GenAI defaults it OFF) | No (token *totals* + cost; no source decomposition, no waste indictment) | Prompt-*version* diff only (registry), not per-step context delta | No — reads `cache_read/creation_input_tokens` from the **response usage** (post-billing). Known double-count bug (#12306) | SDK→DB→dashboard; sees the SDK's view, not the wire; cache = a billed number, not a cause |
| **LangSmith** | LangChain-internal serialization (not raw wire) | No (run tree + token counts; no waste decomposition) | Node-state diffs — **framework graph state**, SDK-bound, not wire bytes | No (response usage only) | Deepest *only* inside LangChain/LangGraph; account-gated SaaS |
| **Helicone** | Yes (it is a proxy) | No — spends the proxy on routing/cache/cost dashboards | No per-step context delta | Partial: tracks cache **hit/miss + savings** from response/headers — still the billed outcome, never *which prefix byte changed* | Proxy accepted, but the product is the gateway+dashboard+account, not prompt anatomy |
| **Arize Phoenix** | Via OTEL spans (content opt-in) | No (eval/RAG-centric) | No (drift = data/embedding drift, not context delta) | No | OTEL→server→eval platform; the spec it rides defaults prompt content OFF |
| **Braintrust** | Eval dataset I/O, not wire | No | No (experiment diffs, not context delta) | No | Eval/experiment platform, account+DB |
| **Traceloop / OpenLLMetry** | OTEL spans, content opt-in/off | No | No | No (usage attributes only) | OTEL emitter → backend; same content-off default tailwind |
| **Portkey** | Yes (gateway) | No (gateway analytics/cost) | No | Partial: cache analytics from gateway, billed-outcome view | Gateway product, account/keys |
| **Lunary / Honeyhive** | SDK-instrumented | No | No (version/eval diffs) | No | SDK→DB→dashboard |
| **vLLora (debug mode)** | Yes — "the full request payload … not what you assume" | No (shows payload; no decomposition/indictment/cross-step) | No | No | Closest in spirit, but single-request inspect; no waste/drift/cache-stability layer |
| **DIY: mitmproxy + tokenizer** | Yes (raw bytes) | No (no LLM semantics, no cross-step model) | No (no timeline/diff model) | No (no prefix-stability logic) | Generic HTTP tool; every semantic must be hand-built |
| **Native OpenAI/Anthropic logging** | Provider-side, your account only | No | No | No (you get the usage object, never a cross-call prefix analysis) | Per-vendor, siloed, post-hoc, no cross-provider/cross-step view |
| **`ctx`** | **Yes — verbatim wire bytes, framework-agnostic, zero-config** | **Yes (F1)** | **Yes (F3)** | **Structurally yes (request-only, candidate C1 below) — nobody else's architecture can** | One local binary against the captured wire; no SDK/DB/account; cache *cause* lives in the request prefix it already has byte-exact |

The decisive structural fact: **every incumbent's cache view is the response
`usage` object** — what the provider *billed* after the fact. The *cause* of a
cache miss is a byte difference in the **request prefix** across consecutive
calls. That is request-only, deterministic, and `ctx` already holds both
request bodies byte-exact in the timeline. The SDK→DB→dashboard field cannot
take this without becoming a wire proxy with a cross-step model — i.e. abandoning
its architecture. This is the same "empty intersection by architecture
incompatibility" the moat already rests on, extended one axis.

---

## (c) Ranked NEW signal candidates

Each: definition · why wire-only+local makes it uniquely possible · why funded
competitors structurally can't/won't · pure-measurement? · fits moat/non-goals ·
request-only or needs response/SSE · feasibility · priority.

### C1 — KV/prompt-cache prefix-stability indictment  ·  **PRIORITY: HIGH**

- **Definition.** Across consecutive same-endpoint requests, compute the length
  (bytes + approx tokens) of the **longest identical leading prefix** of the
  cache-relevant region, in the provider's documented cache-prefix order
  (Anthropic: tools → system → messages; OpenAI: serialized prefix, hits in
  128-token increments, ≥1024-token floor). Report the **first byte offset
  where the prefix diverged** and which region it fell in (tools / system /
  first-N history). New indictment: `cache-prefix-broken` ("prefix stable for N
  bytes / ~T tokens, then diverged inside <region> at offset X — ~T tokens
  re-paid at full input price that a stable prefix would have cached").
- **Why wire-only + local uniquely possible.** Requires the **verbatim,
  byte-exact request body of two consecutive calls in one place**. `ctx` is the
  only thing in the field holding that (F0 already does). It is computed from
  the request alone — no API call, no tokenizer-exactness needed for the
  *byte* claim (the headline is byte-equality; the token figure is the existing
  ±N% estimate, clearly labeled).
- **Why funded competitors can't/won't.** Their cache view is the **response
  usage object** (`cache_read_input_tokens` / `cached_tokens`) — the *billed
  outcome*, available only after the call and not explanatory. They have no
  cross-request byte-prefix model because their data is SDK-serialized spans in
  a DB, not consecutive raw wire bodies. Building C1 means becoming a wire
  proxy with a step timeline — their architecture's opposite. Helicone/Portkey
  get closest (gateways) yet still only surface hit/miss + $ saved, never
  "prefix broke at byte X inside the tool block."
- **Pure measurement?** YES (headline = byte-equality + longest-common-prefix
  length; the token figure rides the existing labeled ±N% tokenizer; no
  judgment, no "the model will ignore X"). The provider-specific *prefix
  ordering* (tools→system→messages) is documented vendor behavior, not a guess
  — but anything beyond byte-equality (e.g. "this WOULD have cached") is
  contestable and stays behind `--deep` as a non-asserted estimate.
- **Fits moat/non-goals?** YES — deepens (per-step diff ∩ waste indictment ∩
  wire capture) exactly on its strongest axis. It is the single
  highest-consequence, most-misdiagnosed waste in agent context (cache read =
  0.1× input price; a broken prefix silently 10×s cost). Not a dashboard, not
  routing, not a score.
- **Request-only or needs response/SSE?** **Request-only** for the headline
  byte claim (the moat-pure part). An *optional* `--deep` cross-check against
  the response `usage` (did the provider's billed cache-read corroborate our
  byte-derived prediction?) needs the response — already buffered by F0, no SSE
  needed.
- **Feasibility.** High. It is a longest-common-prefix over two `String`s the
  timeline already holds, plus a region classifier `ctx` already has (system /
  tools / history). Reuses the F1 indictment seam (ruff-style ruleset, §8).
- **Honest caveat `[INFER]`.** The exact internal canonicalization a provider
  applies before hashing (whitespace, JSON key order) is not fully public;
  `ctx` measures **wire-byte** prefix stability, which is the necessary
  condition the engineers themselves are told to optimize ("byte-identical
  prefix"). State this regime explicitly in the indictment text — it is a
  *necessary-condition* measurement, not a hit-rate prediction.

### C2 — System-prompt / tool-schema drift fingerprint across steps  ·  **PRIORITY: HIGH**

- **Definition.** Per session, hash (and size) the `system` block and **each
  named tool schema** at every step. Emit `system-prompt-drift` /
  `tool-schema-drift` when a same-named component's bytes change mid-session,
  with the step index where it changed and the byte/token delta. Distinct from
  F1 `preamble-repay` (which counts *identical* re-payment) — this is the
  opposite failure: a component the engineer believes is stable silently
  **mutates** between turns (the #1 cause of both cache invalidation and
  "forgotten instruction at step 8").
- **Why wire-only + local uniquely possible.** The framework mutates this
  *below the user's code*; only the wire shows the truth, and only a tool that
  holds all steps can diff a component against its own earlier self.
- **Why funded competitors can't/won't.** They have prompt-*version* diff (a
  registry concept: v1 vs v2 of a managed prompt) — **not** "the same logical
  component changed bytes between step 3 and step 4 of one live run." That
  needs the per-step wire timeline they don't have.
- **Pure measurement?** YES — hash equality + byte/token deltas + step index.
  No judgment.
- **Fits moat/non-goals?** YES — squarely in per-step context diff ∩ waste.
- **Request-only or needs response?** Request-only.
- **Feasibility.** High. The component segmentation already exists in
  `compose.rs`; this is a per-component cross-step equality check, a sibling of
  `preamble-repay`. Same indictment seam.
- **Caveat.** Tool-schema identity is keyed by tool *name*; a renamed tool
  reads as remove+add (correct, but worth stating in `--deep`).

### C3 — Context-window headroom & growth-rate slope  ·  **PRIORITY: MED**

- **Definition.** Per step: `prompt_tokens` (already computed) as a fraction of
  the model's known context window (the model id is on the wire; window sizes
  are a static, offline table — a registry seam, §8). Plus the **tokens/turn
  slope** across the session (least-effort: last−first over turns, or per-step
  deltas). Report "step 9: ~78% of the 200k window; growing ~6.2k tok/turn;
  ~3.4 turns of headroom at this rate."
- **Why wire-only + local uniquely possible.** Needs the per-step assembled
  token series, which is the F0 timeline. Offline static window table keeps it
  zero-config (no API).
- **Why competitors can't/won't.** They show per-call token totals; none plots
  the *assembled-context* growth slope vs the window from the wire because they
  don't model the prompt as a per-step series of assembled bytes.
- **Pure measurement?** YES for the byte/token series and the linear slope
  (arithmetic on the existing ±N% token estimate, labeled). NOT pure if it
  said "you will overflow / the model will truncate" — that projection is
  contestable and must stay a *neutral arithmetic extrapolation behind
  `--deep`*, phrased as "at the observed mean rate", never asserted as fate.
- **Fits moat/non-goals?** YES if disciplined to the arithmetic; the "headroom"
  framing must not become a predictive judgment (evalint smell). Headline =
  the fraction and the measured slope only.
- **Request-only or needs response?** Request-only (window size = static table
  keyed by the wire model id).
- **Feasibility.** High mechanically; the discipline risk (not the code) is the
  reason this is MED not HIGH — the static window table is a small maintained
  registry (acceptable, §8 seam) and the extrapolation must be worded as pure
  arithmetic.
- **Caveat `[INFER]`.** Context-window sizes drift per model release; the table
  is a maintained approximation and must be labeled like the tokenizer ±N%.

### C4 — Sampling / decoding parameter drift mid-session  ·  **PRIORITY: MED · IMPLEMENTED (D-014, 2026-05-18)**

- **Definition.** Track `temperature`, `top_p`, `top_k`, `max_tokens`,
  `stop`/`stop_sequences`, presence/frequency penalties, `seed`,
  `response_format`/`tool_choice` across steps. Emit `param-drift` when any
  changes within one logical session, naming the field, the old→new value, and
  the step. Pure determinism-surface fact (this is also exactly the primitive
  `agentlock` will consume — it shares F0).
- **Why wire-only + local uniquely possible.** These are request fields the
  framework often sets/overrides invisibly; the wire is ground truth.
- **Why competitors can't/won't.** Incumbents log params per call but do not
  **assert a cross-step drift fact** on the assembled-context timeline; it is
  not their unit of analysis (they think in spans/cost, not "field X changed at
  step 5").
- **Pure measurement?** YES — value equality across steps, named field, step
  index.
- **Fits moat/non-goals?** YES — it is the determinism-surface, the shared F0
  substrate the line composes on; `ctx` surfacing it as a context fact is in
  scope (and feeds `agentlock` later — explicitly NOT building `agentlock`'s
  lockfile here).
- **Request-only or needs response?** Request-only.
- **Feasibility.** High — currently these fields are parsed-then-discarded by
  the adapter; surfacing them is additive to `Assembled` behind the existing
  component/indictment seams, no architecture change.
- **Caveat.** Must stay a *reported fact*, not "this drift caused
  non-determinism" (that claim belongs to `agentlock`'s scoped framing, and
  even there is "attribute", never "reproduce").

### C5 — Multimodal / non-text payload weight attribution  ·  **PRIORITY: MED · IMPLEMENTED (D-015, 2026-05-18)**

- **Definition.** Detect image/audio/file blocks in the wire body (base64 data
  URIs, `image`/`input_audio` content parts) and attribute their **byte
  weight** (and a labeled, coarse token-estimate where a documented
  image-token rule exists) as a distinct F1 component, instead of the bytes
  silently inflating "history". Indictment: `non-text-payload-weight` ("step 4:
  2.1 MB of inline image data = ~N est. tokens, 38% of the assembled body").
- **Why wire-only + local uniquely possible.** The base64 payload only exists
  on the wire; SDK spans usually elide or summarize it.
- **Why competitors can't/won't.** Content capture is OFF by default in OTEL
  GenAI; even when on, large media is typically truncated/dropped by the SDK
  exporter — the opposite of `ctx`'s verbatim capture.
- **Pure measurement?** YES for **bytes** and block counts. The token estimate
  for images is provider-formula-dependent and must be labeled estimate-only
  (or omitted from the headline, bytes-only).
- **Fits moat/non-goals?** YES — a real, currently-invisible slice of "what's
  in the context window."
- **Request-only or needs response?** Request-only.
- **Feasibility.** Med — needs content-part type detection in the adapter
  (additive); image-token formulas are a small per-model registry (§8 seam) or
  omitted to stay strictly pure.
- **Caveat.** Keep the headline byte-based; the token figure for media is the
  weakest tokenizer regime — label it harder than text.

### C6 — Same-body re-send / retry-replay detection  ·  **PRIORITY: MED**

- **Definition.** Emit `request-replayed` when two steps have **byte-identical
  request bodies** (often a retry after a 429/5xx, or an idempotent re-issue),
  reporting the count and the duplicated token weight (a real, re-billed cost
  the engineer usually cannot see). With responses (F0 buffers them), annotate
  the status of the replayed attempts (e.g. "step 5 == step 4 body; step 4
  returned 529").
- **Why wire-only + local uniquely possible.** Retries are emitted by HTTP
  client/framework layers below the user's code; only the wire shows the
  identical re-send, and only a cross-step holder can detect the duplicate.
- **Why competitors can't/won't.** Retry storms are discussed as a *tracing*
  concern (count retry spans); none asserts "this exact assembled prompt was
  re-sent verbatim, here is the duplicated token cost" from wire byte-equality.
- **Pure measurement?** YES — full-body byte-equality + count (+ status from
  the buffered response).
- **Fits moat/non-goals?** YES — adjacent to `repeated-block-across-turns` but
  whole-body, a distinct waste class. Borders `guard`'s territory (loop/cost
  breaker) — `ctx` only *reports the fact*, never intervenes (no
  circuit-breaker here; that is `guard`).
- **Request-only or needs response?** Request-only for the core fact; status
  annotation uses the already-buffered response.
- **Feasibility.** High — a hash-equality pass over `request.body`.
- **Caveat.** Streaming-vs-buffered does not affect this (request-side).

### C7 — Header-derived deterministic facts (provider / model / beta flags)  ·  **PRIORITY: LOW**

- **Definition.** Surface request-header facts already on the wire: declared
  model, `anthropic-version`, `anthropic-beta` feature flags, declared
  `Content-Encoding`, declared `accept-encoding` — as a small factual
  determinism panel; emit `beta-flag-drift` / `api-version-drift` if they
  change mid-session.
- **Why wire-only + local uniquely possible.** Headers are stripped by SDK
  abstractions; the proxy sees them raw.
- **Why competitors can't/won't.** Not their unit of analysis; SDK spans
  rarely carry raw request headers.
- **Pure measurement?** YES — verbatim header values + cross-step equality.
- **Fits moat/non-goals?** YES, minor — a cheap honest fact panel.
- **Request-only or needs response?** Request-only. **BLOCKER for the
  persisted/`ctx open` path:** `store.rs` does **not** persist request headers
  (D-009 explicitly notes headers are not stored; `load()` replays with `&[]`).
  So C7 works on a *live* `ctx run` but is **blind post-hoc** until a
  schema/seam change persists a header allowlist (auth already redacted by F0).
  This persistence gap is the honest reason C7 is LOW, not the value.
- **Feasibility.** Live: trivial. Post-hoc: needs a header-persistence change
  to `store.rs` (additive column / allowlist) — a small but real F0-substrate
  change shared with `agentlock`/`guard`; flag it, do not silently fold in
  (D-006/D-009 discipline).

### Lower-tier / partially-covered (named, not ranked separately)

- **Duplicated/near-duplicated RAG block detection (deeper).** F1 already does
  *verbatim* repeated-block across turns. A deeper *near*-duplicate (e.g.
  normalized-whitespace or shingled overlap) is **contestable** the moment it
  is "near" rather than "exact" — it may live behind `--deep` as a labeled
  similarity figure, **never** in the headline. Mostly covered; the exact-match
  case is done. LOW marginal value, real discipline risk.
- **Message-role/position distribution shift.** Countable (role histogram per
  step, shift across steps) and pure, but **low signal** — it rarely localizes
  a real bug better than C2/C3 and risks looking like a stats dashboard. LOW.
- **Multi-provider prompt divergence (same logical step to 2 providers
  differs).** Genuinely wire-only and pure (byte/■token diff of two captured
  bodies), but it needs a reliable notion of "the same logical step" across
  providers, which the wire does **not** give for free — pairing is heuristic,
  and a heuristic pairing in the headline is the kind of contestable inference
  the project rejects. Defer; if ever built, `--deep` only, with the pairing
  basis stated. LOW/▸RESEARCH.

---

## (d) Explicitly EXCLUDED ideas (violate locked constraints)

- **Any cache-hit-rate *prediction* or "this prompt will/won't cache" verdict.**
  evalint-class judgment. C1's headline is byte-prefix length only; a hit-rate
  claim is excluded from the headline by construction.
- **"The model will ignore / forget X at step N."** The lost-in-the-middle /
  forgotten-instruction framing is real demand but is a **quality prediction** —
  KILLED with evalint. `ctx` may show *where* a block sits and *that* it
  drifted (C2), never that the model will ignore it.
- **LLM-judge / semantic similarity scoring of context blocks.** No judge, no
  score, no embedding similarity in the headline (RAG near-dup deferred to
  `--deep` as a labeled number at most).
- **Reconstructing the agent graph / trace-tree / skeleton from the wire.**
  REFUTED (PROJECT.md §9/§10, D-001). None of C1–C7 reconstructs structure;
  all are per-step or cross-step facts on captured bytes.
- **API-exact token counts (calling a count endpoint).** Violates
  offline/zero-config; every token figure here rides the existing labeled ±N%
  offline tokenizer. The byte claims are exact; the token figures are not, and
  say so.
- **Anything needing a server / account / hosted store / dashboard / retention.**
  Kill-zone. C1–C7 are one-binary, ephemeral-by-default computations; C7's
  post-hoc gap is a *local SQLite schema* question, never a hosted store.
- **Acting on the signal (rate-limit, breakpoint-insertion, circuit-break,
  routing).** C1 says "the prefix broke at byte X"; it must NOT insert
  `cache_control` for you, throttle, or break the loop — that is `guard` /
  out of scope. `ctx` indicts; it does not intervene or remediate.
- **`ctx open` header-dependent signals as a v1 headline.** C7 (and any
  future header-derived indictment) cannot be a *post-hoc* headline until
  `store.rs` persists headers — surfaced as an honest blocker, not silently
  shipped half-working (D-009 discipline).

---

## (e) Honest unknowns, `[INFER]` flags, and sources

**Unknowns / inference flags**

- `[INFER]` Provider cache **canonicalization** internals (pre-hash whitespace/
  key-order normalization) are not fully public. C1 measures **wire-byte**
  prefix stability — the *necessary condition* providers explicitly tell
  engineers to satisfy ("byte-identical prefix"). It is honest as a
  necessary-condition measurement, dishonest if sold as a hit-rate predictor;
  the indictment text must state this regime (same discipline as the ±N%
  tokenizer label).
- `[INFER]` Context-window sizes (C3) and image→token formulas (C5) drift per
  model release; both are small maintained offline registries (§8 seam) and
  must carry an approximation label like the tokenizer.
- `[INFER]` OpenAI's documented cache mechanics (≥1024-token floor, 128-token
  increments, `prompt_cache_key` routing-stickiness, ~15 req/min/prefix
  guidance) are vendor-stated behavior, not independently verified by `ctx`;
  C1's headline does not depend on them (byte-prefix only) — they inform only
  the `--deep` explanatory text.
- **Verified by code-read, not assumed:** F0 persists request bytes but **not
  request headers** (`store.rs` schema; `load()` uses `&[]`) — this is the
  concrete, code-grounded blocker that makes C7 LOW. The adapter parses then
  **discards** sampling/cache_control/metadata fields today — C4 is therefore
  additive, not a re-architecture.
- The competitive teardown rows are grounded in primary docs/issues below;
  where a tool's internal roadmap is unknown the claim is restricted to
  *current public capability* and the *structural* (architecture) argument,
  which is the durable one and the same epistemic shape as the existing moat.

**Sources (primary preferred; current month May 2026)**

- Anthropic prompt caching (cache_control breakpoints, tools→system→messages
  prefix order, ~20-block lookback, model token floors):
  https://platform.claude.com/docs/en/build-with-claude/prompt-caching
- Anthropic response usage cache fields (`cache_creation_input_tokens`,
  `cache_read_input_tokens`, 0.1× read price):
  https://platform.claude.com/docs/en/build-with-claude/prompt-caching ·
  https://startdebugging.net/2026/04/how-to-add-prompt-caching-to-an-anthropic-sdk-app-and-measure-the-hit-rate/
- OpenAI prompt caching (exact-prefix, 1024-token floor, 128-token increments,
  `prompt_cache_key`, 60→87% case, ~15 req/min/prefix):
  https://developers.openai.com/api/docs/guides/prompt-caching ·
  https://developers.openai.com/cookbook/examples/prompt_caching_201
- KV-cache prefix-stability engineering (byte-identical prefix, JSON key order,
  tool-order, truncation breaking the prefix):
  https://ankitbko.github.io/blog/2025/08/prompt-engineering-kv-cache/ ·
  https://sankalp.bearblog.dev/how-prompt-caching-works/ ·
  https://github.com/NousResearch/hermes-agent/issues/13631
- Langfuse cache-token tracking + the response-usage double-count bug
  (evidence incumbents read billed response usage, not request prefix):
  https://langfuse.com/docs/observability/features/token-and-cost-tracking ·
  https://github.com/langfuse/langfuse/issues/12306 ·
  https://github.com/orgs/langfuse/discussions/4858
- Helicone caching analytics (hit/miss + savings = billed outcome):
  https://docs.helicone.ai/features/advanced-usage/caching
- LangSmith node-state diffs are framework-graph, SDK-bound:
  https://www.digitalapplied.com/blog/agent-observability-platforms-langsmith-langfuse-arize-2026
- mitmproxy LLM use (raw bytes, zero LLM semantics — the DIY baseline):
  https://dzlab.github.io/genai/2025/06/07/mitmproxy-zed/ ·
  https://news.ycombinator.com/item?id=46799898
- Retry/idempotency framing as a *tracing* concern (no wire byte-equality
  replay fact — C6 gap in the field):
  https://www.buildmvpfast.com/blog/idempotent-ai-agent-retry-safe-patterns-production-workflow-2026
- OTEL GenAI content-capture OFF-by-default (the standing tailwind) — carried
  from PROJECT.md §3 / RESEARCH.md, not re-fetched here.

---

*Derived from the ctx codebase (compose/adapter/timeline/proxy/diff/view/store/
cli), PROJECT.md, RESEARCH.md, DECISIONS.md D-001..D-009, and caliper docs
08/09/11, plus the May 2026 web sources above. Settled decisions are not
relitigated; soft claims are flagged; byte claims are exact, token figures ride
the labeled ±N% offline tokenizer.*
