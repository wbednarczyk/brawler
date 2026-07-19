-- Analyst recommendations (v0.58.0, ADR 0073). Tracks sell-side recommendations
-- (rating, target price, issuing firm) for tracked GPW companies strictly as
-- attributed third-party opinions — never as advice. Revision history accumulates
-- append-only from ingestion start (it cannot be backfilled, ADR 0073 §Context),
-- and each new entry emits a deterministic `recommendation_change` signal so the
-- revision surfaces in the feed, Today home, digests, and alert rules.
--
-- Fed by the `biznesradar-rekomendacje` analyst-recommendation adapter (registry
-- descriptor + runtime wiring in slice A2). The GPW ticker 301-redirects to the
-- canonical BiznesRadar slug; robots.txt is policy-clean for /rekomendacje-spolki/.
--
-- Forward-only, idempotent and self-healing (data-model migration rules): the
-- table uses IF NOT EXISTS, and every seed upsert re-runs without disturbing data.

PRAGMA foreign_keys = ON;

-- Append-only recommendation history: one row per issued recommendation. The
-- source page carries no "rating before" — `direction`, `rating_prev` and
-- `target_prev` are DERIVED at ingest by comparing against the latest prior stored
-- entry of the same firm (none → 'initiate'). Ratings are stored verbatim in the
-- source vocabulary (e.g. 'akumuluj'); target/price figures are decimal-exact TEXT
-- (the repo convention for money/percentage values — see ownership_stakes).
CREATE TABLE IF NOT EXISTS analyst_recommendations (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    firm TEXT NOT NULL,
    analyst TEXT,
    rating TEXT NOT NULL,
    rating_prev TEXT,
    direction TEXT NOT NULL CHECK(direction IN ('upgrade', 'downgrade', 'initiate', 'reiterate')),
    target_price TEXT,
    target_currency TEXT,
    target_prev TEXT,
    price_at_issue TEXT,
    published_at TEXT NOT NULL,
    source_url TEXT NOT NULL,
    report_url TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Natural-key uniqueness (ADR 0073 decision 4): a recommendation is identified by
-- (company, firm, publication date, rating, target). COALESCE(target_price, '')
-- so two null-target entries with the same other columns still collide (SQLite
-- treats bare NULLs as distinct in a UNIQUE index). Re-ingesting the same page is
-- a no-op — the deterministic PK id and this index both enforce dedupe.
CREATE UNIQUE INDEX IF NOT EXISTS idx_analyst_recommendations_natural_key
    ON analyst_recommendations(company_id, firm, published_at, rating, COALESCE(target_price, ''));

-- Per-company, newest-first read path (list read model orders by published_at).
CREATE INDEX IF NOT EXISTS idx_analyst_recommendations_company_published
    ON analyst_recommendations(company_id, published_at DESC);

-- Seed the deterministic `recommendation_change` signal category. Empty patterns:
-- the rule classifier never matches this category by text (the adapter emits the
-- signal directly, not via title classification); the category exists so signal
-- rows, badges, filters and alert rules can join it. Not a forward-looking dated
-- category, so derives_event = 0. Mirrors the `short_position_change` seed (0080).
INSERT INTO signal_categories (id, key, display_name, derives_event, rule_definition_json) VALUES
    ('sigcat_recommendation_change', 'recommendation_change', 'Analyst recommendation change', 0,
     '{"patterns":[],"confidence":1.0}')
ON CONFLICT(key) DO UPDATE SET
    display_name = excluded.display_name,
    derives_event = excluded.derives_event,
    rule_definition_json = excluded.rule_definition_json,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

-- Catalog rows for the adapter (wired alongside the REGISTRY descriptor in the
-- same milestone; 0071-style upsert keeps this idempotent and self-healing).
-- source_type / fetch_mode / markets must match the registry descriptor
-- (drift-guard `registry_matches_seeded_catalog`).
INSERT INTO source_adapters (
    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
) VALUES (
    'biznesradar-rekomendacje', 'BiznesRadar Rekomendacje', 'analyst_recommendation', 'public_page', 1, 86400
)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    source_type = excluded.source_type,
    fetch_mode = excluded.fetch_mode,
    default_poll_interval_seconds = excluded.default_poll_interval_seconds,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES ('biznesradar-rekomendacje', 'GPW');
