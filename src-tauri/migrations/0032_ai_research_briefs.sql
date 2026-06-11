CREATE TABLE ai_research_brief_jobs (
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

CREATE INDEX idx_ai_research_brief_jobs_scope
ON ai_research_brief_jobs(scope_type, scope_id, created_at);

CREATE INDEX idx_ai_research_brief_jobs_status
ON ai_research_brief_jobs(status);

CREATE TABLE ai_research_briefs (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES ai_research_brief_jobs(id) ON DELETE CASCADE,
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

CREATE INDEX idx_ai_research_briefs_scope
ON ai_research_briefs(scope_type, scope_id, generated_at);

CREATE INDEX idx_ai_research_briefs_job
ON ai_research_briefs(job_id);

CREATE TABLE ai_research_brief_citations (
    id TEXT PRIMARY KEY,
    brief_id TEXT NOT NULL REFERENCES ai_research_briefs(id) ON DELETE CASCADE,
    citation_key TEXT NOT NULL,
    evidence_type TEXT NOT NULL,
    evidence_id TEXT NOT NULL,
    label TEXT NOT NULL,
    snippet TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(brief_id, citation_key)
);

CREATE INDEX idx_ai_research_brief_citations_brief
ON ai_research_brief_citations(brief_id, citation_key);
