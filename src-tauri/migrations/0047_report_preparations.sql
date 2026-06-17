-- Report-season cockpit preparation state (ADR 0044, v0.43.0).
-- The only new persisted state for the milestone: per-occurrence prepare->process
-- workflow status. The cockpit calendar and pre-report card are backend-owned read
-- models composed from existing domains and add no stored projection.
--
-- Keyed by the stable company_events.source_event_key of the report occurrence (not
-- the volatile event row id) so the state survives calendar re-derivation (ADR 0036).
-- Idempotent and self-healing per the migration discipline in AGENTS.md.

CREATE TABLE IF NOT EXISTS report_preparations (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies (id) ON DELETE CASCADE,
    event_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'upcoming',
    prepared_at TEXT,
    processed_at TEXT,
    linked_report_document_id TEXT REFERENCES report_documents (id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, event_key)
);

CREATE INDEX IF NOT EXISTS idx_report_preparations_company
    ON report_preparations (company_id);
