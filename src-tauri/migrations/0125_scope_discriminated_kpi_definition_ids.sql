-- Re-key the `kpi_definitions` rows the old scope-blind id scheme produced
-- (issue #149, epic #229 T7).
--
-- `storage/financials.rs::kpi_definition_id` used to build the PRIMARY KEY from
-- `metric_key` ALONE (`kpidef_<key>`), even though the table is unique on
-- `(metric_key, scope, company_id, sector)` — a metric key legitimately exists
-- once per scope bucket. Two failure modes followed:
--
--   1. a company-scoped definition for a key that already has a canonical row
--      could not be created at all (PK conflict on `kpidef_<key>`);
--   2. a company/sector-scoped definition for a key with NO canonical twin
--      squatted on the id a later canonical seed needs — and every canonical
--      seed is `INSERT OR IGNORE`, so that seed silently did nothing and the
--      canonical concept never entered the catalog.
--
-- The forward fix gives non-canonical scopes a discriminated id
-- (`kpidef_<key>__c_<company_id>` / `kpidef_<key>__s_<sector>`); canonical rows
-- keep the bare id because facts, `kpi_relevance`, quality-framework criteria
-- and the metric-history reads all key on it. This migration moves the rows
-- already stored under the old scheme.
--
-- Measured on the maintainer's database (2026-07-30 copy,
-- `private/realdata/trust-audit-worktest.sqlite3`): 110 definitions — 80
-- canonical, 30 sector, 0 company-scoped; ZERO non-canonical rows carry a
-- `kpidef_<metric_key>` id (the curated sector packs from 0034/0048/0089/…
-- use their own hand-written prefixed ids, e.g. `kpidef_bank_nim`, and are
-- deliberately NOT re-keyed); ZERO `financial_facts` and ZERO `kpi_relevance`
-- rows reference a non-canonical definition. So this migration is a no-op on
-- the owner's data today and exists so the latent rows a pre-fix build could
-- have written cannot survive into a fixed one.
--
-- FK discipline. `kpi_relevance.definition_id` and `financial_facts.definition_id`
-- both `REFERENCES kpi_definitions(id) ON DELETE CASCADE` with no `ON UPDATE`
-- action, and the migration runner keeps `PRAGMA foreign_keys = ON`. That rules
-- out both naive orders: copy-then-delete trips
-- `idx_kpi_definitions_key` (old and new row share the
-- `(metric_key, scope, company_id, sector)` tuple), and re-pointing a child
-- before its new parent exists — or moving the parent while children still
-- reference it — is an immediate FK violation.
--
-- `PRAGMA defer_foreign_keys` is the sanctioned SQLite answer and unlike
-- `PRAGMA foreign_keys` it DOES take effect inside a transaction: enforcement
-- is postponed to the runner's COMMIT, where the whole re-key is already
-- consistent. It resets automatically at that commit, so no later migration
-- and no runtime connection inherits a relaxed setting. `ON DELETE CASCADE`
-- never fires: the parent is UPDATEd in place, never deleted. Child uniqueness
-- is safe by construction — the new id did not exist, so no
-- `UNIQUE(company_id, definition_id)` or slot-index row can already point at it.

PRAGMA defer_foreign_keys = ON;

CREATE TABLE kpi_definition_rekey_0125 (
    old_id TEXT PRIMARY KEY,
    new_id TEXT NOT NULL
);

INSERT INTO kpi_definition_rekey_0125 (old_id, new_id)
SELECT
    d.id,
    d.id || '__'
        || CASE d.scope WHEN 'company' THEN 'c' WHEN 'sector' THEN 's' ELSE d.scope END
        || '_'
        || CASE d.scope
               WHEN 'company' THEN IFNULL(d.company_id, '')
               WHEN 'sector' THEN IFNULL(d.sector, '')
               ELSE IFNULL(d.company_id, IFNULL(d.sector, ''))
           END
FROM kpi_definitions d
WHERE d.scope <> 'canonical'
  -- Exactly the rows the old scope-blind function produced. Curated packs with
  -- their own id shape do not match and stay where every reference expects them.
  AND d.id = 'kpidef_' || LOWER(d.metric_key);

UPDATE kpi_definitions
SET id = (SELECT r.new_id FROM kpi_definition_rekey_0125 r WHERE r.old_id = id),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id IN (SELECT old_id FROM kpi_definition_rekey_0125);

UPDATE kpi_relevance
SET definition_id = (
        SELECT r.new_id FROM kpi_definition_rekey_0125 r WHERE r.old_id = definition_id
    )
WHERE definition_id IN (SELECT old_id FROM kpi_definition_rekey_0125);

UPDATE financial_facts
SET definition_id = (
        SELECT r.new_id FROM kpi_definition_rekey_0125 r WHERE r.old_id = definition_id
    )
WHERE definition_id IN (SELECT old_id FROM kpi_definition_rekey_0125);

DROP TABLE kpi_definition_rekey_0125;
