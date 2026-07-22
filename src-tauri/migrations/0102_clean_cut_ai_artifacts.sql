-- Clean cut: remove the in-app AI analysis layer's artifacts (ADR 0084 decision
-- 5, revised 2026-07-20 — supersedes that decision's original "keep stored
-- outputs readable / no table drops" text).
--
-- Rationale: backward compatibility with a retired subsystem buys nothing and
-- leaves zombie tables, zombie read surfaces, and a deterministic-coverage
-- number inflated by a source that no longer exists. The boundary below was set
-- against a measurement of the owner's live database, NOT by table name —
-- names mislead here (see the KEPT list at the end of this file).
--
-- Forward, idempotent, self-healing per the data-model migration rules: every
-- table drop is `IF EXISTS`, every reference fix-up is a guarded UPDATE, and the
-- runner applies this file exactly once (version-tracked in `schema_migrations`).
-- Destructive by design, and covered by the app's pre-migration snapshot +
-- rotating backups (v0.38.0, ADR 0032).

-- ---------------------------------------------------------------------------
-- 1. Resolve every reference INTO the AI-sourced facts before deleting them.
--    Done explicitly rather than leaning on `ON DELETE SET NULL`, so the intent
--    is visible in the migration and the outcome does not depend on whether
--    foreign keys happen to be enforced by the connection running it.
-- ---------------------------------------------------------------------------

-- A manual claim verified by an AI fact keeps the claim (the manual path stays)
-- but loses its verification link — the fact behind it is gone.
UPDATE management_claims
SET verifying_fact_id = NULL
WHERE verifying_fact_id IN (
    SELECT fact_id FROM financial_fact_provenance
    WHERE source_tier IN ('ai', 'ai_text')
);

-- A surviving deterministic fact may supersede an AI fact; clear the pointer.
UPDATE financial_facts
SET supersedes_id = NULL
WHERE supersedes_id IN (
    SELECT fact_id FROM financial_fact_provenance
    WHERE source_tier IN ('ai', 'ai_text')
);

-- `autopilot_run.produced_fact_ids_json` is a soft reference (a JSON array, no
-- foreign key), read by run-level undo. Scrub the deleted ids so undo cannot
-- later act on — or miscount — facts that no longer exist. Guarded on
-- `json_valid` because legacy rows may hold '' rather than '[]'.
UPDATE autopilot_run
SET produced_fact_ids_json = COALESCE((
        SELECT json_group_array(value)
        FROM json_each(autopilot_run.produced_fact_ids_json)
        WHERE value NOT IN (
            SELECT fact_id FROM financial_fact_provenance
            WHERE source_tier IN ('ai', 'ai_text')
        )
    ), '[]')
WHERE json_valid(produced_fact_ids_json)
  AND EXISTS (
        SELECT 1
        FROM json_each(autopilot_run.produced_fact_ids_json)
        WHERE value IN (
            SELECT fact_id FROM financial_fact_provenance
            WHERE source_tier IN ('ai', 'ai_text')
        )
  );

-- ---------------------------------------------------------------------------
-- 2. Delete the AI-sourced facts and their provenance.
--
--    Order matters: the FACT goes first, then its provenance row. That way no
--    fact is ever left without provenance at any point (the invariant every
--    read model depends on) — only a provenance row briefly outlives its fact,
--    inside this transaction, before the next statement removes it.
--
--    Scope is both AI-sourced provenance values. `ai` is what the AI confirm
--    path wrote (see `storage::kpi_extraction`: "`ai` (not `ai_text`) because
--    this path sends the native document to the model"); `ai_text` is what the
--    retired tier-4 path wrote via `SourceTier::AiText`. The owner's live
--    database measured `ai`=26 / `ai_text`=0 (alongside `esef`=530, `pdf`=882),
--    so covering both is risk-free there and correct for any other database.
--    After the cut no tier could reproduce or validate these values, and leaving
--    them would inflate the deterministic-coverage measurement.
-- ---------------------------------------------------------------------------

DELETE FROM financial_facts
WHERE id IN (
    SELECT fact_id FROM financial_fact_provenance
    WHERE source_tier IN ('ai', 'ai_text')
);

DELETE FROM financial_fact_provenance
WHERE source_tier IN ('ai', 'ai_text');

-- ---------------------------------------------------------------------------
-- 3. Drop the AI tables. Children before parents so an enforced foreign key
--    never blocks a drop; `IF EXISTS` makes each one self-healing on a database
--    where the table is already absent.
-- ---------------------------------------------------------------------------

-- Feed-item analysis.
DROP TABLE IF EXISTS ai_analysis_tags;
DROP TABLE IF EXISTS ai_analysis_source_references;
DROP TABLE IF EXISTS ai_analysis_results;
DROP TABLE IF EXISTS ai_analysis_jobs;

-- Research briefs.
DROP TABLE IF EXISTS ai_research_brief_citations;
DROP TABLE IF EXISTS ai_research_briefs;
DROP TABLE IF EXISTS ai_research_brief_jobs;

-- Research digests.
DROP TABLE IF EXISTS ai_research_digest_citations;
DROP TABLE IF EXISTS ai_research_digests;
DROP TABLE IF EXISTS ai_research_digest_jobs;

-- Claim extraction (the manual `management_claims` path is untouched).
DROP TABLE IF EXISTS claim_extraction_proposals;
DROP TABLE IF EXISTS claim_extraction_jobs;

-- KPI extraction staging ledger. Confirmed proposals were already materialized
-- into `financial_facts`; only the staging rows and unreviewed OCR/AI-origin
-- proposals go with it.
DROP TABLE IF EXISTS kpi_extraction_proposals;
DROP TABLE IF EXISTS kpi_extraction_jobs;

-- Ownership OCR + AI holder-type proposals (holder types stay user-editable on
-- `ownership_stakes`).
DROP TABLE IF EXISTS ownership_ocr_proposal_rows;
DROP TABLE IF EXISTS ownership_ocr_proposals;
DROP TABLE IF EXISTS ownership_holder_type_proposals;

-- The tier-4 OCR extraction layout (distinct from the deterministic
-- `company_extraction_profile`, which is KEPT).
DROP TABLE IF EXISTS company_ocr_extraction_profile;

-- ---------------------------------------------------------------------------
-- 4. Strip the AI narrative from the morning briefing. The TABLE and its items
--    stay: the briefing is a deterministic composition (`gather_sources` +
--    `compose_briefing`) and that is now the only briefing.
--
--    Plain `DROP COLUMN` (SQLite 3.35+) rather than a table rebuild: a rebuild
--    would have to drop and recreate `morning_briefings`, and
--    `morning_briefing_items` references it `ON DELETE CASCADE` — the rebuild
--    would silently delete every item row. None of these columns is indexed, so
--    the direct drop is safe. Re-runnability comes from the migration runner's
--    once-only guarantee (`schema_migrations`), which is the documented contract
--    for column-level migrations.
-- ---------------------------------------------------------------------------

ALTER TABLE morning_briefings DROP COLUMN narrative_markdown;
ALTER TABLE morning_briefings DROP COLUMN narrative_provider_id;
ALTER TABLE morning_briefings DROP COLUMN narrative_model;

-- ---------------------------------------------------------------------------
-- 4b. Strip the remaining AI columns from tables that otherwise survive. The
--     ROWS stay in every case — only dead columns go, so this is not a data
--     deletion.
--
--     * `history_sweeps.ai_calls_used` / `ai_call_limit` — the tier-4 budget
--       accounting (migration 0065). Tier-4 is retired, so nothing reads or
--       increments them, but the owner's live database still carries real
--       leftover accounting (20 sweeps with `ai_calls_used > 0`) — exactly the
--       residue the clean cut is meant to remove. Every non-AI sweep field
--       (status, candidate/enqueue counters, trigger, timestamps) is untouched.
--     * `management_claims.extraction_proposal_id` — a soft reference (no
--       foreign key) into `claim_extraction_proposals`, which this migration
--       drops. Keeping it would leave a column that can only ever hold a
--       dangling id. The claims themselves and every other column stay.
--
--     Neither column is indexed, so the direct `DROP COLUMN` is safe.
-- ---------------------------------------------------------------------------

ALTER TABLE history_sweeps DROP COLUMN ai_calls_used;
ALTER TABLE history_sweeps DROP COLUMN ai_call_limit;

ALTER TABLE management_claims DROP COLUMN extraction_proposal_id;

-- ---------------------------------------------------------------------------
-- 5. Delete the retired AI settings rows. `youtube_transcription_provider` (and
--    its model/timeout siblings) are deliberately NOT in this list: transcription
--    is data acquisition, not analysis, and is the one remaining AI dependency
--    (ADR 0084 decision 3). Reads of every key below are already gone from the
--    code; the tolerant-read rule covers any that linger in an older binary.
-- ---------------------------------------------------------------------------

DELETE FROM settings
WHERE key IN (
    'ai_analysis_mode',
    'ai_workers',
    'ai_provider_concurrency',
    'capability_providers',
    'general_analysis_provider',
    'general_analysis_model',
    'general_analysis_timeout_seconds',
    'openai_compatible_base_url',
    'espi_ai_fallback_enabled',
    'history_sweep_ai_call_limit'
);

-- ---------------------------------------------------------------------------
-- KEPT — measured as NOT AI despite their names (ADR 0084 decision 5):
--   * `criterion_results`          — all `source='engine'`; deterministic DSL
--                                    evaluations (ADR 0046). The AI-assessor
--                                    path never wrote to the owner's database.
--   * `company_extraction_profile` — deterministic PDF layouts (ADR 0061 dec. 3).
--   * `morning_briefings` / `morning_briefing_items` rows — deterministic
--                                    composition.
--   * `management_claims`          — the manual claims path.
--   * `financial_facts` from deterministic tiers, and their provenance.
--   * `youtube_transcription_*`    — transcription settings.
-- ---------------------------------------------------------------------------
