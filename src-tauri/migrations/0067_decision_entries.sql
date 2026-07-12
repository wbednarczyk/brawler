-- Decision journal entries (ADR 0071, v0.52.0): the early, forward-compatible
-- slice of the ADR 0043 thesis-workbench journal. Records the user's own
-- judgments (buy / pass / keep_watching / sell_note) with a Markdown rationale
-- and the domain date the decision was made. Decision support only: the app
-- records and mirrors judgments, it never grades them (ADR 0042 posture).
--
-- Entries are IMMUTABLE once saved — corrections are appended as follow-up
-- entries. The follow-up carries superseded_by_entry_id: the id of the entry
-- superseded BY this entry. The link lives on the NEW row pointing back, so no
-- UPDATE of a prior row is ever needed and the triggers below stay absolute.
-- The delete trigger carves out exactly one path: the FK cascade when the
-- owning company is removed (the parent row is already gone when the cascade
-- fires, so the WHEN clause only permits that case).
-- Idempotent and self-healing per the migration discipline in CLAUDE.md.

CREATE TABLE IF NOT EXISTS decision_entries (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('buy', 'pass', 'keep_watching', 'sell_note')),
    rationale_md TEXT NOT NULL,
    decided_at TEXT NOT NULL,
    superseded_by_entry_id TEXT REFERENCES decision_entries (id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_decision_entries_company
    ON decision_entries (company_id);
CREATE INDEX IF NOT EXISTS idx_decision_entries_decided_at
    ON decision_entries (decided_at);

CREATE TRIGGER IF NOT EXISTS decision_entries_immutable_update
BEFORE UPDATE ON decision_entries
BEGIN
    SELECT RAISE(ABORT, 'decision_entries are immutable: append a follow-up entry instead');
END;

CREATE TRIGGER IF NOT EXISTS decision_entries_immutable_delete
BEFORE DELETE ON decision_entries
WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
BEGIN
    SELECT RAISE(ABORT, 'decision_entries are immutable: append a follow-up entry instead');
END;
