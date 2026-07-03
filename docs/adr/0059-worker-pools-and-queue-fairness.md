# ADR 0059: Worker Pools, Per-Source Serialization, Per-Provider Concurrency, Chunked Refresh, Dead-Letter

Status: Accepted (2026-07-01)

Amends [ADR 0050](0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (durable job queue),
[ADR 0055](0055-autonomous-report-pipeline-trust-ladder.md) (autopilot pipeline), and
[ADR 0028](0028-multi-provider-ai-boundary.md) (adds a per-provider concurrency decision).

## Context

The durable job queue ([ADR 0050](0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md))
ran a **single worker thread** draining every job kind FIFO. Real owner use surfaced a hard failure:
the scheduled `bankier-company-komunikaty` refresh fans out over ~100 tracked companies serially
(1 s between each) in **one** job, monopolizing the worker for minutes. Autopilot pipeline jobs
(`autopilot_stage`) sat `pending` behind it and never ran — the autopilot feature was effectively
unusable despite being correct (confirmed in the owner's live DB: 2026 runs created, never processed).

Two compounding defects:

- **No isolation.** One slow kind starves latency-sensitive kinds through the shared worker.
- **Immortal poison job.** `reclaim_stale_running` reset every `running` row to `pending` on startup
  **without checking `max_attempts`**, so a job that hangs (never reaches `mark_failed`) was resurrected
  every restart — the bankier refresh reached `attempts=15` against `max_attempts=2` and re-hung each
  session, permanently eating the worker.

## Decision

Two orthogonal mechanisms — **pools** for throughput isolation, **locks/limits** for safety — plus
chunking the heavy refresh and a dead-letter guard. Threads (not async) are kept deliberately: at this
scale (~8 workers on a local single-user app) idle threads are trivial, and the AI provider limit, not
the thread count, is the real ceiling. The pools/locks are designed **orthogonal to threads-vs-async**,
so a future async migration is a worker-loop swap without discarding them.

### 1. Isolated worker pools (lanes)

The single worker becomes **named lanes**, each a set of job kinds drained by its own dedicated
threads. `claim_next_for_kinds(kinds)` scopes the atomic claim to a lane's kinds. A slow refresh can no
longer starve autopilot, because autopilot has its own threads. Default layout (worker counts constant
now, settings-driven in decision 5):

| Lane | Kinds | Workers |
|---|---|---|
| `sources` | `scheduled_source_refresh`, `scheduled_registry_refresh`, `source_company_refresh` | 2 |
| `autopilot` | `autopilot_stage` | 3 |
| `ai` | `ai_analysis`, `kpi_extraction`, `claim_extraction`, `research_brief`, `research_digest` | 2 |
| `indexing` | `content_embedding` | 1 |

Every registered kind belongs to exactly one lane. Transcription is not yet a queue kind (out of scope;
it joins `ai` if migrated).

### 2. Per-source serialization lock (exactly one)

A source may be refreshed by **at most one worker at a time** (politeness + no duplicate work). An
in-memory keyed guard (`try_acquire_source(adapter_id) -> Option<Guard>`, released on `Drop`) enforces
this; a worker that cannot acquire re-queues the job with a short backoff and frees its thread. Chosen
**per-source**, not per-host (two different sources on the same host may run concurrently) — the mechanism
does not preclude tightening to per-host later.

### 3. Chunked source refresh (resumable)

A company-scoped scheduled refresh (bankier-company) becomes a **planner** that enqueues one idempotent
`source_company_refresh` job **per company** instead of one monolith looping all companies. The
per-source lock serializes them (politeness preserved); other lanes run alongside; and unfinished
per-company jobs persist across restarts (resumable — no "restart from zero"). The heavy monolith that
starved the worker ceases to exist.

### 4. Dead-letter poison jobs

`reclaim_stale_running` now **dead-letters** a `running` row whose `attempts >= max_attempts` (mark
`failed`) instead of resurrecting it; rows with attempts left resume as before. A job that hangs and
keeps getting reclaimed is retired after its attempts are spent, so it can never permanently starve the
queue across restarts.

### 5. Per-provider AI concurrency limit + configurable pools

A concurrency limit is enforced **per AI provider** (each `provider_id` its own semaphore), acquired on
the provider-call path so it is **shared across the `autopilot` and `ai` lanes** (both consume the same
provider quota). This — not the thread count — bounds AI cost/rate. Lane worker counts and the
per-provider limit are user settings (defaults: sources 2 / autopilot 3 / ai 2 / indexing 1; provider
concurrency 2) with tolerant defaults and a forward migration, following the `db_max_connections` pattern.

## Consequences

- Autopilot is never starved by source refresh; a hung job is retired, not immortal; heavy refreshes are
  resumable and polite. The r2d2/WAL connection pool already supports multiple concurrent workers
  ([ADR 0032](0032-search-and-backup-boundaries.md)); SQLite serializes writes (milliseconds), fine at
  this scale.
- Cross-cutting infrastructure epic (like Test-architecture / Architecture-v2) — **no `milestone:` label**,
  no roadmap renumbering.
- A starvation-guard test (autopilot lane processes its job with an older source job queued) locks the
  fix; the dead-letter guard has its own test.

## Guardrail (ADR 0045)

When a new **category** of durable work is added, it gets its own lane (or a deliberate lane assignment)
and — if it shares an external resource (a host, a provider) — the corresponding lock/limit. A single
undifferentiated worker/pool that lets a slow kind starve a latency-sensitive one is the regression this
prevents. Recorded as a Definition-of-Done item in [engineering-workflow.md](../engineering-workflow.md).

## Status notes

Accepted 2026-07-01, discovered while validating the v0.49 autopilot against the owner's real database
(the `d60305c` ranking fix landed first; then autopilot ran end-to-end with real Gemini but its runs sat
`pending` behind the bankier moloch — this ADR). Design co-decided with the maintainer: separate lanes,
**per-source lock = exactly 1** (not per-host), **per-provider** AI limit, chunk the moloch, dead-letter
poison jobs, threads-now/async-migratable. Delivered in slices: **Slice 1 (lanes + dead-letter reclaim)**
done — the starvation fix; **Slice 2 (per-source lock + chunked refresh)** done — `try_acquire_source`
(RAII, exactly-one), `JobQueueStore::defer` (source-busy requeue, no attempt consumed), and the
`source_company_refresh` planner that retires the bankier monolith; **Slice 3 (per-provider limit +
configurable settings)** done — a `GatedAnalysisProvider` decorator backed by one `tokio::sync::Semaphore`
per provider id in `AppState` (built via `jobs::build_gated_analysis_provider`, shared across the autopilot
+ ai lanes), plus the `queue` settings (`sources_workers`/`autopilot_workers`/`ai_workers`/
`ai_provider_concurrency`, migration `0056`, Settings → Database → **Background work**).
