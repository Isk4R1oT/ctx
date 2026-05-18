# ctx — UX-OVERHAUL-BRIEF (binding execution contract, D-017)

> BINDING for the onboarding-UX overhaul. Adds the phased build
> sequence; does NOT restate the spec. On conflict, `docs/PROJECT.md` +
> `docs/DECISIONS.md` are canonical. Build on main HEAD `b73dabb`
> (C1–C7 / D-009..D-016 shipped+verified). Goal: connect in seconds /
> a couple of commands, **zero functionality lost**, strictly inside
> the locked moat. Amends D-001's CLI surface (recorded as D-017).

## 0. Root cause (why onboarding is "complex & depends on what connects to what")

`run.rs::origin_of()` (line 23) reduces the upstream to `scheme://host`
— it **destroys the upstream path**. `execute()` then injects a
synthetic `OPENAI_BASE_URL=…/v1`. Result: any provider whose real base
has a path prefix (OpenRouter `/api/v1`, Azure
`/openai/deployments/…`, sub-path gateways) breaks, and the user must
manually re-supply the upstream via `CTX_UPSTREAM_*` **and** a client
`/api/v1` hack **and** know the client must read `OPENAI_BASE_URL`.
One bug (path truncation) + one missing layer (auto-resolve upstream)
= ~all the friction. This is correctness, not docs.

## 1. Best-practice model (grounded; Stripe CLI / LiteLLM / Helicone / ngrok / uv)

Layered, default = "configure nothing":
- (a) zero-config: resolve provider+upstream from the request itself;
- (b) one obvious flag for the rest (`--to` / `--provider`);
- (c) instant offline proof it works (`ctx demo`, no key, ~2s);
- (d) self-diagnose when nothing captured (actionable reason, never a
  silent empty result — the Stripe "terminal explains" pattern);
- (e) single static binary, one install.

## 2. Hard invariants (HALT & report if any breaks)

- **Zero functionality lost.** Every C1–C7 / F0–F3 behaviour and the
  D-005 `--json` contract stay byte-identical (snapshots verified).
- **Moat intact.** No server, account, dashboard, scoring, judge;
  evalint stays KILLED. Registry/flags/demo are pure routing +
  pure-measurement + offline fixtures only. Single static binary.
- **Verbatim forwarding.** Forward = resolved-upstream-base (FULL,
  path-preserving, trailing-slash-trimmed) + the client's request path
  VERBATIM; the injected `*_BASE_URL` is the proxy ROOT (no synthetic
  `/v1`) so the client's own path construction is preserved exactly.
- **Auto-resolution never guesses silently.** Unknown ⇒ explicit
  actionable diagnostic (P6), never a wrong upstream / fabricated
  capture.
- rust-cc discipline per phase: TDD red-first proven, compiler-truth
  loop, `just check`/`just test` green, `cargo deny`/`machete` ok,
  cargo-mutants `--in-diff` **0 missed on a REAL non-vacuous baseline**
  (eliminate by construction / exact-boundary pins; equivalent mutants
  removed structurally), real e2e (green ≠ works), atomic commit per
  phase, honest DECISIONS/RUSTCC record. No false green.
- MITM-via-system-proxy (hardcoded-base clients) needs a local CA ⇒
  against zero-config/seconds ⇒ OUT of the default; if ever added it is
  an opt-in, honestly-labelled power mode, never the headline.

## 3. EXECUTE (phased; each EXIT green before the next)

- **P1 — Preserve the full upstream path (root-cause fix).**
  `origin_of`→`base_of` (keep `scheme://host[:port]` + path, trim a
  trailing `/`, reject `null`); `ProxyState` carries full bases; proxy
  `forward()` builds `base + incoming_path` verbatim; inject the proxy
  ROOT (no `/v1`) for the resolved provider(s). EXIT: a path-
  composition unit test + the wire_capture integration test prove
  OpenRouter `/api/v1`, Azure `/openai/deployments/X`, a sub-path
  gateway, plain OpenAI, and Anthropic all forward correctly;
  F0/F1/F2/F3 + all snapshots byte-identical; clippy 0; nextest green;
  cargo-mutants 0-missed real baseline; a REAL OpenRouter run
  **without** the `CTX_UPSTREAM_*`/`/api/v1` hack returns a real 200
  and F1 decomposes. Atomic commit; record in D-017.
- **P2 — Provider registry + auto-resolve.** A tiny offline table
  (openai/anthropic/openrouter/azure/google/groq/mistral/together/
  fireworks/deepseek → full base) keyed by key-prefix (`sk-ant-`,
  `sk-or-`, `sk-`, `AIza`, …) + path + headers; used only when the
  upstream is not explicitly given. Pure data + pure resolution. EXIT:
  unit table pinned (`black_box`), `ctx run -- <openai|anthropic|
  openrouter client>` works with NO env/flag; real e2e; mutants 0.
- **P3 — `--to <url>` / `--provider <name>` flags** (cli.rs; the
  D-001 amend). Explicit upstream, full-path preserved; `--provider`
  resolves via the P2 registry. Deprecate-but-honor `CTX_UPSTREAM_*`
  (kept working, documented as legacy). EXIT: flag parsing tests; the
  before→after one-liner works; mutants 0.
- **P4 — Broad env injection.** Inject the full recognised set
  (`OPENAI_BASE_URL`, `OPENAI_API_BASE`, `ANTHROPIC_BASE_URL`, and the
  other common SDK base envs) so SDK users "just work". EXIT: tests
  per injected var; zero-regression.
- **P5 — `ctx demo`.** New subcommand: an in-process echo upstream +
  a built-in multi-turn (+image +retry +param/header/system change)
  scenario that fires **all C1–C7** offline, no key, ~2s; prints the
  inspect commands. EXIT: `ctx demo` deterministically shows every
  signal; snapshot-pinned; mutants 0; doc updated (collapses the
  manual runbook to one command).
- **P6 — Zero-capture self-diagnostic + install.** On 0 captures /
  all-unparsed / misroute, print the precise actionable reason+fix
  (pure factual, no judgement) instead of an empty composition;
  `just install` / a documented one-line install; `ctx` on PATH.
  EXIT: a forced-zero-capture test asserts the diagnostic; install
  documented; final combined re-verify (all phases) green.

## 4. Source of truth

CANONICAL `docs/PROJECT.md` (locked decisions, §4 mechanism, §8 seams,
§9 non-goals), `docs/DECISIONS.md` (D-001 CLI surface — **amended here
by D-017, not relitigated**; D-005 `--json`; D-009 store/headers;
D-010..D-016 signals), `docs/11-cli-design-system.md` (mandatory CLI
aesthetic), `/Users/igor/Projects/rust-cc/COMPILER-TRUTH.md`. Local git
only; never publish/rename.

> The plan = §0–§2 + this brief; PROJECT.md/DECISIONS.md canonical.
> Execute §3 P1→P6 with §2 invariants intact. Make onboarding seconds
> WITHOUT losing any functionality or leaving the moat; halt honestly
> on any breach.
