-- Typed ESPI/EBI event classification (milestone v0.40.0, ADR 0034).
-- Adds the seeded-but-extensible signal_categories registry and the
-- canonical company_signals classification output. Calendar company_events
-- are derived only for forward-looking dated categories (derives_event = 1).

CREATE TABLE signal_categories (
    id TEXT PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    -- JSON rule definition consumed by the interpretation-layer RuleClassifier
    -- (ADR 0035). Shape: { "patterns": [..], "confidence": 0.0..1.0 }
    -- where any case-insensitive substring match against the filing text selects
    -- the category. An empty/absent "patterns" never rule-matches (e.g. `other`,
    -- which is reachable only via the AI fallback).
    rule_definition_json TEXT NOT NULL DEFAULT '{}',
    -- 1 when a confirmed signal in this category materializes a calendar
    -- company_events row (forward-looking dated categories only).
    derives_event INTEGER NOT NULL DEFAULT 0 CHECK(derives_event IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE company_signals (
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
    -- Classification identity: a given filing has at most one signal per category.
    UNIQUE(feed_item_id, category)
);

CREATE INDEX idx_company_signals_company_id ON company_signals(company_id);
CREATE INDEX idx_company_signals_feed_item_id ON company_signals(feed_item_id);
CREATE INDEX idx_company_signals_category ON company_signals(category);
CREATE INDEX idx_company_signals_status ON company_signals(status);
CREATE INDEX idx_company_signals_signal_date ON company_signals(signal_date);

-- Seed the fixed starting taxonomy. Patterns are an initial conservative set,
-- refined empirically from real Bankier ESPI filings in the classifier task.
-- Only dividend and general_meeting derive calendar events (ADR 0034).
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
     '{"patterns":[],"confidence":0.0}');

-- The opt-in AI classification fallback is disabled by default (ADR 0034): no
-- provider calls happen until the user enables it.
INSERT OR IGNORE INTO settings (key, value)
VALUES ('espi_ai_fallback_enabled', 'false');
