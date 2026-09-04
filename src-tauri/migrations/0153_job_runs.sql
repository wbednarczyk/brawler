-- Activity occurrence history (ADR 0109, #133): one row per attempt of
-- background work, queue jobs and awaited direct work alike, so the Activity
-- panel can state what ran, when, for how long and with what outcome. Never
-- reused per id (one legal running -> terminal update); explicit GC keeps it
-- bounded (data-model.md § Job runs — retention at settlement/reconcile).

CREATE TABLE IF NOT EXISTS job_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    activity_key TEXT NOT NULL,
    run_key TEXT NOT NULL,
    kind TEXT NOT NULL,
    family TEXT NOT NULL,
    company_id TEXT REFERENCES companies(id) ON DELETE CASCADE,
    subject TEXT NOT NULL,
    target_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'retry_scheduled', 'interrupted')),
    attempt INTEGER NOT NULL DEFAULT 1,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    error TEXT
);

CREATE INDEX IF NOT EXISTS idx_job_runs_status ON job_runs (status);
CREATE INDEX IF NOT EXISTS idx_job_runs_finished_at ON job_runs (finished_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_job_runs_activity_key ON job_runs (activity_key);
