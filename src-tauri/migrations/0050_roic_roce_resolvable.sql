-- ROIC and ROCE were uncomputable: the v0.34 catalog seeded
--   roic = nopat / invested_capital
--   roce = operating_profit / capital_employed
-- but `nopat`, `invested_capital`, and `capital_employed` were neither reported
-- facts nor derived definitions, so the metrics service resolved them to None and
-- returned Unavailable for both headline quality metrics even with full facts
-- (issue 674cb5a). ADR 0046 Decision 1 claimed these were "built-in resolvers
-- documented alongside the engine", which was never implemented — a doc/code drift.
--
-- Resolve them the same data-driven way every other derived metric works: seed the
-- intermediates as derived kpi_definitions with formulas over reported inputs the
-- catalog already carries (total_equity, net_debt, cash). Because no income-tax
-- fact is extracted, ROIC is redefined pre-tax (EBIT over invested capital) rather
-- than guessing an effective tax rate for NOPAT — see ADR 0046 Decision 1.
--
-- Conventions (documented in ADR 0046):
--   invested_capital = total_equity + net_debt          (equity + net interest-bearing debt; excludes cash)
--   capital_employed = total_equity + net_debt + cash   (equity + gross debt; differs from invested capital by cash)
--   roic (pre-tax)   = operating_profit / invested_capital
--   roce             = operating_profit / capital_employed
--
-- Idempotent and self-healing: INSERT OR IGNORE keys off the unique
-- (metric_key, scope, company_id, sector) index; the guarded UPDATE only rewrites
-- the canonical ROIC formula while it still holds the original NOPAT definition, so
-- a user-customised formula is never clobbered and re-running converges.

INSERT OR IGNORE INTO kpi_definitions
    (id, scope, metric_key, label, value_kind, unit, computation, formula)
VALUES
    ('kpidef_invested_capital', 'canonical', 'invested_capital',
     'Invested capital', 'monetary', NULL, 'derived', 'total_equity + net_debt'),
    ('kpidef_capital_employed', 'canonical', 'capital_employed',
     'Capital employed', 'monetary', NULL, 'derived', 'total_equity + net_debt + cash');

UPDATE kpi_definitions
   SET formula = 'operating_profit / invested_capital',
       updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
 WHERE metric_key = 'roic'
   AND scope = 'canonical'
   AND formula = 'nopat / invested_capital';
