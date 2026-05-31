UPDATE source_adapters
SET enabled = 0,
    default_poll_interval_seconds = 0,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 'gpw-espi-ebi';

UPDATE source_adapters
SET source_type = 'official_report',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 'bankier-company-komunikaty';
