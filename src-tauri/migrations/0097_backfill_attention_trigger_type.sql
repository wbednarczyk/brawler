-- Backfill attention_events.trigger_type from the owning alert rule (v0.57.0 fix
-- wave 2, ADR 0068 / W4).
--
-- Change: the attention-event writer now STAMPS trigger_type directly on every
-- row (previously it was left NULL for rule-backed events and derived only at
-- read time via COALESCE against the joined rule). Stamping makes the trigger a
-- first-class column so a direct read / grouping that does not join alert_rules
-- (Today grouping, diagnostics, future consumers) sees the real trigger instead
-- of NULL/empty. This migration repairs legacy rows so grouping is correct
-- immediately on update.
--
-- Forward-only, idempotent, self-healing (data-model migration rules): only rows
-- with a NULL/empty trigger_type AND a still-resolvable owning rule are updated
-- (a rule-backed event cannot outlive its rule — it CASCADE-deletes — so the join
-- always resolves for live rows). System events (rule_id IS NULL) already carry
-- their trigger and are untouched. Re-running is a no-op.

UPDATE attention_events
SET trigger_type = (
        SELECT alert_rules.trigger_type
        FROM alert_rules
        WHERE alert_rules.id = attention_events.rule_id
    )
WHERE (trigger_type IS NULL OR trigger_type = '')
  AND rule_id IS NOT NULL
  AND EXISTS (
        SELECT 1 FROM alert_rules WHERE alert_rules.id = attention_events.rule_id
    );
