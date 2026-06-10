# ADR 0016: Provider-Neutral AI Analysis Framework

## Status

Accepted

## Context

Brawler needs AI-assisted source-grounded analysis for feed items, but the app should not become tied to one provider or one model. Gemini is already used for YouTube transcription and is the first practical live provider for general analysis, while future providers such as OpenAI, Anthropic, and compatible local or hosted models should remain possible.

AI provider calls can be slow or fail because of credentials, network timeouts, provider limits, model changes, or parsing problems. The UI must remain responsive and the app must preserve local job state.

Financial AI output is decision support only. It must cite source material and avoid buy/sell/hold recommendations or personalized portfolio advice.

## Decision

General AI analysis uses a provider-neutral framework:

- AI analysis runs through asynchronous local job records.
- The frontend starts work through typed Tauri commands and then observes local job state.
- Provider implementations live behind a Rust provider boundary.
- Gemini is the first live general-analysis provider, but stored jobs and results use provider IDs, model names, prompt versions, and source references rather than Gemini-specific assumptions.
- Settings expose provider, model, timeout, and mode configuration through extensible provider/model lists.
- The default AI analysis mode is `source_grounded`.
- Prompt presets and prompt versions are stable IDs so behavior can be audited and changed deliberately.
- AI output must cite source material and must not include buy/sell/hold recommendations or personalized portfolio allocation advice.

The framework may support chat-like UI affordances and custom user questions, but provider responses are still stored as source-grounded local analysis results, not as investment advice.

## Consequences

- Gemini-specific code stays in a provider implementation, not in storage, UI contracts, or job orchestration.
- Future providers can be added by implementing the provider boundary and exposing supported model/config options.
- Provider calls do not block the UI.
- Jobs preserve queued, running, succeeded, failed, and cancelled states with recoverable local error codes.
- Provider errors must not expose API keys, full prompts, full source bodies, or raw provider responses by default.
- Live provider smoke tests are opt-in and secret-backed; default CI uses mocks or test samples.
- Prompt templates should be versioned and reviewed when analysis behavior changes materially.
