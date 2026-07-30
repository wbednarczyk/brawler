-- Heal the companies that migration 0106 could not reach (issue #203 residual,
-- epic #229 T7).
--
-- 0106 seeded the IFRS core KPI set (`revenue`, `operating_profit`,
-- `net_profit`, `total_assets`, `total_equity`) into `kpi_relevance` for the
-- companies that existed WHEN IT APPLIED — its own scope note says so. Every
-- company created afterwards started with an empty denominator, so
-- `expected_primary_metric_keys` returned `None` and the completeness check
-- silently never fired for it. The forward fix seeds at creation
-- (`storage/financials.rs::seed_core_kpi_relevance`, called from
-- `storage/companies.rs::create_company`); this migration heals the rows
-- already stored.
--
-- Measured on the maintainer's database (2026-07-30 copy,
-- `private/realdata/trust-audit-worktest.sqlite3`): 52 companies, 250
-- `kpi_relevance` rows (all `source = 'core'`), 2 companies with ZERO rows —
-- `company_gpw_ale` and `company_gpw_zzzldt`, both created after 0106 applied.
-- This migration adds 10 rows there.
--
-- Deliberately the VERBATIM 0106 statement: same deterministic
-- `kpirel_core_<company>_<metric_key>` ids, same `INSERT OR IGNORE` against
-- `UNIQUE(company_id, definition_id)`, same per-definition `NOT EXISTS` guard.
-- A curated (`user`/`agent`/`sector`) row is therefore never overwritten,
-- re-ranked or duplicated — the seed only fills what is absent — and a company
-- already holding all five keeps exactly what it has. Re-running converges.

INSERT OR IGNORE INTO kpi_relevance
    (id, company_id, definition_id, status, source, rank)
SELECT
    'kpirel_core_' || c.id || '_' || d.metric_key,
    c.id,
    d.id,
    'active',
    'core',
    'primary'
FROM companies c
JOIN kpi_definitions d
  ON d.scope = 'canonical'
 AND d.metric_key IN (
        'revenue',
        'operating_profit',
        'net_profit',
        'total_assets',
        'total_equity'
     )
WHERE NOT EXISTS (
    SELECT 1
    FROM kpi_relevance existing
    WHERE existing.company_id = c.id
      AND existing.definition_id = d.id
);
