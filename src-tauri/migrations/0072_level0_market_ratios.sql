-- v0.53 Level-0 market ratios + market cap (ADR 0067 Decision 4, T4).
-- Append-only, idempotent (INSERT OR IGNORE on stable ids per the
-- kpi_definitions convention). Evaluated by the derived-metrics engine
-- (src-tauri/src/fundamentals/metrics.rs): the four quote-derived rows below
-- ("reported"-shaped) are resolved from a `QuoteFacts` snapshot the caller
-- builds from bounded, indexed `daily_quotes` reads (`MarketDataStore`,
-- migration 0071) -- never a `financial_facts` row and never a corpus scan.
-- The remaining rows are ordinary derived formulas over those four plus
-- existing canonical facts (shares_outstanding, eps_diluted, total_equity,
-- net_debt, ebitda, dividend_per_share, operating_cash_flow, capex).

-- Quote-derived scalars (one as-of-date snapshot; not a fundamentals fact).
INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_close', 'canonical', 'close', 'Close (latest)', 'monetary', 'per_share', 'reported', NULL),
('kpidef_week52_high', 'canonical', 'week52_high', '52-week high', 'monetary', 'per_share', 'reported', NULL),
('kpidef_week52_low', 'canonical', 'week52_low', '52-week low', 'monetary', 'per_share', 'reported', NULL),
('kpidef_valuation_percentile_52w', 'canonical', 'valuation_percentile_52w', 'Own-history price percentile (52w)', 'ratio', NULL, 'reported', NULL);

-- Level-0 market ratios (all derived; ADR 0067 Decision 4).
INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_market_cap', 'canonical', 'market_cap', 'Market capitalization', 'monetary', NULL, 'derived', 'close * shares_outstanding'),
('kpidef_pe_ratio', 'canonical', 'pe_ratio', 'P/E ratio', 'ratio', NULL, 'derived', 'close / eps_diluted_ttm'),
('kpidef_pbv_ratio', 'canonical', 'pbv_ratio', 'P/BV ratio', 'ratio', NULL, 'derived', 'market_cap / total_equity'),
('kpidef_ev_ebitda', 'canonical', 'ev_ebitda', 'EV/EBITDA', 'ratio', NULL, 'derived', '(market_cap + net_debt) / ebitda_ttm'),
('kpidef_dividend_yield', 'canonical', 'dividend_yield', 'Dividend yield', 'percentage', NULL, 'derived', 'dividend_per_share / close'),
('kpidef_fcf_yield', 'canonical', 'fcf_yield', 'FCF yield', 'percentage', NULL, 'derived', 'free_cash_flow_ttm / market_cap'),
('kpidef_distance_from_52w_high', 'canonical', 'distance_from_52w_high', 'Distance from 52-week high', 'percentage', NULL, 'derived', '(close - week52_high) / week52_high'),
('kpidef_distance_from_52w_low', 'canonical', 'distance_from_52w_low', 'Distance from 52-week low', 'percentage', NULL, 'derived', '(close - week52_low) / week52_low');
