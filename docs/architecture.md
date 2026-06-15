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

The database runs in WAL mode and is accessed through an `r2d2` connection pool rather than a single shared connection, so background jobs and the UI read concurrently. Startup uses a single bootstrap connection to run migrations, write a pre-migration snapshot, and read pool configuration before building the pool. Full-text search is served by a unified `search_index` FTS5 virtual table maintained as derived state by per-source triggers. Automatic rotating backups and pre-migration snapshots use `VACUUM INTO`; restore is a restart operation. These data-layer boundaries are defined in [ADR 0032](adr/0032-search-and-backup-boundaries.md) and detailed in [Data Model](data-model.md).

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

## Source Refresh Scheduling

Source refresh and feed cleanup are currently scheduled in the frontend (`src/app/useAppLifecycleEffects.ts`) with `window.setTimeout`/`setInterval` over the user's poll interval, with start jitter and a backoff that doubles the interval after repeated failures.

Idle-session study (v0.38.0, issue `28d6409`):

- Webview timers are subject to background throttling. Chromium-based webviews (WebView2 on Windows) throttle and coalesce timers when the window is hidden/minimized, and the OS can suspend the webview process under memory pressure or sleep. WebKitGTK behaves similarly when occluded.
- For the current cadence (poll intervals of minutes), coarse background throttling (~once per minute) does not materially break a multi-minute interval, and screen lock alone does not stop timers. The real stall risk is OS-level app/process suspension, which a Rust-side timer cannot avoid either while the app is suspended.
- Boundary reminder: refresh runs only while the app is open (background/closed fetching is out of scope until the managed-AI frontier; see [roadmap.md](roadmap.md)).

Decision: keep frontend-driven scheduling for v0.38.0. Move scheduling ownership to a Rust-side scheduler as future hardening — it is more resilient to webview timer throttling and centralizes timing for the autonomous report pipeline (v0.50.0). Tracked as a follow-up implementation issue; not required for this milestone.

## Extensibility Boundaries

Source adapters should return normalized records through a common interface. Adapters must declare source type, rate limits, supported markets, and allowed fetch mode.

AI providers should implement provider-neutral interfaces. Gemini is already the first live AI provider for YouTube transcription and may be extended first for general analysis, but summarization, significance labeling, note extraction, and future AI workflows must remain behind provider/model/credential boundaries that can support OpenAI, Anthropic, and other providers later. General AI analysis is governed by [ADR 0016](adr/0016-provider-neutral-ai-analysis-framework.md).

The AI surface is split into two layers ([ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md)). The **generative** layer (above) handles summarize/extract/assess and is provider-neutral and remote by default. The **interpretative** layer (`src-tauri/src/interpretation`) is on-device semantic lookup exposed as task-level capability contracts — `Classifier`, `SimilarityProvider`, `Matcher`, `SemanticSearch`. Feature code binds to a capability, never to a model: each capability has interchangeable implementations selected through a registry, with a deterministic **static** baseline (rules, lexical/FTS5, fuzzy) as the shipped default and an optional embedding-model implementation layered behind the same trait. The layer may only produce disposable, derived artifacts (a future vector index is a cache rebuilt from canonical data), so the model is reversible to static with no consumer change and no data loss.

Modularity and configurability are core architecture constraints. Provider, source, credential, model, and workflow settings should be represented as explicit boundaries instead of one-off hard-coded behavior when the feature is expected to evolve.

Code organization should follow those boundaries. Large shell files are architecture debt unless they are intentional state roots, facades, composition points, or cohesive domain views. Current module ownership and future extraction triggers are defined in [Modularization Design](modularization-design.md).

Gemini is preferred only for the YouTube press conference transcription workflow because the Gemini API currently has native video/audio understanding and YouTube URL support. M10 requires a working live `provider_gemini` path for supported public YouTube URLs, while automated tests continue to use mocked responses or offline test samples. The implementation must still keep provider boundaries pluggable.

Provider credentials should use a reusable credential boundary rather than provider-specific ad hoc storage. The first credential is the Gemini YouTube transcription API key, but the same boundary must be able to describe future API keys, username/password credentials, session tokens, or other source-specific secret material. Runtime secrets live in the OS keychain and are referenced by provider, purpose, and secret kind; only non-secret status metadata is exposed to the UI.

Global search is a typed-command domain governed by [ADR 0032](adr/0032-search-and-backup-boundaries.md): one search command over the unified `search_index`, with DTOs in `src/api/search.ts` and no SQL in command modules. Backups, pre-migration snapshots, restore, and the connection pool are storage-layer boundaries — UI and command code request backups/restore and read/write pool configuration through typed commands, never by touching files or pool internals directly. Pool configuration is user-tunable through the normal settings boundary and applied at startup.

Import/export is a local typed-command domain governed by [ADR 0018](adr/0018-import-export-boundaries.md). M20 separates format adapters, validation, preview/planning, domain apply, storage operations, commands, and UI workflow. The first section adapters cover research data JSON for companies, watchlists, memberships, and notebooks, plus settings YAML for allowlisted non-secret settings. Future full backup, restore, cloud sync, or alternate file-format adapters should plug into those boundaries instead of reading arbitrary files or dumping runtime tables directly.

Licensing is a local entitlement module governed by [ADR 0017](adr/0017-license-gate.md). Public-opening work keeps the open desktop core usable without a license token while preserving parser, verifier, entitlement-policy, secret-store, storage, command, and presentation boundaries so future paid-feature, subscription, or hosted-activation policies can be added as adapters after later ADR approval.

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
