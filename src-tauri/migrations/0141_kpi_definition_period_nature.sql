-- Epic #398 / ADR 0100 decision 6: `is_flow_key` (fundamentals/metrics.rs)
-- conflated two different questions -- is a figure an instant or a duration
-- (its period nature), and may it be summed into a trailing twelve months
-- (TTM additivity) -- behind one predicate. A ratio is duration-reported yet
-- TTM-ineligible, so one predicate cannot serve both.
--
-- `kpi_definitions` gains `period_nature` (`instant | duration`) as the
-- storage truth for the first axis, retiring the hand-maintained
-- `STOCK_METRIC_KEYS` const from runtime use (its own doc comment
-- anticipated this: "at which point this list becomes a seeded migration and
-- this const goes away"). The const itself stays, repurposed as this
-- migration's authoring source and the guard gate's reference list
-- (`every_stock_metric_key_backfills_to_instant_period_nature`,
-- storage/tests/quality_frameworks.rs).
--
-- Backfill mirrors `is_flow_key`'s ACTUAL runtime semantics exactly: that
-- function classified by `metric_key` membership in `STOCK_METRIC_KEYS`
-- alone, with no scope/company guard -- unlike `origin`/`statement_group`
-- (migrations 0129/0130), which deliberately withhold the canonical
-- classification from a company/user row sharing a metric_key (ADR 0077
-- no-repaint). So this backfill matches every row (any scope) by
-- `metric_key` too, preserving today's behaviour bit-for-bit rather than
-- introducing a guard the runtime predicate never had.
--
-- Append-only, idempotent, self-healing: ADD COLUMN with a DEFAULT converges
-- on every database regardless of prior state; the backfill UPDATE only ever
-- touches rows still at the default (`period_nature = 'duration'`), so a
-- re-run (or a future curated override) is never clobbered.

ALTER TABLE kpi_definitions ADD COLUMN period_nature TEXT NOT NULL DEFAULT 'duration';

UPDATE kpi_definitions
SET period_nature = 'instant'
WHERE period_nature = 'duration'
  AND metric_key IN (
    -- Balance sheet -- canonical reported (migration 0034).
    'cash', 'current_assets', 'current_liabilities', 'inventories', 'inventory',
    'long_term_debt', 'net_debt', 'retained_earnings', 'total_assets',
    'total_debt', 'total_equity', 'total_liabilities',
    -- Balance sheet -- WDF issuer rows (migration 0112).
    'wdf_equity_parent', 'wdf_noncurrent_assets', 'wdf_noncurrent_liabilities',
    'wdf_share_capital',
    -- Per-share balances (migration 0131, #309).
    'wdf_book_value_per_share', 'wdf_rozwodniona_wartosc_ksiegowa_na_jedna_akcje',
    -- Derived aggregates of the above (migrations 0034/0050/0072).
    'capital_employed', 'invested_capital', 'market_cap', 'working_capital',
    -- Counts and durations measured on a date, not over a span.
    'properties_count', 'shares_outstanding', 'walt',
    -- Quote scalars (ADR 0067 decision 4).
    'close', 'week52_high', 'week52_low',
    -- Sector packs -- banking / specialty-finance balances (migration 0034).
    'erc', 'total_deposits', 'total_loans'
  );
