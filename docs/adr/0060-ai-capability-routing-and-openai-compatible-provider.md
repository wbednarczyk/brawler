# ADR 0060: Per-Capability AI Provider Routing + Generic OpenAI-Compatible Provider

Status: Superseded (2026-07-20) by [ADR 0084](0084-retire-in-app-ai-layer.md) — the in-app AI analysis layer this ADR routes is removed in `v0.59.0`; per-capability routing, the provider pool, and the OpenAI-compatible adapter go with it. Historical: Accepted (2026-07-02); the document-KPI premise reframe is recorded in [Amendments (2026-07-02)](#amendments-2026-07-02) below.

> **Note (2026-07-01):** [ADR 0061](0061-deterministic-fundamentals-data-gathering.md) supersedes this
> ADR's premise that KPI/claim extraction should route documents to Gemini-Pro. KPI extraction becomes
> **structured-first + deterministic** (ESEF/iXBRL → PDF-parse → HTML witness), with AI only a last-resort
> fallback **over extracted text**. This ADR's per-capability routing + generic OpenAI-compatible provider
> **remain valid** for the text/qualitative capabilities and are the basis for ADR 0061's **AI provider
> pool** (ordered failover). The "document" capability tier here is reframed as text-tier (no native PDF).

Extends [ADR 0028](0028-multi-provider-ai-boundary.md) (multi-provider AI boundary: the
`AiAnalysisProvider` port, registry/factory, one global provider + per-provider model registry) and
relates to [ADR 0035](0035-two-layer-ai-and-local-interpretative-layer.md) (per-capability adoption
only where it beats the baseline).

Cross-cutting AI-quality epic — like the Test-architecture and Architecture-v2 foundational epics it
carries **no `milestone:` label** and triggers no roadmap renumbering.

## Context

Autopilot/analysis quality is the live pain point: the default analysis provider is Gemini on
`gemini-3.5-flash` (fast, small) for **every** AI task, set by a single global
`general_analysis_provider`. Real owner use of the v0.49 autopilot exposed weak KPI extraction, and the
owner wants "a bigger, real, free model" — especially on the high-value autonomous-pipeline path.

Two facts shape the design:

- **Document input is the differentiator.** KPI and claim extraction send the report **document
  natively** (`complete_document` / `DocumentSupport`); Gemini ingests the PDF directly. Most free
  open-model hosts (Groq, OpenRouter, Cerebras) are **text-only**, so routing document extraction
  through them forces a lossy PDF→text step — a quality *regression* risk exactly where accuracy matters
  most. Text tasks (feed analysis, research briefs/digests, ESPI event-date/classification) have no such
  constraint.
- **The free open-model hosts are OpenAI-compatible.** Groq, OpenRouter, Cerebras, Together, and local
  **Ollama** all speak the OpenAI chat-completions wire format, so one adapter with a configurable
  endpoint unlocks all of them.

The single global provider cannot express "Gemini-Pro for documents, a free open model for text," so the
hybrid the owner chose needs a routing seam and a new provider.

## Decision

### 1. Per-capability provider routing (map with a global fallback)

Replace the single global analysis provider with a **capability → (provider, model) map**, resolved per
AI task, **falling back to `general_analysis_provider`/`general_analysis_model`** when a capability has
no explicit override. Backward-compatible: with an empty map every capability uses the global provider
exactly as today. Capabilities (each already a distinct provider-call site):

| Capability key | Kind | Provider call |
|---|---|---|
| `kpi_extraction` | **document** | `complete_document` |
| `claim_extraction` | **document** | `complete_document` |
| `feed_analysis` | text | `analyze` |
| `research_brief` | text | `generate_research_brief` |
| `research_digest` | text | `generate_research_digest` |
| `event_date` | text | `complete_document`/text |
| `signal_classification` | text | text |

Stored as JSON in `settings` (the `shortcut_bindings` pattern): `capability_providers = { "<key>":
{ "provider": "...", "model": "..." } }`. Each job's `provider_for_job` resolves its capability's entry,
else the global default. YouTube transcription keeps its own existing `youtube_transcription_provider`
(out of scope here).

Document capabilities **default to a Gemini document-capable model** (see decision 3); text capabilities
default to the global provider (which the owner can point at the free open-model provider). A resolved
provider whose `document_support()` is `None` must **not** be used for a document capability — the
resolver rejects it (surfacing a settings error) rather than silently degrading to PDF→text.

### 2. One generic OpenAI-compatible provider

Add a single provider `provider_openai_compatible` built from a user-set **base URL + model + keychain
API key** (parametrizing the existing OpenAI adapter's OpenAI chat-completions transport). One addition
covers Groq, OpenRouter, Cerebras, Together, and local Ollama; concrete hosts are documented as presets
in `wiki/` (not hardcoded providers, which rot as hosts change). `document_support()` is `None`
(text-only) so decision 1 keeps documents off it by default. Credential lives in the OS keychain under
its own key (never SQLite/`.env` in runtime).

### 3. Gemini document tier + real-data validation gate

The document capabilities default to a bigger Gemini model (`gemini-3.1-pro-preview`, already in the
catalog) on Google's free tier — keeping **native document input**. **Flipping any document-tier default
model is gated on real-data validation** (the standing real-data-validation-precedes-implementation
guardrail, [ADR 0045](0045-guardrail-harvest-loop.md) / [testing.md](../testing.md)): build a
hand-labeled ground-truth set from the owner's real DB (`private/realdata/`) and measure extraction
precision/recall for the new model vs `gemini-3.5-flash` **before** changing the shipped default. Free
open-model text routing is validated the same way before it becomes a default.

## Amendments (2026-07-02)

- **(a) Document-premise reframe (ADR 0061):** KPI/claim extraction is structured-first + deterministic
  per [ADR 0061](0061-deterministic-fundamentals-data-gathering.md); AI is a last-resort tier. The
  KPI/claim jobs still send native documents today, so the document-capability resolver guard (a provider
  with `DocumentSupport::None` must not be resolved for a document capability) stays in force.
- **(b) Ordered list = pool:** the `capability_providers` map value is an **ordered list**, not a single
  entry: `capability_providers = { "<key>": [ {provider, model}, ... ] }`. One element = plain routing;
  empty/missing key = fallback to `general_analysis_provider`; the list order is the failover order of the
  provider pool (ADR 0061 decision 5): failover only on availability errors (429 / 5xx / timeout /
  connection), never on a valid-200-with-bad-content; a freshly-failed member enters a short cooldown (60
  s, runtime-only state) and is skipped-first but still tried last — the pool never dead-ends.
- **(c) Decision 3 default flip stays gated:** the document-tier default model is NOT flipped in this
  implementation slice; it remains gated on the real-data validation described in decision 3. The resolver
  default is the global fallback.
- **(d) Compatible provider is `DocumentSupport::TextOnly`, not `None`:** the generic OpenAI-compatible
  provider reuses the OpenAI adapter, which is TextOnly and implements `complete_document` for
  extracted-text documents — required for the `event_date` capability. Decision 1's guard semantics are
  unchanged (TextOnly still must not serve a native-document capability by default routing).
- **(e) Provider identity note:** the `AiAnalysisProvider::provider_id()` contract returns `&'static str`,
  which suffices for the single constant id `provider_openai_compatible`; distinct ids per configured
  endpoint would require a trait-signature change and is explicitly out of scope.

## Consequences

- The owner can mix "Gemini-Pro for the autonomous-pipeline document extraction" with "a free open model
  (Groq/OpenRouter/…) for text analysis," per capability, with a safe global fallback.
- New surface: a `capability_providers` settings map + resolver, one OpenAI-compatible provider +
  credential, a Settings AI section (per-capability provider/model table + the compatible-provider
  config), a migration, ts-rs types, and `wiki/` preset docs.
- Quality is **claimed only after real-data measurement**, never from the model name alone.
- Adding a provider is still an ADR-gated act; free-tier hosted inference is not a paid API but is a new
  external data recipient, so this ADR is its approval and source-strategy records the policy.

## Guardrail (ADR 0045)

A provider resolved for a **document** capability must support document input, or the resolver errors
(no silent PDF→text degrade) — a guard test pins this. Any new AI capability added later declares its
kind (document/text) and joins the map with a default, so routing never silently falls back to a
mismatched provider.

## Open / to confirm at implementation

- `gemini-3.1-pro-preview` free-tier availability + rate limits (verify against Google AI Studio at
  implementation; fall back to the best free document-capable Gemini model if unavailable).
- Whether the per-capability UI groups by document/text tier for clarity.

## Status notes

Proposed 2026-07-01. Direction (hybrid: Gemini-Pro for documents + a free OpenAI-compatible provider for
text, validated on real data first) and the three architecture decisions (per-capability map; one generic
OpenAI-compatible provider; near-term cross-cutting epic, no milestone label) co-decided with the
maintainer. Delivered in slices: (1) this ADR + doc/contract updates + Radicle epic; (2) the
OpenAI-compatible provider + credential + settings; (3) per-capability routing map + resolver + migration
+ ts-rs; (4) Gemini document-tier default behind the real-data gate; (5) the Settings AI section + wiki
presets; (6) real-data extraction precision/recall measurement.

Accepted and implemented 2026-07-02 together with [ADR 0061](0061-deterministic-fundamentals-data-gathering.md)
decision 5 (the AI provider pool), as amended above.
