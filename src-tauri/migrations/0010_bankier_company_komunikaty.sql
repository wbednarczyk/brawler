INSERT INTO source_adapters (
    id,
    display_name,
    source_type,
    fetch_mode,
    enabled,
    default_poll_interval_seconds
)
VALUES (
    'bankier-company-komunikaty',
    'Bankier Company Komunikaty',
    'official_report_secondary',
    'public_json',
    1,
    900
);

INSERT INTO source_adapter_markets (source_adapter_id, market)
VALUES ('bankier-company-komunikaty', 'GPW');
