-- Fundamentals extraction OUTCOMES (v0.59.0, ADR 0061 decision 2 "the good gate"
-- + ADR 0084 decision 4/6). The persistence half of "never silently wrong".
--
-- Problem this closes: `financial_fact_provenance` is keyed by `fact_id`, so it
-- can only record a run that PRODUCED a fact. A Flagged / Empty / unreadable
-- outcome — exactly the case the user most needs to see — left nothing durable
-- behind, and a flagged period was indistinguishable from a period that was
-- never attempted. `diagnostic_events` is not a substitute: it is developer-mode
-- gated and trimmed to 7 days / 1000 rows, so it is observability, not a record.
--
-- One row per attempted extraction SLOT — (company, document, fiscal year,
-- period type, period end) — upserted on re-run, so the table answers "what did
-- the pipeline last conclude about this period?" for every period it ever
-- touched, emitting or not. Emitting runs are recorded too: absence of a row
-- then unambiguously means "never attempted".
--
-- Forward-only, idempotent and self-healing (data-model migration rules).

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS fundamentals_extraction_outcomes (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- The stored report document the attempt ran over. Not an FK: a document can
    -- be re-captured or pruned, and losing the document must never delete the
    -- record that the pipeline tried and what it concluded.
    report_document_id TEXT NOT NULL,
    fiscal_year INTEGER NOT NULL,
    period_type TEXT NOT NULL,
    period_end TEXT NOT NULL,
    -- Which tier produced (or attempted) the set. NULL = no tier could read the
    -- document at all.
    tier TEXT,
    -- The pipeline verdict, same vocabulary as `Acceptance::as_str()`.
    acceptance TEXT NOT NULL CHECK(acceptance IN (
        'accepted', 'accepted_via_witness', 'accepted_unreviewed', 'flagged', 'empty'
    )),
    -- TYPED reason the run landed where it did — never English prose (ADR 0084
    -- decision 6). `emitted` is the non-failure value.
    reason_code TEXT NOT NULL CHECK(reason_code IN (
        'emitted',
        'validation_failed',
        'structure_drift',
        'witness_disagreement',
        'no_deterministic_tier',
        'no_period_derived',
        'document_unreadable'
    )),
    -- Structured detail for the reason (failing identity ids + residuals, failing
    -- comparative cross-checks, the unreadable document's error). NULL when the
    -- reason carries no detail.
    detail_json TEXT,
    -- The serialized `DriftReport` when the pipeline detected layout drift. This
    -- is the payload the pipeline already computed and previously DROPPED on a
    -- non-emitting run; the ADR 0061 decision-3 learning loop reads it from here.
    drift_json TEXT,
    structure_changed INTEGER NOT NULL DEFAULT 0 CHECK(structure_changed IN (0, 1)),
    -- How many facts the run actually persisted (0 for every non-emitting run).
    fact_count INTEGER NOT NULL DEFAULT 0,
    -- How many times this slot has been attempted (incremented by the upsert), so
    -- a repeatedly-failing period is visibly repeatedly-failing.
    attempt_count INTEGER NOT NULL DEFAULT 1,
    first_attempted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_attempted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- The slot identity. One row per (company, document, period) — a re-run updates
-- in place rather than accumulating history, so the read model always shows the
-- CURRENT verdict for a period (stale flags cannot outlive their fix).
CREATE UNIQUE INDEX IF NOT EXISTS idx_fundamentals_extraction_outcomes_slot
    ON fundamentals_extraction_outcomes(
        company_id, report_document_id, fiscal_year, period_type, period_end
    );

-- The review read path: a company's non-emitting outcomes, newest attempt first.
CREATE INDEX IF NOT EXISTS idx_fundamentals_extraction_outcomes_review
    ON fundamentals_extraction_outcomes(company_id, acceptance, last_attempted_at DESC);
