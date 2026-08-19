-- Chunked staging drafts (ADR 0102 decisions 6-9, 11; epic #399 S6): a draft
-- is a sub-resource of a run, never a new run state (STAGEABLE_STATUSES and
-- the ADR 0098 dec. 6 lifecycle are untouched). `lease_epoch` binds a draft to
-- `kpi_ingest_runs.attempt_count` at open time -- that column already only
-- increments on a genuine claim (absent/expired lease), never on a same-
-- holder renewal (storage/kpi_ingest_runs.rs claim_run_on_connection), so it
-- is exactly the existing "epoch" a lease takeover bumps; no new run column
-- needed. `status='superseded'` is set lazily, the first time any operation
-- (open/append/finalize) notices a draft's lease_epoch no longer matches the
-- run's current attempt_count -- never eagerly by the takeover itself.

CREATE TABLE kpi_ingest_drafts (
    draft_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES kpi_ingest_runs(id) ON DELETE CASCADE,
    lease_epoch INTEGER NOT NULL,
    expected_observations INTEGER NOT NULL CHECK (expected_observations >= 1),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'superseded')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- One active draft per run (ADR 0102 dec. 6) -- a partial index so a
-- superseded row never blocks a fresh open.
CREATE UNIQUE INDEX idx_kpi_ingest_drafts_one_active_per_run
    ON kpi_ingest_drafts (run_id)
    WHERE status = 'active';

CREATE INDEX idx_kpi_ingest_drafts_run ON kpi_ingest_drafts (run_id);

-- A chunk's identity is (draft_id, chunk_index); `chunk_hash` is always
-- server-computed (never trusted from the client, ADR 0102 dec. 8) over the
-- canonical `payload_json` bytes -- the replay/conflict idempotency check.
CREATE TABLE kpi_ingest_draft_chunks (
    draft_id TEXT NOT NULL REFERENCES kpi_ingest_drafts(draft_id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
    chunk_hash TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    observation_count INTEGER NOT NULL CHECK (observation_count >= 1),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (draft_id, chunk_index)
);

CREATE INDEX idx_kpi_ingest_draft_chunks_draft ON kpi_ingest_draft_chunks (draft_id);
