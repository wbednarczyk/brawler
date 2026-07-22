-- Aggregator FALLBACK-sourcing as a distinct extraction outcome (v0.59.0,
-- ADR 0085 amendment 2026-07-21, owner decision).
--
-- The amendment reverses "the witness may never create a fact on its own": when
-- every deterministic tier read nothing for a slot, a clearly-labelled
-- third-party number beats no number at all. Its condition 3 requires that
-- agreement, disagreement and fallback-sourcing be **three distinct recorded
-- states** — a fallback must never read as though the issuer's filing had been
-- parsed successfully.
--
-- `reason_code` is a CHECK-constrained vocabulary, and SQLite cannot alter a
-- CHECK in place, so this is the standard table rebuild. Deliberately NOT solved
-- by overloading `emitted` (which would erase the distinction the amendment asks
-- for) or `accepted_via_witness` (which means the opposite: the filing WAS read
-- and the aggregator agreed with it).
--
-- Forward-only. Idempotence comes from the migration runner's applied-version
-- ledger (this rebuild runs exactly once), NOT from a guard inside the SQL — the
-- `DROP TABLE` below is unconditional, so this file must never be re-pointed at
-- a different version number. Existing rows are copied before the drop and the
-- indexes are recreated after the rename; `migration_0105_preserves_outcome_rows`
-- in storage/tests/migration_safety.rs is the test that proves no row is lost.

PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS fundamentals_extraction_outcomes_0105 (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    report_document_id TEXT NOT NULL,
    fiscal_year INTEGER NOT NULL,
    period_type TEXT NOT NULL,
    period_end TEXT NOT NULL,
    tier TEXT,
    acceptance TEXT NOT NULL CHECK(acceptance IN (
        'accepted', 'accepted_via_witness', 'accepted_unreviewed', 'flagged', 'empty'
    )),
    -- `witness_fallback` is new: the run persisted at least one fact SOURCED from
    -- the aggregator because no deterministic tier produced a value for that slot.
    reason_code TEXT NOT NULL CHECK(reason_code IN (
        'emitted',
        'validation_failed',
        'structure_drift',
        'witness_disagreement',
        'witness_fallback',
        'no_deterministic_tier',
        'no_period_derived',
        'document_unreadable'
    )),
    detail_json TEXT,
    drift_json TEXT,
    structure_changed INTEGER NOT NULL DEFAULT 0 CHECK(structure_changed IN (0, 1)),
    fact_count INTEGER NOT NULL DEFAULT 0,
    attempt_count INTEGER NOT NULL DEFAULT 1,
    first_attempted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_attempted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT OR IGNORE INTO fundamentals_extraction_outcomes_0105
SELECT
    id, company_id, report_document_id, fiscal_year, period_type, period_end,
    tier, acceptance, reason_code, detail_json, drift_json, structure_changed,
    fact_count, attempt_count, first_attempted_at, last_attempted_at
FROM fundamentals_extraction_outcomes;

DROP TABLE fundamentals_extraction_outcomes;

ALTER TABLE fundamentals_extraction_outcomes_0105
    RENAME TO fundamentals_extraction_outcomes;

-- Indexes travel with the table, so they are recreated against the new one.
CREATE UNIQUE INDEX IF NOT EXISTS idx_fundamentals_extraction_outcomes_slot
    ON fundamentals_extraction_outcomes(
        company_id, report_document_id, fiscal_year, period_type, period_end
    );

CREATE INDEX IF NOT EXISTS idx_fundamentals_extraction_outcomes_review
    ON fundamentals_extraction_outcomes(company_id, acceptance, last_attempted_at DESC);

PRAGMA foreign_keys = ON;
