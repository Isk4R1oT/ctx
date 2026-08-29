# ctx

**`htop` for LLM prompts.** A single static Rust binary that sits as a transparent
proxy at the model-API boundary and shows you the prompt your agent *actually
sent* — its composition, its waste, and the verbatim bytes.

No SDK. No server. No account. No cloud. Wrap the command you already run:

```bash
ctx run -- python my_agent.py
```

## What it finds

Real output from an agent with three tool definitions:

```
> composition step 0                        146 tokens
component  system                            12   8%
component  tool-schemas                     123  84%
component  history                           11   7%
waste      unused-loaded-tools    wasted_tokens=123
           3 of 3 loaded tools never called this run
summary: 146 tokens, 1 finding(s)  (offline o200k approximation, ±10%)
```

84% of that prompt is tool schemas the model never touched. You cannot see this
from your code — the number depends on which tools loaded and which the model
actually used, and it changes every run.

Other findings it reports:

| finding | what it means |
|---|---|
| `unused-loaded-tools` | tool schemas paid for and never called |
| `unpruned-history` | conversation carried forward turn after turn |
| `preamble-repay` | the system prompt paid for again on every step |

## Why a proxy and not an SDK

Frameworks assemble the prompt internally. LangChain, LiteLLM, Pydantic AI and
Instructor each build a different request from the same-looking code — and none
of them hands you the final bytes. On the wire they are identical, so that is
where `ctx` looks.

The same code through a raw SDK and through LangChain goes out differently:

```
raw SDK     {"role":…,"content":…}   "max_tokens":60
LangChain   {"content":…,"role":…}   "max_completion_tokens":60, "stream":false
```

Key order changes the bytes. Prefix caches compare bytes.

## Verified

Each of these was run live against a real model API, with no per-framework
configuration:

| | |
|---|---|
| raw `openai` SDK | ✅ |
| LangChain | ✅ |
| LiteLLM | ✅ |
| Pydantic AI | ✅ |
| Instructor | ✅ |
| Anthropic Messages API | ✅ |
| Google (OpenAI-compatible endpoint) | ✅ |
| Google Gemini *native* API | ❌ `contents` / `system_instruction` shape is not parsed |
| streaming responses | ✅ |
| child exit codes (0 / 1 / 3 / 42) | ✅ propagated |
| upstream 401 | ✅ capture still works |

`--provider google` resolves to Google's **OpenAI-compatible** endpoint, which
is parsed like any other. The *native* Gemini API (`:generateContent`, with
`contents` / `system_instruction` / `function_declarations`) is a different
shape and yields `no captured prompt`. Two wire formats are understood:
OpenAI-compatible and Anthropic Messages.

## Usage

```bash
ctx run -- <your command>                  # infer upstream from env
ctx run --to https://api.deepseek.com -- <cmd>   # explicit upstream
ctx run --provider anthropic -- <cmd>      # known-provider shortcut
ctx run --save run.db -- <cmd>             # keep the session

ctx view run.db     # the verbatim assembled prompt of one step
ctx diff run.db     # what changed between step N and N-1
ctx --json run -- <cmd>                    # machine-readable
```

`ctx` sets the provider base-URL environment variables for the child process and
forwards everything upstream unchanged. It never modifies the request.

## Honest limits

- **Token counts are an offline approximation.** `ctx` never calls an API to
  count tokens — that would cost money to measure cost. Measured against the
  provider's own `usage.prompt_tokens`: within ~10% on plain prompts, and
  10–20% low on tool-heavy ones, where the remaining gap is the tokenizer's
  treatment of JSON schemas.
- **The native Gemini API is not parsed** — use Google's OpenAI-compatible endpoint (see above).
- **The base URL must be reachable from an env var or `--to`.** A client with a
  hardcoded base URL cannot be intercepted; `ctx` will tell you it captured
  nothing rather than pretend.
- **Nothing is stored unless you pass `--save`.** No history, no account, no
  telemetry, no daemon.

## Install

```bash
cargo install --path .
```

Rust stable (edition 2021). Single binary, no runtime dependencies.

MIT.
