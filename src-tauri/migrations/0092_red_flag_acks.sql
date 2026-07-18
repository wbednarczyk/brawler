-- Red-flag acknowledgements + derived-flag signal categories (v0.57.0 T7,
-- ADR 0083 Decisions 8/9). Active red flags are a COMPUTED read model
-- (`red_flags_view`) — never stored; only the user's acknowledgement state and
-- the raise-events (signals, for alerting/dedup) persist (ADR 0083 "Rejected: a
-- stored red_flags table"). Raising a derived flag follows the KNF pattern
-- (0080): one synthetic feed_items row + one confirmed company_signals row in a
-- new empty-pattern category, so existing ADR 0068 signal_category alert rules
-- fire with zero new alert plumbing.
--
-- Forward-only, idempotent and self-healing (data-model migration rules): tables
-- use IF NOT EXISTS, every seed upsert re-runs without disturbing data.

PRAGMA foreign_keys = ON;

-- The only persisted red-flag state: one row per acknowledged flag instance. The
-- flag_id is deterministic (type + company + evidence identity, `rf:<type>:<company>:<evidence>`),
-- so a re-detection of the same evidence maps to the same flag_id and the read
-- model keeps it out of the active list — an acked flag never re-surfaces and
-- (via INSERT OR IGNORE on the signal) never re-fires an alert.
CREATE TABLE IF NOT EXISTS red_flag_acks (
    flag_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    flag_type TEXT NOT NULL,
    acked_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_red_flag_acks_company ON red_flag_acks(company_id);

-- Seed the three derived-flag signal categories (empty patterns: the rule
-- classifier never matches these by text — detection jobs write the signal
-- directly, KNF `short_position_change` precedent). The category exists so signal
-- rows, badges, filters and alert rules can join it. Not forward-looking dated
-- categories, so derives_event = 0. `auditor_opinion` and `short_position_change`
-- already exist and are composed at read (no new raising).
INSERT INTO signal_categories (id, key, display_name, derives_event, rule_definition_json) VALUES
    ('sigcat_report_delay', 'report_delay', 'Report delay', 0,
     '{"patterns":[],"confidence":1.0}'),
    ('sigcat_fund_exit', 'fund_exit', 'Fund exit', 0,
     '{"patterns":[],"confidence":1.0}'),
    ('sigcat_score_deterioration', 'score_deterioration', 'Score deterioration', 0,
     '{"patterns":[],"confidence":1.0}')
ON CONFLICT(key) DO UPDATE SET
    display_name = excluded.display_name,
    derives_event = excluded.derives_event,
    rule_definition_json = excluded.rule_definition_json,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

-- Internal source adapter that owns the synthetic feed items a raised red flag
-- writes (feed_items.source_adapter_id is NOT NULL / FK). enabled = 0 so the
-- scheduler never polls it — it has no fetch descriptor; the store writes its
-- rows directly (KNF `short_position_change` precedent).
INSERT INTO source_adapters (
    id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
) VALUES (
    'brawler-red-flags', 'Brawler — derived red flags', 'derived', 'internal', 0, 86400
)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    source_type = excluded.source_type,
    fetch_mode = excluded.fetch_mode,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
