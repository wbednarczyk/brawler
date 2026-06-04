# Architecture

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Contracts](contracts.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), [Project Practices](project-practices.md), [Modularization Design](modularization-design.md), and relevant ADRs in [docs/adr](adr/).

## Chosen Stack

- Desktop shell: Tauri
- UI: React + TypeScript
- Backend/domain runtime: Rust inside the Tauri application
- Storage: SQLite
- Data posture: local-only for v1
- Scheduling: in-app scheduler while the desktop app is running

## Runtime Shape

Tauri owns the desktop shell, window lifecycle, app packaging, local permissions, background tasks, and command/event bridge between the UI and Rust domain modules.

Rust owns domain behavior:

- company registry
- watchlists
- per-company notebooks
- source adapters
- ingestion scheduler
- article/report normalization
- deduplication
- AI provider abstraction
- video/transcript processing orchestration
- transcript-to-note selection workflows
- local settings and secrets
- local license entitlement validation
- SQLite persistence
- Tauri command handlers and event streams

The React UI talks to Rust through typed Tauri commands. Feed, job, transcription, and notebook updates should be emitted through Tauri events.

## Data Flow

1. The user starts the desktop app.
2. Tauri starts Rust application state and opens the SQLite database.
3. Rust schedules source polling while the app is running.
4. Source adapters fetch official/public/RSS data according to source policy.
5. Raw source records are normalized into feed items.
6. Deduplication prevents repeated reports or articles from appearing multiple times.
7. The UI displays the investor inbox and receives job/feed updates through Tauri events.
8. The user can create notebook entries from feed items.
9. AI analysis, transcription, and note extraction can be requested once provider configuration exists.

## Local Storage

SQLite stores local app state and fetched content. The initial database should be migration-managed from the first code milestone.

SQLite is the runtime source of truth for non-secret settings. YAML is an import/export/bootstrap format. API keys and provider secrets live in the OS keychain.

SQLite data and local logs live in the OS app data directory by default. Development builds may override the data directory through a dev-only setting or environment variable.

Data must include enough origin to audit a feed item:

- source adapter ID
- source URL
- fetched timestamp
- publication timestamp when available
- original language
- matched company identity
- attribution/display source
- raw source reference or checksum
- notebook note origin links
- transcript segment origin links

## Extensibility Boundaries

Source adapters should return normalized records through a common interface. Adapters must declare source type, rate limits, supported markets, and allowed fetch mode.

AI providers should implement provider-neutral interfaces. Gemini is already the first live AI provider for YouTube transcription and may be extended first for general analysis, but summarization, significance labeling, note extraction, and future AI workflows must remain behind provider/model/credential boundaries that can support OpenAI, Anthropic, and other providers later. General AI analysis is governed by [ADR 0016](adr/0016-provider-neutral-ai-analysis-framework.md).

Modularity and configurability are core architecture constraints. Provider, source, credential, model, and workflow settings should be represented as explicit boundaries instead of one-off hard-coded behavior when the feature is expected to evolve.

Code organization should follow those boundaries. Large shell files are architecture debt unless they are intentional state roots, facades, composition points, or cohesive domain views. Current module ownership and future extraction triggers are defined in [Modularization Design](modularization-design.md).

Gemini is preferred only for the YouTube press conference transcription workflow because the Gemini API currently has native video/audio understanding and YouTube URL support. M10 requires a working live `provider_gemini` path for supported public YouTube URLs, while automated tests continue to use mocked responses or offline test samples. The implementation must still keep provider boundaries pluggable.

Provider credentials should use a reusable credential boundary rather than provider-specific ad hoc storage. The first credential is the Gemini YouTube transcription API key, but the same boundary must be able to describe future API keys, username/password credentials, session tokens, or other source-specific secret material. Runtime secrets live in the OS keychain and are referenced by provider, purpose, and secret kind; only non-secret status metadata is exposed to the UI.

Licensing is a local entitlement module governed by [ADR 0017](adr/0017-friend-test-license-gate.md). M17 uses local author and friend-test policies with versioned signed tokens, Ed25519 public-key verification, separate author/friend-test signing keys, OS-keychain storage for raw accepted tokens, SQLite storage for derived metadata only, and an app-shell gate for normal navigation. Parser, verifier, entitlement-policy, secret-store, storage, command, and presentation boundaries must remain explicit so future community/open-core, paid-feature, subscription, or hosted-activation policies can be added as adapters after later ADR approval.

Provider model choice and request timeout are configurable. Gemini YouTube transcription defaults to the cheapest configured model that passed M10 live smoke validation with direct YouTube/video input. The runtime timeout defaults to 300 seconds, can be changed in Settings, and may be overridden by `BRAWLER_GEMINI_REQUEST_TIMEOUT_SECONDS` for development/live-smoke runs.

Notebook entries should be source-linked. A note can originate from manual entry, a feed item, an AI summary, a transcript segment, or a selected AI-suggested claim.

Premium or hosted convenience features must be added behind explicit interfaces, not by making local-first behavior depend on cloud services.

## Build And Test Posture

The codebase should be easy to build in GitHub Actions. Default CI must stay fast, require no secrets, and avoid live external services.

Testing should be lean and behavior-focused:

- Rust unit tests for domain logic, migrations, adapters, and provider mapping.
- Frontend component tests for critical UI workflows.
- Test-sample-based tests for source adapters and AI provider contracts.
- A small number of smoke tests for desktop startup and command availability.

## Security And Observability Posture

The React frontend must call typed Tauri commands only. It must not receive API keys, full license tokens, private signing material, execute arbitrary shell commands, or receive broad filesystem access. Source and provider requests happen in Rust.

V1 uses local-only observability. Telemetry, remote error reporting, remote log shipping, hosted metrics, and hosted tracing require a future ADR. Source and job errors surface in the Sources screen. Developer mode and local observability are governed by [ADR 0015](adr/0015-developer-mode-local-observability.md).

Developer diagnostics are planned as a local-only V1 framework, separate from normal user-facing status UI and runtime log files. Modules should report typed, redacted diagnostic events through a shared boundary when developer mode is enabled instead of building module-specific debug panels. Diagnostic payloads must not include secrets, full prompts, raw provider responses, full source bodies, full transcript text, or license private material by default. Diagnostic event structure should remain cheap to map to future OpenTelemetry-style concepts, but the app should not add OpenTelemetry dependencies or compatibility-only code unless the implementation cost is clearly low.

Local logs are a V1 framework for append-only runtime records, rotation, and conservative debugging outside normal user-facing status UI. They use the Rust `log` facade, a local JSON Lines backend under the app data logs directory, shared observability redaction, `info` as the default level, configurable level and rotation settings, and a Developer-mode Diagnostics log viewer. The default rotation policy is five files of five MiB each.

Metrics are a V1 framework for local operational counters, gauges, and durations in Developer mode. Metrics must be operational health signals, not user behavior analytics. The metrics code should separate collectors, an internal typed sample model, and presentation/export adapters so the in-app Developer-mode view is the first adapter and future Prometheus or other local metrics integrations can be added without rewriting collection logic. M16 uses on-demand snapshots from durable local state plus explicit in-memory runtime counters for process-lifetime signals; those runtime counters reset when the app restarts.
