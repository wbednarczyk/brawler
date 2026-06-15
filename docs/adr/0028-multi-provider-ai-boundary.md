# ADR 0028: Multi-Provider AI Boundary (Async, Ports-and-Adapters)

## Status

Accepted.

## Context

Brawler's AI analysis layer is provider-neutral in name — there is an `AiAnalysisProvider` trait in `src-tauri/src/providers/analysis/types.rs` with provider-agnostic request/response types — but Gemini is the only real implementation, and the boundary leaks Gemini assumptions in the wiring:

- provider dispatch is a hardcoded `match provider_id { "provider_gemini" => GeminiAnalysisProvider::live(...) }` duplicated across `jobs/ai_analysis.rs`, `jobs/research_briefs.rs`, `jobs/research_digests.rs`, and `jobs/transcript_runner.rs`;
- settings validate the provider against a `["provider_gemini"]` allowlist and models against a gemini-only list (`storage/settings.rs`, `storage/import_export/settings_import.rs`);
- credential lookup calls per-provider helpers (`read_gemini_general_analysis_api_key`) directly inside each match arm;
- there is no mechanism to send a report document (PDF/ESEF) to a provider — `GeminiAnalysisPart` is text-only;
- the provider/HTTP layer is `reqwest::blocking`.

The v0.35.0 milestone adds Claude (Anthropic) and OpenAI (ChatGPT) as AI providers, all free with a user-supplied key, **before** AI KPI extraction (v0.36.0) is built — so the report-document input path is designed against multiple providers rather than retrofitted. This ADR records the architecture decisions that govern that work.

A boundary assessment confirmed the `AiAnalysisProvider` trait and the keychain-backed `CredentialDescriptor` are sound; the coupling is at the wiring layer and is mechanical to fix. The harder, durable decisions are: how providers are implemented (in-house vs SDK), sync vs async, how documents cross the boundary, and how selection/credentials/models are configured.

## Decision

### 1. Multi-provider, bring-your-own-key, all free in core

Gemini, Claude, and OpenAI are all selectable, all free, each using the user's own API key stored in the OS keychain. This is the open-core line: AI features are free with your own key; the future paid tier is **managed/hosted AI** (and cloud sync/backup, signed installers), not gating which provider you may use. No entitlement changes in this milestone. No pricing detail in public docs.

### 2. Hand-roll all providers now; ports-and-adapters for a future SDK swap

There is **no official Rust SDK** for Anthropic or OpenAI — both ship official SDKs only for Python/TypeScript (and some JVM/.NET). Every Rust option is community-maintained: `async-openai` is mature, but the Anthropic crates are younger and uneven. Mixing "crate for OpenAI, hand-rolled for Anthropic and Gemini" would be inconsistent; the choice is therefore all-or-none, and with no official SDKs the answer is **none for now — hand-roll all three**.

The code is structured so that swapping the in-house implementation for official SDKs later is a localized change, per provider, when those SDKs exist:

- The provider-neutral **port** — the async `AiAnalysisProvider` trait plus its neutral request/response/error types — is the stable contract. The registry, jobs, settings, and UI depend only on it.
- Each provider is an **adapter** that maps neutral⇄wire and calls a shared in-house transport.
- When an official SDK lands, add a new adapter implementing the same trait and flip one line in the registry. The trait, neutral types, registry, jobs, settings, and UI do not move. Blast radius of a swap = one provider.

Module layout under `src-tauri/src/providers/analysis/`:

```
types.rs       # PORT: neutral request/response/error + async AiAnalysisProvider trait (stable contract)
registry.rs    # provider_id -> Box<dyn AiAnalysisProvider> + its CredentialDescriptor
transport.rs   # shared in-house client: async reqwest + retry/backoff + provider-error mapping
document.rs    # neutral document-input part + per-provider/model capability flags
gemini.rs      # GeminiAnalysisProvider   \
anthropic.rs   # ClaudeAnalysisProvider    > each: neutral<->wire mapping over the shared transport
openai.rs      # OpenAiAnalysisProvider    /
```

The shared `transport.rs` is "the in-house lib"; it also removes the ad-hoc error/retry duplication currently inside `gemini.rs`. The registry+port shape mirrors the existing `source_adapters` and transcript-provider patterns and is the template for future modular work.

### 3. Async by default

The provider/job layer migrates from blocking to async. Rationale: Tauri already runs on tokio (so `reqwest::blocking` wraps async anyway); the future official SDKs and all current community crates are async-first, so async now makes the later swap a drop-in instead of a prerequisite refactor; and async enables the concurrency the autonomous report pipeline (v0.50.0) needs and the streaming the AI-waiting-animation work wants.

- `AiAnalysisProvider` becomes an async trait. Because the registry holds `Box<dyn AiAnalysisProvider>`, dynamic dispatch over async methods uses the **`async-trait`** crate — an accepted, deliberate dependency (small, ubiquitous), consistent with not being needlessly conservative about dependencies.
- The Gemini adapter and its injectable HTTP client trait, the three analysis jobs (`ai_analysis`, `research_briefs`, `research_digests`) and their Tauri commands, and the **transcription** provider + `transcript_runner` all migrate to async. Transcription stays Gemini-only; it migrates for consistency, not to become multi-provider.
- Tests use `#[tokio::test]` with a mockable async transport seam; CI stays offline (no live provider calls or secrets).

**Standing requirement:** AI-based features in Brawler are async by default from now on, unless async genuinely does not fit a given feature.

### 4. Credentials: one key per provider, purpose is not credential identity

The keychain stores exactly one secret per provider — the API key — and nothing else (no analysis results, prompts, or outputs). Purpose (analysis vs transcription) is a *usage* decided by settings, not part of the credential's identity.

The current design is wrong here: it files the same Gemini key under two purpose-scoped entries — `brawler/gemini/youtube_transcription/api_key` and a separate `general_analysis` entry — so a user's single Google key is stored twice and the credential pretends "analysis" is a thing you authenticate. This is over-segmentation; the only thing per-purpose keys would enable is two different keys for the same provider (e.g. separate billing), which is YAGNI.

Decision:

- One `CredentialDescriptor` per provider: `provider_gemini:api_key`, `provider_anthropic:api_key`, `provider_openai:api_key`.
- A generic command surface keyed by `provider_id` — `get_provider_credential_status` / `set_provider_api_key` / `clear_provider_api_key` — replacing the per-provider, per-purpose `*_gemini_transcription_*` commands.
- Transcription reads the unified Gemini provider key; whichever provider settings select for a feature, that provider's single key is used.
- No `.env` in runtime code (dev-only fallback as today).

**Clean cut, no backward compatibility.** The legacy purpose-scoped descriptors (`gemini_transcription_descriptor`, `gemini_general_analysis_descriptor`) and their commands are removed outright. We do not read or fall back to the old entries. As this is pre-1.0, the user re-enters the key once into the unified slot; the credential settings best-effort clear the known legacy keychain entries so no orphaned secrets linger, then drop the legacy target constants.

### 5. Settings: one global provider + per-provider model registry

A single global active AI provider and model apply to all AI features (analysis, briefs, digests) — simplest mental model and settings surface. Per-feature provider selection is explicitly rejected for now.

- Replace the `provider_gemini`-only provider allowlist and the gemini-only model list with a **per-provider model registry**; each provider seeds a curated model list and a **balanced mid-tier default** (quality/cost compromise; the user can switch up or down).
- Exact model IDs are verified live at implementation time and seeded via a settings migration — they are not hardcoded in this ADR to avoid staleness.
- Settings UI gains a provider dropdown and a model dropdown driven by the registry.

### 6. Document input: hybrid (native + text fallback)

The neutral request/part model gains a document-input part that carries **either native bytes + MIME type or extracted text**. Each provider/model is **capability-flagged** for native document support:

- Gemini and Claude implement native document (PDF) input;
- OpenAI uses native file input where the model supports it, text otherwise;
- a model without native document support receives locally-extracted text.

Scope note: in v0.35.0 no feature *sends* a document yet (extraction is v0.36.0). This milestone builds the abstraction and proves it with **Gemini native**, and **defers the local PDF-text-extraction dependency to v0.36.0**, when the text-fallback path is first exercised. This deferral is a recorded decision, not a gap.

**v0.36.0 resolution (native-first; no PDF-text dependency added):** when AI KPI extraction first exercised the document path, the deferred local PDF-text-extraction dependency was evaluated and **declined**. GPW periodic reports are table-heavy, and pure-Rust PDF text extractors garble financial tables badly enough to undermine the per-fact confirm-correctness goal. The decision is therefore **native-multimodal-first**: extraction routes through a document-capable provider — Gemini (native, proven in v0.35.0) and **Claude native PDF** (added in v0.36.0). OpenAI remains text-path/degraded until its adapter gains native file input. No PDF→text crate is introduced; the `AnalysisDocument::Text` fallback stays in the model for providers/models that can only take text, but no v0.36.0 feature produces it from a PDF. This keeps extraction fidelity high where correctness matters and avoids a fragile dependency.

### 7. Non-streaming responses for now

AI jobs persist a final result, so responses are non-streaming in this milestone. Streaming (and the AI-waiting-animation UX) is a separate, later concern; the async foundation makes it natural to add.

## Consequences

Positive:

- Adding a provider becomes: implement the async trait, register it, add a credential descriptor and model-registry entry — no edits to duplicated dispatch.
- The in-house→official-SDK swap is localized to one adapter + one registry line, with no churn to the port, jobs, settings, or UI.
- Async unlocks concurrency (autonomous pipeline) and streaming (later) and aligns the stack with its actual tokio runtime.
- The shared transport pays down existing error/retry duplication; the port/registry shape is a reusable structural template.

Negative / costs:

- The async migration touches the provider traits, three jobs, the transcript runner, their commands, and their tests — bounded, and cheaper now at one provider than later at three.
- One new dependency (`async-trait`). Justified and small.
- Hand-rolling means we own request/response mapping and wire-format drift for three providers; mitigated by the shared transport and per-adapter tests, and revisited when official SDKs exist.
- The hybrid document model carries two representations; capability flags add a small matrix to maintain.

## Alternatives Considered

- **Adopt community SDKs now** (`async-openai` + an Anthropic crate): rejected. No official SDKs exist, so this trades hand-rolling for trusting uneven community maintenance and a large transitive tree on paid, privacy-sensitive integrations; it creates provider asymmetry (strong OpenAI crate, weak Anthropic) while still hand-rolling Gemini; and we use only a thin slice of each API. Re-evaluate when official SDKs ship — the ports-and-adapters design exists precisely to make that swap cheap.
- **Stay blocking**: rejected. It hides an async runtime, blocks concurrency and streaming, and would make any future SDK adoption require the async refactor first anyway.
- **Per-feature provider selection**: rejected for now as unnecessary settings/wiring surface; revisit if a real need appears.
- **Text-extraction-only document input**: rejected — loses tables/layout that financial figures depend on. **Native-only** rejected — gates providers without a universal floor. Hybrid keeps fidelity where available with a universal fallback.

## Implementation

Tracked under epic `fb20c2f` (milestone v0.35.0):

1. This ADR — multi-provider AI boundary.
2. Migrate the AI provider/job layer (analysis + transcription) from blocking to async (`async-trait`).
3. Provider registry/factory — replace the duplicated dispatch.
4. Generalize settings (per-provider model registry), collapse credentials to one key per provider with a generic `provider_id`-keyed command surface (remove the legacy purpose-scoped Gemini descriptors and commands outright, no fallback), provider/model selection UI.
5. Claude (Anthropic) analysis adapter.
6. OpenAI (ChatGPT) analysis adapter.
7. Document-input abstraction on the trait (retrofit Gemini native), capability flags.
8. Multi-provider tests + contracts/docs checkpoint.

The local PDF-text-extraction dependency for the document text-fallback path was deferred to v0.36.0 (AI KPI extraction), and on evaluation there was **declined** in favour of native-multimodal-first delivery (Gemini + Claude native PDF). See the "v0.36.0 resolution" note under decision 6 above. v0.36.0 extraction work (epic `9879941`) follows: KPI extraction contracts and prompt boundary, Claude native PDF input, the async extraction job persisting pending facts, the per-fact review/confirmation UI, the feed-item extraction action, and tests/docs.
