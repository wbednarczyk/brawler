-- ADR 0095: retire the html_positional extraction tier (expanded scope,
-- adversarial-review scope expansion 2026-08-05: full removal now, not
-- deferred).
--
-- The tier's first ground-truth measurement (epic #40/#182, 2026-08-05: 32
-- real filings, 1035 machine-verified labels) found currency-aware
-- precision/recall of 5.6% / 2.0% (78 of its "matches" store no currency at
-- all) against ESEF's 99.3% / 72.4% -- the positional layout heuristics
-- (column picking, note-reference stripping, basis stamping) are the app's
-- most defect-dense surface per fact produced, for near-zero marginal
-- coverage the WDF cover-note tier and BiznesRadar-primary already supply.
-- The owner decision (2026-08-05, ADR 0095 decision 3) is to DELETE the
-- stored `pdf`-tier facts outright rather than demote them below the
-- aggregator: demotion would still surface a measured-5.6%-precision, often
-- currency-less value wherever BiznesRadar does not cover the slot, which
-- "never silently wrong" forbids. Deleting frees the slots honestly -- the
-- history sweep sees the periods as uncovered again, and ESEF/WDF/
-- BiznesRadar re-fill them under their own provenance on the next refresh/
-- sweep (including the 4 witness-corroborated values -- the corroboration
-- itself proves the aggregator already holds them). The production route
-- and parser that produced these facts are REMOVED in the same change
-- (`fundamentals::extraction::html_positional` deleted;
-- `jobs::structured_extraction`'s positional dispatch arm removed) and the
-- coherence guard (`storage::kpi_extraction::write_fact_provenance_fields`)
-- now refuses `source_tier='pdf'` on every NEW write -- no positional (or
-- any other `pdf`-tier) fact can be written again after this migration
-- runs. `SourceTier::Pdf` itself stays in the enum as a legacy READ value
-- only (historical snapshots / MCP surfaces may still name it).
--
-- This migration supersedes the never-shipped draft repair migrations from
-- the earlier #182 fix round (all uncommitted, never run on any
-- installation -- legitimately rewritten/deleted in place rather than left
-- as dead history the repo would otherwise have to carry forward). This
-- version (0135) is the only one that ever shipped, and 0134 is the highest
-- version any earlier draft occupied that survives.
-- Migration 0134 (bug #324 tier/method coherence re-stamp) is
-- left in place: its 7 target rows are deleted by THIS migration a moment
-- later in version order -- harmless, zero-churn.
--
-- Scope: every `financial_facts` row that is `pdf`-tier by EITHER signal --
-- `extraction_method = 'html_positional'` (the surviving tier's own marker)
-- OR its `financial_fact_provenance.source_tier = 'pdf'` (catches any
-- legacy `pdf`/`api` row from the ADR 0086 dec. 1 retired PDF-fact arm too,
-- and is robust to the tier/method pairing regardless of which signal a
-- given row happens to carry) -- plus that provenance row (deleted FIRST,
-- 0107/0128 precedent: no FK/cascade, would otherwise orphan), plus any
-- already-orphaned `source_tier='pdf'` provenance row whose fact is long
-- gone (provenance has no FK, so a historical fact delete can leave one
-- behind -- the tier must end at ZERO rows either way), and any
-- `financial_periods` row this leaves with ZERO facts. Conservative on the
-- period cleanup: a period is only removed when (a) it currently holds at
-- least one fact, (b) EVERY fact it holds is in the retirement set (so
-- deleting them empties it), and (c) NOTHING else still references it --
-- not a `report_documents` row (the source document itself stays a
-- historical record), not a `management_claims.source_period_id` link, not
-- a `framework_evaluations.period_id` link (both FKs are ON DELETE SET
-- NULL: the delete would not fail, it would silently sever a
-- user-meaningful link, which is worse). A period anything still points at
-- is left alone rather than guessed empty.
--
-- The retirement set is materialized ONCE into a temp table before any
-- delete runs: the provenance delete (step 2) would otherwise remove the
-- very `source_tier='pdf'` signal the period-cleanup and fact-delete steps
-- need to identify their own targets.
--
-- Forward, idempotent, self-healing: a DB with no matching row (never had
-- one, or already migrated) runs every statement as a no-op.

-- 0) Materialize the exact retirement set once.
CREATE TEMP TABLE _adr0095_retired_facts AS
SELECT f.id AS fact_id
FROM financial_facts f
LEFT JOIN financial_fact_provenance p ON p.fact_id = f.id
WHERE f.extraction_method = 'html_positional' OR p.source_tier = 'pdf';

-- 1) Provenance first (no FK/cascade -- must not orphan): every in-set
--    fact's provenance row, plus any already-orphaned `pdf`-tier provenance
--    row whose fact was deleted at some earlier point (no FK means such
--    rows can exist; the retired tier must end at zero rows either way).
DELETE FROM financial_fact_provenance
WHERE fact_id IN (SELECT fact_id FROM _adr0095_retired_facts)
   OR (source_tier = 'pdf'
       AND fact_id NOT IN (SELECT id FROM financial_facts));

-- 2) Periods that are about to become fully orphaned by step 3's fact
--    delete: every fact they currently hold is in the retirement set, and
--    nothing else references them -- no report_documents row, no
--    management_claims.source_period_id link, no
--    framework_evaluations.period_id link (both period FKs are ON DELETE
--    SET NULL and would be silently severed, not rejected).
DELETE FROM financial_periods
WHERE EXISTS (
        SELECT 1 FROM financial_facts f WHERE f.period_id = financial_periods.id
    )
  AND NOT EXISTS (
        SELECT 1 FROM financial_facts f
        WHERE f.period_id = financial_periods.id
          AND f.id NOT IN (SELECT fact_id FROM _adr0095_retired_facts)
    )
  AND NOT EXISTS (
        SELECT 1 FROM report_documents rd WHERE rd.period_id = financial_periods.id
    )
  AND NOT EXISTS (
        SELECT 1 FROM management_claims mc
        WHERE mc.source_period_id = financial_periods.id
    )
  AND NOT EXISTS (
        SELECT 1 FROM framework_evaluations fe
        WHERE fe.period_id = financial_periods.id
    );

-- 3) The facts themselves.
DELETE FROM financial_facts WHERE id IN (SELECT fact_id FROM _adr0095_retired_facts);

DROP TABLE _adr0095_retired_facts;
