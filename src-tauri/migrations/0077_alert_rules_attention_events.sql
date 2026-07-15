-- v0.54 Attention routing (ADR 0068, plan v0.54-attention-routing-briefing §T2).
-- User-owned alert rules + the attention events their evaluation emits.
-- Append-only, immutable once applied (data-model rules). Idempotent and
-- self-healing: CREATE TABLE IF NOT EXISTS is a no-op when present; reads of a
-- missing/absent row tolerate it (no attention event = nothing to show).

PRAGMA foreign_keys = ON;

-- A user-owned alert rule (ADR 0068 decision 3). One trigger type + a scope
-- (a single company or a whole watchlist) + an enabled flag. Trigger-specific
-- columns stay NULL for the trigger types that do not use them:
--   * signal_category  -> signal_category (a signal_categories.key)
--   * price_enters_range -> price_min + price_max (inclusive close band)
--   * price_week52_low / autopilot_run_completed -> none.
CREATE TABLE IF NOT EXISTS alert_rules (
    id TEXT PRIMARY KEY,
    trigger_type TEXT NOT NULL CHECK (trigger_type IN (
        'signal_category',
        'autopilot_run_completed',
        'price_enters_range',
        'price_week52_low'
    )),
    signal_category TEXT REFERENCES signal_categories (key),
    price_min REAL,
    price_max REAL,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('company', 'watchlist')),
    scope_ref TEXT NOT NULL,            -- company_id (company scope) | watchlist_id (watchlist scope)
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_alert_rules_trigger ON alert_rules (trigger_type);
CREATE INDEX IF NOT EXISTS idx_alert_rules_scope ON alert_rules (scope_type, scope_ref);

-- One fired attention event (ADR 0068). Every event links to its evidence
-- (evidence_type + evidence_ref) so the surfaces can click through to the
-- signal / autopilot run / quote that raised it. Dedup: at most one event per
-- (rule, evidence) — re-running evaluation over the same evidence is a no-op
-- (INSERT ... ON CONFLICT DO NOTHING at the storage layer, mirroring
-- classify_and_store_signal). fired_at carries the evidence's DOMAIN date
-- (signal/quote date; now() for autopilot) so dedup and the per-rule daily
-- throttle are deterministic and never keyed on wall-clock ingestion time.
CREATE TABLE IF NOT EXISTS attention_events (
    id TEXT PRIMARY KEY,
    rule_id TEXT NOT NULL REFERENCES alert_rules (id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies (id) ON DELETE CASCADE,
    evidence_type TEXT NOT NULL,        -- 'company_signal' | 'autopilot_run' | 'daily_quote'
    evidence_ref TEXT NOT NULL,         -- signal id | autopilot run id | quote date (YYYY-MM-DD)
    fired_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    seen INTEGER NOT NULL DEFAULT 0 CHECK (seen IN (0, 1)),
    dismissed INTEGER NOT NULL DEFAULT 0 CHECK (dismissed IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (rule_id, evidence_type, evidence_ref)
);

CREATE INDEX IF NOT EXISTS idx_attention_events_rule ON attention_events (rule_id);
CREATE INDEX IF NOT EXISTS idx_attention_events_company ON attention_events (company_id);
CREATE INDEX IF NOT EXISTS idx_attention_events_open
    ON attention_events (dismissed, fired_at);
