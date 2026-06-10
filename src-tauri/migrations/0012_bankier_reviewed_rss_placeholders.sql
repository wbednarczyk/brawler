PRAGMA foreign_keys = ON;

INSERT INTO source_adapters (
    id,
    display_name,
    source_type,
    fetch_mode,
    enabled,
    default_poll_interval_seconds
) VALUES
    (
        'bankier-firma-rss',
        'Bankier Firma RSS',
        'public_media',
        'rss',
        0,
        0
    ),
    (
        'bankier-wiadomosci-rss',
        'Bankier Wiadomosci RSS',
        'public_media',
        'rss',
        0,
        0
    )
ON CONFLICT(id) DO NOTHING;

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES
    ('bankier-firma-rss', 'GPW'),
    ('bankier-wiadomosci-rss', 'GPW');
