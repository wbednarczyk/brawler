-- Repair the stored zero-effect extraction outcomes (issue #243, epic #40 S5
-- residue; ADR 0091).
--
-- Before S5, `record_extraction_outcome` recorded the NEWLY PRODUCED fact count:
-- re-running a landed period (every slot re-observed, nothing new) overwrote a
-- healthy `fact_count = 12` with `0` while keeping `reason_code = 'emitted'`.
-- The forward fix (S5, `slot_fact_count`) stops new rows from entering that
-- state; this migration heals the rows already stored:
--
--   1. Widen the `reason_code` CHECK with `facts_superseded` — an emitting row
--      whose facts are genuinely no longer at the slot (removed with the retired
--      PDF arm, superseded by a better document, or a retired aggregator
--      gap-fill) must STATE that instead of claiming an emission it cannot
--      evidence. SQLite cannot alter a CHECK in place, so this is the standard
--      table rebuild (the 0105 shape; `migration_0119_*` in
--      storage/tests/migration_safety.rs proves no row is lost).
--   2. Recount `fact_count` from the facts actually AT the slot (matched via
--      `financial_facts.source_document_ref` + the period) for every emitting
--      row that recorded zero.
--   3. Rows still at zero after the recount get `reason_code = 'facts_superseded'`
--      (their acceptance is history and stays).
--
-- Idempotent by predicate: a healed row no longer matches either UPDATE. The
-- rebuild itself runs exactly once via the migration runner's applied-version
-- ledger — this file must never be re-pointed at a different version number.

PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS fundamentals_extraction_outcomes_0119 (
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
    -- `facts_superseded` is new: recorded emission whose facts are no longer at
    -- the slot — the repaired honest state, never written by a live run.
    reason_code TEXT NOT NULL CHECK(reason_code IN (
        'emitted',
        'validation_failed',
        'structure_drift',
        'witness_disagreement',
        'witness_fallback',
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

INSERT OR IGNORE INTO fundamentals_extraction_outcomes_0119
SELECT
    id, company_id, report_document_id, fiscal_year, period_type, period_end,
    tier, acceptance, reason_code, detail_json, drift_json, structure_changed,
    fact_count, attempt_count, first_attempted_at, last_attempted_at
FROM fundamentals_extraction_outcomes;

DROP TABLE fundamentals_extraction_outcomes;

ALTER TABLE fundamentals_extraction_outcomes_0119
    RENAME TO fundamentals_extraction_outcomes;

CREATE UNIQUE INDEX IF NOT EXISTS idx_fundamentals_extraction_outcomes_slot
    ON fundamentals_extraction_outcomes(
        company_id, report_document_id, fiscal_year, period_type, period_end
    );

CREATE INDEX IF NOT EXISTS idx_fundamentals_extraction_outcomes_review
    ON fundamentals_extraction_outcomes(company_id, acceptance, last_attempted_at DESC);

PRAGMA foreign_keys = ON;

-- Step 2: recount from the facts at the slot. Narrow by predicate to the lying
-- rows only — healthy rows keep their per-run semantics untouched.
UPDATE fundamentals_extraction_outcomes
SET fact_count = (
    SELECT COUNT(*)
    FROM financial_facts f
    JOIN financial_periods p ON f.period_id = p.id
    WHERE p.company_id = fundamentals_extraction_outcomes.company_id
      AND p.fiscal_year = fundamentals_extraction_outcomes.fiscal_year
      AND p.period_type = fundamentals_extraction_outcomes.period_type
      AND f.source_document_ref = fundamentals_extraction_outcomes.report_document_id
)
WHERE acceptance IN ('accepted', 'accepted_via_witness', 'accepted_unreviewed')
  AND fact_count = 0
  AND reason_code IN ('emitted', 'witness_fallback');

-- Step 3: whatever is still at zero cannot evidence its claimed emission —
-- state the superseded case instead.
UPDATE fundamentals_extraction_outcomes
SET reason_code = 'facts_superseded'
WHERE acceptance IN ('accepted', 'accepted_via_witness', 'accepted_unreviewed')
  AND fact_count = 0
  AND reason_code IN ('emitted', 'witness_fallback');
