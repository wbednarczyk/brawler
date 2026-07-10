-- Document-kind taxonomy for report_documents (ADR 0077 §1, trusted-extraction
-- F1 / T1.2). doc_kind marks which stored documents can carry extractable
-- financial data: periodic_ssf | periodic_jsf | auditor_opinion | presentation
-- | governance | other. Classification is deterministic Rust code
-- (fundamentals::extraction::classify::classify_doc_kind over title + url), NOT
-- a SQL backfill — this migration only adds the nullable column and its index.
--
-- NULL = not yet classified: rows that predate this column stay NULL until the
-- idempotent reclassify_report_documents command runs (or the next set-on-write
-- ingest/upsert fills them). Reads tolerate a missing/NULL value everywhere and
-- treat it as "unclassified" (safe default). Nullable ADD COLUMN + IF NOT EXISTS
-- keep this append-only and self-healing; the runner applies it exactly once.

ALTER TABLE report_documents ADD COLUMN doc_kind TEXT;

CREATE INDEX IF NOT EXISTS idx_report_documents_doc_kind
    ON report_documents(company_id, doc_kind);
