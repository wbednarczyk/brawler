-- v0.53 review follow-up (owner, 2026-07-14): P/E defined over the inputs we
-- actually have. `close / eps_diluted_ttm` left P/E empty for most companies
-- (eps_diluted is rarely extracted), while net_profit is the single
-- best-covered canonical fact. market_cap / net_profit_ttm is the same ratio
-- (price×shares / profit) without needing a per-share fact. Forward repair of
-- the 0072 seed (shipped migrations are immutable).

UPDATE kpi_definitions
SET formula = 'market_cap / net_profit_ttm'
WHERE id = 'kpidef_pe_ratio';
