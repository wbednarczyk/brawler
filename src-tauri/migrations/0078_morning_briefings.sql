-- v0.54 Morning briefing (ADR 0068 decision 4, plan v0.54-attention-routing-briefing §T5).
-- A daily/on-demand briefing: a DETERMINISTICALLY composed item list ("what changed
-- in my companies + what needs doing") plus an OPTIONAL AI-phrased narrative with
-- citations. With no provider configured the briefing still persists as the
-- structured item list (narrative_markdown stays NULL) — never blocked, never an error.
-- Append-only, immutable once applied (data-model rules). Idempotent + self-healing:
-- CREATE TABLE IF NOT EXISTS is a no-op when present; reads of an absent row tolerate
-- it (no briefing = nothing to show).

PRAGMA foreign_keys = ON;

-- One composed briefing. `since` is the domain-date lower bound the composer used
-- (YYYY-MM-DD; '' on the first-ever briefing = include everything): items are the
-- new confirmed signals / completed autopilot runs / fired alerts whose DOMAIN date
-- is strictly after `since`, plus the current claims-due + upcoming-report snapshot.
-- The narrative is decision-support only (facts + citations, never advice); it is
-- stored ONLY when every citation it references resolves to a composed item
-- (citation-integrity tripwire, ADR 0068) — otherwise narrative_markdown is NULL.
CREATE TABLE IF NOT EXISTS morning_briefings (
    id TEXT PRIMARY KEY,
    composed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    since TEXT NOT NULL DEFAULT '',
    narrative_markdown TEXT,               -- NULL: no provider configured OR narrative rejected on citation integrity
    narrative_provider_id TEXT,
    narrative_model TEXT,
    language TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_morning_briefings_composed ON morning_briefings (composed_at);

-- One composed briefing item. This table is BOTH the structured list the Today card
-- renders AND the citeable evidence set the narrative resolves against — each row
-- carries a stable `citation_key` (e.g. 'b1') and the (evidence_type, evidence_ref)
-- pair a narrative citation must match, mirroring ai_research_digest_citations. A
-- briefing is a historical snapshot, so `company_id` is denormalized TEXT (not an FK):
-- deleting a company must not rewrite a past briefing. `position` is the stable
-- composed order (by DOMAIN date, never created_at).
CREATE TABLE IF NOT EXISTS morning_briefing_items (
    id TEXT PRIMARY KEY,
    briefing_id TEXT NOT NULL REFERENCES morning_briefings (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    item_type TEXT NOT NULL,               -- 'signal' | 'autopilot_run' | 'claim_due' | 'report_date' | 'attention_event'
    company_id TEXT NOT NULL,
    domain_date TEXT NOT NULL,             -- ordering key (YYYY-MM-DD)
    citation_key TEXT NOT NULL,            -- stable key the narrative cites (e.g. 'b1')
    evidence_type TEXT NOT NULL,           -- citation-resolution type (matches the narrative citation)
    evidence_ref TEXT NOT NULL,            -- source id: signal id | run id | claim id | attention-event id | company:event_key
    title TEXT NOT NULL,
    detail TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (briefing_id, citation_key)
);

CREATE INDEX IF NOT EXISTS idx_morning_briefing_items_briefing
    ON morning_briefing_items (briefing_id, position);
