-- KPI staged observations + immutable commit receipts (ADR 0098 decisions 3,
-- 4, 5; epic #352, card #359). Staging is run-owned PRE-CANONICAL workflow
-- state -- no fact reader ever sees it (dec. 3); a proposal becomes a fact
-- only through the commit transaction (#363).

PRAGMA foreign_keys = ON;

-- Run-owned period descriptor (B2 sol review, round 2). A `financial_periods`
-- row legally does not exist until the commit transaction creates it (ADR
-- dec. 5), yet staging needs a stable period identity to stage against, so
-- the run itself carries a durable natural-key descriptor mirroring
-- `financial_periods` `UNIQUE(company_id, fiscal_year, period_type)`.
-- `period_type` vocabulary matches `financial_periods.period_type`
-- (data-model.md § financial_periods / storage/financials.rs:525): FY, H1,
-- H2, Q1-Q4, 9M, M01-M12. All-or-none (both columns set or both NULL) and
-- descriptor<->period_id consistency are enforced in Rust
-- (storage/kpi_ingest_runs.rs) -- SQLite CHECK cannot express a two-column
-- all-or-none rule via ALTER TABLE ADD COLUMN, only single-column ones.
ALTER TABLE kpi_ingest_runs ADD COLUMN period_fiscal_year INTEGER;
ALTER TABLE kpi_ingest_runs ADD COLUMN period_type TEXT CHECK (
    period_type IS NULL OR period_type IN (
        'FY', 'H1', 'H2', 'Q1', 'Q2', 'Q3', 'Q4', '9M',
        'M01', 'M02', 'M03', 'M04', 'M05', 'M06', 'M07', 'M08', 'M09', 'M10', 'M11', 'M12'
    )
);

-- LLM-proposed observations, run-owned (dec. 3). One run = one period
-- (B2 sol review round 2) -- observations carry no period of their own, the
-- run's period_id / period descriptor above is the period context.
-- `revision` is shared with `kpi_ingest_runs.manifest_revision` (B1 sol
-- review): `stage_observations` atomically bumps the run's counter and
-- inserts a COMPLETE snapshot under the new revision in one transaction, so
-- two racing stagers serialize on the run row rather than interleaving.
CREATE TABLE IF NOT EXISTS kpi_staged_observations (
    -- kpiobs_{32 hex} -- opaque, sha256(run_id, revision, ordinal, now_nanos),
    -- mirroring kpi_ingest_runs' id policy.
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES kpi_ingest_runs(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    -- 0-based, contiguous within (run_id, revision) -- the store enforces the
    -- no-gaps invariant; SQLite CHECK cannot express it declaratively.
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),

    -- Raw, exactly as the document states it (M4 sol review) -- kept
    -- separate from the normalized_* columns below for audit; never
    -- normalized or validated by the store.
    raw_label TEXT NOT NULL,
    raw_value TEXT NOT NULL,
    raw_currency TEXT,
    raw_unit_scale TEXT,

    -- Normalized. normalized_value is decimal-exact TEXT (the valuation_runs
    -- convention). currency is normalized at the store write boundary
    -- (storage/kpi_ingest_staging.rs mirrors storage::financials::
    -- normalize_currency, which is private to that module).
    normalized_value TEXT,
    currency TEXT,
    unit_scale TEXT CHECK (unit_scale IS NULL OR unit_scale IN ('ones', 'thousands', 'millions')),

    -- Period dimensions (ADR 0098 dec. 3: measure window, attribution,
    -- scope). Deliberately STRICTER than financial_facts, which does not
    -- CHECK variant/measure_window -- staging is where the hardness belongs,
    -- an intentional asymmetry (this is a proposal, not yet trusted).
    -- variant and data_quality are NOT per-observation: variant is unnamed by
    -- dec. 3, data_quality has been run-level since migration 0137.
    measure_window TEXT CHECK (
        measure_window IS NULL OR measure_window IN ('flow', 'point_in_time', 'trailing', 'cumulative', 'duration')
    ),
    attribution TEXT CHECK (attribution IS NULL OR attribution IN ('total', 'owners_of_parent', 'nci')),
    scope TEXT CHECK (scope IS NULL OR scope IN ('standalone', 'consolidated')),

    -- Mapping candidate; `no_definition` is the existing mapping-failure
    -- token (jobs/record_financial_facts.rs outcome vocabulary).
    metric_key_candidate TEXT,
    mapping_status TEXT NOT NULL DEFAULT 'unmapped' CHECK (mapping_status IN ('unmapped', 'mapped', 'no_definition')),

    -- Structural citation locator -- a NEW shape vs financial_facts' flat
    -- `citation` TEXT: page/table/row/quote. Nullable in storage --
    -- mandatory-per-fact is validation's job (#361), not the schema's, so a
    -- repair proposal can be staged with an incomplete citation plus a
    -- diagnostic code.
    citation_page INTEGER CHECK (citation_page IS NULL OR citation_page >= 1),
    citation_table TEXT,
    citation_row TEXT,
    citation_quote TEXT,

    -- Per-observation validation state (reuses the 0057 provenance
    -- vocabulary); validation_codes_json's stable code registry lands in
    -- #361. Always starts 'none'/NULL at staging time -- only
    -- apply_validation_results ever sets these.
    validation_state TEXT NOT NULL DEFAULT 'none' CHECK (validation_state IN ('none', 'passed', 'unreviewed', 'flagged')),
    validation_codes_json TEXT,

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    UNIQUE (run_id, revision, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_kpi_staged_observations_run_revision
    ON kpi_staged_observations (run_id, revision);

-- Immutable commit outcome (dec. 5): append-only, zero UPDATE path in the
-- store (the `valuation_runs` idiom). One receipt per run; idempotent replay
-- of a committed manifest returns this stored row rather than re-executing
-- the commit write primitives (dec. 5 -- a replay would answer differently).
CREATE TABLE IF NOT EXISTS kpi_ingest_commit_receipts (
    -- kpircpt_{32 hex}.
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES kpi_ingest_runs(id) ON DELETE CASCADE,
    manifest_hash TEXT NOT NULL,
    manifest_revision INTEGER NOT NULL CHECK (manifest_revision >= 1),
    terminal_status TEXT NOT NULL CHECK (terminal_status IN ('complete', 'partial')),
    period_id TEXT REFERENCES financial_periods(id) ON DELETE SET NULL,
    accepted_count INTEGER NOT NULL CHECK (accepted_count >= 0),
    -- Durable format discriminator for outcomes_json (M5 sol review round 2):
    -- bumped only if the shape below ever changes, so a reader can branch on
    -- it forever instead of guessing from content.
    outcomes_schema_version INTEGER NOT NULL DEFAULT 1,
    -- Versioned array of {observationId, revision, ordinal, metricKey,
    -- factId, outcome, detail?}. NOT a reuse of jobs::record_financial_facts
    -- ::FactOutcome (that type has no fact_id -- M5 sol review). `outcome`
    -- reuses the same outcome vocabulary: created|reobserved|upgraded|
    -- divergent|no_definition|implausible|identity_violation.
    outcomes_json TEXT NOT NULL,
    committed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    UNIQUE (run_id)
);
