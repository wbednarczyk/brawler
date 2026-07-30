-- Positive corroboration on a fact's provenance (epic #229 T5, ADR 0086 dec. 4).
--
-- Reversed witnessing recorded only its NEGATIVE half: when BiznesRadar (the
-- PRIMARY source for core KPIs) disagreed with an issuer-held slot, a
-- `witness_disagreement` extraction outcome was written. Agreement — the far
-- more common and equally informative case — wrote NOTHING, so "an independent
-- source read the same figure" was unknowable after the run.
--
-- Three nullable columns record it per fact:
--   `witness_value`     — the aggregator's own figure at corroboration time.
--   `witness_page_url`  — the report page it was read from (the evidence link).
--   `corroborated_at`   — when the agreement was last observed (refreshed by a
--                         later pull, so the stamp always names the latest look).
-- NULL = never corroborated; readers treat absence as "no witness", never as a
-- disagreement.
--
-- No CHECK: these are observed values, not a vocabulary. `validation_status`
-- keeps its own semantics — the corroboration path upgrades `passed`/`unreviewed`
-- to `witness_confirmed` for ISSUER-held slots only; a MANUAL slot is stamped
-- but its status is never touched (ADR 0086 dec. 3 — the user's own entry is
-- never re-labelled by the automaton).
--
-- Append-only and idempotent by the runner's applied-version ledger.

ALTER TABLE financial_fact_provenance ADD COLUMN witness_value TEXT;
ALTER TABLE financial_fact_provenance ADD COLUMN witness_page_url TEXT;
ALTER TABLE financial_fact_provenance ADD COLUMN corroborated_at TEXT;
