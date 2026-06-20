-- Durable job queue (Architecture v2 / ADR 0050, ahead of the v0.49 autonomous
-- pipeline). Replaces fire-and-forget spawn_blocking tasks with a persisted
-- queue so work survives a crash mid-job and can be retried/resumed.
--
-- Local-first with no new dependency: it builds on the existing SQLite/r2d2/
-- migrations stack. A single in-process worker loop claims/executes/retries
-- rows while the app is open (background/closed execution remains out of scope).
--
-- Append-only, idempotent, self-healing: CREATE TABLE IF NOT EXISTS so it
-- converges on every database regardless of prior state.

CREATE TABLE IF NOT EXISTS job_queue (
    id TEXT PRIMARY KEY,
    -- Handler discriminator (e.g. 'content_embedding', 'source_refresh'); a
    -- registered JobHandler claims this kind.
    kind TEXT NOT NULL,
    -- Opaque JSON payload the handler deserializes.
    payload TEXT NOT NULL DEFAULT '{}',
    -- pending | running | succeeded | failed.
    status TEXT NOT NULL DEFAULT 'pending',
    -- Attempts made so far; incremented when a job is claimed, so a crash mid-run
    -- still counts against max_attempts and cannot loop forever.
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    -- Earliest time the job may be claimed (used for retry backoff).
    available_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- When the row was last claimed into 'running'; used to reclaim stale rows.
    locked_at TEXT,
    -- Last failure message, for observability.
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- The claim access path: the next claimable row is the oldest pending row whose
-- available_at has passed.
CREATE INDEX IF NOT EXISTS idx_job_queue_claimable
    ON job_queue (status, available_at);
