-- v0.57 Company Health: balance-sheet inputs behind Piotroski F + Altman Z″
-- (ADR 0083 Decision 5; data-model § Company Health). Append-only, idempotent
-- (INSERT OR IGNORE on stable ids per the kpi_definitions convention). These are
-- seeded so the four new ESEF/structured-xHTML extraction concepts
-- (fundamentals/extraction/esef.rs concept_to_metric_key + the tier-2/3 Polish
-- dictionary in pdf.rs) resolve to catalog metrics, and so the two derived
-- liquidity ratios compute from them via the derived-metrics engine
-- (src-tauri/src/fundamentals/metrics.rs) — no financial_facts scan.

-- Reported balance-sheet line items (extracted; monetary base units).
INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_current_assets', 'canonical', 'current_assets',
 'Current assets', 'monetary', NULL, 'reported', NULL),
('kpidef_current_liabilities', 'canonical', 'current_liabilities',
 'Current liabilities', 'monetary', NULL, 'reported', NULL),
('kpidef_retained_earnings', 'canonical', 'retained_earnings',
 'Retained earnings', 'monetary', NULL, 'reported', NULL),
('kpidef_long_term_debt', 'canonical', 'long_term_debt',
 'Long-term debt', 'monetary', NULL, 'reported', NULL);

-- Derived liquidity metrics over the two current items (ADR 0083 Decision 5).
--   working_capital = current_assets - current_liabilities   (Altman Z″ X1 numerator)
--   current_ratio   = current_assets / current_liabilities   (Piotroski F liquidity)
INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_working_capital', 'canonical', 'working_capital',
 'Working capital', 'monetary', NULL, 'derived', 'current_assets - current_liabilities'),
('kpidef_current_ratio', 'canonical', 'current_ratio',
 'Current ratio', 'ratio', NULL, 'derived', 'current_assets / current_liabilities');
