-- GPW ESPI/EBI reconciliation second witness (v0.55.0, ADR 0069 decision 2 as
-- amended 2026-07-15; plan v0.55 T3). Re-enables the GPW ESPI/EBI catalog row in
-- a WITNESS role (never a feed source), adds the persisted reconciliation-pair
-- ledger, and extends attention_events so a SYSTEM event (no user rule) can be
-- raised when the primary Bankier channel misses an official report the witness
-- saw (`espi_only`).
--
-- Forward-only, idempotent and self-healing (data-model migration rules).

PRAGMA foreign_keys = ON;

-- 1. Re-enable the GPW ESPI/EBI catalog row (disabled since 0011). Idempotent.
--    The adapter now runs as a reconciliation witness — it fetches the official
--    ESPI/EBI listing but reconciles it against Bankier-sourced reports instead
--    of ingesting into the feed (no dual ingestion; ADR 0069 D2).
UPDATE source_adapters SET enabled = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 'gpw-espi-ebi';

-- 2. Persisted reconciliation-pair ledger. One row per reconciled disclosure:
--      * matched      — a witness ESPI/EBI item that a Bankier-sourced report
--                       for the same tracked company/date/report also carries.
--      * espi_only    — a witness item with no matching Bankier report (the
--                       primary channel missed it) — raises an attention event.
--      * bankier_only — a Bankier-sourced report for a tracked company in the
--                       reconciliation window with no matching witness item.
--    `witness_title` holds the human title of the disclosure (the witness item's
--    title for matched/espi_only; the Bankier report title for bankier_only).
--    `id` is a deterministic, status-independent key derived in Rust from
--    (witness_adapter_id, company, disclosure_date, report identity) so a
--    re-reconciled pair UPDATEs in place (idempotent) rather than duplicating,
--    and its status can flip (espi_only -> matched) once the primary catches up.
--    `company_id` is nullable: an untracked issuer may be recorded as unmatched
--    context, or skipped.
CREATE TABLE IF NOT EXISTS source_reconciliation_results (
    id TEXT PRIMARY KEY,
    witness_adapter_id TEXT NOT NULL,
    company_id TEXT REFERENCES companies(id) ON DELETE CASCADE,
    report_number TEXT,
    report_type TEXT,
    disclosure_date TEXT NOT NULL,
    witness_title TEXT NOT NULL,
    witness_url TEXT,
    status TEXT NOT NULL CHECK(status IN ('matched', 'bankier_only', 'espi_only')),
    primary_feed_item_id TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_source_reconciliation_company
    ON source_reconciliation_results (company_id);
CREATE INDEX IF NOT EXISTS idx_source_reconciliation_status_date
    ON source_reconciliation_results (status, disclosure_date);

-- 3. Extend attention_events for SYSTEM events (ADR 0069 D2 amendment). Until now
--    every event linked to a user-owned alert_rule (rule_id NOT NULL) and derived
--    its trigger_type from that rule. A reconciliation `espi_only` result raises
--    an event with NO user rule and the system trigger_type 'source_reconciliation'.
--    SQLite cannot drop a NOT NULL via ALTER, so rebuild the table: rule_id becomes
--    nullable and a `trigger_type` column carries the system trigger directly (rule
--    events keep it NULL and still derive from the rule via the read-model join).
--    Nothing references attention_events by FK, so the rebuild is a plain
--    create/copy/drop/rename inside the migration transaction.
CREATE TABLE attention_events_new (
    id TEXT PRIMARY KEY,
    rule_id TEXT REFERENCES alert_rules (id) ON DELETE CASCADE,   -- NULL for system events
    trigger_type TEXT,                  -- set for system events; NULL derives from the rule
    company_id TEXT NOT NULL REFERENCES companies (id) ON DELETE CASCADE,
    evidence_type TEXT NOT NULL,
    evidence_ref TEXT NOT NULL,
    fired_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    seen INTEGER NOT NULL DEFAULT 0 CHECK (seen IN (0, 1)),
    dismissed INTEGER NOT NULL DEFAULT 0 CHECK (dismissed IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (rule_id, evidence_type, evidence_ref)
);

INSERT INTO attention_events_new (
    id, rule_id, trigger_type, company_id, evidence_type, evidence_ref,
    fired_at, seen, dismissed, created_at
)
SELECT
    id, rule_id, NULL, company_id, evidence_type, evidence_ref,
    fired_at, seen, dismissed, created_at
FROM attention_events;

DROP TABLE attention_events;
ALTER TABLE attention_events_new RENAME TO attention_events;

CREATE INDEX IF NOT EXISTS idx_attention_events_rule ON attention_events (rule_id);
CREATE INDEX IF NOT EXISTS idx_attention_events_company ON attention_events (company_id);
CREATE INDEX IF NOT EXISTS idx_attention_events_open
    ON attention_events (dismissed, fired_at);

-- Dedup for system events (rule_id IS NULL): the UNIQUE(rule_id, …) above cannot
-- dedup them because SQLite treats NULL rule_id as distinct. A reconciliation
-- event's evidence_ref is the (stable) reconciliation-record id, so one event per
-- record — a re-run of the same espi_only pair never raises a second event.
CREATE UNIQUE INDEX IF NOT EXISTS idx_attention_events_system_dedup
    ON attention_events (trigger_type, evidence_type, evidence_ref)
    WHERE rule_id IS NULL;
