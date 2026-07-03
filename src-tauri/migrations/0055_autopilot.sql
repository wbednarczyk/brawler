-- Autonomous report pipeline (North Star, v0.49.0, ADR 0055): per-company
-- trust-ladder mode + a persisted record of each autonomous run. The global
-- confirm-before-commit guarantee is unchanged; these tables only scope
-- automation to companies the user explicitly opts in.
--
-- Append-only, idempotent, self-healing: CREATE TABLE IF NOT EXISTS so it
-- converges on every database regardless of prior state. financial_facts is
-- NOT migrated -- its confirmation_state already carries 'auto_unreviewed'
-- (designed additively in v0.34.0).

-- Per-company trust-ladder mode. A company with no row reads as 'off' (reads
-- tolerate a missing row -- a safe default).
CREATE TABLE IF NOT EXISTS company_autopilot_settings (
    company_id TEXT PRIMARY KEY,
    -- off: nothing automatic. assist: auto-fetch + auto-extract, but facts land
    -- 'pending' for user confirmation. autopilot: full loop; facts auto-committed
    -- as 'auto_unreviewed' (cited, flagged, reversible).
    mode TEXT NOT NULL DEFAULT 'off' CHECK (mode IN ('off', 'assist', 'autopilot')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- One row per autonomous run: the durable record behind the single notification,
-- the review queue, and run-level undo.
CREATE TABLE IF NOT EXISTS autopilot_run (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    -- The detected report this run processes.
    report_document_id TEXT NOT NULL,
    -- detection: refresh-completion hook. manual: user-initiated re-run.
    trigger TEXT NOT NULL DEFAULT 'detection' CHECK (trigger IN ('detection', 'manual')),
    -- The company autopilot mode captured at run time.
    mode TEXT NOT NULL CHECK (mode IN ('assist', 'autopilot')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'partial')),
    -- Current/last stage reached in the chained pipeline.
    stage TEXT NOT NULL DEFAULT 'fetch'
        CHECK (stage IN ('fetch', 'extract', 'diff', 'cross_reference', 'notify')),
    -- The composed result: what changed + cross-references.
    summary_text TEXT,
    kpi_delta_json TEXT,
    report_diff_ref TEXT,
    cross_refs_json TEXT,
    -- financial_facts ids this run created, so a single undo reverts exactly those.
    produced_fact_ids_json TEXT NOT NULL DEFAULT '[]',
    -- Drives the Today/Pulse "what changed" surface (ADR 0054).
    notification_state TEXT NOT NULL DEFAULT 'unread'
        CHECK (notification_state IN ('unread', 'read', 'dismissed')),
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- Detection is idempotent: at most one run per (company, report document).
    UNIQUE (company_id, report_document_id)
);

-- Access path for the notification/review queue (recent runs by company + state).
CREATE INDEX IF NOT EXISTS idx_autopilot_run_company_state
    ON autopilot_run (company_id, notification_state, created_at);
