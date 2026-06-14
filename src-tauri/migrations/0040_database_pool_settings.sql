-- Default connection-pool tuning settings (ADR 0032). Values are read with
-- safe-range clamping and default fallback, but seeding them lets the normal
-- settings update path persist user changes.
INSERT OR IGNORE INTO settings (key, value, value_type) VALUES
    ('db_max_connections', '4', 'number'),
    ('db_busy_timeout_ms', '5000', 'number'),
    ('db_acquire_timeout_ms', '10000', 'number');
