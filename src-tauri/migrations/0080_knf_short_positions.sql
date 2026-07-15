-- KNF short-selling registry adapter: storage + typed signal (v0.55.0, ADR 0069
-- decision 3; taxonomy base ADR 0034). Mirrors the public KNF national register
-- of net short positions (holder, size %, date) for tracked GPW companies matched
-- by ISIN, keeps an append-only change history, and seeds the deterministic
-- `short_position_change` signal category.
--
-- Forward-only, idempotent and self-healing (data-model migration rules): tables
-- use IF NOT EXISTS, and every seed upsert re-runs without disturbing data.

PRAGMA foreign_keys = ON;

-- Current-state mirror of the register: one row per (company, holder). A row with
-- exited_at IS NULL is a position currently in the register (>= 0.5%); a non-NULL
-- exited_at marks a position that dropped out of the register (below threshold).
CREATE TABLE IF NOT EXISTS short_positions (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    holder_name TEXT NOT NULL,
    isin TEXT NOT NULL,
    net_position_pct REAL NOT NULL,
    position_date TEXT NOT NULL,
    modify_date TEXT,
    exited_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(company_id, holder_name)
);

CREATE INDEX IF NOT EXISTS idx_short_positions_company ON short_positions(company_id);

-- Append-only change history: one row per detected register change.
CREATE TABLE IF NOT EXISTS short_position_events (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    holder_name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('entered', 'increased', 'decreased', 'exited')),
    from_pct REAL,
    to_pct REAL,
    position_date TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_short_position_events_company ON short_position_events(company_id);

-- Seed the deterministic `short_position_change` signal category. Empty patterns:
-- the rule classifier never matches this category by text (register changes emit
-- signals directly, not via title classification); the category exists so signal
-- rows, badges, filters and alert rules can join it. Not a forward-looking dated
-- category, so derives_event = 0.
INSERT INTO signal_categories (id, key, display_name, derives_event, rule_definition_json) VALUES
    ('sigcat_short_position_change', 'short_position_change', 'Short position', 0,
     '{"patterns":[],"confidence":1.0}')
ON CONFLICT(key) DO UPDATE SET
    display_name = excluded.display_name,
    derives_event = excluded.derives_event,
    rule_definition_json = excluded.rule_definition_json,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

-- Catalog rows for the adapter (wired alongside the REGISTRY descriptor in the
-- same change; 0071-style upsert keeps this idempotent and self-healing).
INSERT INTO source_adapters (
    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
) VALUES (
    'knf-short-selling', 'KNF Short Selling Register', 'disclosure', 'public_json', 1, 86400
)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    source_type = excluded.source_type,
    fetch_mode = excluded.fetch_mode,
    default_poll_interval_seconds = excluded.default_poll_interval_seconds,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES ('knf-short-selling', 'GPW');
