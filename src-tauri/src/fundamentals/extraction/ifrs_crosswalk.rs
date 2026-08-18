//! Curated IFRS taxonomy → `metric_key` crosswalk — the single naming
//! authority for the ESEF/iXBRL path (ADR 0100 decision 2, epic #398).
//!
//! Layer 1 (`storage::report_tagged_facts`) captures every tagged fact
//! unconditionally, with no naming decision (decision 1). This table is
//! where naming happens: a taxonomy concept's LOCAL NAME (prefix-agnostic,
//! matching `esef::concept_to_metric_key`'s existing convention) maps to a
//! `metric_key` plus the descriptive columns a fresh `kpi_definitions` seed
//! needs — `label`, `value_kind`, `statement_group`, `period_nature`.
//! Curated in-repo, seeded by migration; nothing is minted at runtime.
//!
//! **Reuse before mint** (decision 2): a concept maps onto an EXISTING
//! catalog key whenever one already holds facts for the same real-world
//! figure, however unattractive that key's name — `EquityAttributableTo-
//! OwnersOfParent` → `wdf_equity_parent`, `ProfitLossAttributableToOwners-
//! OfParent` → `wdf_net_profit_parent`, `ProfitLossBeforeTax` →
//! `wdf_pretax_profit`, plus (found while building this table)
//! `IssuedCapital` → `wdf_share_capital`, `NoncurrentAssets` →
//! `wdf_noncurrent_assets`, `NoncurrentLiabilities` →
//! `wdf_noncurrent_liabilities`, `IncreaseDecreaseInCashAndCashEquivalents`
//! → `wdf_net_cash_change`, and `PurchaseOfProperty...NoncurrentAssets` →
//! `capex` — all pre-existing ESPI cover-note (`espi_cover_note.rs`) or
//! universal-pack canonical keys already carrying hundreds of facts. The
//! Polish-literal WDF row keys (e.g. `wdf_calkowity_dochod*`) are
//! deliberately NOT reused as generic targets: migration 0112's own comment
//! marks them as "verbatim cover-note row identities, not IFRS concepts" —
//! several near-duplicate phrasings were kept apart on purpose because the
//! cover-note mapper cannot safely collapse them, so folding a new IFRS
//! concept onto one would silently merge issuer-row variants (the exact
//! fragmentation ADR 0077 decision 8 "no repaint" forbids). `Comprehensive-
//! Income` and its OCI siblings therefore mint a clean `total_comprehensive_
//! income`-family key instead.
//!
//! Seed content: the 22 arms `esef::concept_to_metric_key` mapped by hand
//! (moved verbatim — same concept, same key) plus every concept observed at
//! ≥3 of the 8 sampled GPW filings in the epic's harvest
//! (`ifrs-crosswalk-candidates.txt`, 117 concepts), covering 123 distinct
//! taxonomy concepts in total. `period_nature` is copied directly from that
//! harvest, which measured zero instant/duration conflicts across issuers
//! (ADR 0100 decision 6) — never re-derived here.
//!
//! This module does not change `esef::parse_esef` or
//! `esef::concept_to_metric_key` behaviour — it is the input the next slice
//! (the Layer 1 → Layer 2 projection) resolves against.

/// One taxonomy concept's naming decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrosswalkEntry {
    /// The taxonomy concept's local name, prefix-agnostic (matches
    /// `report_tagged_facts.concept_local_name` and the convention
    /// `esef::concept_to_metric_key` already uses).
    pub concept: &'static str,
    /// The canonical catalog key this concept resolves to.
    pub metric_key: &'static str,
    /// Human-readable label — used only to seed a `kpi_definitions` row that
    /// has none yet; a reused key keeps its own existing seeded label.
    pub label: &'static str,
    /// `kpi_definitions.value_kind` vocabulary: monetary | percentage |
    /// ratio | count | physical | duration.
    pub value_kind: &'static str,
    /// `kpi_definitions.statement_group` vocabulary: income | balance |
    /// cash_flow | per_share | other.
    pub statement_group: &'static str,
    /// `kpi_definitions.period_nature` vocabulary: instant | duration.
    pub period_nature: &'static str,
}

/// Sorted (by `concept`) and deduplicated: each taxonomy concept appears
/// exactly once. Two concepts MAY share a `metric_key` (e.g. `Revenue` and
/// `RevenueFromContractsWithCustomers` both resolve to `revenue`, the same
/// duplication the original `esef.rs` match arm already had) — that is
/// concept plurality, not a crosswalk duplicate.
const ENTRIES: &[CrosswalkEntry] = &[
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInInventories", metric_key: "adj_change_in_inventories", label: "Adjustment — change in inventories", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInTradeAndOtherReceivables", metric_key: "adj_change_in_trade_receivables", label: "Adjustment — change in trade and other receivables", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDepreciationAndAmortisationExpense", metric_key: "adj_depreciation_and_amortisation", label: "Adjustment — depreciation and amortisation", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncomeTaxExpense", metric_key: "adj_income_tax_expense", label: "Adjustment — income tax expense", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInTradeAndOtherPayables", metric_key: "adj_change_in_trade_payables", label: "Adjustment — change in trade and other payables", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForLossesGainsOnDisposalOfNoncurrentAssets", metric_key: "adj_losses_gains_on_disposal_of_noncurrent_assets", label: "Adjustment — losses/gains on disposal of non-current assets", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForReconcileProfitLoss", metric_key: "adjustments_to_reconcile_profit_loss", label: "Adjustments to reconcile profit/loss", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForUnrealisedForeignExchangeLossesGains", metric_key: "adj_unrealised_fx_losses_gains", label: "Adjustment — unrealised FX losses/gains", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdministrativeExpense", metric_key: "administrative_expense", label: "Administrative expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "Assets", metric_key: "total_assets", label: "Total assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "BasicEarningsLossPerShare", metric_key: "eps_basic", label: "EPS (basic)", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "BasicEarningsLossPerShareFromContinuingOperations", metric_key: "eps_basic_continuing", label: "EPS (basic, continuing operations)", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "CashAndCashEquivalents", metric_key: "cash", label: "Cash and equivalents", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CashAndCashEquivalentsIfDifferentFromStatementOfFinancialPosition", metric_key: "cash_per_cash_flow_statement", label: "Cash and equivalents (cash flow statement)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "instant" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInFinancingActivities", metric_key: "financing_cash_flow", label: "Financing cash flow", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInInvestingActivities", metric_key: "investing_cash_flow", label: "Investing cash flow", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInOperatingActivities", metric_key: "operating_cash_flow", label: "Operating cash flow", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInOperations", metric_key: "cash_flows_from_operations", label: "Cash flows from operations (before working capital)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsUsedInObtainingControlOfSubsidiariesOrOtherBusinessesClassifiedAsInvestingActivities", metric_key: "cash_used_in_business_acquisitions", label: "Cash used in business acquisitions", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ComprehensiveIncome", metric_key: "total_comprehensive_income", label: "Total comprehensive income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ComprehensiveIncomeAttributableToNoncontrollingInterests", metric_key: "comprehensive_income_attributable_nci", label: "Comprehensive income attributable to NCI", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ComprehensiveIncomeAttributableToOwnersOfParent", metric_key: "comprehensive_income_attributable_parent", label: "Comprehensive income attributable to parent", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "CostOfSales", metric_key: "cost_of_sales", label: "Cost of sales", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "CurrentAssets", metric_key: "current_assets", label: "Current assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentLeaseLiabilities", metric_key: "current_lease_liabilities", label: "Current lease liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentLiabilities", metric_key: "current_liabilities", label: "Current liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentProvisionsForEmployeeBenefits", metric_key: "current_provisions_employee_benefits", label: "Current provisions for employee benefits", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTaxAssets", metric_key: "current_tax_assets", label: "Current tax assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTaxAssetsCurrent", metric_key: "current_tax_assets", label: "Current tax assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTaxLiabilities", metric_key: "current_tax_liabilities", label: "Current tax liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTaxLiabilitiesCurrent", metric_key: "current_tax_liabilities", label: "Current tax liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTradeReceivables", metric_key: "current_trade_receivables", label: "Current trade receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DeferredTaxAssets", metric_key: "deferred_tax_assets", label: "Deferred tax assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DeferredTaxLiabilities", metric_key: "deferred_tax_liabilities", label: "Deferred tax liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DepositsFromCustomers", metric_key: "total_deposits", label: "Deposits", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DilutedEarningsLossPerShare", metric_key: "eps_diluted", label: "EPS (diluted)", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "DilutedEarningsLossPerShareFromContinuingOperations", metric_key: "eps_diluted_continuing", label: "EPS (diluted, continuing operations)", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsPaid", metric_key: "dividends_paid", label: "Dividends paid", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsPaidClassifiedAsFinancingActivities", metric_key: "dividends_paid_financing_activities", label: "Dividends paid (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsPaidToEquityHoldersOfParentClassifiedAsFinancingActivities", metric_key: "dividends_paid_to_owners_of_parent_financing", label: "Dividends paid to owners of parent (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsRecognisedAsDistributionsToOwnersPerShare", metric_key: "dividend_per_share", label: "Dividend per share", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "EffectOfExchangeRateChangesOnCashAndCashEquivalents", metric_key: "fx_effect_on_cash", label: "FX effect on cash", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "Equity", metric_key: "total_equity", label: "Total equity", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "EquityAndLiabilities", metric_key: "total_equity_and_liabilities", label: "Total equity and liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "EquityAttributableToOwnersOfParent", metric_key: "wdf_equity_parent", label: "Equity attributable to parent", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "FeeAndCommissionExpense", metric_key: "fee_commission_expense", label: "Fee and commission expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FeeAndCommissionIncome", metric_key: "fee_commission_income", label: "Fee and commission income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FeeAndCommissionIncomeExpense", metric_key: "net_fee_commission_income", label: "Net fee & commission income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FinanceCosts", metric_key: "finance_costs", label: "Finance costs", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FinanceIncome", metric_key: "finance_income", label: "Finance income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "Goodwill", metric_key: "goodwill", label: "Goodwill", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "GrossProfit", metric_key: "gross_profit", label: "Gross profit", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ImpairmentLossImpairmentGainAndReversalOfImpairmentLossDeterminedInAccordanceWithIFRS9", metric_key: "ifrs9_impairment_loss_gain", label: "IFRS 9 impairment loss/gain", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxExpenseContinuingOperations", metric_key: "income_tax_expense", label: "Income tax expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxRelatingToRemeasurementsOfDefinedBenefitPlansOfOtherComprehensiveIncome", metric_key: "income_tax_oci_remeasurement_defined_benefit_plans", label: "Income tax on OCI — defined benefit plan remeasurements", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxesPaidRefundClassifiedAsOperatingActivities", metric_key: "income_taxes_paid_refund_operating", label: "Income taxes paid/refunded (operating activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseInCashAndCashEquivalents", metric_key: "wdf_net_cash_change", label: "Net change in cash", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseInCashAndCashEquivalentsBeforeEffectOfExchangeRateChanges", metric_key: "net_change_in_cash_before_fx", label: "Net change in cash before FX effect", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughAppropriationOfRetainedEarnings", metric_key: "retained_earnings_appropriation", label: "Retained earnings appropriation", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughChangesInOwnershipInterestsInSubsidiariesThatDoNotResultInLossOfControl", metric_key: "equity_changes_in_ownership_interests_in_subsidiaries", label: "Equity — changes in ownership interests in subsidiaries", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughSharebasedPaymentTransactions", metric_key: "equity_sharebased_payment_transactions", label: "Equity — share-based payment transactions", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughTransfersAndOtherChangesEquity", metric_key: "equity_other_transfers", label: "Other equity transfers", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "InflowsOfCashFromInvestingActivities", metric_key: "investing_cash_inflows", label: "Investing cash inflows", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "InsuranceRevenue", metric_key: "gross_insurance_revenue", label: "Gross insurance revenue", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IntangibleAssetsAndGoodwill", metric_key: "intangible_assets_and_goodwill", label: "Intangible assets and goodwill", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "InterestExpense", metric_key: "interest_expense", label: "Interest expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestPaidClassifiedAsFinancingActivities", metric_key: "interest_paid_financing_activities", label: "Interest paid (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestReceivedClassifiedAsInvestingActivities", metric_key: "interest_received_investing", label: "Interest received (investing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestRevenueCalculatedUsingEffectiveInterestMethod", metric_key: "interest_revenue_effective_interest_method", label: "Interest revenue (effective interest method)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestRevenueExpense", metric_key: "net_interest_income", label: "Net interest income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "Inventories", metric_key: "inventories", label: "Inventories", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "IssuedCapital", metric_key: "wdf_share_capital", label: "Share capital", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "Liabilities", metric_key: "total_liabilities", label: "Total liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "LoansAndAdvancesToCustomers", metric_key: "total_loans", label: "Loans", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "LongtermBorrowings", metric_key: "long_term_debt", label: "Long-term debt", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncontrollingInterests", metric_key: "noncontrolling_interests", label: "Non-controlling interests", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentAssets", metric_key: "wdf_noncurrent_assets", label: "Non-current assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentAssetsOrDisposalGroupsClassifiedAsHeldForSale", metric_key: "noncurrent_assets_held_for_sale", label: "Non-current assets held for sale", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentLeaseLiabilities", metric_key: "noncurrent_lease_liabilities", label: "Non-current lease liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentLiabilities", metric_key: "wdf_noncurrent_liabilities", label: "Non-current liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentProvisionsForEmployeeBenefits", metric_key: "noncurrent_provisions_employee_benefits", label: "Non-current provisions for employee benefits", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherAdjustmentsForWhichCashEffectsAreInvestingOrFinancingCashFlow", metric_key: "other_adjustments_investing_or_financing_cash_effects", label: "Other adjustments — investing/financing cash effects", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherAdjustmentsToReconcileProfitLoss", metric_key: "other_adjustments_to_reconcile_profit_loss", label: "Other adjustments to reconcile profit/loss", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncome", metric_key: "other_comprehensive_income", label: "Other comprehensive income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeBeforeTaxGainsLossesOnRemeasurementsOfDefinedBenefitPlans", metric_key: "oci_remeasurement_defined_benefit_plans_before_tax", label: "OCI — defined benefit plan remeasurements (before tax)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeNetOfTaxExchangeDifferencesOnTranslation", metric_key: "oci_fx_translation_net_of_tax", label: "OCI — FX translation (net of tax)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeThatWillBeReclassifiedToProfitOrLossNetOfTax", metric_key: "oci_reclassifiable_net_of_tax", label: "OCI reclassifiable to profit or loss (net of tax)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeThatWillNotBeReclassifiedToProfitOrLossNetOfTax", metric_key: "oci_non_reclassifiable_net_of_tax", label: "OCI not reclassifiable to profit or loss (net of tax)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherCurrentFinancialAssets", metric_key: "other_current_financial_assets", label: "Other current financial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCurrentNonfinancialAssets", metric_key: "other_current_nonfinancial_assets", label: "Other current non-financial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherExpenseByFunction", metric_key: "other_expense_by_function", label: "Other expense by function", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherIncome", metric_key: "other_income", label: "Other income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherNoncurrentFinancialAssets", metric_key: "other_noncurrent_financial_assets", label: "Other non-current financial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherReserves", metric_key: "other_reserves", label: "Other reserves", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherShorttermProvisions", metric_key: "other_shortterm_provisions", label: "Other short-term provisions", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OutflowsOfCashFromInvestingActivities", metric_key: "investing_cash_outflows", label: "Investing cash outflows", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PaymentsOfLeaseLiabilitiesClassifiedAsFinancingActivities", metric_key: "lease_liability_payments_financing", label: "Lease liability payments (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PaymentsToAcquireOrRedeemEntitysShares", metric_key: "payments_to_acquire_or_redeem_own_shares", label: "Payments to acquire/redeem own shares", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromBorrowingsClassifiedAsFinancingActivities", metric_key: "proceeds_from_borrowings_financing", label: "Proceeds from borrowings (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromDisposalsOfPropertyPlantAndEquipmentIntangibleAssetsOtherThanGoodwillInvestmentPropertyAndOtherNoncurrentAssets", metric_key: "proceeds_from_disposal_of_ppe_and_intangibles", label: "Proceeds from disposal of PP&E and intangibles", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromIssueOfBondsNotesAndDebentures", metric_key: "proceeds_from_issue_of_bonds", label: "Proceeds from issue of bonds, notes and debentures", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLoss", metric_key: "net_profit", label: "Net profit", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossAttributableToNoncontrollingInterests", metric_key: "net_profit_attributable_nci", label: "Net profit attributable to NCI", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossAttributableToOwnersOfParent", metric_key: "wdf_net_profit_parent", label: "Net profit attributable to parent", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossBeforeTax", metric_key: "wdf_pretax_profit", label: "Profit before tax", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossFromOperatingActivities", metric_key: "operating_profit", label: "Operating profit (EBIT)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "PropertyPlantAndEquipment", metric_key: "property_plant_equipment", label: "Property, plant and equipment", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "PropertyPlantAndEquipmentIncludingRightofuseAssets", metric_key: "property_plant_equipment_incl_rou", label: "Property, plant and equipment (incl. right-of-use)", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "Provisions", metric_key: "provisions", label: "Provisions", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "PurchaseOfPropertyPlantAndEquipmentIntangibleAssetsOtherThanGoodwillInvestmentPropertyAndOtherNoncurrentAssets", metric_key: "capex", label: "Capital expenditure", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PurchaseOfTreasuryShares", metric_key: "purchase_of_treasury_shares", label: "Purchase of treasury shares", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "RepaymentsOfBondsNotesAndDebentures", metric_key: "repayments_of_bonds", label: "Repayments of bonds, notes and debentures", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "RepaymentsOfBorrowingsClassifiedAsFinancingActivities", metric_key: "repayments_of_borrowings_financing", label: "Repayments of borrowings (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ReserveOfExchangeDifferencesOnTranslation", metric_key: "fx_translation_reserve", label: "FX translation reserve", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "RetainedEarnings", metric_key: "retained_earnings", label: "Retained earnings", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "RetainedEarningsExcludingProfitLossForReportingPeriod", metric_key: "retained_earnings_excl_current_period", label: "Retained earnings (excl. current period)", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "RetainedEarningsProfitLossForReportingPeriod", metric_key: "retained_earnings_current_period_profit", label: "Retained earnings — current period profit", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "Revenue", metric_key: "revenue", label: "Revenue", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "RevenueFromContractsWithCustomers", metric_key: "revenue", label: "Revenue from contracts with customers", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "RightofuseAssets", metric_key: "right_of_use_assets", label: "Right-of-use assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "SharePremium", metric_key: "share_premium", label: "Share premium", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "TreasuryShares", metric_key: "treasury_shares", label: "Treasury shares", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "WeightedAverageShares", metric_key: "weighted_average_shares", label: "Weighted average shares", value_kind: "count", statement_group: "other", period_nature: "duration" },
];

/// The curated concept → metric_key naming table. Iterate this rather than
/// re-deriving concept mappings elsewhere — it is the single naming
/// authority (ADR 0100 decision 2).
pub fn entries() -> &'static [CrosswalkEntry] {
    ENTRIES
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_VALUE_KINDS: &[&str] = &[
        "monetary",
        "percentage",
        "ratio",
        "count",
        "physical",
        "duration",
    ];
    const VALID_STATEMENT_GROUPS: &[&str] =
        &["income", "balance", "cash_flow", "per_share", "other"];
    const VALID_PERIOD_NATURES: &[&str] = &["instant", "duration"];

    #[test]
    fn every_metric_key_is_valid_snake_case_and_bounded() {
        for entry in entries() {
            let key = entry.metric_key;
            assert!(
                !key.is_empty() && key.len() <= 256,
                "{key} must be non-empty and <= 256 bytes"
            );
            let mut chars = key.chars();
            let first = chars.next().expect("non-empty");
            assert!(
                first.is_ascii_lowercase(),
                "{key} (concept {}) must start with a lowercase ascii letter",
                entry.concept
            );
            assert!(
                key.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "{key} (concept {}) must match ^[a-z][a-z0-9_]*$",
                entry.concept
            );
        }
    }

    #[test]
    fn concepts_are_sorted_and_deduplicated() {
        let concepts: Vec<&str> = entries().iter().map(|e| e.concept).collect();
        let mut sorted = concepts.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            concepts.len(),
            "no concept may appear twice in the crosswalk"
        );
        assert!(
            concepts.windows(2).all(|pair| pair[0] < pair[1]),
            "entries() must be sorted strictly ascending by concept"
        );
    }

    /// ADR 0100 decision 2's explicit reuse-before-mint examples, plus the
    /// additional reuse pairs found while building this table (module doc).
    /// Each MUST resolve to the exact existing key, never a fresh mint.
    #[test]
    fn reuse_before_mint_pairs_resolve_to_the_existing_keys() {
        let by_concept: std::collections::HashMap<&str, &str> = entries()
            .iter()
            .map(|e| (e.concept, e.metric_key))
            .collect();

        let expected: &[(&str, &str)] = &[
            ("EquityAttributableToOwnersOfParent", "wdf_equity_parent"),
            ("ProfitLossAttributableToOwnersOfParent", "wdf_net_profit_parent"),
            ("ProfitLossBeforeTax", "wdf_pretax_profit"),
            ("Inventories", "inventories"),
            ("IssuedCapital", "wdf_share_capital"),
            ("NoncurrentAssets", "wdf_noncurrent_assets"),
            ("NoncurrentLiabilities", "wdf_noncurrent_liabilities"),
            (
                "IncreaseDecreaseInCashAndCashEquivalents",
                "wdf_net_cash_change",
            ),
            (
                "PurchaseOfPropertyPlantAndEquipmentIntangibleAssetsOtherThanGoodwillInvestmentPropertyAndOtherNoncurrentAssets",
                "capex",
            ),
            (
                "DividendsRecognisedAsDistributionsToOwnersPerShare",
                "dividend_per_share",
            ),
            ("InterestExpense", "interest_expense"),
        ];
        for (concept, key) in expected {
            assert_eq!(
                by_concept.get(concept).copied(),
                Some(*key),
                "{concept} must reuse the existing key {key}"
            );
        }

        // The negative case: `inventory` (0 facts) must never be used —
        // `inventories` (771 facts) is the live key.
        assert_ne!(by_concept.get("Inventories"), Some(&"inventory"));
    }

    #[test]
    fn vocabulary_columns_are_within_their_documented_sets() {
        for entry in entries() {
            assert!(
                VALID_VALUE_KINDS.contains(&entry.value_kind),
                "{}: unknown value_kind {}",
                entry.concept,
                entry.value_kind
            );
            assert!(
                VALID_STATEMENT_GROUPS.contains(&entry.statement_group),
                "{}: unknown statement_group {}",
                entry.concept,
                entry.statement_group
            );
            assert!(
                VALID_PERIOD_NATURES.contains(&entry.period_nature),
                "{}: unknown period_nature {}",
                entry.concept,
                entry.period_nature
            );
            assert!(
                !entry.label.is_empty(),
                "{}: label must not be empty",
                entry.concept
            );
        }
    }

    /// The 22-arm `esef::concept_to_metric_key` table moved verbatim: same
    /// concept, same key. A future edit to either table must keep them in
    /// sync until the projection slice retires the old function.
    #[test]
    fn covers_every_concept_esef_concept_to_metric_key_maps() {
        let moved_verbatim: &[(&str, &str)] = &[
            ("Assets", "total_assets"),
            ("Liabilities", "total_liabilities"),
            ("Equity", "total_equity"),
            ("CashAndCashEquivalents", "cash"),
            ("CurrentAssets", "current_assets"),
            ("CurrentLiabilities", "current_liabilities"),
            ("RetainedEarnings", "retained_earnings"),
            ("LongtermBorrowings", "long_term_debt"),
            ("Revenue", "revenue"),
            ("RevenueFromContractsWithCustomers", "revenue"),
            ("GrossProfit", "gross_profit"),
            ("ProfitLossFromOperatingActivities", "operating_profit"),
            ("ProfitLoss", "net_profit"),
            ("BasicEarningsLossPerShare", "eps_basic"),
            ("DilutedEarningsLossPerShare", "eps_diluted"),
            (
                "CashFlowsFromUsedInOperatingActivities",
                "operating_cash_flow",
            ),
            (
                "CashFlowsFromUsedInInvestingActivities",
                "investing_cash_flow",
            ),
            (
                "CashFlowsFromUsedInFinancingActivities",
                "financing_cash_flow",
            ),
            ("InterestRevenueExpense", "net_interest_income"),
            ("FeeAndCommissionIncomeExpense", "net_fee_commission_income"),
            ("LoansAndAdvancesToCustomers", "total_loans"),
            ("DepositsFromCustomers", "total_deposits"),
            ("InsuranceRevenue", "gross_insurance_revenue"),
        ];
        let by_concept: std::collections::HashMap<&str, &str> = entries()
            .iter()
            .map(|e| (e.concept, e.metric_key))
            .collect();
        for (concept, key) in moved_verbatim {
            assert_eq!(
                by_concept.get(concept).copied(),
                Some(*key),
                "{concept} must be present, mapped to {key} (moved verbatim from esef.rs)"
            );
        }
    }

    #[test]
    fn at_least_117_concepts_from_the_ge3_issuer_harvest_are_covered() {
        // Guardrail against the table shrinking silently — 117 is the exact
        // count of concepts observed at >=3 of the 8 sampled GPW filings
        // (ifrs-crosswalk-candidates.txt), the spec's coverage floor.
        assert!(
            entries().len() >= 117,
            "expected at least 117 crosswalk entries, got {}",
            entries().len()
        );
    }
}
