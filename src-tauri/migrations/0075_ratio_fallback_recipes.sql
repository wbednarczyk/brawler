-- v0.53 owner decision (2026-07-14): ratios compute from WHICHEVER inputs
-- exist — a `coalesce(...)` fallback chain per ratio; empty only when no
-- recipe resolves. P/E gets the full chain today (net_profit is the
-- best-covered fact; per-share EPS variants are the fallbacks). The other
-- level-0 ratios keep their single financially-sound recipe until a second
-- sound one exists — the mechanism is now in the formula DSL for all of them.
-- Forward repair of the 0072/0074 seed (shipped migrations are immutable).

UPDATE kpi_definitions
SET formula = 'coalesce(market_cap / net_profit_ttm, close / eps_diluted_ttm, close / eps_basic_ttm)'
WHERE id = 'kpidef_pe_ratio';
