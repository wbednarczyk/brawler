PRAGMA foreign_keys = ON;

INSERT INTO source_adapters (
    id,
    display_name,
    source_type,
    fetch_mode,
    enabled,
    default_poll_interval_seconds
) VALUES (
    'gpw-market-events-rss',
    'GPW Market Events RSS',
    'official_calendar',
    'rss',
    1,
    21600
)
ON CONFLICT(id) DO NOTHING;

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES ('gpw-market-events-rss', 'GPW');
