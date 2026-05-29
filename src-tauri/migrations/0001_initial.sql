PRAGMA foreign_keys = ON;

CREATE TABLE companies (
    id TEXT PRIMARY KEY,
    exchange TEXT NOT NULL,
    ticker TEXT NOT NULL,
    qualified_ticker TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    isin TEXT,
    cik TEXT,
    lei TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_companies_exchange_ticker ON companies(exchange, ticker);
CREATE INDEX idx_companies_isin ON companies(isin);

CREATE TABLE company_aliases (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(company_id, alias)
);

CREATE TABLE company_source_ids (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    source_adapter_id TEXT NOT NULL,
    source_key TEXT NOT NULL,
    source_value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(source_adapter_id, source_key, source_value)
);

CREATE TABLE watchlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE watchlist_companies (
    watchlist_id TEXT NOT NULL REFERENCES watchlists(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY(watchlist_id, company_id)
);

CREATE TABLE source_adapters (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    source_type TEXT NOT NULL,
    fetch_mode TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    default_poll_interval_seconds INTEGER NOT NULL DEFAULT 900,
    last_success_at TEXT,
    last_error_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE source_adapter_markets (
    source_adapter_id TEXT NOT NULL REFERENCES source_adapters(id) ON DELETE CASCADE,
    market TEXT NOT NULL,
    PRIMARY KEY(source_adapter_id, market)
);

CREATE TABLE source_adapter_state (
    source_adapter_id TEXT NOT NULL REFERENCES source_adapters(id) ON DELETE CASCADE,
    state_key TEXT NOT NULL,
    state_value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY(source_adapter_id, state_key)
);

CREATE TABLE feed_items (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    source_adapter_id TEXT NOT NULL REFERENCES source_adapters(id),
    source_name TEXT NOT NULL,
    source_url TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT,
    body_text TEXT,
    language TEXT,
    published_at TEXT,
    fetched_at TEXT NOT NULL,
    dedupe_key TEXT NOT NULL,
    read INTEGER NOT NULL DEFAULT 0 CHECK(read IN (0, 1)),
    saved INTEGER NOT NULL DEFAULT 0 CHECK(saved IN (0, 1)),
    attribution TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(source_adapter_id, dedupe_key)
);

CREATE INDEX idx_feed_items_published_at ON feed_items(published_at);
CREATE INDEX idx_feed_items_fetched_at ON feed_items(fetched_at);

CREATE TABLE feed_item_companies (
    feed_item_id TEXT NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    match_type TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY(feed_item_id, company_id)
);

CREATE TABLE notebook_entries (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    body_format TEXT NOT NULL DEFAULT 'markdown',
    kind TEXT NOT NULL DEFAULT 'manual',
    claim_status TEXT,
    event_date TEXT,
    review_after TEXT,
    review_date TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_notebook_entries_company_id ON notebook_entries(company_id);
CREATE INDEX idx_notebook_entries_review_date ON notebook_entries(review_date);

CREATE TABLE notebook_entry_tags (
    notebook_entry_id TEXT NOT NULL REFERENCES notebook_entries(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY(notebook_entry_id, tag)
);

CREATE TABLE notebook_entry_provenance (
    id TEXT PRIMARY KEY,
    notebook_entry_id TEXT NOT NULL REFERENCES notebook_entries(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,
    source_id TEXT,
    source_url TEXT,
    label TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE transcript_jobs (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    source_type TEXT NOT NULL,
    source_url TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    finished_at TEXT,
    error TEXT
);

CREATE INDEX idx_transcript_jobs_company_id ON transcript_jobs(company_id);
CREATE INDEX idx_transcript_jobs_status ON transcript_jobs(status);

CREATE TABLE transcript_segments (
    id TEXT PRIMARY KEY,
    transcript_job_id TEXT NOT NULL REFERENCES transcript_jobs(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    start_seconds INTEGER,
    end_seconds INTEGER,
    speaker TEXT,
    text TEXT NOT NULL,
    language TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX idx_transcript_segments_job_id ON transcript_segments(transcript_job_id);

CREATE TABLE ai_analysis_results (
    id TEXT PRIMARY KEY,
    feed_item_id TEXT NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    summary TEXT NOT NULL,
    significance TEXT NOT NULL,
    reasoning TEXT NOT NULL,
    language TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE ai_analysis_tags (
    ai_analysis_result_id TEXT NOT NULL REFERENCES ai_analysis_results(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY(ai_analysis_result_id, tag)
);

CREATE TABLE ai_analysis_source_references (
    id TEXT PRIMARY KEY,
    ai_analysis_result_id TEXT NOT NULL REFERENCES ai_analysis_results(id) ON DELETE CASCADE,
    source_url TEXT NOT NULL,
    label TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

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

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL DEFAULT 'string',
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

INSERT INTO source_adapters (
    id,
    display_name,
    source_type,
    fetch_mode,
    enabled,
    default_poll_interval_seconds
) VALUES (
    'gpw-espi-ebi',
    'GPW ESPI/EBI',
    'official_report',
    'public_page',
    1,
    900
);

INSERT INTO source_adapter_markets (source_adapter_id, market)
VALUES ('gpw-espi-ebi', 'GPW');

INSERT INTO settings (key, value, value_type) VALUES
    ('theme', 'dark', 'string'),
    ('accent_palette', 'night-neon', 'string'),
    ('poll_interval_seconds', '900', 'integer'),
    ('youtube_transcription_provider', 'gemini', 'string'),
    ('general_analysis_provider', '', 'string'),
    ('ai_analysis_mode', 'source_grounded', 'string'),
    ('settings_import_export_format', 'yaml', 'string');
