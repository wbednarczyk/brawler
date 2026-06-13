-- Company fundamentals foundation (ADR 0027).
-- Three layers: catalog (kpi_definitions) / relevance (kpi_relevance) / facts (financial_facts),
-- plus financial_periods and company statement-type/reporting-standard/fiscal-offset columns.

-- Company-level discriminators that select canonical packs and drive period math.
ALTER TABLE companies ADD COLUMN statement_type TEXT NOT NULL DEFAULT 'industrial';
ALTER TABLE companies ADD COLUMN reporting_standard TEXT NOT NULL DEFAULT 'ifrs';
ALTER TABLE companies ADD COLUMN fiscal_year_end_month INTEGER NOT NULL DEFAULT 12;

-- Reporting periods. period_type is a fiscal label (FY, H1, H2, Q1..Q4, 9M, M01..M12),
-- independent of the calendar; period_end_date anchors it.
CREATE TABLE financial_periods (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    fiscal_year INTEGER NOT NULL,
    period_type TEXT NOT NULL,
    period_end_date TEXT,
    report_evidence_ref TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(company_id, fiscal_year, period_type)
);

CREATE INDEX idx_financial_periods_company ON financial_periods(company_id, fiscal_year);

-- Catalog: what a metric IS. scope = canonical | sector | company.
-- computation = reported | derived (formula references other metric keys).
CREATE TABLE kpi_definitions (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    company_id TEXT REFERENCES companies(id) ON DELETE CASCADE,
    sector TEXT,
    metric_key TEXT NOT NULL,
    label TEXT NOT NULL,
    value_kind TEXT NOT NULL,
    unit TEXT,
    computation TEXT NOT NULL DEFAULT 'reported',
    formula TEXT,
    display_format TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- A metric key is unique within its scope bucket (canonical global, per sector, or per company).
CREATE UNIQUE INDEX idx_kpi_definitions_key
ON kpi_definitions(metric_key, scope, IFNULL(company_id, ''), IFNULL(sector, ''));

CREATE INDEX idx_kpi_definitions_company ON kpi_definitions(company_id);

-- Relevance: which KPIs matter for a company, over time. Lifecycle, not a static set.
CREATE TABLE kpi_relevance (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    definition_id TEXT NOT NULL REFERENCES kpi_definitions(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'active',
    source TEXT NOT NULL,
    rank TEXT,
    first_seen_period TEXT,
    last_seen_period TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(company_id, definition_id)
);

CREATE INDEX idx_kpi_relevance_company ON kpi_relevance(company_id, status);

-- Facts: values, referencing a definition (never the relevance profile, so values are never orphaned).
-- value_numeric is decimal-exact text (base units, signed); parsed with rust_decimal.
CREATE TABLE financial_facts (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    period_id TEXT NOT NULL REFERENCES financial_periods(id) ON DELETE CASCADE,
    definition_id TEXT NOT NULL REFERENCES kpi_definitions(id) ON DELETE CASCADE,
    value_numeric TEXT NOT NULL,
    currency TEXT,
    statement_basis TEXT NOT NULL DEFAULT 'consolidated',
    attribution TEXT NOT NULL DEFAULT 'total',
    variant TEXT NOT NULL DEFAULT 'reported',
    measure_window TEXT NOT NULL DEFAULT 'flow',
    data_quality TEXT NOT NULL DEFAULT 'final',
    as_reported_value TEXT,
    as_reported_scale TEXT,
    reporting_standard TEXT,
    extraction_method TEXT NOT NULL DEFAULT 'manual',
    confidence TEXT,
    confirmation_state TEXT NOT NULL DEFAULT 'confirmed',
    supersedes_id TEXT REFERENCES financial_facts(id) ON DELETE SET NULL,
    source_document_ref TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- One value per fully-qualified slot; estimate and final coexist via data_quality + supersedes_id.
CREATE UNIQUE INDEX idx_financial_facts_slot
ON financial_facts(period_id, definition_id, statement_basis, attribution, variant, measure_window, data_quality);

CREATE INDEX idx_financial_facts_company ON financial_facts(company_id);
CREATE INDEX idx_financial_facts_definition ON financial_facts(definition_id);

-- Seed canonical packs (scope = canonical, global) and sector packs (scope = sector).
-- ids are deterministic so reseeding/migration is idempotent across environments.

-- Universal pack.
INSERT INTO kpi_definitions (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_net_profit', 'canonical', 'net_profit', 'Net profit', 'monetary', NULL, 'reported', NULL),
('kpidef_total_assets', 'canonical', 'total_assets', 'Total assets', 'monetary', NULL, 'reported', NULL),
('kpidef_total_equity', 'canonical', 'total_equity', 'Total equity', 'monetary', NULL, 'reported', NULL),
('kpidef_eps_basic', 'canonical', 'eps_basic', 'EPS (basic)', 'monetary', 'per_share', 'reported', NULL),
('kpidef_eps_diluted', 'canonical', 'eps_diluted', 'EPS (diluted)', 'monetary', 'per_share', 'reported', NULL),
('kpidef_shares_outstanding', 'canonical', 'shares_outstanding', 'Shares outstanding', 'count', 'shares', 'reported', NULL),
('kpidef_operating_cash_flow', 'canonical', 'operating_cash_flow', 'Operating cash flow', 'monetary', NULL, 'reported', NULL),
('kpidef_cash', 'canonical', 'cash', 'Cash and equivalents', 'monetary', NULL, 'reported', NULL),
('kpidef_dividend_per_share', 'canonical', 'dividend_per_share', 'Dividend per share', 'monetary', 'per_share', 'reported', NULL);

-- Industrial pack.
INSERT INTO kpi_definitions (id, scope, sector, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_revenue', 'canonical', NULL, 'revenue', 'Revenue', 'monetary', NULL, 'reported', NULL),
('kpidef_gross_profit', 'canonical', NULL, 'gross_profit', 'Gross profit', 'monetary', NULL, 'reported', NULL),
('kpidef_operating_profit', 'canonical', NULL, 'operating_profit', 'Operating profit (EBIT)', 'monetary', NULL, 'reported', NULL),
('kpidef_ebitda', 'canonical', NULL, 'ebitda', 'EBITDA', 'monetary', NULL, 'reported', NULL),
('kpidef_net_debt', 'canonical', NULL, 'net_debt', 'Net debt', 'monetary', NULL, 'reported', NULL),
('kpidef_gross_margin', 'canonical', NULL, 'gross_margin', 'Gross margin', 'percentage', NULL, 'derived', 'gross_profit / revenue'),
('kpidef_operating_margin', 'canonical', NULL, 'operating_margin', 'Operating margin', 'percentage', NULL, 'derived', 'operating_profit / revenue'),
('kpidef_net_margin', 'canonical', NULL, 'net_margin', 'Net margin', 'percentage', NULL, 'derived', 'net_profit / revenue');

-- Cash-flow pack.
INSERT INTO kpi_definitions (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_investing_cash_flow', 'canonical', 'investing_cash_flow', 'Investing cash flow', 'monetary', NULL, 'reported', NULL),
('kpidef_financing_cash_flow', 'canonical', 'financing_cash_flow', 'Financing cash flow', 'monetary', NULL, 'reported', NULL),
('kpidef_capex', 'canonical', 'capex', 'Capital expenditure', 'monetary', NULL, 'reported', NULL),
('kpidef_free_cash_flow', 'canonical', 'free_cash_flow', 'Free cash flow', 'monetary', NULL, 'derived', 'operating_cash_flow - capex');

-- Capital-efficiency pack (all derived).
INSERT INTO kpi_definitions (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_roe', 'canonical', 'roe', 'Return on equity', 'percentage', NULL, 'derived', 'net_profit_ttm / total_equity_avg'),
('kpidef_roa', 'canonical', 'roa', 'Return on assets', 'percentage', NULL, 'derived', 'net_profit_ttm / total_assets_avg'),
('kpidef_roic', 'canonical', 'roic', 'Return on invested capital', 'percentage', NULL, 'derived', 'nopat / invested_capital'),
('kpidef_roce', 'canonical', 'roce', 'Return on capital employed', 'percentage', NULL, 'derived', 'operating_profit / capital_employed'),
('kpidef_net_debt_to_ebitda', 'canonical', 'net_debt_to_ebitda', 'Net debt / EBITDA', 'ratio', NULL, 'derived', 'net_debt / ebitda_ttm'),
('kpidef_fcf_conversion', 'canonical', 'fcf_conversion', 'FCF conversion', 'percentage', NULL, 'derived', 'free_cash_flow / net_profit');

-- Insurance sector pack.
INSERT INTO kpi_definitions (id, scope, sector, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_ins_gross_written_premium', 'sector', 'insurance', 'gross_written_premium', 'Gross written premium', 'monetary', NULL, 'reported', NULL),
('kpidef_ins_net_earned_premium', 'sector', 'insurance', 'net_earned_premium', 'Net earned premium', 'monetary', NULL, 'reported', NULL),
('kpidef_ins_gross_insurance_revenue', 'sector', 'insurance', 'gross_insurance_revenue', 'Gross insurance revenue', 'monetary', NULL, 'reported', NULL),
('kpidef_ins_claims_ratio', 'sector', 'insurance', 'claims_ratio', 'Claims ratio', 'percentage', NULL, 'reported', NULL),
('kpidef_ins_combined_ratio', 'sector', 'insurance', 'combined_ratio', 'Combined ratio', 'percentage', NULL, 'reported', NULL),
('kpidef_ins_technical_result', 'sector', 'insurance', 'technical_result', 'Technical result', 'monetary', NULL, 'reported', NULL),
('kpidef_ins_investment_result', 'sector', 'insurance', 'investment_result', 'Investment result', 'monetary', NULL, 'reported', NULL);

-- Banking sector pack.
INSERT INTO kpi_definitions (id, scope, sector, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_bank_net_interest_income', 'sector', 'banking', 'net_interest_income', 'Net interest income', 'monetary', NULL, 'reported', NULL),
('kpidef_bank_net_fee_commission_income', 'sector', 'banking', 'net_fee_commission_income', 'Net fee & commission income', 'monetary', NULL, 'reported', NULL),
('kpidef_bank_operating_income', 'sector', 'banking', 'operating_income', 'Operating income', 'monetary', NULL, 'reported', NULL),
('kpidef_bank_operating_expenses', 'sector', 'banking', 'operating_expenses', 'Operating expenses', 'monetary', NULL, 'reported', NULL),
('kpidef_bank_total_loans', 'sector', 'banking', 'total_loans', 'Loans', 'monetary', NULL, 'reported', NULL),
('kpidef_bank_total_deposits', 'sector', 'banking', 'total_deposits', 'Deposits', 'monetary', NULL, 'reported', NULL),
('kpidef_bank_nim', 'sector', 'banking', 'nim', 'Net interest margin', 'percentage', NULL, 'reported', NULL),
('kpidef_bank_cost_income_ratio', 'sector', 'banking', 'cost_income_ratio', 'Cost / income ratio', 'percentage', NULL, 'reported', NULL),
('kpidef_bank_npl_ratio', 'sector', 'banking', 'npl_ratio', 'NPL ratio', 'percentage', NULL, 'reported', NULL),
('kpidef_bank_cost_of_risk', 'sector', 'banking', 'cost_of_risk', 'Cost of risk', 'ratio', NULL, 'reported', NULL),
('kpidef_bank_cet1', 'sector', 'banking', 'cet1', 'CET1 ratio', 'percentage', NULL, 'reported', NULL),
('kpidef_bank_tcr', 'sector', 'banking', 'tcr', 'Total capital ratio', 'percentage', NULL, 'reported', NULL);

-- Specialty-finance sector pack.
INSERT INTO kpi_definitions (id, scope, sector, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_spf_recoveries', 'sector', 'specialty_finance', 'recoveries', 'Recoveries', 'monetary', NULL, 'reported', NULL),
('kpidef_spf_erc', 'sector', 'specialty_finance', 'erc', 'Estimated remaining collections', 'monetary', NULL, 'reported', NULL),
('kpidef_spf_cash_ebitda', 'sector', 'specialty_finance', 'cash_ebitda', 'Cash EBITDA', 'monetary', NULL, 'reported', NULL),
('kpidef_spf_portfolio_purchases', 'sector', 'specialty_finance', 'portfolio_purchases', 'Portfolio purchases', 'monetary', NULL, 'reported', NULL);

-- REIT sector pack.
INSERT INTO kpi_definitions (id, scope, sector, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_reit_ffo', 'sector', 'reit', 'ffo', 'Funds from operations', 'monetary', NULL, 'reported', NULL),
('kpidef_reit_affo', 'sector', 'reit', 'affo', 'Adjusted funds from operations', 'monetary', NULL, 'reported', NULL),
('kpidef_reit_affo_payout_ratio', 'sector', 'reit', 'affo_payout_ratio', 'AFFO payout ratio', 'percentage', NULL, 'reported', NULL),
('kpidef_reit_occupancy', 'sector', 'reit', 'occupancy', 'Occupancy', 'percentage', NULL, 'reported', NULL),
('kpidef_reit_properties_count', 'sector', 'reit', 'properties_count', 'Properties', 'count', 'properties', 'reported', NULL),
('kpidef_reit_same_store_noi', 'sector', 'reit', 'same_store_noi', 'Same-store NOI', 'monetary', NULL, 'reported', NULL),
('kpidef_reit_walt', 'sector', 'reit', 'walt', 'Weighted average lease term', 'duration', 'years', 'reported', NULL);
