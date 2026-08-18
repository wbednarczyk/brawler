-- Version-aware re-extraction as a durable batch job (epic #398 Item B, ADR
-- 0100 consequence: "existing tagged filings do not widen by themselves").
-- A batch selects the company's successful ESEF-tier runs whose stored
-- `pipelineVersion` is below the current build and re-arms them through the
-- existing `rearm_run` + stage queue (`autopilot::enqueue_first_stage`), the
-- same primitives the history sweep's own re-arm uses -- but a SEPARATE
-- candidate selector (this table, not `history_sweeps`), so the sweep's own
-- "never re-arm a run that emitted facts" rule stays untouched.
--
-- Same doctrine as `history_sweeps` (migration 0062): one durable row per
-- batch, the record behind batch progress and the Coverage panel's status
-- line. Append-only, idempotent, self-healing (`CREATE TABLE IF NOT EXISTS`).
CREATE TABLE IF NOT EXISTS pipeline_reextraction_batches (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    -- Successful ESEF-tier runs found with a stale stored pipeline version.
    candidates_total INTEGER NOT NULL DEFAULT 0,
    -- Candidates successfully re-armed (rearm_run + enqueue_first_stage).
    runs_enqueued INTEGER NOT NULL DEFAULT 0,
    -- Candidates a storage error prevented re-arming.
    runs_failed INTEGER NOT NULL DEFAULT 0,
    -- The `autopilot_run` ids this batch re-armed (JSON array), so batch
    -- progress can derive per-run status without a parallel query -- the
    -- `history_sweeps.enqueued_run_ids_json` idiom.
    enqueued_run_ids_json TEXT NULL,
    -- A storage-level abort that failed the whole batch.
    error TEXT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Access path for the latest batch per company.
CREATE INDEX IF NOT EXISTS idx_pipeline_reextraction_batches_company_created
    ON pipeline_reextraction_batches (company_id, created_at);
