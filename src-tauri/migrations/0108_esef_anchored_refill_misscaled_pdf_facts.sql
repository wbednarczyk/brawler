-- v0.59 ESEF-anchored delete-for-refill of misscaled unconfirmed PDF facts.
--
-- A uniformly ×1000 (or ×1000000) misscaled statement passes every same-period
-- accounting identity — assets still equal liabilities + equity when the WHOLE
-- statement is off by one multiplier — so the deterministic "good gate"
-- auto-CONFIRMS it and no comparative column exists to object. The one witness
-- that DOES see the error is the company's OWN ESEF/iXBRL filing for the same
-- metric: a tagged instance reads the value at its true scale. This migration
-- deletes-for-refill the unconfirmed PDF facts whose magnitude is grossly
-- inconsistent with that ESEF anchor, so the fixed parsers re-create them at the
-- right scale on the next extraction pass (the delete-and-refill pattern
-- migrations 0099/0107 already established).
--
-- Predicate — a fact is a target only when it is ALL of:
--   * PDF-tier  (`financial_fact_provenance.source_tier = 'pdf'`),
--   * UNCONFIRMED  (`confirmation_state IN ('auto_unreviewed','pending')` —
--     NEVER `confirmed`: a user- or witness-trusted value is off-limits and is
--     instead surfaced for manual review, never silently deleted), and
--   * grossly off its ESEF anchor: at least 100× the anchor magnitude (a dropped
--     `w tys.`/`w mln` multiplier or a repainted total) OR at most 1/100 of it (a
--     note reference or a sub-line read as the value).
--
-- The ESEF anchor = MAX(|value|) over the SAME company + SAME metric across ANY
-- period among facts with `source_tier = 'esef'`. Magnitude (absolute value) is
-- used on both sides so a mostly-negative metric (a cash-flow line) anchors on
-- its true scale rather than a small positive outlier — the same |·| convention
-- the runtime plausibility gate uses. A company+metric with NO esef fact has NO
-- anchor and is NOT touched here: those surface through the runtime
-- `implausible_against_history` gate (fundamentals::validation) instead. Because
-- migrations run before any extraction, this cleans the medians THAT gate reads
-- before it ever evaluates on the maintainer's machine — the sequencing holds by
-- construction.
--
-- Forward, idempotent, self-healing per the data-model migration rules: every
-- statement is a guarded predicate, a clean database (no esef anchor, or values
-- already at scale) matches nothing, and the runner applies this file exactly
-- once (version-tracked in `schema_migrations`). Covered by the pre-migration
-- snapshot + rotating backups (ADR 0032). No file on disk is touched — DB rows
-- only. A `confirmed` fact of any magnitude, a within-100× unconfirmed fact, and
-- a company with no esef anchor all survive.

-- Materialise the target set once. The runner applies the whole file in a single
-- `execute_batch` on one connection, so this TEMP table is visible to every
-- detach/delete below and vanishes with the connection. Reading the identical
-- set from one place keeps the five statements from drifting apart.
CREATE TEMP TABLE _m0108_refill_targets AS
WITH esef_anchor AS (
    SELECT ff.company_id,
           ff.definition_id,
           MAX(ABS(CAST(ff.value_numeric AS REAL))) AS anchor_magnitude
    FROM financial_facts ff
    JOIN financial_fact_provenance pp ON pp.fact_id = ff.id
    WHERE pp.source_tier = 'esef'
      AND CAST(ff.value_numeric AS REAL) <> 0
    GROUP BY ff.company_id, ff.definition_id
)
SELECT f.id AS fact_id
FROM financial_facts f
JOIN financial_fact_provenance p ON p.fact_id = f.id
JOIN esef_anchor a
    ON a.company_id = f.company_id
   AND a.definition_id = f.definition_id
WHERE p.source_tier = 'pdf'
  AND f.confirmation_state IN ('auto_unreviewed', 'pending')
  AND (
        ABS(CAST(f.value_numeric AS REAL)) >= 100.0 * a.anchor_magnitude
     OR (CAST(f.value_numeric AS REAL) <> 0
         AND ABS(CAST(f.value_numeric AS REAL)) * 100.0 <= a.anchor_magnitude)
      );

-- Detach soft references to the targets first (an autopilot run's produced id
-- list, a manual claim's verification link, a superseding pointer), so undo and
-- claim reads never act on ids about to vanish. supersedes_id/verifying_fact_id
-- are `ON DELETE SET NULL`, so this is belt-and-braces when FKs are enforced and
-- the sole guard when they are not.
UPDATE autopilot_run
SET produced_fact_ids_json = COALESCE((
        SELECT json_group_array(value)
        FROM json_each(autopilot_run.produced_fact_ids_json)
        WHERE value NOT IN (SELECT fact_id FROM _m0108_refill_targets)
    ), '[]')
WHERE json_valid(produced_fact_ids_json)
  AND EXISTS (
        SELECT 1 FROM json_each(autopilot_run.produced_fact_ids_json)
        WHERE value IN (SELECT fact_id FROM _m0108_refill_targets)
  );

UPDATE management_claims
SET verifying_fact_id = NULL
WHERE verifying_fact_id IN (SELECT fact_id FROM _m0108_refill_targets);

UPDATE financial_facts
SET supersedes_id = NULL
WHERE supersedes_id IN (SELECT fact_id FROM _m0108_refill_targets);

-- Delete the facts. Order matches migrations 0102/0107: the FACT goes first so
-- no fact is ever left without its provenance row (the invariant read models
-- depend on); `financial_fact_provenance` has no FK to `financial_facts`, so its
-- now-orphaned row is removed immediately after.
DELETE FROM financial_facts
WHERE id IN (SELECT fact_id FROM _m0108_refill_targets);

DELETE FROM financial_fact_provenance
WHERE fact_id NOT IN (SELECT id FROM financial_facts);

DROP TABLE _m0108_refill_targets;
