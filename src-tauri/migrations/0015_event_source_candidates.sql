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
        'bankier-kalendarium-html',
        'Bankier Kalendarium',
        'public_calendar',
        'public_page',
        0,
        21600
    ),
    (
        'strefa-report-calendar',
        'Strefa Report Calendar',
        'public_calendar',
        'public_page',
        0,
        21600
    ),
    (
        'money-calendar',
        'Money Calendar',
        'public_calendar',
        'public_page',
        0,
        21600
    )
ON CONFLICT(id) DO NOTHING;

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES
    ('bankier-kalendarium-html', 'GPW'),
    ('strefa-report-calendar', 'GPW'),
    ('money-calendar', 'GPW');
