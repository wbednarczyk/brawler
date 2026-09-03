# ADR 0109: Activity Center — Occurrence Ledger Over Existing Background Work, Topbar Placement

Status: Accepted (2026-09-03, owner approval of the F3d plan; epic #410 / #133)

Amends [ADR 0091](0091-failure-path-and-real-state-testing.md) decision 1 (surface exclusivity
now governs *notification*, not *status*) and [ADR 0097](0097-toasts-are-action-feedback-only.md)
decision 4 (the topbar Activity control is a work-in-progress signal, not an attention channel).
Builds on [ADR 0050](0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md) (durable
queue), [ADR 0059](0059-worker-pools-and-queue-fairness.md) (lanes), [ADR 0036](0036-report-document-storage-and-backfill.md)
(backfill progress) and [ADR 0107](0107-company-view-paradigm.md) (the `Tool` union as the
navigation target).

## Context

Brawler runs its background work through fifteen registered `job_queue` kinds plus a handful of
awaited commands (manual source refresh, transcript fetch, direct history backfill / aggregator /
registry refresh). Each surface shows its own corner of it — the Coverage drain counter, Sources
health, the transcript list, run rows on Today, `job_failed` attention events — and nothing answers
"what is the app doing right now, what just finished, take me there" (#133).

The queue cannot answer it alone. `job_queue` (migration 0051) has no company, start or finish
column; `locked_at` is cleared on every settle path and `updated_at` is touched by reschedule,
claim, defer, retry and startup reclaim; recurring jobs reuse one stable id per target, so the row
is overwritten each run and holds no history. `sources_in_flight` is the worker serialization
lock, not a refresh registry — the manual refresh path never takes it. A non-terminal domain row
(`autopilot_run`, `history_sweeps`, `transcript_jobs`) does not mean live execution: a crash can
strand it, and only the queue row is authoritative for "running". Read on the owner's live
database (2026-09-03): 4 151 queue rows, 0 pending/running; residue of retired kinds still in
`failed`.

## Decisions

1. **Identity = the domain task, never a company bucket.** One activity item per task: a
   sweep or re-extraction batch is a parent with member progress (its child runs suppressed as
   separate items); an automatic report reading triggered by detection/manual is one item per
   document; extraction jobs are one item per document; refreshes one per adapter/company; KPI
   ingest one per run (its validate/commit jobs fold in); a transcript one per job. Each item
   carries a typed `activity_key`, a family, a raw subject (ticker, adapter name, document title
   — never prose, ADR 0087 dec. 4) and a typed navigation target expressed in the existing
   Spółka `Tool` union (`{t:"dokumenty", documentId}` lands on the item itself) or a screen.
   A typed resolver per kind is gate-enforced over `registered_kinds()` (the `failure_surface`
   pattern); a registered kind with a malformed payload becomes an explicit `corrupted` item;
   rows of unregistered (retired) kinds are excluded. The panel groups items per company; the
   grouping is presentation, not identity.

2. **`job_runs` — an occurrence history, no new registry.** Migration `0153` adds a table whose
   identity is never reused: one row per attempt with `activity_key`, family, `company_id`
   (first-class, `ON DELETE CASCADE`), subject, target, status
   (`running | succeeded | failed | retry_scheduled | interrupted`), attempt, real `started_at` /
   `finished_at`, error. It is written at exactly two seams: the queue worker's single dispatch
   path and the direct-activity registry (decision 3). Dispatch is made atomic and panic-safe:
   `begin_attempt` runs after the source lock (an insert failure skips the handler and defers the
   claim; a deferred job writes nothing); the handler runs under `catch_unwind` so a panic takes
   the ordinary retry/terminal path; the queue row and that exact occurrence settle in ONE
   `BEGIN IMMEDIATE` transaction. Retention runs in that settle transaction and after startup
   reconciliation — never on insert: newest 500 finished rows by `(finished_at DESC, id DESC)`,
   nothing older than 30 days. "Append-only" in this repo names the migration rule; this table has
   one legal `running → terminal` update and explicit GC.

3. **Direct-activity registry for awaited work.** An in-memory RAII registry on `AppState`
   (`ActivityGuard`: Drop on unwind = `interrupted`) instrumented at the shared cores so Tauri
   and MCP callers are covered alike: per-adapter refresh inside the refresh sweep, the aggregator
   pull, the direct history backfill, the direct/stale-checked registry refresh, the transcript
   runner. Queue handlers call the unwrapped cores (no double count). Excluded: KPI ingest over
   MCP — the run's live lease is its activity signal; the headless-only fundamentals rebuild.
   `sources_in_flight` is never read as an activity signal.

4. **Honest liveness + safe startup reconciliation.** `active` = a queue row literally `running`
   with an open occurrence, a registry entry, or a leased KPI run; `queued` = queue `pending`
   (retry backoff included) or an unleased KPI run; `stalled` = a non-terminal domain row with
   no live backing job (rare after reconcile). Startup order, pinned after the existing KPI-run
   reclaim and generic queue reclaim and before any worker lane starts: open occurrences →
   `interrupted`; transcript `running` → `failed` (`interrupted`); a report-reading run with ANY
   stage job pending/running is left alone, otherwise a non-terminal run whose reachable stage job
   is terminally failed, absent, or succeeded without a live successor → `failed` with that job's
   error; a sweep/batch whose parent job is absent or terminal while the domain row is
   non-terminal → `failed`; KPI runs are never terminalized here. Re-arming a stuck `pending` run
   and transactional sweep create+enqueue are a separate card, not this ADR.

5. **Status ledger, not a notification channel (amends ADR 0091 dec. 1 and ADR 0097 dec. 4).**
   The Activity panel lists outcomes — including failures with their raw error and an "Otwórz"
   to the item's home — as *state*. Notification stays exclusive: a terminal failure is announced
   once, on the surface `jobs::failure_surface` names (Today event, Sources health row, run card).
   The topbar control signals work in progress only (`active` / `queued` counts, last finished
   time) and never a failure count; the Today badge remains the only ambient attention channel.

6. **Placement and reach.** A topbar icon next to the Sources pill opens the panel as a `Modal`
   (the palette's primitive; F3c focus contract). While any `[aria-modal]` dialog is open the
   global shortcut dispatcher suppresses app shortcuts (the dialog keeps Escape). The command
   palette carries `Otwórz aktywność`; no dedicated shortcut. Read model: one pool checkout per
   call, SQL-bound candidate sets (window 7 days, cap 40 after the per-key collapse), polled every
   15 s for the summary (independently of the scheduler-status read) and every 5 s while the
   panel is open, through a controller with the attention-controller posture (one request at a
   time, last-known-good, request sequencing).

## Rejected

- **A new task registry table the jobs "publish to"** — the queue already is the durable
  registry; a second one drifts. Only the occurrence history is new, and it is written at the
  queue's own settle point.
- **`job_queue.updated_at` as the finish time / "latest state" as history** — recurring rows are
  overwritten; the timestamp is a transition proxy touched by five paths. Honest only for the
  current attempt, useless for "what ran".
- **`sources_in_flight` as the running signal** — a serialization lock taken by every keyed
  queue kind and skipped by the manual path; it double-counts and misses at once.
- **Company-bucket rows ("PZU · 12 read, 1 failed")** — hides which document failed and why;
  the owner's per-company grouping is kept at the presentation layer instead.
- **A failure counter on the topbar icon / a sidebar badge** — recreates the triple-announcement
  ADR 0097 removed.
- **A non-modal anchored popover primitive** — a new primitive for one consumer; `Modal` plus
  the one-modal shortcut policy covers the keyboard model.
- **Extending `SchedulerStatus` with activity counts** — mixes next-due with liveness; a separate
  summary command costs two indexed counts.

## Consequences

- New commands `list_activity` / `get_activity_summary` (contracts.md § Activity), DTOs
  `ActivityView` / `ActivityItem` / `ActivityFamily` / `ActivityTarget` / `ActivitySummary`;
  migration 0153; `storage/activity_reads.rs`, `storage/activity_registry.rs`,
  `jobs/activity_identity.rs`, `jobs/activity_reconcile.rs`; frontend `ActivityIndicator`,
  `ActivityPanel`, `useActivityController`.
- Gates added: registered-kind → identity enumeration; identity per real payload shape; atomic
  settle + panic containment; per-branch startup reconciliation; retention bound; checkout-bounded
  and 100k-row volume reads; modal shortcut suppression; palette copy; PL/EN label parity.
- `formatJobKindDisplayName` (Today's raw-kind label) stays and gains its missing kinds with a
  parity test; the Activity families carry their own PL/EN labels.
- Docs: contracts, ui-information-architecture (topbar + panel), ui-flows (Activity flow +
  destination table), product-spec, architecture (queue vs awaited work, reconciliation), data-model
  (legacy `jobs` table retired in prose; `job_runs`), ux-journeys (journey-independent utility),
  wiki.
- Follow-up card: re-arm stuck `pending` report-reading runs + transactional sweep
  create+enqueue.
