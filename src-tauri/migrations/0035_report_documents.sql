-- Persisted report documents (ADR 0027): the actual report files behind fundamentals,
-- from ESPI/EBI attachments, user-supplied PDF URLs, or captured article URLs.
-- financial_facts.source_document_ref points at report_documents.id (soft reference).

CREATE TABLE report_documents (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    period_id TEXT REFERENCES financial_periods(id) ON DELETE SET NULL,
    source_type TEXT NOT NULL,              -- espi_attachment | user_url | article
    origin_ref TEXT,                        -- feed item / evidence id the document came from
    url TEXT NOT NULL,                       -- original source URL
    local_path TEXT,                         -- relative path under the app data dir; null until fetched
    content_type TEXT,
    content_hash TEXT,                       -- optional content dedup (sha256) when available
    byte_size INTEGER,
    title TEXT,
    attribution TEXT,
    fetch_status TEXT NOT NULL DEFAULT 'pending',  -- pending | fetched | failed
    fetch_error TEXT,
    fetched_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(company_id, url)
);

CREATE INDEX idx_report_documents_company ON report_documents(company_id, source_type);
CREATE INDEX idx_report_documents_period ON report_documents(period_id);
CREATE INDEX idx_report_documents_hash ON report_documents(content_hash);
