PRAGMA foreign_keys = ON;

INSERT INTO source_adapters (
    id,
    display_name,
    source_type,
    fetch_mode,
    enabled,
    default_poll_interval_seconds
) VALUES (
    'bankier-market-rss',
    'Bankier Giełda RSS',
    'public_media',
    'rss',
    1,
    900
)
ON CONFLICT(id) DO NOTHING;

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES ('bankier-market-rss', 'GPW');
