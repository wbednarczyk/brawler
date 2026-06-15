-- Self-healing guard for the v0.40.0 signal tables. A database that recorded
-- migration 0041 as applied but is missing the tables (observed on a portable
-- install whose schema_migrations advanced without the table DDL persisting)
-- would otherwise fail every source refresh with "no such table:
-- signal_categories". These statements are idempotent: CREATE TABLE IF NOT
-- EXISTS is a no-op when the table is present, and the seed upserts. See ADR 0034.

CREATE TABLE IF NOT EXISTS signal_categories (
    id TEXT PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    rule_definition_json TEXT NOT NULL DEFAULT '{}',
    derives_event INTEGER NOT NULL DEFAULT 0 CHECK(derives_event IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS company_signals (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    feed_item_id TEXT NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
    category TEXT NOT NULL REFERENCES signal_categories(key),
    confidence REAL NOT NULL DEFAULT 0.0,
    classified_by TEXT NOT NULL CHECK(classified_by IN ('rule', 'ai')),
    status TEXT NOT NULL CHECK(status IN ('confirmed', 'proposed')),
    signal_date TEXT,
    provider_id TEXT,
    model_id TEXT,
    derived_event_id TEXT REFERENCES company_events(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(feed_item_id, category)
);

CREATE INDEX IF NOT EXISTS idx_company_signals_company_id ON company_signals(company_id);
CREATE INDEX IF NOT EXISTS idx_company_signals_feed_item_id ON company_signals(feed_item_id);
CREATE INDEX IF NOT EXISTS idx_company_signals_category ON company_signals(category);
CREATE INDEX IF NOT EXISTS idx_company_signals_status ON company_signals(status);
CREATE INDEX IF NOT EXISTS idx_company_signals_signal_date ON company_signals(signal_date);

INSERT OR IGNORE INTO settings (key, value)
VALUES ('espi_ai_fallback_enabled', 'false');

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
