-- Immutable KPI ingest validation attempts (ADR 0098 dec. 4; epic #352, card
-- #361). Every validation of a staging revision — pass or fail — appends one
-- row here; NO UPDATE/DELETE path exists (append-only, the `valuation_runs`
-- idiom). This is what lets a rejected manifest's diagnostics survive a
-- re-stage/re-validate cycle (`kpi_ingest_runs.manifest_hash` only ever
-- points at the current READY attempt, and staging still zeroes it on every
-- new revision per migration 0138 — that freeze rule is unchanged).
--
-- Deliberately NO ALTER on `kpi_ingest_runs`: a column there could hold only
-- one outcome per run, but re-validating the SAME revision after
-- `invalidate_manifest` is legal (attempt+1) and a `failed` verdict must be
-- durable evidence a caller can read back, not a value the next attempt
-- overwrites in place.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS kpi_ingest_validation_attempts (
    -- kpiatt_{32 hex} -- the `generate_observation_id` id policy.
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES kpi_ingest_runs(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    -- COALESCE(MAX(attempt), 0) + 1 per (run_id, revision), computed inside
    -- the same transaction that inserts this row (storage/kpi_ingest_staging.rs).
    attempt INTEGER NOT NULL CHECK (attempt >= 1),
    outcome TEXT NOT NULL CHECK (outcome IN ('ready', 'failed')),
    manifest_hash TEXT NOT NULL,
    -- Canonical manifest bytes (fundamentals::kpi_manifest::SealedManifest) --
    -- sha256(manifest_json) == manifest_hash by construction.
    manifest_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (run_id, revision, attempt)
);

CREATE INDEX IF NOT EXISTS idx_kpi_ingest_validation_attempts_run_revision
    ON kpi_ingest_validation_attempts (run_id, revision);
