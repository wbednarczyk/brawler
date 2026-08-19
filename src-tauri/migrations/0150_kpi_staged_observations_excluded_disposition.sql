-- `excluded` joins `kpi_staged_observations.mapping_status` (ADR 0102
-- decision 1; epic #399 S5): full-capture doctrine needs a disposition for
-- "deliberately not mapped, with a reason" distinct from an accidental
-- `unmapped` (which stays Flagged). `exclusion_reason` carries that reason,
-- required exactly when `mapping_status = 'excluded'` (enforced in Rust,
-- storage/kpi_ingest_staging.rs -- SQLite CHECK cannot express a
-- column-conditional-on-another-column rule cleanly here either).
--
-- Migration 0138's `mapping_status` CHECK is baked at CREATE TABLE time, so
-- SQLite cannot widen it in place -- the standard table rebuild (the 0119/
-- 0123 shape; `rebuild_preserves_rows_constraints_indexes` in
-- storage/tests/migration_safety.rs proves no row is lost). Nothing else
-- about the table changes: same columns, same ids, same indexes, same FK.

PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS kpi_staged_observations_0150 (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES kpi_ingest_runs(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),

    raw_label TEXT NOT NULL,
    raw_value TEXT NOT NULL,
    raw_currency TEXT,
    raw_unit_scale TEXT,

    normalized_value TEXT,
    currency TEXT,
    unit_scale TEXT CHECK (unit_scale IS NULL OR unit_scale IN ('ones', 'thousands', 'millions')),

    measure_window TEXT CHECK (
        measure_window IS NULL OR measure_window IN ('flow', 'point_in_time', 'trailing', 'cumulative', 'duration')
    ),
    attribution TEXT CHECK (attribution IS NULL OR attribution IN ('total', 'owners_of_parent', 'nci')),
    scope TEXT CHECK (scope IS NULL OR scope IN ('standalone', 'consolidated')),

    metric_key_candidate TEXT,
    -- `excluded` is new (ADR 0102 dec. 1): a deliberate "captured, not
    -- mapped, with a reason" disposition -- distinct from an accidental
    -- `unmapped`, which stays validated as Flagged.
    mapping_status TEXT NOT NULL DEFAULT 'unmapped' CHECK (
        mapping_status IN ('unmapped', 'mapped', 'no_definition', 'excluded')
    ),
    -- Required exactly when mapping_status = 'excluded' (Rust-enforced at
    -- the write boundary); NULL for every other disposition.
    exclusion_reason TEXT,

    citation_page INTEGER CHECK (citation_page IS NULL OR citation_page >= 1),
    citation_table TEXT,
    citation_row TEXT,
    citation_quote TEXT,

    validation_state TEXT NOT NULL DEFAULT 'none' CHECK (validation_state IN ('none', 'passed', 'unreviewed', 'flagged')),
    validation_codes_json TEXT,

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    UNIQUE (run_id, revision, ordinal)
);

INSERT OR IGNORE INTO kpi_staged_observations_0150
SELECT
    id, run_id, revision, ordinal, raw_label, raw_value, raw_currency, raw_unit_scale,
    normalized_value, currency, unit_scale, measure_window, attribution, scope,
    metric_key_candidate, mapping_status, NULL AS exclusion_reason,
    citation_page, citation_table, citation_row, citation_quote,
    validation_state, validation_codes_json, created_at, updated_at
FROM kpi_staged_observations;

DROP TABLE kpi_staged_observations;

ALTER TABLE kpi_staged_observations_0150 RENAME TO kpi_staged_observations;

CREATE INDEX IF NOT EXISTS idx_kpi_staged_observations_run_revision
    ON kpi_staged_observations (run_id, revision);

PRAGMA foreign_keys = ON;
