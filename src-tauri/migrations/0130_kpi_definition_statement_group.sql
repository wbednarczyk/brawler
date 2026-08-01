-- Card #307 (grouped fundamentals matrix): `kpi_definitions` gains a durable
-- `statement_group` column so "which statement does this KPI belong to" is a
-- single source of truth the matrix, Compare, and coverage reads can all use —
-- not a frontend-only guess re-derived from the metric key's spelling.
--
-- Vocabulary: income | balance | cash_flow | per_share | other. `other` is the
-- column DEFAULT and covers: every cross-statement/derived RATIO (margins,
-- ROE/ROA/ROIC/ROCE, liquidity/leverage ratios, market multiples, sector
-- ratios like NIM/CET1/claims_ratio), market/valuation price data (close,
-- 52-week levels, market cap, yields), non-GAAP sector aggregates (FFO/AFFO,
-- cash EBITDA, same-store NOI) and operating KPIs with no statement line
-- (occupancy, properties count, WALT) — none of these is itself a reported
-- statement line, so forcing one into income/balance/cash_flow/per_share would
-- misrepresent it. Every runtime-created row (company-scoped custom KPIs,
-- user-scope quality-framework metrics, MCP agent-minted definitions) is left
-- at the DEFAULT regardless of its metric_key — the matrix's own display rule
-- routes scope='company' rows into "KPI operacyjne spółki" ahead of
-- statement_group, so a company-scoped 'other' row still lands somewhere
-- sensible (frontend factMatrix.ts), and an agent/user row sharing a canonical
-- metric_key (e.g. a company's own "revenue" concept, ADR 0077 d.8) must NOT
-- silently inherit the canonical classification — same id-shape guard 0129
-- established for `origin`.
--
-- Append-only, idempotent, self-healing: ADD COLUMN with a DEFAULT converges
-- on every database regardless of prior state; the backfill UPDATEs are
-- id-glob-guarded (`id NOT GLOB '*__*'` — the same "bare id = migration-seeded"
-- shape 0129 backfills `origin` by) so a re-run touches nothing new and a
-- runtime row can never be mistaken for a seeded one.
--
-- Classification judged by hand against the canonical/sector packs (migrations
-- 0034, 0048, 0050, 0072, 0089, 0110, 0111, 0112) — see docs/data-model.md
-- § kpi_definitions for the reviewed list.

ALTER TABLE kpi_definitions ADD COLUMN statement_group TEXT NOT NULL DEFAULT 'other';

-- Income statement lines.
UPDATE kpi_definitions
SET statement_group = 'income'
WHERE id NOT GLOB '*__*'
  AND metric_key IN (
    'revenue', 'gross_profit', 'operating_profit', 'net_profit',
    'net_profit_discontinued', 'ebitda',
    'net_interest_income', 'net_fee_commission_income',
    'gross_insurance_revenue', 'gross_written_premium', 'net_earned_premium',
    'technical_result', 'investment_result',
    'operating_income', 'operating_expenses', 'interest_expense',
    'wdf_net_profit_parent', 'wdf_pretax_profit',
    'wdf_calkowite_dochody_netto',
    'wdf_calkowite_dochody_netto_przypadajace_na_akcjonariuszy_jednostki_dominujacej',
    'wdf_calkowity_dochod', 'wdf_calkowity_dochod_netto',
    'wdf_calkowity_dochod_przypadajacy_akcjonariuszom_jednostki_dominujacej',
    'wdf_calkowity_dochod_strata_netto',
    'wdf_calkowity_dochod_strata_netto_akcjonariuszy_jednostki_dominujacej',
    'wdf_laczne_calkowite_dochody',
    'wdf_zysk_netto_przed_odpisami_aktualizujacymi_netto',
    'wdf_zysk_strata_brutto_ze_sprzedazy',
    'wdf_zysk_strata_netto_przypadajacy_na_udzialy_niekontrolujace'
  );

-- Balance sheet lines.
UPDATE kpi_definitions
SET statement_group = 'balance'
WHERE id NOT GLOB '*__*'
  AND metric_key IN (
    'total_assets', 'total_equity', 'total_liabilities', 'cash',
    'current_assets', 'current_liabilities', 'retained_earnings',
    'long_term_debt', 'inventories', 'inventory', 'total_debt', 'net_debt',
    'total_loans', 'total_deposits',
    'wdf_equity_parent', 'wdf_share_capital',
    'wdf_noncurrent_assets', 'wdf_noncurrent_liabilities'
  );

-- Cash flow statement lines. `recoveries` (collection cash inflows, "wpłaty od
-- osób zadłużonych") sits here beside `portfolio_purchases` (collection cash
-- outflows) — both are cash motions of the debt-collection business, not P&L
-- lines; the P&L-recognized revenue is the issuer's own reported `revenue`.
UPDATE kpi_definitions
SET statement_group = 'cash_flow'
WHERE id NOT GLOB '*__*'
  AND metric_key IN (
    'operating_cash_flow', 'investing_cash_flow', 'financing_cash_flow',
    'capex', 'free_cash_flow', 'wdf_net_cash_change',
    'portfolio_purchases', 'recoveries'
  );

-- Per-share statement figures (not market/price data — close, 52-week levels
-- etc. stay 'other', they are prices, not reported per-share accounting lines).
UPDATE kpi_definitions
SET statement_group = 'per_share'
WHERE id NOT GLOB '*__*'
  AND metric_key IN ('eps_basic', 'eps_diluted', 'dividend_per_share');
