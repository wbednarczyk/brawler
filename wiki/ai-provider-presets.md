# AI provider pools and the OpenAI-compatible provider

Brawler can route each AI capability — feed analysis, research briefs and
digests, ESPI event dates, signal classification, and KPI/claim extraction —
to its own provider, and give each one a **failover pool** of several
providers instead of a single point of failure. One provider it can route to
is a generic **OpenAI-compatible** provider that works with any host that
speaks the **OpenAI chat-completions** wire format — not just Gemini, Claude,
and OpenAI. This covers free and self-hosted open-model hosts like Groq,
OpenRouter, Together, Cerebras, and a **local Ollama** install: you point it
at a base URL and pick a model name, and it works like any other provider.

## Preset base URLs

| Host | Base URL | Notes |
|---|---|---|
| Groq | `https://api.groq.com/openai/v1` | Fast, free-tier open models. |
| OpenRouter | `https://openrouter.ai/api/v1` | Aggregator — many open and paid models behind one key. |
| Ollama (local) | `http://localhost:11434/v1` | Runs on your own machine; nothing leaves your computer. |
| Together | `https://api.together.xyz/v1` | Hosted open models. |
| Cerebras | `https://api.cerebras.ai/v1` | Fast inference on select open models. |

Model names are **host-specific and freeform** — Brawler does not curate a
model list for this provider, since every host publishes its own names (e.g.
`llama-3.3-70b-versatile` on Groq vs. `meta-llama/llama-3.3-70b` on
OpenRouter). Check the host's own model list/docs for the exact id to type
in.

## Setup

1. **Set the base URL.** Settings → AI → **OpenAI-compatible base URL** — paste
   one of the presets above (or your own OpenAI-compatible endpoint). It must
   start with `http://` or `https://`.
2. **Set the API key.** Settings → Credentials → find **OpenAI-compatible
   (custom)** and save a key. Brawler requires a non-empty value here for
   every provider it can call, including this one — if your host doesn't
   actually check the key (a local Ollama install typically doesn't), enter
   any placeholder value; Brawler needs the entry to exist, the host decides
   whether it validates it.
3. **Pick models per capability.** Use the general AI provider/model fields
   for a single default, or open the **AI capability routing** section to set
   this provider (with a specific model) for individual capabilities —
   mixing it with Gemini, Claude, or OpenAI as needed.

## Routing AI capabilities to a pool

Settings → AI → **AI capability routing** lets you pick, per capability, an
**ordered list** of provider/model entries instead of one global provider.
Pick a capability, add a provider/model row, and reorder rows (move up/down)
to set the failover order — the first row is tried first. Leaving a
capability's list empty just falls back to your general AI provider, so this
is entirely opt-in.

The routable capabilities are:

| Capability | What it's for | Can it use OpenAI-compatible / OpenAI? |
|---|---|---|
| Feed analysis | Reading and scoring feed items | Yes |
| Research brief | One-off research write-ups | Yes |
| Research digest | Recurring research summaries | Yes |
| Event date | Reading dates out of event filings | Yes |
| Signal classification | Categorizing detected company signals | Yes |
| KPI extraction | Reading figures out of a report document | **No — Gemini or Claude only** |
| Claim extraction | Reading management claims out of a report document | **No — Gemini or Claude only** |

KPI and claim extraction hand Brawler the **report document itself**, not
just text, so they need a provider that can read a document natively. OpenAI
and the OpenAI-compatible provider are text-only — Brawler rejects the
combination up front rather than letting an extraction run fail later.

## Failover pools

Each capability's routing list can hold more than one provider/model entry.
Brawler tries them in order and only moves to the next one when the current
provider is genuinely *unavailable* (rate-limited, erroring, timing out, or
unreachable) — never because of a response it just doesn't like the content
of. A member that just failed is tried last for about a minute before
becoming eligible to go first again, so a flaky free-tier host degrades
gracefully instead of blocking the whole capability, and every member is
still tried even if the whole pool is temporarily cooling down. This makes
OpenAI-compatible hosts a good *fallback* entry behind a paid provider, or a
primary entry with a paid provider as backup.

## Privacy

Requests for a capability routed to the OpenAI-compatible provider go
**directly** from your machine to the base URL you configured — Brawler adds
no intermediary. For local Ollama, that means the request never leaves your
computer. For a hosted service (Groq, OpenRouter, Together, Cerebras, or any
other host you point it at), the request goes to that host under its own
privacy terms — the same as it would for Gemini, Claude, or OpenAI. The API
key is always stored in your OS keychain, never in the database or in
exported settings.
