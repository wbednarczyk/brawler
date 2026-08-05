# Architecture

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related references: [Contracts](contracts.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), [Engineering Workflow](engineering-workflow.md), [Modularization Design](modularization-design.md), and relevant ADRs in [docs/adr](adr/).

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
- transcript provider abstraction (the only AI dependency — [ADR 0084](adr/0084-retire-in-app-ai-layer.md))
- MCP surface (the external agent's typed read/write access to the domain)
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
9. Video transcription can be requested once its provider is configured; all other interpretation happens outside the app, through the MCP port (BYOA — [ADR 0084](adr/0084-retire-in-app-ai-layer.md)).

**Terminal states name their invalidation (harvested 2026-07-10, ADR 0045).** Whenever a pipeline stores a terminal conclusion — a dedup marker, a "cannot extract/process" outcome, a skip — the design must answer *what makes this conclusion stale* in the same change (a capability upgrade? fresh budget? new configuration?) and encode that re-arm/invalidation path. Three separate "permanent blindness" defects in the trusted-extraction epic (tier-eligibility, run dedup, budget skips) shared this one root cause: a terminal state with no invalidation answer. Precedent mechanics: the sweep run re-arm rules ([ADR 0077](adr/0077-trusted-extraction-foundations.md) §3, [data-model.md](data-model.md) History Sweeps).

## Local Storage

SQLite stores local app state and fetched content. The initial database should be migration-managed from the first code milestone.

SQLite is the runtime source of truth for non-secret settings. YAML is an import/export/bootstrap format. API keys and provider secrets live in the OS keychain.

SQLite data and local logs live in the OS app data directory by default. Development builds may override the data directory through a dev-only setting or environment variable.

The database runs in WAL mode and is accessed through an `r2d2` connection pool rather than a single shared connection, so background jobs and the UI read concurrently. Startup uses a single bootstrap connection to run migrations, write a pre-migration snapshot, and read pool configuration before building the pool. Full-text search is served by a unified `search_index` FTS5 virtual table maintained as derived state by per-source triggers. Automatic rotating backups and pre-migration snapshots use `VACUUM INTO`; restore is a restart operation. These data-layer boundaries are defined in [ADR 0032](adr/0032-search-and-backup-boundaries.md) and detailed in [Data Model](data-model.md).

The storage surface is reached through domain-grouped stores (`CompanyStore`, `FeedStore`, `WatchlistStore`, `JobQueueStore`, …), not a single 207-method `AppState` god-facade. Under [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (Architecture v2) this decomposition is **complete**: every `storage/*` domain has a store owning a cheap-clone `Database` connection handle (`storage/database.rs`) and exposing only its domain's operations, and `AppState` is now a **composition root** — it holds the `Database` + cross-cutting infra (pool/seed, `checkout`, db status, backup, metrics, backfill progress), constructs the stores via accessors (`state.companies()`, `state.feed()`, …), and its former per-domain methods are thin delegations kept so existing call sites stay green. New storage methods go on the relevant store. These are concrete, SQLite-coupled structs (a structural split of the facade), **not** a repository port; the storage non-port stance of [ADR 0039](adr/0039-ports-and-adapters-posture.md) stands.

The frontend mirrors this decomposition. Under the same [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) work, `AppStateRoot` and the former god-hooks are split into **feature-scoped view-model contexts** — one per screen (`createScreenContext`, assembled once in the root and read by the screen) plus the per-domain `SettingsContext` — so a screen reads its own view-model instead of a prop-drilled bundle from a single root. Delivered in v0.45.1 as prop-drilling removal; **fine-grained per-domain re-render subscription** (a screen re-rendering only when the specific slice it reads changes) is the remaining part, landing before the cross-cutting interaction state of v0.48 (feed triage + command palette). Composition stays primitive-first ([ADR 0037](adr/0037-ui-component-framework-and-authoring-contract.md)).

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

A Rust-side scheduler (`jobs/scheduler.rs`, spawned at startup on a dedicated blocking thread, [ADR 0055](adr/0055-autonomous-report-pipeline-trust-ladder.md)) **owns the refresh cadence**. Each tick it re-arms `scheduled_source_refresh` / `scheduled_registry_refresh` jobs on the durable `job_queue` for every adapter whose poll interval has elapsed — via `JobQueueStore::reschedule`, a stable-id primitive that resets a terminal/pending row back to `pending` but never disturbs a `running` one, so the queue keeps **one row per recurring job** instead of accumulating a row per fire. The worker executes the refresh; **autopilot detection rides each refresh completion** (`run_detection_sweep`). The same tick loop also owns the daily app-open auto-triggers — the morning briefing, the BiznesRadar-primary fundamentals pull, and the FX daily pull — each enqueuing at most once per day.

The queue itself is a **durable SQLite-backed job queue** (`job_queue` table, migration `0051`; `storage/jobs.rs` for the `JobQueueStore` that enqueues idempotently and atomically claims the next runnable row, retries with capped exponential backoff up to `max_attempts`, and reclaims crash-residue `running` rows on startup) drained off the UI thread by the `JobWorker` + `JobHandler` registry (`jobs/queue.rs`, handlers assembled in `jobs/handlers.rs`). Currently registered handlers cover source/registry refresh, quote/company backfill, the FX daily pull, morning briefing, history sweeps, and the ownership/management-holdings extraction jobs — every genuinely fire-and-forget job enqueues here instead of a detached `spawn_blocking`, so a crash mid-run resumes. Commands that *await* offloaded work and return a result synchronously (source refresh, feed cleanup, transcript processing, history backfill) correctly stay as awaited `spawn_blocking` — synchronous IPC offloading CPU work, not fire-and-forget.

Gating mirrors the UI exactly (license `canUseApp` + poll interval + enabled adapters). The scheduler publishes a per-adapter next-due snapshot (`SchedulerStatus`, epoch-ms) to `AppState`, read by the `get_scheduler_status` command; the frontend only mirrors this snapshot for the "next refresh at …" display and reloads views when a background refresh has fired — it does not decide *when* to refresh (a webview timer throttles/suspends when the window is hidden, which is why this lives server-side, [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md)). Local-first holds: the scheduler runs only while the app is open; no background/closed execution (managed-AI frontier, see [roadmap.md](roadmap.md)). Feed-prune remains a small frontend maintenance timer, not source scheduling.

## Extensibility Boundaries

### Ports and Adapters Posture

Brawler is **hexagonal (Ports and Adapters) at its external seams and package-by-feature inside the Rust domain core** ([ADR 0039](adr/0039-ports-and-adapters-posture.md)). The metapattern is applied where it pays off — a seam with more than one plausible implementation or a replaceable external dependency — and deliberately declined where it does not.

- **Ports (bind to the interface, never the implementation):** source adapters, AI providers ([ADR 0016](adr/0016-provider-neutral-ai-analysis-framework.md), [ADR 0028](adr/0028-multi-provider-ai-boundary.md)), the interpretative capability contract (`Classifier`, [ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md) — the reference hexagon; the embedding-model similarity/matcher/search ports beneath it were retired, [ADR 0080](adr/0080-retire-embedding-model.md)), credentials, search/backups/pool ([ADR 0032](adr/0032-search-and-backup-boundaries.md)), import/export format adapters ([ADR 0018](adr/0018-import-export-boundaries.md)), licensing ([ADR 0017](adr/0017-license-gate.md)), and the UI↔Rust typed-command seam.
- **Core internals:** organized by domain slice (`companies/`, `feed/`, `notebooks/`, …) per [Modularization Design](modularization-design.md), not onion-style layers.
- **Storage is intentionally not a repository port.** SQLite is the single local-first source of truth; domain `storage/*` code may be SQLite-coupled. A `Repository` trait / domain-vs-row split is deferred until a real second backend, a sync/replication engine, or a non-SQLite durable target becomes planned scope (the storage-port trigger in [ADR 0039](adr/0039-ports-and-adapters-posture.md)). Do not add a port whose population is permanently one adapter.

Source adapters should return normalized records through a common interface. Adapters must declare source type, rate limits, supported markets, and allowed fetch mode. This port is being **realized** under [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (Architecture v2, delivered in v0.45.1) as a `SourceAdapter` trait + a registry. **Realized so far:** the trait and a descriptor `REGISTRY` (`src-tauri/src/source_adapters/registry.rs`) are the single source of truth for every adapter's static identity and capability metadata — id, display name, source URL, source type, fetch mode, supported markets, visibility tier, default poll interval, rate-limit policy, policy note. The source catalog (`storage/registry.rs`) and the visibility/enablement logic now read from it (collapsing a ~100-line SQL `CASE` ladder + scattered constants), and a drift-guard test binds the registry to the seed migrations. The **dispatch half** is also realized: the source-refresh path (`jobs/source_refresh.rs`) iterates a `RuntimeAdapter` registry (`runtime_adapters()`) instead of a hardcoded sweep list + per-id `match` — each adapter declares how it refreshes (`Feed` / `Calendar` / `Directory` / `Disabled`), so adding a runtime source is one registry entry. The canonical-identity text transforms — the pure normalization every name/media matcher shares — are owned by `storage::feed_matching` as the SSOT, with ADR 0049 invariant coverage. The **ingestion pipeline spine** is realized in `storage/ingestion.rs` (AV3): the shared downstream stage every feed-item adapter ran in copied form — the **outcome-recording stage** (mark the adapter healthy + record item counters) — is owned once there, and the feed-item ingest paths (Bankier media RSS, GPW ESPI/EBI listings, Bankier company komunikaty) all feed it. The spine is now **shared by every ingest path**: a unified **upsert stage** (`ingestion::upsert_feed_item` over a `NormalizedFeedItem`) replaces the three near-duplicate feed-item INSERTs (media RSS, GPW listings, Bankier company), and the **outcome-recording stage** is used by all five paths including the two calendar/event ingests. The per-adapter parse + the legitimately source-specific match-strategy (media fuzzy vs. structured ticker/ISIN) and dedup-strategy (duplicate signature vs. dedupe key) plug into this shared spine. The pipeline is thus: per-adapter parse → shared `feed_matching` normalize/resolve → per-source dedup → shared `upsert_feed_item` → derive events/signals → shared `record_source_outcome`.

The **transcript provider** implements a provider-neutral interface (`VideoTranscriptProvider`); Gemini is its live implementation. This is the only remaining AI dependency in the app — transcription is data acquisition (speech to text), not interpretation; interpretation happens in the user's own agent over the MCP port (BYOA). Any future in-app inference capability re-enters only via a fresh eval-gated ADR that beats the deterministic baseline on real data ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)).

The two-layer AI split of [ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md) is fully retired ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)); only the deterministic ESPI rule classifier and `signal_dates` parser survive, as ordinary domain code.

The optional on-device **embedding model** that once sat beneath these contracts was retired ([ADR 0080](adr/0080-retire-embedding-model.md)); any future semantic-search/RAG capability re-enters only through a fresh eval-gated ADR, never by resurrecting this code.

Modularity and configurability are core architecture constraints. Provider, source, credential, model, and workflow settings should be represented as explicit boundaries instead of one-off hard-coded behavior when the feature is expected to evolve.

Code organization should follow those boundaries. Large shell files are architecture debt unless they are intentional state roots, facades, composition points, or cohesive domain views. Current module ownership and future extraction triggers are defined in [Modularization Design](modularization-design.md).

Gemini is preferred only for the YouTube press conference transcription workflow because the Gemini API currently has native video/audio understanding and YouTube URL support. M10 requires a working live `provider_gemini` path for supported public YouTube URLs, while automated tests continue to use mocked responses or offline test samples. The implementation must still keep provider boundaries pluggable.

Provider credentials should use a reusable credential boundary rather than provider-specific ad hoc storage. The same boundary must be able to describe future API keys, username/password credentials, session tokens, or other source-specific secret material. Runtime secrets live in the OS keychain, one `CredentialDescriptor` per provider (`provider_gemini:api_key`, `provider_anthropic:api_key`, `provider_openai:api_key`, …); only non-secret status metadata is exposed to the UI. Purpose (e.g. analysis vs transcription) is a settings-level *usage* selection, not part of credential identity — a single provider key may serve multiple purposes ([ADR 0028](adr/0028-multi-provider-ai-boundary.md) decision 4).

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
