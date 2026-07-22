-- v0.59: seed the IFRS core KPI set into `kpi_relevance` (owner decision
-- 2026-07-21, card fb93394 / ADR 0061 decision 2(d)).
--
-- The completeness check ("did this document fill the KPIs this company is
-- expected to report?") reads `expected_primary_metric_keys`, which is built
-- from `kpi_relevance` rows that are `active` and ranked `primary`. That table
-- had ZERO rows in production, so the check never fired and recall had no
-- denominator. This seeds a common core set — revenue, operating profit, net
-- profit, total assets, total equity — defensible for any IFRS reporter, as a
-- starting denominator while the durable per-sector/per-company selection is
-- studied separately (card 3569d99).
--
-- Forward, idempotent and self-healing:
--   * `source = 'core'` marks an app-seeded row, distinguishable from a
--     `user`/`agent`/`sector` curated one.
--   * `NOT EXISTS` (plus `INSERT OR IGNORE` against the
--     `UNIQUE(company_id, definition_id)` constraint) means a curated row for
--     the same metric is never overwritten, re-ranked, or duplicated — the
--     seed only fills what is absent.
--   * Deterministic ids, so re-applying converges instead of accumulating.
--   * A missing canonical definition simply seeds nothing for that metric
--     rather than failing the migration.
--
-- Scope note: this seeds the companies that exist when it applies. Companies
-- added later have no seeded denominator until the durable selection lands.

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
