-- Parent-attributable KPI definitions (review finding 2026-07-22, ADR 0086).
--
-- The extraction dictionary and the WDF cover-note reader map
-- parent-attributable rows to `wdf_net_profit_parent` / `wdf_equity_parent`,
-- but no migration ever seeded those catalog definitions — so every such fact
-- silently dropped at the defensive `NoDefinition` skip (visible as the
-- aggregator pull's `noDefinition` counter). Seed both, following the 0110
-- `inventories` convention (INSERT OR IGNORE on a stable id, canonical scope).

INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_wdf_net_profit_parent', 'canonical', 'wdf_net_profit_parent',
 'Net profit attributable to parent', 'monetary', NULL, 'reported', NULL),
('kpidef_wdf_equity_parent', 'canonical', 'wdf_equity_parent',
 'Equity attributable to parent', 'monetary', NULL, 'reported', NULL);
