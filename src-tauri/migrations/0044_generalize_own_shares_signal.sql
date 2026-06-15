-- Generalize the `buyback` signal category to `own_shares`. The original
-- "Share buyback" label mis-typed own-share *sales* (e.g. "Sprzedaż akcji
-- własnych") as buybacks. The generalized category covers all own-share /
-- treasury-share transactions (purchases and sales). See ADR 0034.
--
-- Forward-only and idempotent: it adds/updates the own_shares category,
-- repoints any existing signals off buyback (before the foreign key would
-- block removing it), then drops the obsolete buyback category. Re-running is a
-- no-op once buyback is gone.

INSERT INTO signal_categories (id, key, display_name, derives_event, rule_definition_json) VALUES
    ('sigcat_own_shares', 'own_shares', 'Own-share transactions', 0,
     '{"patterns":["akcji własnych"],"confidence":0.9}')
ON CONFLICT(key) DO UPDATE SET
    display_name = excluded.display_name,
    derives_event = excluded.derives_event,
    rule_definition_json = excluded.rule_definition_json,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

UPDATE company_signals
SET category = 'own_shares',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE category = 'buyback';

DELETE FROM signal_categories WHERE key = 'buyback';
