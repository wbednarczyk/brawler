# Architecture

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Contracts](contracts.md), [Data Model](data-model.md), [Source Strategy](source-strategy.md), [Engineering Workflow](engineering-workflow.md), [Modularization Design](modularization-design.md), and relevant ADRs in [docs/adr](adr/).

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

The storage surface is reached through domain-grouped stores (`CompanyStore`, `FeedStore`, `WatchlistStore`, `JobQueueStore`, …), not a single 207-method `AppState` god-facade. Under [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (Architecture v2) this decomposition is **complete**: every `storage/*` domain has a store owning a cheap-clone `Database` connection handle (`storage/database.rs`) and exposing only its domain's operations, and `AppState` is now a **composition root** — it holds the `Database` + cross-cutting infra (pool/seed, `checkout`, db status, backup, metrics, backfill progress, embedding-download state), constructs the stores via accessors (`state.companies()`, `state.feed()`, …), and its former per-domain methods are thin delegations kept so existing call sites stay green. New storage methods go on the relevant store. These are concrete, SQLite-coupled structs (a structural split of the facade), **not** a repository port; the storage non-port stance of [ADR 0039](adr/0039-ports-and-adapters-posture.md) stands.

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

Decision: keep frontend-driven scheduling for v0.38.0. Move scheduling ownership to a Rust-side scheduler as future hardening — it is more resilient to webview timer throttling and centralizes timing for the autonomous report pipeline (v0.49.0). Tracked as a follow-up implementation issue; not required for this milestone.

This hardening is being built under [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (Architecture v2, ahead of v0.49.0): a **durable SQLite-backed job queue** replaces the fire-and-forget `spawn_blocking` model. **Realized so far:** the `job_queue` table (migration `0051`), the `JobQueueStore` domain store (`storage/jobs.rs`) that enqueues idempotently and **atomically claims** the next runnable row (claim + attempt-increment in one statement, so no double-claim and a crash mid-run still counts), retries with capped exponential backoff up to `max_attempts`, and **reclaims** crash-residue `running` rows on startup; and the in-process `JobWorker` + `JobHandler` registry (`jobs/queue.rs`, handlers assembled in `jobs/handlers.rs`) that drains the queue off the UI thread. The worker is **spawned at app startup** (`lib.rs`): it reclaims any crash-residue `running` rows, then drains the queue on a dedicated blocking thread. The first migrated job is the **startup content-embedding refresh** (ADR 0035) — previously a fire-and-forget `spawn_blocking`, now enqueued onto the queue so a crash mid-embed resumes and the idempotent work retries. Local-first holds — a single worker runs only while the app is open; no background/closed execution (that remains the managed-AI frontier). **All genuine fire-and-forget jobs are now migrated:** the startup + strategy-select content-embedding refresh and the five user-initiated AI jobs (AI analysis, claim extraction, KPI extraction, research brief, research digest) enqueue onto the queue instead of detached `spawn_blocking`; the worker runs them, so a crash mid-run resumes. Each keeps its own per-job status table that the UI polls — the queue is the durable *execution* mechanism, the per-job tables remain the *status/result* store, so the UI contract is unchanged (single attempt, domain failures recorded in the per-job table). The commands that *await* offloaded work and return a result synchronously (source refresh, feed cleanup, transcript processing, history backfill, similarity queries) correctly stay as awaited `spawn_blocking` — they are synchronous IPC offloading CPU work, not fire-and-forget — as does the one-off model-weights download (its own in-memory progress, re-triggerable). The Rust-side scheduler enqueuing onto the queue is the remaining hardening, tracked with the autonomous pipeline.

## Extensibility Boundaries

### Ports and Adapters Posture

Brawler is **hexagonal (Ports and Adapters) at its external seams and package-by-feature inside the Rust domain core** ([ADR 0039](adr/0039-ports-and-adapters-posture.md)). The metapattern is applied where it pays off — a seam with more than one plausible implementation or a replaceable external dependency — and deliberately declined where it does not.

- **Ports (bind to the interface, never the implementation):** source adapters, AI providers ([ADR 0016](adr/0016-provider-neutral-ai-analysis-framework.md), [ADR 0028](adr/0028-multi-provider-ai-boundary.md)), the interpretative capability contracts (`Classifier`/`SimilarityProvider`/`Matcher`/`SemanticSearch`, [ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md) — the reference hexagon) and the lower-level `Embedder` engine port beneath them (`v0.45.0`), credentials, search/backups/pool ([ADR 0032](adr/0032-search-and-backup-boundaries.md)), import/export format adapters ([ADR 0018](adr/0018-import-export-boundaries.md)), licensing ([ADR 0017](adr/0017-license-gate.md)), and the UI↔Rust typed-command seam.
- **Core internals:** organized by domain slice (`companies/`, `feed/`, `notebooks/`, …) per [Modularization Design](modularization-design.md), not onion-style layers.
- **Storage is intentionally not a repository port.** SQLite is the single local-first source of truth; domain `storage/*` code may be SQLite-coupled. A `Repository` trait / domain-vs-row split is deferred until a real second backend, a sync/replication engine, or a non-SQLite durable target becomes planned scope (the storage-port trigger in [ADR 0039](adr/0039-ports-and-adapters-posture.md)). Do not add a port whose population is permanently one adapter.

Source adapters should return normalized records through a common interface. Adapters must declare source type, rate limits, supported markets, and allowed fetch mode. This port is being **realized** under [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (Architecture v2, ahead of v0.46.0) as a `SourceAdapter` trait + a registry. **Realized so far:** the trait and a descriptor `REGISTRY` (`src-tauri/src/source_adapters/registry.rs`) are the single source of truth for every adapter's static identity and capability metadata — id, display name, source URL, source type, fetch mode, supported markets, visibility tier, default poll interval, rate-limit policy, policy note. The source catalog (`storage/registry.rs`) and the visibility/enablement logic now read from it (collapsing a ~100-line SQL `CASE` ladder + scattered constants), and a drift-guard test binds the registry to the seed migrations. The **dispatch half** is also realized: the source-refresh path (`jobs/source_refresh.rs`) iterates a `RuntimeAdapter` registry (`runtime_adapters()`) instead of a hardcoded sweep list + per-id `match` — each adapter declares how it refreshes (`Feed` / `Calendar` / `Directory` / `Disabled`), so adding a runtime source is one registry entry. A shared **entity-resolution** module (`src-tauri/src/entity_resolution.rs`) owns the canonical-identity transforms: the pure text normalization every name/media matcher shares (`storage::feed_matching` delegates to it as the SSOT) and a canonical **story key** (`story_key`) that groups items about the same event across sources — deterministic and order-independent over the matched company set, the direct enabler for story clustering (`v0.46`). These transforms carry the full ADR 0049 invariant + golden coverage. The **ingestion pipeline spine** is realized in `storage/ingestion.rs` (AV3): the shared downstream stages every feed-item adapter ran in copied form — the **story-key stage** (derive the canonical cross-source `story_key` via `entity_resolution`) and the **outcome-recording stage** (mark the adapter healthy + record item counters) — are owned once there, and the feed-item ingest paths (Bankier media RSS, GPW ESPI/EBI listings, Bankier company komunikaty) all feed them. The persisted `story_key` (migration `0052`) is the concrete v0.46 clustering enabler: items from different sources about the same company event on the same day share a key. The spine is now **shared by every ingest path**: a unified **upsert stage** (`ingestion::upsert_feed_item` over a `NormalizedFeedItem`) replaces the three near-duplicate feed-item INSERTs (media RSS, GPW listings, Bankier company), and the **outcome-recording stage** is used by all five paths including the two calendar/event ingests. The per-adapter parse + the legitimately source-specific match-strategy (media fuzzy vs. structured ticker/ISIN) and dedup-strategy (duplicate signature vs. dedupe key) plug into this shared spine. The pipeline is thus: per-adapter parse → shared `entity_resolution` normalize/resolve → per-source dedup → shared `upsert_feed_item` → derive events/signals → shared `record_source_outcome`.

AI providers should implement provider-neutral interfaces. Gemini is already the first live AI provider for YouTube transcription and may be extended first for general analysis, but summarization, significance labeling, note extraction, and future AI workflows must remain behind provider/model/credential boundaries that can support OpenAI, Anthropic, and other providers later. General AI analysis is governed by [ADR 0016](adr/0016-provider-neutral-ai-analysis-framework.md).

The AI surface is split into two layers ([ADR 0035](adr/0035-two-layer-ai-and-local-interpretative-layer.md)). The **generative** layer (above) handles summarize/extract/assess and is provider-neutral and remote by default. The **interpretative** layer (`src-tauri/src/interpretation`) is on-device semantic lookup exposed as task-level capability contracts — `Classifier`, `SimilarityProvider`, `Matcher`, `SemanticSearch`. Feature code binds to a capability, never to a model: each capability has interchangeable implementations selected through a registry, with a deterministic **static** baseline (rules, lexical/FTS5, fuzzy) as the shipped default and an optional embedding-model implementation layered behind the same trait.

Beneath the model-backed implementations sits a second, lower-level boundary added in `v0.45.0`: the **`Embedder`** engine port (`embed(texts) -> vectors`, carrying `model_id` + `dim`), implemented by a pure-Rust on-device encoder (`candle`, default `intfloat/multilingual-e5-small`) whose weights are an optional one-time download. Its output lands in a disposable **vector store** — a `content_embeddings` table co-located with the main SQLite database, scanned by a pure-Rust brute-force cosine. The nearest-neighbour ranking step sits behind an explicit `VectorIndex` trait (`src-tauri/src/interpretation/vector_index.rs`, Architecture v2 / [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) decision 6) whose default `BruteForceVectorIndex` is the O(N) scan adequate at the current corpus scale; `AnnVectorIndex` (pure-Rust HNSW via `instant-distance`) is implemented behind that trait as the scale-time alternative — `sqlite-vec` is **rejected** (a C/native SQLite extension would break the `cargo-xwin` cross-build; `instant-distance`'s dependency tree is pure-Rust and cross-compiles to windows-msvc, validated). It swaps in with no `SimilarityProvider` consumer change. Because an HNSW built over per-call candidates costs more than one linear scan, its **production activation** is the persisted `content_embeddings` path (large, reused index), wired when scale justifies it (near `v0.53`). The **T4 behavioral scale gate** guards the linear-scan contract; any ANN activation must preserve top-k correctness. The capability boundary gives reversibility (switch back to static); the `Embedder` boundary gives model upgradeability (re-embed under a new `model_id`). The layer may only produce disposable, derived artifacts (the vector index is a cache rebuilt from canonical data), so the model is reversible to static with no consumer change and no data loss. `v0.45.0` wires only the model-backed `SimilarityProvider`; model `Classifier`/`Matcher`/`SemanticSearch` follow their consumers.

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
