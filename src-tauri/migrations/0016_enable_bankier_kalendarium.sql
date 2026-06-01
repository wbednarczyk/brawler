PRAGMA foreign_keys = ON;

UPDATE source_adapters
SET
    enabled = 1,
    source_type = 'public_calendar',
    fetch_mode = 'public_page',
    default_poll_interval_seconds = 21600,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 'bankier-kalendarium-html';
