-- Default worker-pool + per-provider AI concurrency settings (ADR 0059). Values
-- are read with safe-range clamping and default fallback, but seeding them lets
-- the normal settings update path persist user changes. Worker counts apply at
-- startup; the per-provider limit bounds concurrent AI calls across the autopilot
-- and ai lanes. Append-only, idempotent (INSERT OR IGNORE) — never overwrites a
-- user's value.
INSERT OR IGNORE INTO settings (key, value, value_type) VALUES
    ('sources_workers', '2', 'number'),
    ('autopilot_workers', '3', 'number'),
    ('ai_workers', '2', 'number'),
    ('ai_provider_concurrency', '2', 'number');
