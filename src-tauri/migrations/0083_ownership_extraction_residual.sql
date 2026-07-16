-- Ownership extraction residual: the pending queue for periodic reports whose
-- mandatory shareholders table the DETERMINISTIC parser could not turn into
-- stakes (v0.56.0 T3, ADR 0072). The deterministic parse writes stakes directly
-- and finally; only its residual — glyph-mangled text layers, image-rendered
-- tables, or a missing section — is parked here for the later AI/OCR path, whose
-- results ALWAYS require confirmation (owner decision 2026-07-16). NO stakes are
-- fabricated for a residual document; recording it here is what lets the coverage
-- view and the AI/OCR slice find the gap without re-scanning every report.
--
-- Forward-only, idempotent and self-healing (data-model migration rules): the
-- table uses IF NOT EXISTS, and the store upserts one row per document
-- (PRIMARY KEY on report_document_id), so a re-run neither errors nor duplicates.
-- A residual row is removed by the extraction job the moment a (later) parser
-- version succeeds on the same document, so the two states never coexist.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS ownership_extraction_residual (
    -- One residual per stored document (idempotent upsert key). ON DELETE CASCADE
    -- drops the residual when its document is pruned.
    report_document_id TEXT PRIMARY KEY
        REFERENCES report_documents(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- The deterministic parser's terminal non-Found state (why it could not
    -- write stakes), so the AI/OCR path can prioritize (a glyph-mangled font is
    -- an OCR job; a missing section may be a wrong-document mismatch).
    parse_state TEXT NOT NULL CHECK(parse_state IN (
        'section_missing', 'table_unparsable', 'glyph_encoded'
    )),
    -- The disclosure date resolved for the document (period end date / document
    -- period derivation / a date near the heading), if any — NULL when even the
    -- date could not be determined. Carried so the AI/OCR write can reuse it.
    detected_as_of TEXT,
    -- The shareholders heading line that anchored the (failed) parse, verbatim —
    -- NULL for section_missing. A human/AI diagnostic aid.
    matched_heading TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_ownership_residual_company
    ON ownership_extraction_residual(company_id);
