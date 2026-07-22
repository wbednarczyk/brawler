-- Persisted per-document reporting-period derivation cache (v0.60, ADR 0077
-- read-model rule + CLAUDE.md "read persisted derived indexes instead of
-- recomputing the corpus per call").
--
-- Problem this closes: `derive_report_period` (jobs/structured_extraction.rs)
-- resolves a stored document's reporting period, and for a bare-titled periodic
-- report — nothing parseable in its title/URL — the last-resort tier reads the
-- file and runs a FULL text extraction just to read the period off the cover
-- page. The Coverage panel (`compute_fundamentals_coverage`) calls this once per
-- document on EVERY load, so a company with a handful of bare-`SSF.pdf` filings
-- re-extracts those PDFs on every panel open. A document is immutable once
-- ingested, so its derived period is a stable function of its bytes — compute it
-- once, persist it, read it thereafter.
--
-- One row per document. `has_period = 0` is an EXPLICIT none-marker: a document
-- whose content yields no derivable period is recorded too, so it is never
-- re-parsed on the next load (the abstention is as cacheable as a hit).
--
-- Invalidation = the `derivation_version` integer, a code constant
-- (`jobs::structured_extraction::DERIVATION_VERSION`). Since the document bytes
-- never change, the only reason to re-derive is a change to the derivation
-- grammar itself: bump the constant, and any row stamped with an older version
-- is re-derived and overwritten on next read (self-healing, forward-only).
--
-- Forward-only, idempotent and self-healing (data-model migration rules).

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS document_derived_periods (
    -- The stored document the period was derived FOR. FK with CASCADE: unlike the
    -- extraction-OUTCOMES record (which must outlive its document), this row is
    -- pure re-derivable cache — if the document is gone, the cached period is
    -- meaningless, so it is swept with it.
    report_document_id TEXT PRIMARY KEY
        REFERENCES report_documents(id) ON DELETE CASCADE,
    -- 1 = a period was derived (the three columns below are non-NULL);
    -- 0 = explicit none-marker (columns NULL, do not re-parse).
    has_period INTEGER NOT NULL CHECK(has_period IN (0, 1)),
    fiscal_year INTEGER,
    period_type TEXT,
    period_end TEXT,
    -- The derivation-grammar version this row was produced under. A row older
    -- than the code constant is re-derived and overwritten on read.
    derivation_version INTEGER NOT NULL,
    derived_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- A hit must carry its full period; a none-marker must carry none. Guards the
    -- two columns against a half-written shape either read path would trip over.
    CHECK(
        (has_period = 1 AND fiscal_year IS NOT NULL AND period_type IS NOT NULL AND period_end IS NOT NULL)
        OR
        (has_period = 0 AND fiscal_year IS NULL AND period_type IS NULL AND period_end IS NULL)
    )
);
