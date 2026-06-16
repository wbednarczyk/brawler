-- Management claims tracker (v0.42.0, epic cbf6999, ADR 0040).
-- Promotes claims from notebook_entries(kind='claim') to a first-class entity with
-- a normalized due period and a user-set verdict. Idempotent and self-healing: the
-- forward migration of existing claim notes keeps each claim's id EQUAL to its
-- originating notebook_entries.id, so existing evidence_links and reminders that
-- reference the claim by id keep resolving without re-pointing.

CREATE TABLE IF NOT EXISTS management_claims (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    statement TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    body_format TEXT NOT NULL DEFAULT 'markdown',
    made_at TEXT,                                   -- YYYY-MM-DD, when the statement was made
    source_period_id TEXT REFERENCES financial_periods(id) ON DELETE SET NULL,
    due_fiscal_year INTEGER,                         -- resurfacing match key (with due_period_type)
    due_period_type TEXT,                            -- FY|H1|H2|Q1..Q4|9M|M01..M12
    status TEXT NOT NULL DEFAULT 'pending',          -- pending|delivered|partially_delivered|missed|revised
    source_evidence_type TEXT NOT NULL DEFAULT 'manual', -- report_document|transcript_segment|feed_item|manual
    source_evidence_id TEXT,                         -- soft reference to the source row
    extraction_proposal_id TEXT,                     -- claim_extraction_proposals(id) when AI-extracted
    target_metric_key TEXT,                          -- quantitative claim: KPI metric key
    target_comparator TEXT,                          -- gte|lte|gt|lt|approx|eq
    target_value_numeric TEXT,                       -- decimal-exact text
    target_unit TEXT,
    verifying_fact_id TEXT REFERENCES financial_facts(id) ON DELETE SET NULL,
    revises_claim_id TEXT REFERENCES management_claims(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_management_claims_company ON management_claims(company_id);
CREATE INDEX IF NOT EXISTS idx_management_claims_due ON management_claims(due_fiscal_year, due_period_type);
CREATE INDEX IF NOT EXISTS idx_management_claims_status ON management_claims(status);

-- Forward migration of existing claim notes. Self-healing: INSERT OR IGNORE on the
-- shared id converges every database regardless of prior state, and re-running never
-- duplicates. The note rows are intentionally left in notebook_entries (reversible);
-- the research read model switches its claim source to this table in the same milestone.
INSERT OR IGNORE INTO management_claims (
    id, company_id, statement, body, body_format, made_at,
    due_fiscal_year, due_period_type, status,
    source_evidence_type, created_at, updated_at
)
SELECT
    n.id,
    n.company_id,
    CASE
        WHEN TRIM(COALESCE(n.title, '')) != '' THEN n.title
        ELSE COALESCE(NULLIF(TRIM(n.body), ''), 'Claim')
    END AS statement,
    COALESCE(n.body, '') AS body,
    COALESCE(NULLIF(TRIM(n.body_format), ''), 'markdown') AS body_format,
    n.event_date AS made_at,
    -- follow_up_after like 'YYYY-Q4' / 'YYYY-H1' / 'YYYY-FY' parsed into year + period type.
    CASE
        WHEN n.follow_up_after GLOB '[0-9][0-9][0-9][0-9]-*'
        THEN CAST(SUBSTR(n.follow_up_after, 1, 4) AS INTEGER)
    END AS due_fiscal_year,
    CASE
        WHEN n.follow_up_after GLOB '[0-9][0-9][0-9][0-9]-*'
        THEN UPPER(SUBSTR(n.follow_up_after, 6))
    END AS due_period_type,
    CASE COALESCE(n.claim_status, 'open')
        WHEN 'open' THEN 'pending'
        WHEN 'delivered' THEN 'delivered'
        WHEN 'partially_delivered' THEN 'partially_delivered'
        WHEN 'missed' THEN 'missed'
        WHEN 'unknown' THEN 'pending'
        WHEN 'not_applicable' THEN 'pending'
        ELSE 'pending'
    END AS status,
    'manual' AS source_evidence_type,
    COALESCE(n.created_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) AS created_at,
    COALESCE(n.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) AS updated_at
FROM notebook_entries n
WHERE n.kind = 'claim' OR n.claim_status IS NOT NULL;
