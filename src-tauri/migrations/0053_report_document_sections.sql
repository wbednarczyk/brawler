-- Report-over-report diff (v0.47.0 / ADR 0052): the derived, rebuildable
-- text-section index over stored financial-statement report documents, plus a
-- per-document extraction state. The deterministic section diff is computed
-- on-demand from these sections; nothing here is a source of truth.
--
-- Market-wide validation (613 real GPW + NewConnect reports, ADR 0052) proved
-- extraction must handle two formats (PDF + ESEF/iXBRL xhtml), survive
-- pdf-extract panics (-> extraction_failed), and flag scanned reports
-- (-> no_text_layer) by text density. Those outcomes live in extraction_state.
--
-- Disposable derived index: dropping these tables loses zero canonical data, it
-- only forces re-extraction. Append-only, idempotent, self-healing
-- (CREATE TABLE IF NOT EXISTS) so it converges on every database.

-- One row per extracted report document: the outcome of running extraction over
-- its stored bytes. Rebuilt when content_hash or extractor_version changes.
CREATE TABLE IF NOT EXISTS report_document_extractions (
    report_document_id TEXT PRIMARY KEY
        REFERENCES report_documents(id) ON DELETE CASCADE,
    -- 'pdf' | 'xhtml' (ESEF/iXBRL): the source format extraction ran over.
    source_format TEXT NOT NULL,
    -- 'extracted' | 'no_text_layer' | 'extraction_failed'. Only 'extracted'
    -- documents are diffable. 'no_text_layer' is a scanned/image report (density
    -- below threshold; OCR out of scope); 'extraction_failed' is a caught
    -- pdf-extract panic or read error -- flagged, never crashing the job.
    extraction_state TEXT NOT NULL,
    -- Extracted text length, for the density (no_text_layer) decision + diagnostics.
    char_count INTEGER NOT NULL DEFAULT 0,
    -- sha256 of the source document bytes extraction ran over; re-extraction skips
    -- a document whose hash + extractor_version are unchanged.
    content_hash TEXT,
    -- Extraction-heuristic version; a bump invalidates and rebuilds affected rows.
    extractor_version INTEGER NOT NULL DEFAULT 1,
    extracted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- One row per detected section of an 'extracted' document, in document order.
-- Identity is (report_document_id, ordinal): heading text is NOT unique, so
-- alignment keys on heading + ordinal -- exact-heading-only matching breaks the
-- deterministic self-diff = empty invariant (ADR 0052).
CREATE TABLE IF NOT EXISTS report_document_sections (
    report_document_id TEXT NOT NULL
        REFERENCES report_documents(id) ON DELETE CASCADE,
    -- 0-based position within the document; the stable per-document section id.
    ordinal INTEGER NOT NULL,
    -- Detected heading text, normalized (leading numbering stripped, lowercased),
    -- or '<preamble>' for content before the first heading.
    heading TEXT NOT NULL,
    -- The section's extracted plain text.
    body TEXT NOT NULL,
    -- sha256 of the source bytes this section was derived from (matches the
    -- extraction row), so staleness is detectable.
    content_hash TEXT,
    extractor_version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (report_document_id, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_report_document_sections_doc
    ON report_document_sections (report_document_id);
