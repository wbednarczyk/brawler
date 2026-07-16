-- Ownership holder-type classification proposals (v0.56.0 T5, ADR 0072 §3).
--
-- Classification order for a holder's `holder_type`: deterministic dictionary
-- (exact + containment) → heuristic name markers (OFE/TFI/FUNDACJA/AKCJE WŁASNE/
-- SKARB PAŃSTWA) → **AI classify-with-confirm** (this table) → manual re-type
-- (always wins). The first three run at ingest and stamp `ownership_stakes`
-- directly. The residual — a dictionary miss with no unambiguous marker — is
-- proposed here by the routable `ownership_holder_classification` AI capability
-- and is NEVER auto-applied: a user confirm applies the type via the existing
-- `set_holder_type` path; a reject just marks the proposal.
--
-- Idempotent per (company, holder): a deterministic id + UNIQUE(company, holder)
-- means re-running the job refreshes a pending/rejected proposal in place — never
-- duplicates, never disturbs a confirmed one. Forward-only, self-healing
-- (IF NOT EXISTS; data-model migration rules).

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS ownership_holder_type_proposals (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- Grouped by the same normalized holder key as `ownership_stakes`
    -- (holder_name_normalized) so a confirm can stamp every one of the holder's
    -- snapshot rows.
    holder_name_normalized TEXT NOT NULL,
    -- The AI-proposed type; same CHECK set as `ownership_stakes.holder_type`.
    proposed_type TEXT NOT NULL CHECK(proposed_type IN (
        'founder_insider', 'family_foundation', 'tfi', 'ofe_pension',
        'state_treasury', 'parent_company', 'treasury_shares',
        'other_institutional', 'free_float_rest'
    )),
    -- Model self-reported confidence in [0,1] (nullable) and a short rationale.
    confidence REAL,
    rationale TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN (
        'pending', 'confirmed', 'rejected'
    )),
    -- Provenance of the proposing model (capability-routed).
    provider_id TEXT,
    model TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(company_id, holder_name_normalized)
);

CREATE INDEX IF NOT EXISTS idx_ownership_holder_type_proposals_company_status
    ON ownership_holder_type_proposals(company_id, status);
