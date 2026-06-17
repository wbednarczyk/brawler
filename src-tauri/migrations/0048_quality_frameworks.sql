-- Quality frameworks: quantitative checks (ADR 0046, v0.44.0).
-- A user-owned framework is a named set of criteria expressed in a free-text DSL
-- over KPI metric keys. A deterministic engine evaluates them against confirmed
-- financial_facts (latest period/TTM) into an immutable, versioned scorecard.
-- Append-only, idempotent, self-healing: CREATE TABLE IF NOT EXISTS + INSERT OR IGNORE.

-- The checklist. origin is a provenance label (app_template | user), NOT an edit lock:
-- every framework is editable/deletable in place regardless of origin.
CREATE TABLE IF NOT EXISTS quality_frameworks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    origin TEXT NOT NULL DEFAULT 'user',
    template_key TEXT,
    cloned_from TEXT REFERENCES quality_frameworks(id) ON DELETE SET NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- A template_key identifies an app template uniquely (used by seed + reset).
CREATE UNIQUE INDEX IF NOT EXISTS idx_quality_frameworks_template_key
ON quality_frameworks(template_key) WHERE template_key IS NOT NULL;

-- A single check. expression is DSL text; expression_ast caches the parsed AST (JSON).
CREATE TABLE IF NOT EXISTS framework_criteria (
    id TEXT PRIMARY KEY,
    framework_id TEXT NOT NULL REFERENCES quality_frameworks(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL DEFAULT 0,
    label TEXT NOT NULL,
    expression TEXT NOT NULL,
    expression_ast TEXT,
    weight TEXT,
    partial_band TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_framework_criteria_framework
ON framework_criteria(framework_id, ordinal);

-- One immutable evaluation run. Pins the framework version and the assessed period.
CREATE TABLE IF NOT EXISTS framework_evaluations (
    id TEXT PRIMARY KEY,
    framework_id TEXT NOT NULL REFERENCES quality_frameworks(id) ON DELETE CASCADE,
    framework_version INTEGER NOT NULL,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    period_id TEXT REFERENCES financial_periods(id) ON DELETE SET NULL,
    pass_count INTEGER NOT NULL DEFAULT 0,
    partial_count INTEGER NOT NULL DEFAULT 0,
    fail_count INTEGER NOT NULL DEFAULT 0,
    unavailable_count INTEGER NOT NULL DEFAULT 0,
    engine_version TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_framework_evaluations_lookup
ON framework_evaluations(framework_id, company_id, created_at DESC);

-- One immutable per-criterion outcome within a run.
CREATE TABLE IF NOT EXISTS criterion_results (
    id TEXT PRIMARY KEY,
    evaluation_id TEXT NOT NULL REFERENCES framework_evaluations(id) ON DELETE CASCADE,
    criterion_id TEXT REFERENCES framework_criteria(id) ON DELETE SET NULL,
    ordinal INTEGER NOT NULL DEFAULT 0,
    label TEXT NOT NULL,
    expression TEXT NOT NULL,
    verdict TEXT NOT NULL,
    measured_value TEXT,
    measured_unit TEXT,
    threshold TEXT,
    inputs_json TEXT,
    note TEXT
);

CREATE INDEX IF NOT EXISTS idx_criterion_results_evaluation
ON criterion_results(evaluation_id, ordinal);

-- Catalog top-up (ADR 0046 Decision 3): seed popular computed metrics so they ship
-- out of the box, independent of any framework. scope = canonical (app-owned, global).
-- INSERT OR IGNORE keeps this idempotent and non-clobbering. fcf_yield/dividend_yield
-- are intentionally omitted: they need market price data, which is out of fundamentals scope.

-- Reported balance-sheet / income inputs the new ratios build on.
INSERT OR IGNORE INTO kpi_definitions (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_current_assets', 'canonical', 'current_assets', 'Current assets', 'monetary', NULL, 'reported', NULL),
('kpidef_current_liabilities', 'canonical', 'current_liabilities', 'Current liabilities', 'monetary', NULL, 'reported', NULL),
('kpidef_total_liabilities', 'canonical', 'total_liabilities', 'Total liabilities', 'monetary', NULL, 'reported', NULL),
('kpidef_total_debt', 'canonical', 'total_debt', 'Total debt', 'monetary', NULL, 'reported', NULL),
('kpidef_interest_expense', 'canonical', 'interest_expense', 'Interest expense', 'monetary', NULL, 'reported', NULL),
('kpidef_inventory', 'canonical', 'inventory', 'Inventory', 'monetary', NULL, 'reported', NULL);

-- Derived liquidity / leverage / coverage / efficiency / payout ratios.
INSERT OR IGNORE INTO kpi_definitions (id, scope, metric_key, label, value_kind, unit, computation, formula) VALUES
('kpidef_current_ratio', 'canonical', 'current_ratio', 'Current ratio', 'ratio', NULL, 'derived', 'current_assets / current_liabilities'),
('kpidef_quick_ratio', 'canonical', 'quick_ratio', 'Quick ratio', 'ratio', NULL, 'derived', '(current_assets - inventory) / current_liabilities'),
('kpidef_debt_to_equity', 'canonical', 'debt_to_equity', 'Debt / equity', 'ratio', NULL, 'derived', 'total_debt / total_equity'),
('kpidef_net_debt_to_equity', 'canonical', 'net_debt_to_equity', 'Net debt / equity', 'ratio', NULL, 'derived', 'net_debt / total_equity'),
('kpidef_debt_ratio', 'canonical', 'debt_ratio', 'Debt ratio', 'percentage', NULL, 'derived', 'total_liabilities / total_assets'),
('kpidef_equity_ratio', 'canonical', 'equity_ratio', 'Equity ratio', 'percentage', NULL, 'derived', 'total_equity / total_assets'),
('kpidef_interest_coverage', 'canonical', 'interest_coverage', 'Interest coverage', 'ratio', NULL, 'derived', 'operating_profit / interest_expense'),
('kpidef_fcf_margin', 'canonical', 'fcf_margin', 'FCF margin', 'percentage', NULL, 'derived', 'free_cash_flow / revenue'),
('kpidef_asset_turnover', 'canonical', 'asset_turnover', 'Asset turnover', 'ratio', NULL, 'derived', 'revenue / total_assets_avg'),
('kpidef_payout_ratio', 'canonical', 'payout_ratio', 'Dividend payout ratio', 'percentage', NULL, 'derived', 'dividend_per_share / eps_basic');
