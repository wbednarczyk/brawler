# ADR 0055: Autonomous Report Pipeline — Trust Ladder, Orchestration, and Run Record

Status: Accepted (2026-06-24)

Milestone: `v0.49.0` (North Star). Epic: Radicle `9a607da`. Composes and does not change: report-document persistence ([ADR 0036](0036-report-document-storage-and-backfill.md)), AI KPI extraction (`v0.35.0`), report-over-report diff ([ADR 0052](0052-report-over-report-diff.md)), the claims tracker ([ADR 0040](0040-management-claims-tracker.md)), report-season/calendar events ([ADR 0044](0044-report-season-cockpit.md)), and the durable job queue ([ADR 0050](0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md), Decision 5). Surfaced in the Today/Pulse attention home ([ADR 0054](0054-mode-based-thesis-centric-shell.md)). Stays decision-support only ([ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md)). Roadmap: [roadmap.md](../roadmap.md).

## Context

Brawler's North Star (roadmap, North Star section) is a closed loop: a tracked company publishes a periodic report and the app — with no manual steps — detects the publication, fetches the document, extracts the figures, summarizes *what changed*, cross-references the result against open claims / research questions / evidence, and surfaces a **single notification**. This milestone is a **capstone**: every ingredient already exists (publication detection from calendar/events, report-document fetch + persistence, AI KPI extraction, report diff, digests, the claims tracker). The new ingredients are **autonomous orchestration** and a **trust ladder** that lets a user opt a specific company into more automation as they come to trust extraction quality.

Two hard constraints frame the design:

- **The confirm-before-commit guarantee does not change globally.** `financial_facts.confirmation_state` already carries an `auto_unreviewed` value (designed additively in `v0.34.0`, [data-model.md](../data-model.md) Company Fundamentals) precisely so autopilot is a per-company opt-in state, not a migration that flips everyone's default. Auto-committed facts must stay visibly flagged, fully cited, and reversible.
- **Local-first, app-open only.** Fetching/analyzing while the app is closed crosses into a hosted/scheduled service — the managed-AI paid frontier, explicitly out of core. Autopilot runs only while the app is open, on the durable job queue's single in-process worker.

The remaining piece of the Architecture-v2 durable-queue work (Radicle `68cda8e`, AV5) — moving source-refresh scheduling from frontend timers ([src/app/sourceScheduler.ts](../../src/app/sourceScheduler.ts)) to a **Rust-side scheduler** — lands in this milestone, because Rust-driven scheduling is what makes detection reliable independent of webview timer throttling.

## Decision

Build the autonomous pipeline as **detection → a chained durable-queue run → a persisted run record → one notification**, gated by a **per-company two-rung trust ladder**.

### 1. Trust ladder: a per-company mode with two "on" rungs

A per-company **autopilot mode** with three values:

- **`off`** (default for every company) — nothing automatic; current manual behavior.
- **`assist`** — on detection, auto-fetch the document and auto-extract KPIs, but produced facts land as **`pending`** for the user to confirm (the existing confirmation flow). The user gets the *work* automatically while keeping the *commit* decision. This is the rung a user sits on **until they trust extraction quality**.
- **`autopilot`** — the full loop: produced facts are auto-committed as **`auto_unreviewed`** (cited, flagged, reversible) and surfaced for optional review. The global confirm-before-commit default is **not** flipped; this is one company opting in.

Stored in a dedicated `company_autopilot_settings` table (not a `companies` column — keeps the core identity table untouched and the setting independently evolvable). Single per-company control rather than per-step toggles: per-step was rejected as premature UI/state/test surface for a single-user app; the two rungs already give the one meaningful trust boundary (trust extraction → trust auto-commit).

### 2. Orchestration: chained durable-queue jobs stamped with a run id

The pipeline is **one durable-queue job per stage**, each enqueuing the next on success, all stamped with a parent `autopilot_run` id — **not** one monolithic job. Stages: `fetch` → `extract` → `diff` → `cross_reference` → `notify`. This is exactly what the durable queue ([ADR 0050](0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md)) was built for: a crash mid-extract resumes **that stage only**, each stage retries with backoff independently, and progress is observable per stage. Each stage **reuses the existing service** (report fetch, KPI extraction, diff, cross-reference) rather than reimplementing it; the autopilot job is a thin orchestrator that calls the same code the manual flow calls. A stage failure that exhausts retries marks the run `failed`/`partial` and still surfaces a notification describing how far it got (no silent dead-end).

### 3. Detection: event-driven off source-refresh completion

A **new report publication** is detected **event-driven**, at the completion of a source-refresh job: when a refresh ingests a new periodic-report `report_document` for a company whose autopilot mode ≠ `off`, the completion hook creates an `autopilot_run` row and enqueues the first stage. No separate polling job and no watermark to maintain — the run reacts as fast as ingestion. The **Rust-side scheduler** (AV5, `68cda8e`) owns the *refresh cadence* by enqueuing `source_refresh` jobs onto the durable queue (replacing the frontend timer); detection then rides each refresh's completion. Detection dedups on `(company_id, report_document_id)` so a given report triggers at most one run (idempotent — re-ingesting the same document does not re-fire).

### 4. Run record: a persisted `autopilot_run` entity

Each run is a **persisted `autopilot_run` row** (new table, new forward migration), not derived on the fly from `auto_unreviewed` facts. It records the company, the report document, the trigger, the mode at run time, the current/last stage, status, the produced summary / KPI deltas / report-diff reference / cross-references, the **ids of facts the run produced** (for run-level undo), and a notification state. This gives three things a derived view cannot guarantee:

- **One stable notification** per run, surfaced in the Today/Pulse "what changed" home ([ADR 0054](0054-mode-based-thesis-centric-shell.md)) — the North Star "single notification."
- **A real review queue** — the unreviewed-facts review surface is driven by run records (and `auto_unreviewed` facts), with a stable run identity to group by.
- **Run-level reversibility** — "undo this autopilot run" reverts exactly the facts that run produced (recorded on the row), beyond per-fact revert.

Reversibility reuses the existing fact supersede/reject mechanics; the run record only adds the grouping needed to undo a whole run at once.

### 5. Decision-support boundary unchanged

The composed notification reports *what changed* and *what to verify* (deltas, diff, cross-referenced claims/questions/evidence) — never a buy/sell/hold judgment ([ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md)). Auto-confirmed facts carry the `auto_unreviewed` provenance so the UI keeps flagging them as not-yet-human-reviewed.

## Consequences

- **New schema:** `company_autopilot_settings` (per-company mode) and `autopilot_run` (run record), both via append-only idempotent forward migrations. `financial_facts` is **not** migrated — `auto_unreviewed` already exists.
- **AV5 closes here:** a Rust-side scheduler (`jobs/scheduler.rs`, startup blocking thread) owns the refresh cadence, re-arming `scheduled_source_refresh` / `scheduled_registry_refresh` jobs on the durable queue via a new `JobQueueStore::reschedule` stable-id primitive (one row per recurring job, never a row-per-fire). It gates exactly as the UI did (license `canUseApp` + poll interval + enabled adapters) and publishes a per-adapter next-due `SchedulerStatus` (read by `get_scheduler_status`). The **frontend `setTimeout`/`setInterval` refresh driver is retired** (a webview timer is throttled when hidden) — the UI now only mirrors the scheduler snapshot for its "next refresh at" display and reloads views when a refresh has fired. (`68cda8e` resolved by this epic; feed-prune remains a separate small frontend timer, a follow-up.)
- **New job kinds** on the durable queue for the five stages, each idempotent and resumable; new typed commands for reading runs, setting per-company mode, reviewing/undoing a run (recorded in [contracts.md](../contracts.md)).
- **Today/Pulse** gains autopilot-run notifications as a first-class "what changed" input — the home ADR 0054 was designed around.
- **AI cost is bounded by design:** auto-extract runs at most once per detected report (dedup on `(company_id, report_document_id)`), only for companies explicitly opted in, and only when AI credentials + the extraction capability are configured; with none configured, `assist`/`autopilot` degrade to fetch + diff (deterministic) and flag extraction as unavailable rather than looping.

## Risks and mitigations

- **Runaway automation / cost.** Mitigated by per-company opt-in (default `off`), one-run-per-report dedup, app-open-only execution, and the queue's capped-backoff retry ceiling.
- **Auto-confirmed wrong facts.** Mitigated by the `auto_unreviewed` flag (never silently `confirmed`), full citation, the review queue, and run-level undo.
- **Detection misses or double-fires.** Mitigated by idempotent dedup on `(company_id, report_document_id)` and by riding refresh completion (no watermark drift).
- **Partial run leaves stale state.** Mitigated by per-stage retry/resume and a notification that honestly reports a `partial`/`failed` run and how far it got.
- **"Autopilot" reads as advice.** Mitigated by the decision-support framing: the notification is *what changed / to verify*, evidence-linked, never prescriptive ([ADR 0042](0042-advisory-verdict-port-and-open-core-boundary.md)).

## Status notes

Accepted 2026-06-24 at `v0.49.0` planning (maintainer sign-off on the four load-bearing forks: two-rung ladder, chained jobs + run id, event-driven detection, persisted run record). Decisions propagated into [roadmap.md](../roadmap.md), [data-model.md](../data-model.md), [contracts.md](../contracts.md), and [architecture.md](../architecture.md) as part of planning, ahead of implementation. Work tracked under epic `9a607da` (sub-issues: trust-ladder contracts `e170a66`, detection `fb74d12`, chain `05ebf87`, auto-confirm + review queue `312d3a7`, composed notification `6a621a3`, scheduler/AV5 `68cda8e`, tests/docs `542aff0`).
