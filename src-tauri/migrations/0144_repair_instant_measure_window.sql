-- ADR 0100 decision 6 (2nd paragraph), epic #398: every fact written before
-- this epic went through the slot-write boundary's `measure_window` default,
-- which ALWAYS defaulted to 'flow' regardless of the definition's
-- `period_nature` (migration 0141) -- so every balance-sheet ('instant')
-- fact was recorded as a period flow (11 341 facts on the maintainer's DB,
-- 4 902 of them instant-nature, 4 484 from the aggregator). The writer fix
-- (`storage/financials.rs` `slot_dims`/`resolve_measure_window`) now derives
-- the default from `period_nature`; this migration repairs the existing
-- rows in the SAME change -- landing the writer fix alone leaves the
-- existing rows wrong, and landing this migration alone would have the
-- very next aggregator pull re-create them at 'flow'.
--
-- `idx_financial_facts_slot` is UNIQUE over (period_id, definition_id,
-- statement_basis, attribution, variant, measure_window, data_quality), so
-- moving a fact's `measure_window` moves it between slots and can collide
-- with an already-occupied destination. On the maintainer's DB no fact is
-- currently anything but 'flow', so every destination is empty and the move
-- is a bare UPDATE -- but the migration must self-heal on a DB where the
-- destination is occupied too.
--
-- Collision resolution (a flow-tier row whose target point_in_time slot
-- already exists) -- never by row order:
--  * Content winner: the repo's real strict source-tier order (highest
--    trust first) -- esef < structured_xhtml < espi_cover_note < pdf <
--    agent < html_aggregator (`fundamentals/extraction/mod.rs` SourceTier;
--    `storage/kpi_extraction.rs` `outranked_stored_tier_of`). A fact with no
--    provenance row, or an unparsed source_tier (`manual`, `ai`, `ai_text`),
--    is untouchable/highest-trust exactly as the runtime treats it -- never
--    outranked, never outranking.
--  * Identity keeper: the point_in_time row's id ALWAYS survives -- it is
--    already correctly typed. Equal values are a re-observation: the flow
--    row's provenance is carried onto the surviving row ONLY when its tier
--    strictly outranks the point_in_time row's (mirrors
--    `apply_structured_precedence`'s Reobserved-upgrade: same value, higher
--    tier takes the label/evidence). Divergent values keep the
--    point_in_time row's value AND provenance untouched, regardless of
--    tier, and record a `diagnostic_events` row (both fact ids/values/tiers)
--    for manual review -- a stronger incoming tier never silently overrides
--    an already-correctly-typed row through this repair.
--  * ONLY the equal-value (re-observation) loser is deleted, after
--    repointing its mutable references (`financial_facts.supersedes_id`,
--    `management_claims.verifying_fact_id`,
--    `autopilot_run.produced_fact_ids_json` -- the same inventory migration
--    0108 enumerates). A DIVERGENT flow row is NEVER deleted: a migration
--    never destroys an observation two sources disagree about (testing.md:
--    a migration never deletes user data) -- it stays in place under its
--    original `measure_window`, and the diagnostic row is the pointer for
--    the owner's manual resolution. Immutable ingest receipts
--    (`kpi_ingest_commit_receipts.outcomes_json`) are NEVER rewritten -- a
--    historical factId embedded there may no longer resolve after this
--    repair (documented in data-model.md).
--
-- EDIT NOTE (2026-08-18, sol review finding 6): the divergent branch
-- originally deleted the flow row after recording the diagnostic. This file
-- was corrected before any release; the single database that had already
-- applied version 144 (the owner's) is verified to have had ZERO collision
-- rows (no `migration_0144` diagnostic events exist there), so the old and
-- new content are behavior-identical everywhere version 144 ever ran.
--
-- Value equality is a trimmed string compare (the `create_or_reobserve_
-- financial_fact` fallback path) -- sufficient here because every fact is
-- written by app code in canonical decimal-string form.
--
-- Forward, idempotent, self-healing: every statement is a guarded predicate
-- keyed off `measure_window = 'flow'` joined to an `instant`-nature
-- definition, so a clean re-run (nothing left at 'flow' for an instant
-- definition) matches nothing.

CREATE TEMP TABLE _m0144_targets AS
SELECT
    f.id AS flow_id,
    f.period_id,
    f.definition_id,
    (
        SELECT p2.id FROM financial_facts p2
        WHERE p2.period_id = f.period_id
          AND p2.definition_id = f.definition_id
          AND p2.statement_basis = f.statement_basis
          AND p2.attribution = f.attribution
          AND p2.variant = f.variant
          AND p2.data_quality = f.data_quality
          AND p2.measure_window = 'point_in_time'
    ) AS pit_id
FROM financial_facts f
JOIN kpi_definitions d ON d.id = f.definition_id
WHERE f.measure_window = 'flow'
  AND d.period_nature = 'instant';

-- Repoint-and-delete applies ONLY to equal-value re-observations. A
-- divergent flow row is preserved in place (see the edit note above), so it
-- must keep its references and its row.
CREATE TEMP TABLE _m0144_repoints AS
SELECT t.flow_id AS old_id, t.pit_id AS new_id
FROM _m0144_targets t
JOIN financial_facts ff ON ff.id = t.flow_id
JOIN financial_facts pf ON pf.id = t.pit_id
WHERE t.pit_id IS NOT NULL
  AND TRIM(ff.value_numeric) = TRIM(pf.value_numeric);

CREATE TEMP TABLE _m0144_collisions AS
SELECT
    t.flow_id,
    t.pit_id,
    CASE WHEN TRIM(ff.value_numeric) = TRIM(pf.value_numeric) THEN 1 ELSE 0 END AS values_equal,
    fp.source_tier AS flow_tier,
    pp.source_tier AS pit_tier
FROM _m0144_targets t
JOIN financial_facts ff ON ff.id = t.flow_id
JOIN financial_facts pf ON pf.id = t.pit_id
LEFT JOIN financial_fact_provenance fp ON fp.fact_id = t.flow_id
LEFT JOIN financial_fact_provenance pp ON pp.fact_id = t.pit_id
WHERE t.pit_id IS NOT NULL;

-- 1) No collision: the fact keeps its id, only measure_window moves.
UPDATE financial_facts
SET measure_window = 'point_in_time',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (SELECT flow_id FROM _m0144_targets WHERE pit_id IS NULL);

-- 2) Collision, equal values, flow tier PARSES and STRICTLY OUTRANKS the
--    point_in_time tier: carry the flow row's provenance onto the
--    surviving point_in_time row (the value is already identical).
--    `extraction_method` lives on `financial_facts`, not the provenance
--    table (`write_fact_provenance_fields` syncs both in the same call at
--    runtime; this mirrors it across two statements).
CREATE TEMP TABLE _m0144_upgrades AS
SELECT flow_id, pit_id FROM _m0144_collisions
WHERE values_equal = 1
  AND flow_tier IN ('esef', 'structured_xhtml', 'espi_cover_note', 'pdf', 'agent', 'html_aggregator')
  AND pit_tier IN ('esef', 'structured_xhtml', 'espi_cover_note', 'pdf', 'agent', 'html_aggregator')
  AND (CASE flow_tier
        WHEN 'esef' THEN 1 WHEN 'structured_xhtml' THEN 2 WHEN 'espi_cover_note' THEN 3
        WHEN 'pdf' THEN 4 WHEN 'agent' THEN 5 WHEN 'html_aggregator' THEN 6 END)
    < (CASE pit_tier
        WHEN 'esef' THEN 1 WHEN 'structured_xhtml' THEN 2 WHEN 'espi_cover_note' THEN 3
        WHEN 'pdf' THEN 4 WHEN 'agent' THEN 5 WHEN 'html_aggregator' THEN 6 END);

UPDATE financial_fact_provenance
SET
    source_tier = (
        SELECT fp.source_tier FROM financial_fact_provenance fp
        JOIN _m0144_upgrades u ON u.flow_id = fp.fact_id
        WHERE u.pit_id = financial_fact_provenance.fact_id
    ),
    validation_status = (
        SELECT fp.validation_status FROM financial_fact_provenance fp
        JOIN _m0144_upgrades u ON u.flow_id = fp.fact_id
        WHERE u.pit_id = financial_fact_provenance.fact_id
    ),
    drift_json = (
        SELECT fp.drift_json FROM financial_fact_provenance fp
        JOIN _m0144_upgrades u ON u.flow_id = fp.fact_id
        WHERE u.pit_id = financial_fact_provenance.fact_id
    ),
    citation = (
        SELECT fp.citation FROM financial_fact_provenance fp
        JOIN _m0144_upgrades u ON u.flow_id = fp.fact_id
        WHERE u.pit_id = financial_fact_provenance.fact_id
    ),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE fact_id IN (SELECT pit_id FROM _m0144_upgrades);

UPDATE financial_facts
SET extraction_method = (
        SELECT ff.extraction_method FROM financial_facts ff
        JOIN _m0144_upgrades u ON u.flow_id = ff.id
        WHERE u.pit_id = financial_facts.id
    ),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (SELECT pit_id FROM _m0144_upgrades);

DROP TABLE _m0144_upgrades;

-- 3) Collision, divergent values: the point_in_time row is ALREADY correct
--    and is kept untouched. Record a durable diagnostic for manual review.
INSERT INTO diagnostic_events (module, scope_type, scope_id, stage, severity, message, metadata_json)
SELECT
    'financial_facts_measure_window_repair',
    'financial_fact',
    c.pit_id,
    'migration_0144',
    'warning',
    'measure_window repair collision: a flow-tier fact diverged from the already-correct point_in_time fact for the same slot; the point_in_time value was kept and the flow-tier row was left in place under its original measure_window for manual resolution (ADR 0100 decision 6)',
    json_object(
        'kept_fact_id', c.pit_id,
        'kept_value', pf.value_numeric,
        'kept_source_tier', c.pit_tier,
        'divergent_fact_id', c.flow_id,
        'divergent_value', ff.value_numeric,
        'divergent_source_tier', c.flow_tier,
        'definition_id', t.definition_id,
        'period_id', t.period_id
    )
FROM _m0144_collisions c
JOIN _m0144_targets t ON t.flow_id = c.flow_id
JOIN financial_facts ff ON ff.id = c.flow_id
JOIN financial_facts pf ON pf.id = c.pit_id
WHERE c.values_equal = 0;

-- 4) Repoint mutable references off every collision loser before deleting it.
UPDATE autopilot_run
SET produced_fact_ids_json = COALESCE((
        SELECT json_group_array(
            COALESCE((SELECT new_id FROM _m0144_repoints WHERE old_id = value), value)
        )
        FROM json_each(autopilot_run.produced_fact_ids_json)
    ), '[]')
WHERE json_valid(produced_fact_ids_json)
  AND EXISTS (
        SELECT 1 FROM json_each(autopilot_run.produced_fact_ids_json)
        WHERE value IN (SELECT old_id FROM _m0144_repoints)
  );

UPDATE management_claims
SET verifying_fact_id = (SELECT new_id FROM _m0144_repoints WHERE old_id = management_claims.verifying_fact_id)
WHERE verifying_fact_id IN (SELECT old_id FROM _m0144_repoints);

UPDATE financial_facts
SET supersedes_id = (SELECT new_id FROM _m0144_repoints WHERE old_id = financial_facts.supersedes_id)
WHERE supersedes_id IN (SELECT old_id FROM _m0144_repoints);

-- 5) Delete the equal-value collision losers, then their now-orphaned provenance rows
--    (the fact goes first so no fact is ever left without its provenance
--    row mid-statement -- migrations 0102/0107/0108 pattern).
DELETE FROM financial_facts
WHERE id IN (SELECT old_id FROM _m0144_repoints);

DELETE FROM financial_fact_provenance
WHERE fact_id NOT IN (SELECT id FROM financial_facts);

DROP TABLE _m0144_collisions;
DROP TABLE _m0144_repoints;
DROP TABLE _m0144_targets;
