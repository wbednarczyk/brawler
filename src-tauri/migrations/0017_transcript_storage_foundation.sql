DROP INDEX IF EXISTS idx_jobs_status;
DROP INDEX IF EXISTS idx_jobs_type;
DROP INDEX IF EXISTS idx_transcript_segments_job_id;
DROP INDEX IF EXISTS idx_transcript_jobs_company_id;
DROP INDEX IF EXISTS idx_transcript_jobs_status;

ALTER TABLE jobs RENAME TO jobs_legacy;
ALTER TABLE transcript_segments RENAME TO transcript_segments_legacy;
ALTER TABLE transcript_jobs RENAME TO transcript_jobs_legacy;

CREATE TABLE transcript_jobs (
    id TEXT PRIMARY KEY,
    company_id TEXT REFERENCES companies(id) ON DELETE SET NULL,
    provider_id TEXT NOT NULL,
    source_type TEXT NOT NULL,
    source_url TEXT NOT NULL,
    source_label TEXT,
    company_resolution_status TEXT NOT NULL DEFAULT 'unresolved',
    recognized_company_candidates_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL,
    error_code TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    finished_at TEXT,
    error TEXT
);

CREATE INDEX idx_transcript_jobs_company_id ON transcript_jobs(company_id);
CREATE INDEX idx_transcript_jobs_status ON transcript_jobs(status);
CREATE INDEX idx_transcript_jobs_company_resolution_status ON transcript_jobs(company_resolution_status);

CREATE TABLE transcript_segments (
    id TEXT PRIMARY KEY,
    transcript_job_id TEXT NOT NULL REFERENCES transcript_jobs(id) ON DELETE CASCADE,
    company_id TEXT REFERENCES companies(id) ON DELETE SET NULL,
    start_seconds INTEGER,
    end_seconds INTEGER,
    speaker TEXT,
    text TEXT NOT NULL,
    language TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_transcript_segments_job_id ON transcript_segments(transcript_job_id);
CREATE INDEX idx_transcript_segments_company_id ON transcript_segments(company_id);

CREATE TRIGGER prevent_transcript_segment_text_update
BEFORE UPDATE OF text ON transcript_segments
BEGIN
    SELECT RAISE(ABORT, 'transcript segment text is immutable');
END;

INSERT INTO transcript_jobs (
    id,
    company_id,
    provider_id,
    source_type,
    source_url,
    source_label,
    company_resolution_status,
    recognized_company_candidates_json,
    status,
    error_code,
    created_at,
    started_at,
    finished_at,
    error
)
SELECT
    id,
    company_id,
    provider_id,
    source_type,
    source_url,
    NULL,
    CASE WHEN company_id IS NULL THEN 'unresolved' ELSE 'provided' END,
    '[]',
    status,
    NULL,
    created_at,
    started_at,
    finished_at,
    error
FROM transcript_jobs_legacy;

INSERT INTO transcript_segments (
    id,
    transcript_job_id,
    company_id,
    start_seconds,
    end_seconds,
    speaker,
    text,
    language,
    created_at
)
SELECT
    id,
    transcript_job_id,
    company_id,
    start_seconds,
    end_seconds,
    speaker,
    text,
    language,
    created_at
FROM transcript_segments_legacy;

CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    adapter_id TEXT REFERENCES source_adapters(id),
    transcript_job_id TEXT REFERENCES transcript_jobs(id),
    status TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    items_fetched INTEGER NOT NULL DEFAULT 0,
    items_created INTEGER NOT NULL DEFAULT 0,
    warnings_json TEXT,
    error TEXT
);

CREATE INDEX idx_jobs_status ON jobs(status);
CREATE INDEX idx_jobs_type ON jobs(type);

INSERT INTO jobs (
    id,
    type,
    adapter_id,
    transcript_job_id,
    status,
    started_at,
    finished_at,
    items_fetched,
    items_created,
    warnings_json,
    error
)
SELECT
    id,
    type,
    adapter_id,
    transcript_job_id,
    status,
    started_at,
    finished_at,
    items_fetched,
    items_created,
    warnings_json,
    error
FROM jobs_legacy;

DROP TABLE jobs_legacy;
DROP TABLE transcript_segments_legacy;
DROP TABLE transcript_jobs_legacy;
