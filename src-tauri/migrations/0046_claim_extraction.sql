-- AI claim extraction (v0.42.0, epic cbf6999, ADR 0040).
-- Mirrors KPI extraction (migration 0037): extraction produces PROPOSALS that never
-- become claims until the user confirms them. One job runs over a stored report
-- document OR a transcript; proposals are the staging + provenance ledger.

CREATE TABLE IF NOT EXISTS claim_extraction_jobs (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,                       -- report_document | transcript
    source_id TEXT NOT NULL,                          -- report_documents(id) | transcript_jobs(id)
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    status TEXT NOT NULL,                             -- queued | running | succeeded | failed
    error_code TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    finished_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_claim_extraction_jobs_company ON claim_extraction_jobs(company_id);
CREATE INDEX IF NOT EXISTS idx_claim_extraction_jobs_source ON claim_extraction_jobs(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_claim_extraction_jobs_status ON claim_extraction_jobs(status);

-- One proposed claim per (job, ordinal). Confirmed proposals are retained as the
-- provenance trail for the materialized claim (claim_id); rejected proposals are kept
-- so the same statement is not re-proposed without intent.
CREATE TABLE IF NOT EXISTS claim_extraction_proposals (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES claim_extraction_jobs(id) ON DELETE CASCADE,
    statement TEXT NOT NULL,
    due_fiscal_year INTEGER,
    due_period_type TEXT,
    target_metric_key TEXT,
    target_comparator TEXT,
    target_value_numeric TEXT,
    target_unit TEXT,
    confidence TEXT,
    source_snippet TEXT,
    source_evidence_type TEXT,                        -- report_document | transcript_segment
    source_evidence_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',            -- pending | confirmed | rejected
    claim_id TEXT REFERENCES management_claims(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_claim_extraction_proposals_job ON claim_extraction_proposals(job_id, status);
