-- Ownership ingestion streams 2 & 3 (v0.56.0, ADR 0072 §2b/§2c, plan v0.56 T4):
-- the `major_holdings_change` ESPI signal that deterministically updates stakes,
-- and the aggregator "Akcjonariat" witness (compare-only, never a source of
-- truth). Seeds one signal category (DATA, not a Rust enum), and adds two small
-- ledger tables that mirror the reconciliation idiom (deterministic id, status,
-- checked_at) so re-runs UPDATE in place rather than duplicating.
--
-- Forward-only, idempotent and self-healing (data-model migration rules): the
-- category upsert re-seeds without touching existing signals; the tables use
-- IF NOT EXISTS.

PRAGMA foreign_keys = ON;

-- 1. `major_holdings_change` signal category (ADR 0034 taxonomy extension).
-- Threshold-crossing notifications under art. 69 of the public-offering act are
-- formulaic; the rule classifier matches the (lowercased) filing title against
-- these substrings. Not a forward-dated category, so it derives no calendar event
-- (derives_event = 0), consistent with the other high-signal disclosure
-- categories. Conservative confidence (only a confident title match becomes a
-- confirmed signal; the resulting-stake write is a separate deterministic parse).
INSERT INTO signal_categories (id, key, display_name, derives_event, rule_definition_json) VALUES
    ('sigcat_major_holdings_change', 'major_holdings_change', 'Major holdings change', 0,
     '{"patterns":["znaczny pakiet akcji","znacznych pakietów akcji","art. 69","zmiana udziału w ogólnej liczbie głosów","zmianie udziału w ogólnej liczbie głosów","przekroczenie progu","zawiadomienie o zmianie udziału","zejście poniżej progu","zawiadomienie o zmianie stanu posiadania"],"confidence":0.9}')
ON CONFLICT(key) DO UPDATE SET
    display_name = excluded.display_name,
    derives_event = excluded.derives_event,
    rule_definition_json = excluded.rule_definition_json,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

-- 2. ESPI notifications the DETERMINISTIC stake parser could not safely turn into
-- a stake (conflicting before/after percentages, or no confidently-extractable
-- holder). NO stake was written — never guess (ADR 0072). Keyed by feed item, so
-- the stake-update sweep attempts each filing exactly once (self-heals: the row
-- is only ever inserted after a clean parse fails). The paired diagnostic event
-- carries the same reason for developer observability.
CREATE TABLE IF NOT EXISTS ownership_espi_unparsed (
    feed_item_id TEXT PRIMARY KEY REFERENCES feed_items(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- Machine-stable reason: holder_unresolved | multiple_holding_values |
    -- multiple_change_values | not_found.
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 3. Aggregator-witness comparison ledger (ADR 0072 §2c; ADR 0061 witness idiom).
-- One row per (adapter, company): the LAST comparison of the aggregator's
-- "Akcjonariat" table against our disclosed current state. The witness NEVER
-- writes ownership_stakes — this is health/observability only. Divergences (a
-- holder present on one side only above threshold, or a percentage differing by
-- more than the tolerance) are counted here and recorded as diagnostic events.
CREATE TABLE IF NOT EXISTS ownership_witness_results (
    id TEXT PRIMARY KEY,
    adapter_id TEXT NOT NULL,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- agree = no divergence; diverged = at least one; no_reference = we hold no
    -- disclosed stakes to compare against yet.
    status TEXT NOT NULL CHECK(status IN ('agree', 'diverged', 'no_reference')),
    holders_compared INTEGER NOT NULL DEFAULT 0,
    divergence_count INTEGER NOT NULL DEFAULT 0,
    checked_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(adapter_id, company_id)
);

CREATE INDEX IF NOT EXISTS idx_ownership_witness_results_company
    ON ownership_witness_results(company_id);

-- 4. Catalog row for the aggregator-witness adapter (wired alongside the REGISTRY
-- descriptor in the same change; idempotent upsert keeps it self-healing). Daily
-- cadence keeps the per-company fetches polite. Witness role — it never ingests.
INSERT INTO source_adapters (
    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
) VALUES (
    'biznesradar-akcjonariat', 'BiznesRadar Akcjonariat (witness)', 'ownership_witness', 'public_page', 1, 86400
)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    source_type = excluded.source_type,
    fetch_mode = excluded.fetch_mode,
    default_poll_interval_seconds = excluded.default_poll_interval_seconds,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

INSERT OR IGNORE INTO source_adapter_markets (source_adapter_id, market)
VALUES ('biznesradar-akcjonariat', 'GPW');
