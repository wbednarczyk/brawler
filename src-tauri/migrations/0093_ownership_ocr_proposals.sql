-- Ownership OCR proposals: the confirm-before-apply review surface for the
-- tier-4 vision/OCR pass over `ownership_extraction_residual` documents
-- (v0.57.0 T8, ADR 0077 tier-4 over ADR 0072 decision 2a). The deterministic
-- parser writes stakes directly and finally; only its residual (glyph-mangled
-- font, image table) is parked. This slice OCRs a residual's stored PDF through
-- the routable `vision_extraction` capability, parses the shareholders table
-- out of the OCR markdown with the SAME deterministic parser, and lands the
-- result here as a proposal the user confirms — NEVER auto-applied. Confirm
-- writes stakes (`source = report_document`) and clears the residual; reject
-- parks the residual with `ocr_state = 'rejected'` so it is not re-proposed.
--
-- Forward-only, idempotent, self-healing (data-model migration rules): all
-- objects use IF NOT EXISTS; the store upserts one proposal per document
-- (PK on report_document_id). Transient/derived — NOT in the import/export
-- bundle (same posture as `ownership_holder_type_proposals`).

PRAGMA foreign_keys = ON;

-- One OCR proposal per residual document (idempotent upsert key). Rows live in
-- the companion table; confirming DELETEs the header (rows cascade) after the
-- stakes are written; rejecting DELETEs it and marks the residual rejected.
CREATE TABLE IF NOT EXISTS ownership_ocr_proposals (
    -- The residual document this proposal resolves (the entry in
    -- ownership_extraction_residual). Confirm writes stakes closing THIS
    -- document's coverage gap and clears its residual.
    report_document_id TEXT PRIMARY KEY
        REFERENCES report_documents(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- The document actually OCR'd (v0.57 T8): equals report_document_id when the
    -- residual is itself a PDF; the fetched **PDF sibling** (same company + period)
    -- when the residual is a pdf2htmlEX xhtml container whose text layer is
    -- unreadable. Provenance of the OCR run — surfaced in the review card.
    source_document_id TEXT NOT NULL REFERENCES report_documents(id) ON DELETE CASCADE,
    -- The disclosure date carried onto every written stake (deterministic,
    -- resolved from the residual / document period — never fabricated).
    as_of TEXT NOT NULL,
    -- The shareholders heading the OCR parse anchored on, verbatim (diagnostic).
    matched_heading TEXT,
    -- Provenance of the OCR run (which vision provider/model produced it).
    provider_id TEXT,
    model TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_ownership_ocr_proposals_company
    ON ownership_ocr_proposals(company_id);

-- The proposed holder rows (name + capital/votes %), stable-ordered by index.
CREATE TABLE IF NOT EXISTS ownership_ocr_proposal_rows (
    id TEXT PRIMARY KEY,
    report_document_id TEXT NOT NULL
        REFERENCES ownership_ocr_proposals(report_document_id) ON DELETE CASCADE,
    row_index INTEGER NOT NULL,
    holder_name_raw TEXT NOT NULL,
    -- Normalized decimal strings (comma→dot, `%`/whitespace stripped), either
    -- side NULL when the filing discloses only one of capital/votes.
    capital_pct TEXT,
    votes_pct TEXT
);

CREATE INDEX IF NOT EXISTS idx_ownership_ocr_proposal_rows_document
    ON ownership_ocr_proposal_rows(report_document_id);

-- OCR lifecycle marker on the residual, so the OCR pass never re-proposes a
-- document that is already under review, that the user rejected, or that OCR
-- could not turn into a table. NULL = eligible for a (bulk) OCR pass.
--   NULL       — never attempted; the bulk pass and catch-up select these.
--   'proposed' — a pending OCR proposal exists; awaits human review.
--   'rejected' — the user rejected the OCR proposal; never re-proposed.
--   'no_table' — OCR ran (or the doc is un-OCRable) and yielded no shareholders
--                table; skipped by the bulk pass, re-armed by the manual
--                per-company action (an explicit user retry).
-- No CHECK on ADD COLUMN (enforced in code, OWNERSHIP_OCR_STATE_* constants).
ALTER TABLE ownership_extraction_residual ADD COLUMN ocr_state TEXT;
