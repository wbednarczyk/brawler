PRAGMA foreign_keys = ON;

INSERT INTO source_adapters (
    id,
    display_name,
    source_type,
    fetch_mode,
    enabled,
    default_poll_interval_seconds
) VALUES (
    'portal-analiz',
    'Portal Analiz',
    'authenticated_research',
    'authenticated',
    0,
    0
)
ON CONFLICT(id) DO NOTHING;

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES ('portal-analiz', 'GPW');
