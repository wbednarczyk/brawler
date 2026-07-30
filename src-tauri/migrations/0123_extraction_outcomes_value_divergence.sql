-- `value_divergence` joins the extraction-outcome reason vocabulary (epic #229
-- T5, #192 residual).
--
-- A re-extraction that reads a DIFFERENT value than the one already stored at a
-- slot (`StructuredFactCommit::Divergent`) never overwrites it — that is the
-- point — but until now the finding lived only in the in-memory run result and a
-- developer-mode diagnostic event (7-day trimmed). The disagreement between two
-- reads of the issuer's OWN filings is exactly the "never silently wrong" case
-- the outcomes table exists for, so it becomes a durable row: acceptance
-- `flagged`, reason `value_divergence`, one row per (document, metric) via the
-- synthetic `documentId#metricKey` slot ref (the reversed-witnessing precedent),
-- so repeated re-extractions upsert instead of duplicating.
--
-- SQLite cannot alter a CHECK in place, so this is the standard table rebuild
-- (the 0119 shape; `migration_0123_*` in storage/tests/migration_safety.rs
-- proves no row is lost). Nothing else about the table changes: same columns,
-- same ids, same indexes.

PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS fundamentals_extraction_outcomes_0123 (
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
    -- `value_divergence` is new: a re-read of a stored slot disagreed with the
    -- committed value. The stored value is KEPT (never overwritten) and the
    -- disagreement is recorded here for ratification.
    reason_code TEXT NOT NULL CHECK(reason_code IN (
        'emitted',
        'validation_failed',
        'structure_drift',
        'witness_disagreement',
        'witness_fallback',
        'value_divergence',
        'no_deterministic_tier',
        'no_period_derived',
        'document_unreadable',
        'facts_superseded'
    )),
    detail_json TEXT,
    drift_json TEXT,
    structure_changed INTEGER NOT NULL DEFAULT 0 CHECK(structure_changed IN (0, 1)),
    fact_count INTEGER NOT NULL DEFAULT 0,
    attempt_count INTEGER NOT NULL DEFAULT 1,
    first_attempted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_attempted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT OR IGNORE INTO fundamentals_extraction_outcomes_0123
SELECT
    id, company_id, report_document_id, fiscal_year, period_type, period_end,
    tier, acceptance, reason_code, detail_json, drift_json, structure_changed,
    fact_count, attempt_count, first_attempted_at, last_attempted_at
FROM fundamentals_extraction_outcomes;

DROP TABLE fundamentals_extraction_outcomes;

ALTER TABLE fundamentals_extraction_outcomes_0123
    RENAME TO fundamentals_extraction_outcomes;

CREATE UNIQUE INDEX IF NOT EXISTS idx_fundamentals_extraction_outcomes_slot
    ON fundamentals_extraction_outcomes(
        company_id, report_document_id, fiscal_year, period_type, period_end
    );

CREATE INDEX IF NOT EXISTS idx_fundamentals_extraction_outcomes_review
    ON fundamentals_extraction_outcomes(company_id, acceptance, last_attempted_at DESC);

PRAGMA foreign_keys = ON;
