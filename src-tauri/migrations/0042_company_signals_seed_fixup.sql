-- Converge the v0.40.0 signal taxonomy on databases that recorded migration
-- 0041 before its seed was finalized during development (migrations run once per
-- version, so edits to 0041 never re-ran on those databases). This forward
-- migration is idempotent: on a fresh database it re-applies the same values
-- 0041 already seeded; on a partially-seeded database it corrects the rule
-- definitions, adds the general_meeting category, and ensures the opt-in setting
-- row exists. See ADR 0034.

-- Ensure the opt-in AI classification fallback setting exists (default disabled).
INSERT OR IGNORE INTO settings (key, value)
VALUES ('espi_ai_fallback_enabled', 'false');

-- Upsert the canonical category registry (key is unique). Corrects rule
-- definitions and adds any missing categories (e.g. general_meeting).
INSERT INTO signal_categories (id, key, display_name, derives_event, rule_definition_json) VALUES
    ('sigcat_insider_transaction', 'insider_transaction', 'Insider transaction', 0,
     '{"patterns":["art. 19 ust. 1 mar","powiadomienie o transakcjach","osób pełniących obowiązki zarządcze","transakcje na akcjach"],"confidence":0.95}'),
    ('sigcat_dividend', 'dividend', 'Dividend', 1,
     '{"patterns":["dywiden"],"confidence":0.92}'),
    ('sigcat_profit_warning', 'profit_warning', 'Profit warning / estimate', 0,
     '{"patterns":["szacunkowe wyniki","wstępne wyniki","szacunkowe skonsolidowane","wstępne skonsolidowane"],"confidence":0.9}'),
    ('sigcat_significant_contract', 'significant_contract', 'Significant contract', 0,
     '{"patterns":["znaczącej umowy","istotnej umowy","zawarcie znaczącej umowy","znacząca umowa"],"confidence":0.9}'),
    ('sigcat_buyback', 'buyback', 'Share buyback', 0,
     '{"patterns":["skup akcji własnych","nabycie akcji własnych","akcji własnych"],"confidence":0.92}'),
    ('sigcat_guidance_change', 'guidance_change', 'Guidance change', 0,
     '{"patterns":["korekta prognozy","zmiana prognozy","aktualizacja prognozy","aktualizacja prognoz"],"confidence":0.9}'),
    ('sigcat_general_meeting', 'general_meeting', 'General meeting', 1,
     '{"patterns":["walnego zgromadzenia","walne zgromadzenie","zwołanie zgromadzenia","zwołanie walnego"],"confidence":0.92}'),
    ('sigcat_other', 'other', 'Other official filing', 0,
     '{"patterns":[],"confidence":0.0}')
ON CONFLICT(key) DO UPDATE SET
    display_name = excluded.display_name,
    derives_event = excluded.derives_event,
    rule_definition_json = excluded.rule_definition_json,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
