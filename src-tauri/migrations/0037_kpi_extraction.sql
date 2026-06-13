-- AI KPI extraction (v0.36.0, epic 9879941).
-- Extraction produces PROPOSALS that are never facts until the user confirms them.
-- kpi_extraction_jobs tracks one async extraction run over a stored report document;
-- kpi_extraction_proposals is the staging + provenance ledger of proposed values.

CREATE TABLE kpi_extraction_jobs (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    report_document_id TEXT NOT NULL REFERENCES report_documents(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    period_hint TEXT,
    status TEXT NOT NULL,                    -- queued | running | succeeded | failed
    error_code TEXT,
    error TEXT,
    detected_fiscal_year INTEGER,
    detected_period_type TEXT,
    detected_period_end_date TEXT,
    detected_currency TEXT,
    detected_language TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    finished_at TEXT
);

CREATE INDEX idx_kpi_extraction_jobs_company ON kpi_extraction_jobs(company_id);
CREATE INDEX idx_kpi_extraction_jobs_document ON kpi_extraction_jobs(report_document_id);
CREATE INDEX idx_kpi_extraction_jobs_status ON kpi_extraction_jobs(status);

-- One proposed KPI value per (job, metric_key). status confirmed proposals are retained
-- as the provenance trail for the materialized fact (fact_id); rejected proposals are kept
-- so the same value is not re-proposed without intent.
CREATE TABLE kpi_extraction_proposals (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES kpi_extraction_jobs(id) ON DELETE CASCADE,
    metric_key TEXT NOT NULL,
    label TEXT NOT NULL,
    value_numeric TEXT NOT NULL,
    unit TEXT,
    currency TEXT,
    as_reported_value TEXT,
    as_reported_scale TEXT,
    measure_window TEXT,
    confidence TEXT,
    source_snippet TEXT,
    is_proposed_kpi INTEGER NOT NULL DEFAULT 0,   -- 1 when beyond the supplied taxonomy
    status TEXT NOT NULL DEFAULT 'pending',        -- pending | confirmed | rejected
    fact_id TEXT REFERENCES financial_facts(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_kpi_extraction_proposals_job ON kpi_extraction_proposals(job_id, status);
