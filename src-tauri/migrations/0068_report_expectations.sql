-- Pre-report expectations (ADR 0071, v0.52.0): what the user expected BEFORE a
-- report landed, so hindsight bias has a check. Keyed by the same stable
-- occurrence key as report_preparations (company_id + the report occurrence's
-- company_events.source_event_key), with the occurrence resolved to the
-- (fiscal_year, period_type) the report covers — that resolution is what the
-- freeze rule joins against the facts coverage.
--
-- Editable until the period's facts are recorded; the store checks coverage
-- inside the update transaction, stamps frozen_at on first observation, and
-- refuses further edits. resolution_note_md / resolved_at hold the user's OWN
-- verdict at review time — the app never scores judgment automatically.
-- expected_value is decimal-exact text in base units, matching
-- financial_facts.value_numeric (parsed with rust_decimal).
-- Idempotent and self-healing per the migration discipline in CLAUDE.md.

CREATE TABLE IF NOT EXISTS report_expectations (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies (id) ON DELETE CASCADE,
    event_key TEXT NOT NULL,
    fiscal_year INTEGER NOT NULL,
    period_type TEXT NOT NULL,
    stance_md TEXT NOT NULL,
    frozen_at TEXT,
    resolution_note_md TEXT,
    resolved_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (company_id, event_key)
);

CREATE INDEX IF NOT EXISTS idx_report_expectations_company
    ON report_expectations (company_id);

CREATE TABLE IF NOT EXISTS report_expectation_metrics (
    id TEXT PRIMARY KEY,
    expectation_id TEXT NOT NULL REFERENCES report_expectations (id) ON DELETE CASCADE,
    metric_key TEXT NOT NULL,
    comparator TEXT NOT NULL CHECK (comparator IN ('lt', 'lte', 'eq', 'gte', 'gt')),
    expected_value TEXT NOT NULL,
    unit TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_report_expectation_metrics_expectation
    ON report_expectation_metrics (expectation_id);
