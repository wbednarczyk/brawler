-- System-scoped attention events: `company_id` becomes nullable (epic #40 S3,
-- ADR 0091 decision 2).
--
-- Until now every attention event belonged to a company (0077 created the table
-- with `company_id NOT NULL`; 0081 rebuilt it for system events but kept that
-- column NOT NULL). A terminal job failure (ADR 0091 decision 1) raises a SYSTEM
-- event whose work is not always company-scoped — a morning briefing, a history
-- sweep or the aggregator pull fails for the whole workspace, not for one issuer.
-- Forcing a company on such an event would either fabricate a scope or silence
-- the failure entirely.
--
-- SQLite cannot drop a NOT NULL via ALTER, so this is a create/copy/drop/rename
-- table rebuild inside the migration transaction — the same shape 0081 used, and
-- the same reasoning: nothing references `attention_events` by foreign key, so no
-- child rows can be orphaned by the swap. Column set, defaults, CHECKs, the
-- UNIQUE(rule_id, evidence_type, evidence_ref) constraint, the `evidence_title`
-- snapshot added by 0114 and every index (including the system dedup partial
-- index) are reproduced verbatim; only the `company_id` nullability changes. No
-- data is transformed: every existing row keeps its company.
--
-- Append-only and immutable once applied (data-model.md): this migration never
-- edits 0077/0081/0114, it moves the schema forward from whatever they left.

CREATE TABLE attention_events_new (
    id TEXT PRIMARY KEY,
    rule_id TEXT REFERENCES alert_rules (id) ON DELETE CASCADE,   -- NULL for system events
    trigger_type TEXT,                  -- set for system events; NULL derives from the rule
    -- NULL for a system event with no company scope (e.g. a failed morning
    -- briefing / history sweep). Company-scoped events are unchanged.
    company_id TEXT REFERENCES companies (id) ON DELETE CASCADE,
    evidence_type TEXT NOT NULL,
    evidence_ref TEXT NOT NULL,
    fired_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    seen INTEGER NOT NULL DEFAULT 0 CHECK (seen IN (0, 1)),
    dismissed INTEGER NOT NULL DEFAULT 0 CHECK (dismissed IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- Durable fire-time evidence title (0114); preserved verbatim by the rebuild.
    evidence_title TEXT,
    UNIQUE (rule_id, evidence_type, evidence_ref)
);

INSERT INTO attention_events_new (
    id, rule_id, trigger_type, company_id, evidence_type, evidence_ref,
    fired_at, seen, dismissed, created_at, evidence_title
)
SELECT
    id, rule_id, trigger_type, company_id, evidence_type, evidence_ref,
    fired_at, seen, dismissed, created_at, evidence_title
FROM attention_events;

DROP TABLE attention_events;
ALTER TABLE attention_events_new RENAME TO attention_events;

CREATE INDEX IF NOT EXISTS idx_attention_events_rule ON attention_events (rule_id);
CREATE INDEX IF NOT EXISTS idx_attention_events_company ON attention_events (company_id);
CREATE INDEX IF NOT EXISTS idx_attention_events_open
    ON attention_events (dismissed, fired_at);

-- Dedup for system events (rule_id IS NULL), reproduced from 0081: SQLite treats
-- NULL rule_id as distinct, so UNIQUE(rule_id, …) cannot dedup them. A job-failure
-- event's evidence_ref is the (stable) job id, so one event per terminally failed
-- job — a reclaim/re-dispatch of the same row never raises a second event.
CREATE UNIQUE INDEX IF NOT EXISTS idx_attention_events_system_dedup
    ON attention_events (trigger_type, evidence_type, evidence_ref)
    WHERE rule_id IS NULL;
