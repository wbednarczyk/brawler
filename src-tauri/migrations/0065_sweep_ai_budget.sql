-- History-sweep AI budget (ADR 0077 §6, T5.2/F3b). The sweep's tier-4 spend is
-- capped by a per-sweep call counter: the limit is SNAPSHOTTED onto the sweep row
-- at creation (`ai_call_limit`) so a mid-sweep settings change never moves the
-- gate, and `ai_calls_used` is bumped atomically as each document enters tier-4
-- (one invocation = one unit; `ai_call_limit = 0` means unlimited). The
-- `autopilot_run.sweep_id` back-reference lets an enqueued extraction run charge
-- the sweep that spawned it (a detection run has none).
--
-- Append-only, idempotent, self-healing: plain ADD COLUMNs (no table rebuild), so
-- the runner applies this exactly once and old rows read the defaults; the new
-- columns are writable immediately (0 for the counters, NULL for the reference).

-- Per-sweep tier-4 budget: units spent, and the ceiling snapshotted at creation.
ALTER TABLE history_sweeps ADD COLUMN ai_calls_used INTEGER NOT NULL DEFAULT 0;
ALTER TABLE history_sweeps ADD COLUMN ai_call_limit INTEGER NOT NULL DEFAULT 0;

-- The sweep an extraction run belongs to (NULL for a detection/manual run, and
-- for legacy rows that predate this column). Charged in `stage_extract` when a
-- sweep run enters tier-4.
ALTER TABLE autopilot_run ADD COLUMN sweep_id TEXT;
