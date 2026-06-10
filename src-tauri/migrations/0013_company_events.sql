CREATE TABLE company_events (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    title TEXT NOT NULL,
    event_date TEXT NOT NULL,
    event_time TEXT,
    status TEXT NOT NULL DEFAULT 'scheduled',
    source_type TEXT NOT NULL DEFAULT 'manual',
    source_adapter_id TEXT REFERENCES source_adapters(id),
    source_event_key TEXT,
    source_url TEXT,
    attribution TEXT,
    fetched_at TEXT,
    manual INTEGER NOT NULL DEFAULT 1 CHECK(manual IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(source_adapter_id, source_event_key)
);

CREATE INDEX idx_company_events_company_id ON company_events(company_id);
CREATE INDEX idx_company_events_event_date ON company_events(event_date);
CREATE INDEX idx_company_events_event_type ON company_events(event_type);
CREATE INDEX idx_company_events_status ON company_events(status);
