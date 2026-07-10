-- History sweep as a durable job (ADR 0077 §3, T3.2). The backfill/manual
-- counterpart to the refresh-time detection sweep: it enqueues a full autopilot
-- run for every canonical periodic report whose period still lacks accepted
-- facts, through the shared `enqueue_extraction_run`. This migration adds the
-- durable sweep record and widens `autopilot_run.trigger` to name the sweep.
--
-- Append-only, idempotent, self-healing: the runner's version guard applies this
-- exactly once; `CREATE TABLE IF NOT EXISTS` converges on a fresh database, and
-- the `autopilot_run` rebuild replicates the 0055 schema verbatim except for the
-- widened trigger CHECK.

-- One row per history sweep: the durable record behind sweep progress (docs
-- swept, runs enqueued, budget) and the coverage panel's status line.
CREATE TABLE IF NOT EXISTS history_sweeps (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    -- backfill: chained from the end of `run_backfill`. manual: user-initiated
    -- ("Extract missing periods").
    trigger TEXT NOT NULL CHECK (trigger IN ('backfill', 'manual')),
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    -- How many canonical periods needed extracting when the sweep ran.
    candidates_total INTEGER NOT NULL DEFAULT 0,
    -- Runs freshly created or re-armed (Created | Rearmed).
    runs_enqueued INTEGER NOT NULL DEFAULT 0,
    -- Candidates whose extraction run was already terminal (DedupedTerminal).
    skipped_existing INTEGER NOT NULL DEFAULT 0,
    -- Candidates a storage error prevented enqueuing (Failed).
    runs_failed INTEGER NOT NULL DEFAULT 0,
    -- Why the sweep enqueued nothing, when it did: e.g. 'automation_off' for a
    -- company in mode `off` (ADR 0077 §3 amendment (c) — never a silent skip).
    skipped_reason TEXT NULL,
    -- The `autopilot_run` ids this sweep enqueued (JSON array), so sweep progress
    -- can derive per-run status without a parallel query.
    enqueued_run_ids_json TEXT NULL,
    -- A storage-level abort that failed the whole sweep.
    error TEXT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Access path for the latest sweep per company.
CREATE INDEX IF NOT EXISTS idx_history_sweeps_company_created
    ON history_sweeps (company_id, created_at);

-- Widen `autopilot_run.trigger` to allow 'history_sweep' (ADR 0077 §3 amendment
-- (a)): the history sweep enqueues full autopilot runs through the shared
-- `enqueue_extraction_run` with `trigger='history_sweep'`. SQLite cannot ALTER a
-- CHECK in place, so rebuild the table with the exact 0055 schema plus the wider
-- CHECK. `autopilot_run` has no foreign keys and exactly one index
-- (`idx_autopilot_run_company_state`), both recreated below.
CREATE TABLE autopilot_run_new (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    report_document_id TEXT NOT NULL,
    trigger TEXT NOT NULL DEFAULT 'detection'
        CHECK (trigger IN ('detection', 'manual', 'history_sweep')),
    mode TEXT NOT NULL CHECK (mode IN ('assist', 'autopilot')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'partial')),
    stage TEXT NOT NULL DEFAULT 'fetch'
        CHECK (stage IN ('fetch', 'extract', 'diff', 'cross_reference', 'notify')),
    summary_text TEXT,
    kpi_delta_json TEXT,
    report_diff_ref TEXT,
    cross_refs_json TEXT,
    produced_fact_ids_json TEXT NOT NULL DEFAULT '[]',
    notification_state TEXT NOT NULL DEFAULT 'unread'
        CHECK (notification_state IN ('unread', 'read', 'dismissed')),
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, report_document_id)
);

INSERT INTO autopilot_run_new (
    id,
    company_id,
    report_document_id,
    trigger,
    mode,
    status,
    stage,
    summary_text,
    kpi_delta_json,
    report_diff_ref,
    cross_refs_json,
    produced_fact_ids_json,
    notification_state,
    last_error,
    created_at,
    updated_at
)
SELECT
    id,
    company_id,
    report_document_id,
    trigger,
    mode,
    status,
    stage,
    summary_text,
    kpi_delta_json,
    report_diff_ref,
    cross_refs_json,
    produced_fact_ids_json,
    notification_state,
    last_error,
    created_at,
    updated_at
FROM autopilot_run;

DROP TABLE autopilot_run;
ALTER TABLE autopilot_run_new RENAME TO autopilot_run;

CREATE INDEX IF NOT EXISTS idx_autopilot_run_company_state
    ON autopilot_run (company_id, notification_state, created_at);
