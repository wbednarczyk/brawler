CREATE TABLE research_reminders (
    id TEXT PRIMARY KEY,
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    company_id TEXT,
    reminder_kind TEXT NOT NULL,
    source_type TEXT,
    source_id TEXT,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    due_at TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    snoozed_until TEXT,
    completed_at TEXT,
    dismissed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_research_reminders_scope
ON research_reminders(scope_type, scope_id, status, due_at);

CREATE INDEX idx_research_reminders_company
ON research_reminders(company_id, status, due_at);

CREATE UNIQUE INDEX idx_research_reminders_source
ON research_reminders(scope_type, scope_id, source_type, source_id)
WHERE source_type IS NOT NULL AND source_id IS NOT NULL;

CREATE TABLE ai_research_digest_jobs (
    id TEXT PRIMARY KEY,
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    evidence_collector_version TEXT NOT NULL,
    renderer_version TEXT NOT NULL,
    status TEXT NOT NULL,
    error_code TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    finished_at TEXT
);

CREATE INDEX idx_ai_research_digest_jobs_scope
ON ai_research_digest_jobs(scope_type, scope_id, created_at);

CREATE INDEX idx_ai_research_digest_jobs_status
ON ai_research_digest_jobs(status);

CREATE TABLE ai_research_digests (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES ai_research_digest_jobs(id) ON DELETE CASCADE,
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    evidence_collector_version TEXT NOT NULL,
    renderer_version TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    content_markdown TEXT NOT NULL,
    language TEXT,
    generated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_ai_research_digests_scope
ON ai_research_digests(scope_type, scope_id, generated_at);

CREATE INDEX idx_ai_research_digests_job
ON ai_research_digests(job_id);

CREATE TABLE ai_research_digest_citations (
    id TEXT PRIMARY KEY,
    digest_id TEXT NOT NULL REFERENCES ai_research_digests(id) ON DELETE CASCADE,
    citation_key TEXT NOT NULL,
    evidence_type TEXT NOT NULL,
    evidence_id TEXT NOT NULL,
    label TEXT NOT NULL,
    snippet TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(digest_id, citation_key)
);

CREATE INDEX idx_ai_research_digest_citations_digest
ON ai_research_digest_citations(digest_id, citation_key);
