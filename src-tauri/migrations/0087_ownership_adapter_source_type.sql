-- Rename the BiznesRadar Akcjonariat adapter's catalog `source_type` from
-- 'ownership_witness' to 'ownership' (ADR 0072 §2c as amended 2026-07-16, plan
-- v0.56 pivot): the aggregator was promoted from a compare-only witness to the
-- automatic ownership BREADTH source, so its catalog type is no longer a witness.
-- Migration 0085 seeded 'ownership_witness'; the REGISTRY descriptor now declares
-- 'ownership' and the drift guard (registry_matches_seeded_catalog) binds the two.
--
-- Forward-only, idempotent, self-healing (data-model migration rules): the guarded
-- UPDATE is a no-op once the row already reads 'ownership', and never touches a row
-- whose type someone changed to something else.

UPDATE source_adapters
SET source_type = 'ownership',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = 'biznesradar-akcjonariat' AND source_type = 'ownership_witness';
