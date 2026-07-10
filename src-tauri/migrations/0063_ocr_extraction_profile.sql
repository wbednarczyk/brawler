-- Per-company OCR-markdown extraction profile (ADR 0077 §4, tier-4).
--
-- The tier-4 last-resort layer OCRs a report to markdown and parses it through
-- a confirmed, versioned OcrExtractionProfile (label map, reporting scale,
-- value-column layout, skip columns, enumerator convention). This is a SEPARATE
-- concern from the tier-2 company_extraction_profile (migration 0057): OCR
-- markdown has a different template fingerprint and different drift semantics
-- (a Nota column, a value-column layout, an enumerator convention), so it gets
-- its own table rather than muddying the tier-2 profile_json versioning.
--
-- Append-only, idempotent, self-healing: CREATE TABLE IF NOT EXISTS so it
-- converges on every database regardless of prior state. A company with no row
-- has never been bootstrapped at the OCR tier; reads tolerate absence (safe
-- default: tier-4 cannot yet parse deterministically for it).
CREATE TABLE IF NOT EXISTS company_ocr_extraction_profile (
    company_id TEXT PRIMARY KEY,
    template_hash TEXT NOT NULL,
    -- Serialized UnitScale (Ones | Thousands | Millions).
    scale TEXT NOT NULL DEFAULT 'Thousands',
    -- Serialized OcrExtractionProfile (label_map, value_column, skip_columns, ...).
    profile_json TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
