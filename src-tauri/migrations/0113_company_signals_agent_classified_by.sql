-- Relax the company_signals.classified_by CHECK to allow 'agent' alongside
-- 'rule' and 'ai' (v0.60.0 M4, ADR 0088 dec. 4). Signals created by the MCP
-- triage tool `classify_filing` are authored by a connected agent, not the
-- deterministic rule classifier — honest provenance labels are core to this
-- app's posture (ADR 0084/0086/0088), so an agent classification must NOT
-- masquerade as `rule`.
--
-- SQLite cannot ALTER a CHECK constraint, so this rebuilds the table (the
-- standard 12-step). Nothing FK-references company_signals, so the drop/rename
-- is safe under the runner's `foreign_keys = ON`. Existing rows are copied
-- verbatim (their `rule`/`ai` values stay valid). Idempotent/self-healing: the
-- temp table is dropped first in case a prior partial run left it, and the
-- migration is version-tracked so it applies exactly once.

DROP TABLE IF EXISTS company_signals_new;

CREATE TABLE company_signals_new (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    feed_item_id TEXT NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
    category TEXT NOT NULL REFERENCES signal_categories(key),
    confidence REAL NOT NULL DEFAULT 0.0,
    classified_by TEXT NOT NULL CHECK(classified_by IN ('rule', 'ai', 'agent')),
    status TEXT NOT NULL CHECK(status IN ('confirmed', 'proposed')),
    signal_date TEXT,
    provider_id TEXT,
    model_id TEXT,
    derived_event_id TEXT REFERENCES company_events(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(feed_item_id, category)
);

INSERT INTO company_signals_new (
    id, company_id, feed_item_id, category, confidence, classified_by, status,
    signal_date, provider_id, model_id, derived_event_id, created_at, updated_at
)
SELECT
    id, company_id, feed_item_id, category, confidence, classified_by, status,
    signal_date, provider_id, model_id, derived_event_id, created_at, updated_at
FROM company_signals;

DROP TABLE company_signals;

ALTER TABLE company_signals_new RENAME TO company_signals;

CREATE INDEX IF NOT EXISTS idx_company_signals_company_id ON company_signals(company_id);
CREATE INDEX IF NOT EXISTS idx_company_signals_feed_item_id ON company_signals(feed_item_id);
CREATE INDEX IF NOT EXISTS idx_company_signals_category ON company_signals(category);
CREATE INDEX IF NOT EXISTS idx_company_signals_status ON company_signals(status);
CREATE INDEX IF NOT EXISTS idx_company_signals_signal_date ON company_signals(signal_date);
