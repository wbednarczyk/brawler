CREATE TABLE company_registry_entries (
    id TEXT PRIMARY KEY,
    exchange TEXT NOT NULL,
    ticker TEXT NOT NULL,
    qualified_ticker TEXT NOT NULL,
    display_name TEXT NOT NULL,
    isin TEXT,
    source_adapter_id TEXT NOT NULL REFERENCES source_adapters(id),
    source_url TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(exchange, ticker)
);

CREATE INDEX idx_company_registry_entries_qualified_ticker ON company_registry_entries(qualified_ticker);
CREATE INDEX idx_company_registry_entries_isin ON company_registry_entries(isin);

INSERT INTO source_adapters (
    id,
    display_name,
    source_type,
    fetch_mode,
    enabled,
    default_poll_interval_seconds
) VALUES (
    'gpw-company-registry',
    'GPW Company Registry',
    'company_registry',
    'public_page',
    1,
    86400
);

INSERT INTO source_adapter_markets (source_adapter_id, market)
VALUES ('gpw-company-registry', 'GPW');
