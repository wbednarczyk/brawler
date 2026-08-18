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
    CrosswalkEntry { concept: "AccrualsClassifiedAsCurrent", metric_key: "accruals_classified_as_current", label: "Accruals classified as current", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "AdjustedWeightedAverageShares", metric_key: "adjusted_weighted_average_shares", label: "Adjusted weighted average shares", value_kind: "count", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForAmortisationExpense", metric_key: "adj_amortisation_expense", label: "Adjustment — amortisation expense", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInAccruedIncomeOtherThanContractAssets", metric_key: "adj_change_in_accrued_income_excl_contract_assets", label: "Adjustment — change in accrued income excluding contract assets", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInContractAssets", metric_key: "adj_change_in_contract_assets", label: "Adjustment — change in contract assets", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInInventories", metric_key: "adj_change_in_inventories", label: "Adjustment — change in inventories", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInOtherAssets", metric_key: "adj_change_in_other_assets", label: "Adjustment — change in other assets", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInPrepaidExpenses", metric_key: "adj_change_in_prepaid_expenses", label: "Adjustment — change in prepaid expenses", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInTradeAccountReceivable", metric_key: "adj_change_in_trade_account_receivable", label: "Adjustment — change in trade account receivable", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDecreaseIncreaseInTradeAndOtherReceivables", metric_key: "adj_change_in_trade_receivables", label: "Adjustment — change in trade and other receivables", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDepreciationAndAmortisationExpense", metric_key: "adj_depreciation_and_amortisation", label: "Adjustment — depreciation and amortisation", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDepreciationExpense", metric_key: "adj_depreciation_expense", label: "Adjustment — depreciation expense", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForDividendIncome", metric_key: "adj_dividend_income", label: "Adjustment — dividend income", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForFairValueGainsLosses", metric_key: "adj_fair_value_gains_losses", label: "Adjustment — fair value gains losses", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForFinanceCosts", metric_key: "adj_finance_costs", label: "Adjustment — finance costs", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForFinanceIncomeCost", metric_key: "adj_finance_income_cost", label: "Adjustment — finance income cost", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForGainLossOnDisposalOfInvestmentsInSubsidiariesJointVenturesAndAssociates", metric_key: "adj_gain_loss_on_disposal_of_investments_in_subsidiaries_jvs", label: "Adjustment — gain loss on disposal of investments in subsidiaries joint ventures and associates", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForGainsLossesOnChangeInFairValueOfDerivatives", metric_key: "adj_gains_losses_on_change_in_fair_value_of_derivatives", label: "Adjustment — gains losses on change in fair value of derivatives", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForImpairmentLossRecognisedInProfitOrLossGoodwill", metric_key: "adj_impairment_loss_in_pl_goodwill", label: "Adjustment — impairment loss recognised in profit or loss goodwill", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForImpairmentLossReversalOfImpairmentLossRecognisedInProfitOrLoss", metric_key: "adj_impairment_loss_reversal_of_impairment_loss_in_pl", label: "Adjustment — impairment loss reversal of impairment loss recognised in profit or loss", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncomeTaxExpense", metric_key: "adj_income_tax_expense", label: "Adjustment — income tax expense", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInContractLiabilities", metric_key: "adj_change_in_contract_liabilities", label: "Adjustment — change in contract liabilities", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInDeferredIncomeIncludingContractLiabilities", metric_key: "adj_change_in_deferred_income_including_contract_liabilities", label: "Adjustment — change in deferred income including contract liabilities", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInDeferredIncomeOtherThanContractLiabilities", metric_key: "adj_change_in_deferred_income_excl_contract_liabilities", label: "Adjustment — change in deferred income excluding contract liabilities", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInDepositsFromCustomers", metric_key: "adj_change_in_deposits_from_customers", label: "Adjustment — change in deposits from customers", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInEmployeeBenefitLiabilities", metric_key: "adj_change_in_employee_benefit_liabilities", label: "Adjustment — change in employee benefit liabilities", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInOtherLiabilities", metric_key: "adj_change_in_other_liabilities", label: "Adjustment — change in other liabilities", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInTradeAccountPayable", metric_key: "adj_change_in_trade_account_payable", label: "Adjustment — change in trade account payable", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForIncreaseDecreaseInTradeAndOtherPayables", metric_key: "adj_change_in_trade_payables", label: "Adjustment — change in trade and other payables", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForInterestExpense", metric_key: "adj_interest_expense", label: "Adjustment — interest expense", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForInterestIncome", metric_key: "adj_interest_income", label: "Adjustment — interest income", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForLossesGainsOnDisposalOfNoncurrentAssets", metric_key: "adj_losses_gains_on_disposal_of_noncurrent_assets", label: "Adjustment — losses/gains on disposal of non-current assets", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForProvisions", metric_key: "adj_provisions", label: "Adjustment — provisions", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForReconcileProfitLoss", metric_key: "adjustments_to_reconcile_profit_loss", label: "Adjustments to reconcile profit/loss", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForSharebasedPayments", metric_key: "adj_sharebased_payments", label: "Adjustment — sharebased payments", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForUndistributedProfitsOfAssociates", metric_key: "adj_undistributed_profits_of_associates", label: "Adjustment — undistributed profits of associates", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForUndistributedProfitsOfInvestmentsAccountedForUsingEquityMethod", metric_key: "adj_undistributed_profits_of_equity_method_investments", label: "Adjustment — undistributed profits of investments (equity method)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsForUnrealisedForeignExchangeLossesGains", metric_key: "adj_unrealised_fx_losses_gains", label: "Adjustment — unrealised FX losses/gains", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdjustmentsToReconcileProfitLossOtherThanChangesInWorkingCapital", metric_key: "adjustments_to_reconcile_profit_loss_excl_changes_in_working", label: "Adjustments to reconcile profit loss excluding changes in working capital", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "AdministrativeExpense", metric_key: "administrative_expense", label: "Administrative expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "Assets", metric_key: "total_assets", label: "Total assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "BasicEarningsLossPerShare", metric_key: "eps_basic", label: "EPS (basic)", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "BasicEarningsLossPerShareFromContinuingOperations", metric_key: "eps_basic_continuing", label: "EPS (basic, continuing operations)", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "BasicEarningsLossPerShareFromDiscontinuedOperations", metric_key: "basic_earnings_loss_per_share_from_discontinued_operations", label: "Basic earnings loss per share from discontinued operations", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "CapitalRedemptionReserve", metric_key: "capital_redemption_reserve", label: "Capital redemption reserve", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CapitalReserve", metric_key: "capital_reserve", label: "Capital reserve", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CashAdvancesAndLoansMadeToOtherPartiesClassifiedAsInvestingActivities", metric_key: "cash_advances_and_loans_made_to_other_parties_investing", label: "Cash advances and loans made to other parties (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashAdvancesAndLoansMadeToRelatedParties", metric_key: "cash_advances_and_loans_made_to_related_parties", label: "Cash advances and loans made to related parties", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashAndCashEquivalents", metric_key: "cash", label: "Cash and equivalents", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CashAndCashEquivalentsIfDifferentFromStatementOfFinancialPosition", metric_key: "cash_per_cash_flow_statement", label: "Cash and equivalents (cash flow statement)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "instant" },
    CrosswalkEntry { concept: "CashFlowsFromLosingControlOfSubsidiariesOrOtherBusinessesClassifiedAsInvestingActivities", metric_key: "cash_flows_from_losing_control_of_subsidiaries_investing", label: "Cash flows from losing control of subsidiaries or other businesses (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInDecreaseIncreaseInShorttermDepositsAndInvestments", metric_key: "cash_flows_from_used_in_change_in_shortterm_deposits_and", label: "Cash flows from decrease increase in shortterm deposits and investments", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInFinancingActivities", metric_key: "financing_cash_flow", label: "Financing cash flow", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInInvestingActivities", metric_key: "investing_cash_flow", label: "Investing cash flow", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInOperatingActivities", metric_key: "operating_cash_flow", label: "Operating cash flow", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInOperations", metric_key: "cash_flows_from_operations", label: "Cash flows from operations (before working capital)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsFromUsedInOperationsBeforeChangesInWorkingCapital", metric_key: "cash_flows_from_used_in_operations_before_changes_in_working", label: "Cash flows from operations before changes in working capital", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashFlowsUsedInObtainingControlOfSubsidiariesOrOtherBusinessesClassifiedAsInvestingActivities", metric_key: "cash_used_in_business_acquisitions", label: "Cash used in business acquisitions", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashReceiptsFromRepaymentOfAdvancesAndLoansMadeToOtherPartiesClassifiedAsInvestingActivities", metric_key: "cash_receipts_from_repayment_of_advances_and_loans_made_to_other", label: "Cash receipts from repayment of advances and loans made to other parties (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "CashReceiptsFromRepaymentOfAdvancesAndLoansMadeToRelatedParties", metric_key: "cash_receipts_from_repayment_of_advances_and_loans_made_to", label: "Cash receipts from repayment of advances and loans made to related parties", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ChangesInEquity", metric_key: "changes_in_equity", label: "Changes in equity", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "ChangesInInventoriesOfFinishedGoodsAndWorkInProgress", metric_key: "changes_in_inventories_of_finished_goods_and_work_in_progress", label: "Changes in inventories of finished goods and work in progress", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ComprehensiveIncome", metric_key: "total_comprehensive_income", label: "Total comprehensive income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ComprehensiveIncomeAttributableToNoncontrollingInterests", metric_key: "comprehensive_income_attributable_nci", label: "Comprehensive income attributable to NCI", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ComprehensiveIncomeAttributableToOwnersOfParent", metric_key: "comprehensive_income_attributable_parent", label: "Comprehensive income attributable to parent", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "CostOfMerchandiseSold", metric_key: "cost_of_merchandise_sold", label: "Cost of merchandise sold", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "CostOfSales", metric_key: "cost_of_sales", label: "Cost of sales", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "CurrentAssets", metric_key: "current_assets", label: "Current assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentAssetsOtherThanAssetsOrDisposalGroupsClassifiedAsHeldForSaleOrAsHeldForDistributionToOwners", metric_key: "current_assets_excl_assets_held_for_sale", label: "Current assets excluding assets or disposal groups classified as held for sale or as held for distribution to owners", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentBondsIssuedAndCurrentPortionOfNoncurrentBondsIssued", metric_key: "current_bonds_issued_and_current_portion_of_bonds_issued", label: "Current bonds issued and current portion of noncurrent bonds issued", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentBorrowingsAndCurrentPortionOfNoncurrentBorrowings", metric_key: "current_borrowings_and_current_portion_of_borrowings", label: "Current borrowings and current portion of noncurrent borrowings", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentContractAssets", metric_key: "current_contract_assets", label: "Current contract assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentContractLiabilities", metric_key: "current_contract_liabilities", label: "Current contract liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentDeferredIncomeIncludingCurrentContractLiabilities", metric_key: "current_deferred_income_including_current_contract_liabilities", label: "Current deferred income including current contract liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentDerivativeFinancialAssets", metric_key: "current_derivative_financial_assets", label: "Current derivative financial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentDerivativeFinancialLiabilities", metric_key: "current_derivative_financial_liabilities", label: "Current derivative financial liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentFinanceLeaseReceivables", metric_key: "current_finance_lease_receivables", label: "Current finance lease receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentFinancialAssetsAtAmortisedCost", metric_key: "current_financial_assets_at_amortised_cost", label: "Current financial assets at amortised cost", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentGovernmentGrants", metric_key: "current_government_grants", label: "Current government grants", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentLeaseLiabilities", metric_key: "current_lease_liabilities", label: "Current lease liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentLiabilities", metric_key: "current_liabilities", label: "Current liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentLiabilitiesOtherThanLiabilitiesIncludedInDisposalGroupsClassifiedAsHeldForSale", metric_key: "current_liabilities_excl_liabilities_held_for_sale", label: "Current liabilities excluding liabilities included in disposal groups classified as held for sale", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentLoansReceivedAndCurrentPortionOfNoncurrentLoansReceived", metric_key: "current_loans_received_and_current_portion_of_loans_received", label: "Current loans received and current portion of noncurrent loans received", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentPayablesOnSocialSecurityAndTaxesOtherThanIncomeTax", metric_key: "current_payables_on_social_security_and_taxes_excl_income_tax", label: "Current payables on social security and taxes excluding income tax", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentPrepaidExpenses", metric_key: "current_prepaid_expenses", label: "Current prepaid expenses", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentPrepayments", metric_key: "current_prepayments", label: "Current prepayments", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentPrepaymentsAndCurrentAccruedIncomeOtherThanCurrentContractAssets", metric_key: "current_prepayments_and_current_accrued_income_excl_current", label: "Current prepayments and current accrued income excluding current contract assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentProvisions", metric_key: "current_provisions", label: "Current provisions", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentProvisionsForEmployeeBenefits", metric_key: "current_provisions_employee_benefits", label: "Current provisions for employee benefits", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentReceivablesFromTaxesOtherThanIncomeTax", metric_key: "current_receivables_from_taxes_excl_income_tax", label: "Current receivables from taxes excluding income tax", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentRestrictedCashAndCashEquivalents", metric_key: "current_restricted_cash", label: "Current restricted cash and cash equivalents", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentSecuredBankLoansReceivedAndCurrentPortionOfNoncurrentSecuredBankLoansReceived", metric_key: "current_secured_bank_loans_received_and_current_portion_of", label: "Current secured bank loans received and current portion of noncurrent secured bank loans received", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTaxAssets", metric_key: "current_tax_assets", label: "Current tax assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTaxAssetsCurrent", metric_key: "current_tax_assets", label: "Current tax assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTaxLiabilities", metric_key: "current_tax_liabilities", label: "Current tax liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTaxLiabilitiesCurrent", metric_key: "current_tax_liabilities", label: "Current tax liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "CurrentTradeReceivables", metric_key: "current_trade_receivables", label: "Current trade receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DeferredTaxAssets", metric_key: "deferred_tax_assets", label: "Deferred tax assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DeferredTaxLiabilities", metric_key: "deferred_tax_liabilities", label: "Deferred tax liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DepositsFromCustomers", metric_key: "total_deposits", label: "Deposits", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DepreciationAndAmortisationExpense", metric_key: "depreciation_amortisation", label: "Depreciation and amortisation expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "DerivativeFinancialLiabilities", metric_key: "derivative_financial_liabilities", label: "Derivative financial liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "DilutedEarningsLossPerShare", metric_key: "eps_diluted", label: "EPS (diluted)", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "DilutedEarningsLossPerShareFromContinuingOperations", metric_key: "eps_diluted_continuing", label: "EPS (diluted, continuing operations)", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "DilutedEarningsLossPerShareFromDiscontinuedOperations", metric_key: "diluted_earnings_loss_per_share_from_discontinued_operations", label: "Diluted earnings loss per share from discontinued operations", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "DistributionCosts", metric_key: "distribution_costs", label: "Distribution costs", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsPaid", metric_key: "dividends_paid", label: "Dividends paid", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsPaidClassifiedAsFinancingActivities", metric_key: "dividends_paid_financing_activities", label: "Dividends paid (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsPaidToEquityHoldersOfParentClassifiedAsFinancingActivities", metric_key: "dividends_paid_to_owners_of_parent_financing", label: "Dividends paid to owners of parent (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsPaidToNoncontrollingInterestsClassifiedAsFinancingActivities", metric_key: "dividends_paid_to_nci_financing", label: "Dividends paid to NCI (financing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsProposedOrDeclaredBeforeFinancialStatementsAuthorisedForIssueButNotRecognisedAsDistributionToOwners", metric_key: "dividends_proposed_not_recognised", label: "Dividends proposed or declared before financial statements authorised for issue but not recognised as distribution to owners", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsProposedOrDeclaredBeforeFinancialStatementsAuthorisedForIssueButNotRecognisedAsDistributionToOwnersPerShare", metric_key: "dividends_proposed_not_recognised_per_share", label: "Dividends proposed or declared before financial statements authorised for issue but not recognised as distribution to owners per share", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsReceivedClassifiedAsInvestingActivities", metric_key: "dividends_received_investing", label: "Dividends received (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsRecognisedAsDistributionsToNoncontrollingInterests", metric_key: "dividends_recognised_as_distributions_to_nci", label: "Dividends recognised as distributions to NCI", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsRecognisedAsDistributionsToOwnersOfParent", metric_key: "dividends_recognised_as_distributions_to_owners_of_parent", label: "Dividends recognised as distributions to owners of parent", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "DividendsRecognisedAsDistributionsToOwnersPerShare", metric_key: "dividend_per_share", label: "Dividend per share", value_kind: "monetary", statement_group: "per_share", period_nature: "duration" },
    CrosswalkEntry { concept: "EffectOfExchangeRateChangesOnCashAndCashEquivalents", metric_key: "fx_effect_on_cash", label: "FX effect on cash", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "EmployeeBenefitsExpense", metric_key: "employee_benefits_expense", label: "Employee benefits expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "Equity", metric_key: "total_equity", label: "Total equity", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "EquityAndLiabilities", metric_key: "total_equity_and_liabilities", label: "Total equity and liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "EquityAttributableToOwnersOfParent", metric_key: "wdf_equity_parent", label: "Equity attributable to parent", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "ExpenseByNature", metric_key: "expense_by_nature", label: "Expense by nature", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FeeAndCommissionExpense", metric_key: "fee_commission_expense", label: "Fee and commission expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FeeAndCommissionIncome", metric_key: "fee_commission_income", label: "Fee and commission income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FeeAndCommissionIncomeExpense", metric_key: "net_fee_commission_income", label: "Net fee & commission income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FinanceCosts", metric_key: "finance_costs", label: "Finance costs", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FinanceIncome", metric_key: "finance_income", label: "Finance income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "FinanceIncomeCost", metric_key: "finance_income_cost", label: "Finance income cost", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "GainRecognisedInBargainPurchaseTransaction", metric_key: "gain_recognised_in_bargain_purchase_transaction", label: "Gain recognised in bargain purchase transaction", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "GainsLossesOnCashFlowHedgesBeforeTax", metric_key: "gains_losses_on_cash_flow_hedges_before_tax", label: "Gains losses on cash flow hedges before tax", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "GainsLossesOnExchangeDifferencesOnTranslationNetOfTax", metric_key: "gains_losses_on_exchange_differences_on_translation_net_of_tax", label: "Gains losses on exchange differences on translation net of tax", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "GainsLossesOnExchangeDifferencesOnTranslationRecognisedInProfitOrLoss", metric_key: "gains_losses_on_exchange_differences_on_translation_in_pl", label: "Gains losses on exchange differences on translation recognised in profit or loss", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "GainsLossesOnNetMonetaryPosition", metric_key: "gains_losses_on_net_monetary_position", label: "Gains losses on net monetary position", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "GeneralAndAdministrativeExpense", metric_key: "general_and_administrative_expense", label: "General and administrative expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "Goodwill", metric_key: "goodwill", label: "Goodwill", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "GrossProfit", metric_key: "gross_profit", label: "Gross profit", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ImpairmentLossImpairmentGainAndReversalOfImpairmentLossDeterminedInAccordanceWithIFRS9", metric_key: "ifrs9_impairment_loss_gain", label: "IFRS 9 impairment loss/gain", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ImpairmentLossReversalOfImpairmentLossRecognisedInProfitOrLoss", metric_key: "impairment_loss_reversal_of_impairment_loss_in_pl", label: "Impairment loss reversal of impairment loss recognised in profit or loss", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxExpenseContinuingOperations", metric_key: "income_tax_expense", label: "Income tax expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxRelatingToCashFlowHedgesOfOtherComprehensiveIncome", metric_key: "income_tax_relating_to_cash_flow_hedges_of_oci", label: "Income tax relating to cash flow hedges of OCI", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxRelatingToComponentsOfOtherComprehensiveIncomeThatWillBeReclassifiedToProfitOrLoss", metric_key: "income_tax_relating_to_components_of_oci_reclassifiable", label: "Income tax relating to components of OCI (reclassifiable)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxRelatingToComponentsOfOtherComprehensiveIncomeThatWillNotBeReclassifiedToProfitOrLoss", metric_key: "income_tax_relating_to_components_of_oci_non_reclassifiable", label: "Income tax relating to components of OCI (non-reclassifiable)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxRelatingToRemeasurementsOfDefinedBenefitPlansOfOtherComprehensiveIncome", metric_key: "income_tax_oci_remeasurement_defined_benefit_plans", label: "Income tax on OCI — defined benefit plan remeasurements", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxesPaidClassifiedAsOperatingActivities", metric_key: "income_taxes_paid_operating", label: "Income taxes paid (operating)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "IncomeTaxesPaidRefundClassifiedAsOperatingActivities", metric_key: "income_taxes_paid_refund_operating", label: "Income taxes paid/refunded (operating activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseInCashAndCashEquivalents", metric_key: "wdf_net_cash_change", label: "Net change in cash", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseInCashAndCashEquivalentsBeforeEffectOfExchangeRateChanges", metric_key: "net_change_in_cash_before_fx", label: "Net change in cash before FX effect", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseInWorkingCapital", metric_key: "change_in_working_capital", label: "Increase decrease in working capital", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughAcquisitionOfSubsidiary", metric_key: "change_through_acquisition_of_subsidiary", label: "Equity change — acquisition of subsidiary", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughAppropriationOfRetainedEarnings", metric_key: "retained_earnings_appropriation", label: "Retained earnings appropriation", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughChangeInEquityOfSubsidiaries", metric_key: "change_through_change_in_equity_of_subsidiaries", label: "Equity change — change in equity of subsidiaries", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughChangesInOwnershipInterestsInSubsidiariesThatDoNotResultInLossOfControl", metric_key: "equity_changes_in_ownership_interests_in_subsidiaries", label: "Equity — changes in ownership interests in subsidiaries", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughDisposalOfSubsidiary", metric_key: "change_through_disposal_of_subsidiary", label: "Equity change — disposal of subsidiary", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughSharebasedPaymentTransactions", metric_key: "equity_sharebased_payment_transactions", label: "Equity — share-based payment transactions", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughTransactionsWithOwners", metric_key: "change_through_transactions_with_owners", label: "Equity change — transactions with owners", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughTransferToStatutoryReserve", metric_key: "change_through_transfer_to_statutory_reserve", label: "Equity change — transfer to statutory reserve", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughTransfersAndOtherChangesEquity", metric_key: "equity_other_transfers", label: "Other equity transfers", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IncreaseDecreaseThroughTreasuryShareTransactions", metric_key: "change_through_treasury_share_transactions", label: "Equity change — treasury share transactions", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "InflowsOfCashFromInvestingActivities", metric_key: "investing_cash_inflows", label: "Investing cash inflows", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "InsuranceRevenue", metric_key: "gross_insurance_revenue", label: "Gross insurance revenue", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "IntangibleAssetsAndGoodwill", metric_key: "intangible_assets_and_goodwill", label: "Intangible assets and goodwill", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "IntangibleAssetsOtherThanGoodwill", metric_key: "intangible_assets_excl_goodwill", label: "Intangible assets excluding goodwill", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "InterestExpense", metric_key: "interest_expense", label: "Interest expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestPaidClassifiedAsFinancingActivities", metric_key: "interest_paid_financing_activities", label: "Interest paid (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestPaidClassifiedAsOperatingActivities", metric_key: "interest_paid_operating", label: "Interest paid (operating)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestReceivedClassifiedAsInvestingActivities", metric_key: "interest_received_investing", label: "Interest received (investing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestReceivedClassifiedAsOperatingActivities", metric_key: "interest_received_operating", label: "Interest received (operating)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestRevenueCalculatedUsingEffectiveInterestMethod", metric_key: "interest_revenue_effective_interest_method", label: "Interest revenue (effective interest method)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "InterestRevenueExpense", metric_key: "net_interest_income", label: "Net interest income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "Inventories", metric_key: "inventories", label: "Inventories", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "InvestmentAccountedForUsingEquityMethod", metric_key: "investment_equity_method", label: "Investment (equity method)", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "InvestmentProperty", metric_key: "investment_property", label: "Investment property", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "InvestmentsInAssociatesAccountedForUsingEquityMethod", metric_key: "investments_in_associates_equity_method", label: "Investments in associates (equity method)", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "InvestmentsInJointVenturesAccountedForUsingEquityMethod", metric_key: "investments_in_joint_ventures_equity_method", label: "Investments in joint ventures (equity method)", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "InvestmentsInSubsidiariesJointVenturesAndAssociates", metric_key: "investments_in_subsidiaries_jvs_associates", label: "Investments in subsidiaries joint ventures and associates", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "IssueOfEquity", metric_key: "issue_of_equity", label: "Issue of equity", value_kind: "monetary", statement_group: "other", period_nature: "duration" },
    CrosswalkEntry { concept: "IssuedCapital", metric_key: "wdf_share_capital", label: "Share capital", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "Liabilities", metric_key: "total_liabilities", label: "Total liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "LiabilitiesIncludedInDisposalGroupsClassifiedAsHeldForSale", metric_key: "liabilities_held_for_sale", label: "Liabilities included in disposal groups classified as held for sale", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "LoansAndAdvancesToCustomers", metric_key: "total_loans", label: "Loans", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "LongtermBorrowings", metric_key: "long_term_debt", label: "Long-term debt", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "MiscellaneousOtherOperatingExpense", metric_key: "miscellaneous_other_operating_expense", label: "Miscellaneous other operating expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "MiscellaneousOtherOperatingIncome", metric_key: "miscellaneous_other_operating_income", label: "Miscellaneous other operating income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "NetDeferredTaxAssets", metric_key: "net_deferred_tax_assets", label: "Net deferred tax assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NetDeferredTaxLiabilities", metric_key: "net_deferred_tax_liabilities", label: "Net deferred tax liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncontrollingInterests", metric_key: "noncontrolling_interests", label: "Non-controlling interests", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentAccrualsAndNoncurrentDeferredIncomeIncludingNoncurrentContractLiabilities", metric_key: "noncurrent_accruals_and_noncurrent_deferred_income_including", label: "Noncurrent accruals and noncurrent deferred income including noncurrent contract liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentAccruedIncomeOtherThanNoncurrentContractAssets", metric_key: "noncurrent_accrued_income_excl_noncurrent_contract_assets", label: "Noncurrent accrued income excluding noncurrent contract assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentAssets", metric_key: "wdf_noncurrent_assets", label: "Non-current assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentAssetsOrDisposalGroupsClassifiedAsHeldForSale", metric_key: "noncurrent_assets_held_for_sale", label: "Non-current assets held for sale", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentAssetsOrDisposalGroupsClassifiedAsHeldForSaleOrAsHeldForDistributionToOwners", metric_key: "noncurrent_assets_held_for_sale", label: "Noncurrent assets or disposal groups classified as held for sale or as held for distribution to owners", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentContractLiabilities", metric_key: "noncurrent_contract_liabilities", label: "Noncurrent contract liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentDeferredIncomeOtherThanNoncurrentContractLiabilities", metric_key: "noncurrent_deferred_income_excl_noncurrent_contract_liabilities", label: "Noncurrent deferred income excluding noncurrent contract liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentDerivativeFinancialAssets", metric_key: "noncurrent_derivative_financial_assets", label: "Noncurrent derivative financial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentDerivativeFinancialLiabilities", metric_key: "noncurrent_derivative_financial_liabilities", label: "Noncurrent derivative financial liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentFinanceLeaseReceivables", metric_key: "noncurrent_finance_lease_receivables", label: "Noncurrent finance lease receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentFinancialAssetsAtAmortisedCost", metric_key: "noncurrent_financial_assets_at_amortised_cost", label: "Noncurrent financial assets at amortised cost", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentGovernmentGrants", metric_key: "noncurrent_government_grants", label: "Noncurrent government grants", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentLeaseLiabilities", metric_key: "noncurrent_lease_liabilities", label: "Non-current lease liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentLiabilities", metric_key: "wdf_noncurrent_liabilities", label: "Non-current liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentPayables", metric_key: "noncurrent_payables", label: "Noncurrent payables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentPortionOfNoncurrentLoansReceived", metric_key: "noncurrent_portion_of_loans_received", label: "Noncurrent portion of noncurrent loans received", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentPortionOfNoncurrentSecuredBankLoansReceived", metric_key: "noncurrent_portion_of_secured_bank_loans_received", label: "Noncurrent portion of noncurrent secured bank loans received", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentPrepayments", metric_key: "noncurrent_prepayments", label: "Noncurrent prepayments", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentPrepaymentsAndNoncurrentAccruedIncomeOtherThanNoncurrentContractAssets", metric_key: "noncurrent_prepayments_and_noncurrent_accrued_income_excl", label: "Noncurrent prepayments and noncurrent accrued income excluding noncurrent contract assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentProvisions", metric_key: "noncurrent_provisions", label: "Noncurrent provisions", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentProvisionsForEmployeeBenefits", metric_key: "noncurrent_provisions_employee_benefits", label: "Non-current provisions for employee benefits", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "NoncurrentReceivables", metric_key: "noncurrent_receivables", label: "Noncurrent receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OperatingExpense", metric_key: "operating_expense", label: "Operating expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherAdjustmentsForNoncashItems", metric_key: "other_adj_noncash_items", label: "Other Adjustment — noncash items", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherAdjustmentsForWhichCashEffectsAreInvestingOrFinancingCashFlow", metric_key: "other_adjustments_investing_or_financing_cash_effects", label: "Other adjustments — investing/financing cash effects", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherAdjustmentsToReconcileProfitLoss", metric_key: "other_adjustments_to_reconcile_profit_loss", label: "Other adjustments to reconcile profit/loss", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherAssets", metric_key: "other_assets", label: "Other assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCashPaymentsToAcquireEquityOrDebtInstrumentsOfOtherEntitiesClassifiedAsInvestingActivities", metric_key: "other_cash_payments_to_acquire_other_entity_securities_investing", label: "Other cash payments to acquire equity or debt instruments of other entities (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherCashReceiptsFromSalesOfEquityOrDebtInstrumentsOfOtherEntitiesClassifiedAsInvestingActivities", metric_key: "other_cash_receipts_from_sales_of_other_entity_securities", label: "Other cash receipts from sales of equity or debt instruments of other entities (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncome", metric_key: "other_comprehensive_income", label: "Other comprehensive income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeBeforeTax", metric_key: "oci_before_tax", label: "OCI before tax", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeBeforeTaxCashFlowHedges", metric_key: "oci_before_tax_cash_flow_hedges", label: "OCI before tax cash flow hedges", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeBeforeTaxExchangeDifferencesOnTranslation", metric_key: "oci_before_tax_exchange_differences_on_translation", label: "OCI before tax exchange differences on translation", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeBeforeTaxGainsLossesFromInvestmentsInEquityInstruments", metric_key: "oci_before_tax_gains_losses_equity_investments", label: "OCI before tax gains losses from investments in equity instruments", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeBeforeTaxGainsLossesOnRemeasurementsOfDefinedBenefitPlans", metric_key: "oci_remeasurement_defined_benefit_plans_before_tax", label: "OCI — defined benefit plan remeasurements (before tax)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeNetOfTaxCashFlowHedges", metric_key: "oci_net_of_tax_cash_flow_hedges", label: "OCI net of tax cash flow hedges", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeNetOfTaxExchangeDifferencesOnTranslation", metric_key: "oci_fx_translation_net_of_tax", label: "OCI — FX translation (net of tax)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeNetOfTaxGainsLossesFromInvestmentsInEquityInstruments", metric_key: "oci_net_of_tax_gains_losses_equity_investments", label: "OCI net of tax gains losses from investments in equity instruments", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeNetOfTaxGainsLossesOnRemeasurementsOfDefinedBenefitPlans", metric_key: "oci_net_of_tax_gains_losses_defined_benefit_remeasurement", label: "OCI net of tax gains losses on remeasurements of defined benefit plans", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeNetOfTaxGainsLossesOnRevaluation", metric_key: "oci_net_of_tax_gains_losses_on_revaluation", label: "OCI net of tax gains losses on revaluation", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeThatWillBeReclassifiedToProfitOrLossBeforeTax", metric_key: "oci_reclassifiable_before_tax", label: "OCI (reclassifiable) before tax", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeThatWillBeReclassifiedToProfitOrLossNetOfTax", metric_key: "oci_reclassifiable_net_of_tax", label: "OCI reclassifiable to profit or loss (net of tax)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeThatWillNotBeReclassifiedToProfitOrLossBeforeTax", metric_key: "oci_non_reclassifiable_before_tax", label: "OCI (non-reclassifiable) before tax", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherComprehensiveIncomeThatWillNotBeReclassifiedToProfitOrLossNetOfTax", metric_key: "oci_non_reclassifiable_net_of_tax", label: "OCI not reclassifiable to profit or loss (net of tax)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherCurrentAssets", metric_key: "other_current_assets", label: "Other current assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCurrentFinancialAssets", metric_key: "other_current_financial_assets", label: "Other current financial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCurrentFinancialLiabilities", metric_key: "other_current_financial_liabilities", label: "Other current financial liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCurrentLiabilities", metric_key: "other_current_liabilities", label: "Other current liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCurrentNonfinancialAssets", metric_key: "other_current_nonfinancial_assets", label: "Other current non-financial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCurrentNonfinancialLiabilities", metric_key: "other_current_nonfinancial_liabilities", label: "Other current nonfinancial liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCurrentPayables", metric_key: "other_current_payables", label: "Other current payables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherCurrentReceivables", metric_key: "other_current_receivables", label: "Other current receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherEquityInterest", metric_key: "other_equity_interest", label: "Other equity interest", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherExpenseByFunction", metric_key: "other_expense_by_function", label: "Other expense by function", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherExpenseByNature", metric_key: "other_expense_by_nature", label: "Other expense by nature", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherFinanceIncomeCost", metric_key: "other_finance_income_cost", label: "Other finance income cost", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherIncome", metric_key: "other_income", label: "Other income", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherInflowsOutflowsOfCashClassifiedAsFinancingActivities", metric_key: "other_inflows_outflows_of_cash_financing", label: "Other inflows outflows of cash (financing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherInflowsOutflowsOfCashClassifiedAsInvestingActivities", metric_key: "other_inflows_outflows_of_cash_investing", label: "Other inflows outflows of cash (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherInflowsOutflowsOfCashClassifiedAsOperatingActivities", metric_key: "other_inflows_outflows_of_cash_operating", label: "Other inflows outflows of cash (operating)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "OtherLongtermProvisions", metric_key: "other_longterm_provisions", label: "Other longterm provisions", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherNoncurrentAssets", metric_key: "other_noncurrent_assets", label: "Other noncurrent assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherNoncurrentFinancialAssets", metric_key: "other_noncurrent_financial_assets", label: "Other non-current financial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherNoncurrentFinancialLiabilities", metric_key: "other_noncurrent_financial_liabilities", label: "Other noncurrent financial liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherNoncurrentLiabilities", metric_key: "other_noncurrent_liabilities", label: "Other noncurrent liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherNoncurrentNonfinancialAssets", metric_key: "other_noncurrent_nonfinancial_assets", label: "Other noncurrent nonfinancial assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherNoncurrentNonfinancialLiabilities", metric_key: "other_noncurrent_nonfinancial_liabilities", label: "Other noncurrent nonfinancial liabilities", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherNoncurrentPayables", metric_key: "other_noncurrent_payables", label: "Other noncurrent payables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherNoncurrentReceivables", metric_key: "other_noncurrent_receivables", label: "Other noncurrent receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherReceivables", metric_key: "other_receivables", label: "Other receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherReserves", metric_key: "other_reserves", label: "Other reserves", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OtherShorttermProvisions", metric_key: "other_shortterm_provisions", label: "Other short-term provisions", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "OutflowsOfCashFromInvestingActivities", metric_key: "investing_cash_outflows", label: "Investing cash outflows", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PaymentsFromChangesInOwnershipInterestsInSubsidiaries", metric_key: "payments_from_ownership_changes_in_subsidiaries", label: "Payments from changes in ownership interests in subsidiaries", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PaymentsOfLeaseLiabilitiesClassifiedAsFinancingActivities", metric_key: "lease_liability_payments_financing", label: "Lease liability payments (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PaymentsToAcquireOrRedeemEntitysShares", metric_key: "payments_to_acquire_or_redeem_own_shares", label: "Payments to acquire/redeem own shares", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromBorrowingsClassifiedAsFinancingActivities", metric_key: "proceeds_from_borrowings_financing", label: "Proceeds from borrowings (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromChangesInOwnershipInterestsInSubsidiaries", metric_key: "proceeds_from_ownership_changes_in_subsidiaries", label: "Proceeds from changes in ownership interests in subsidiaries", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromContributionsOfNoncontrollingInterests", metric_key: "proceeds_from_contributions_of_nci", label: "Proceeds from contributions of NCI", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromDisposalsOfPropertyPlantAndEquipmentIntangibleAssetsOtherThanGoodwillInvestmentPropertyAndOtherNoncurrentAssets", metric_key: "proceeds_from_disposal_of_ppe_and_intangibles", label: "Proceeds from disposal of PP&E and intangibles", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromGovernmentGrantsClassifiedAsFinancingActivities", metric_key: "proceeds_from_government_grants_financing", label: "Proceeds from government grants (financing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromGovernmentGrantsClassifiedAsInvestingActivities", metric_key: "proceeds_from_government_grants_investing", label: "Proceeds from government grants (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromIssueOfBondsNotesAndDebentures", metric_key: "proceeds_from_issue_of_bonds", label: "Proceeds from issue of bonds, notes and debentures", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromIssuingShares", metric_key: "proceeds_from_issuing_shares", label: "Proceeds from issuing shares", value_kind: "count", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromSalesOfIntangibleAssetsClassifiedAsInvestingActivities", metric_key: "proceeds_from_sales_of_intangible_assets_investing", label: "Proceeds from sales of intangible assets (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromSalesOfInterestsInAssociates", metric_key: "proceeds_from_sales_of_interests_in_associates", label: "Proceeds from sales of interests in associates", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromSalesOfInvestmentProperty", metric_key: "proceeds_from_sales_of_investment_property", label: "Proceeds from sales of investment property", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromSalesOfPropertyPlantAndEquipmentClassifiedAsInvestingActivities", metric_key: "proceeds_from_sales_of_ppe_investing", label: "Proceeds from sales of PP&E (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProceedsFromSalesOrMaturityOfFinancialInstrumentsClassifiedAsInvestingActivities", metric_key: "proceeds_from_sales_or_maturity_of_financial_instr_investing", label: "Proceeds from sales or maturity of financial instruments (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLoss", metric_key: "net_profit", label: "Net profit", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossAttributableToNoncontrollingInterests", metric_key: "net_profit_attributable_nci", label: "Net profit attributable to NCI", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossAttributableToOwnersOfParent", metric_key: "wdf_net_profit_parent", label: "Net profit attributable to parent", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossBeforeTax", metric_key: "wdf_pretax_profit", label: "Profit before tax", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossFromContinuingOperations", metric_key: "net_profit_continuing", label: "Profit loss from continuing operations", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossFromDiscontinuedOperations", metric_key: "net_profit_discontinued", label: "Profit loss from discontinued operations", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ProfitLossFromOperatingActivities", metric_key: "operating_profit", label: "Operating profit (EBIT)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "PropertyPlantAndEquipment", metric_key: "property_plant_equipment", label: "Property, plant and equipment", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "PropertyPlantAndEquipmentIncludingRightofuseAssets", metric_key: "property_plant_equipment_incl_rou", label: "Property, plant and equipment (incl. right-of-use)", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "Provisions", metric_key: "provisions", label: "Provisions", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "PurchaseOfFinancialInstrumentsClassifiedAsInvestingActivities", metric_key: "purchase_of_financial_instr_investing", label: "Purchase of financial instruments (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PurchaseOfIntangibleAssetsClassifiedAsInvestingActivities", metric_key: "purchase_of_intangible_assets_investing", label: "Purchase of intangible assets (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PurchaseOfInterestsInInvestmentsAccountedForUsingEquityMethod", metric_key: "purchase_of_interests_in_equity_method_investments", label: "Purchase of interests in investments (equity method)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PurchaseOfInvestmentProperty", metric_key: "purchase_of_investment_property", label: "Purchase of investment property", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PurchaseOfPropertyPlantAndEquipmentClassifiedAsInvestingActivities", metric_key: "purchase_of_ppe_investing", label: "Purchase of PP&E (investing)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PurchaseOfPropertyPlantAndEquipmentIntangibleAssetsOtherThanGoodwillInvestmentPropertyAndOtherNoncurrentAssets", metric_key: "capex", label: "Capital expenditure", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "PurchaseOfTreasuryShares", metric_key: "purchase_of_treasury_shares", label: "Purchase of treasury shares", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "RawMaterialsAndConsumablesUsed", metric_key: "raw_materials_and_consumables_used", label: "Raw materials and consumables used", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "RepaymentsOfBondsNotesAndDebentures", metric_key: "repayments_of_bonds", label: "Repayments of bonds, notes and debentures", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "RepaymentsOfBorrowingsClassifiedAsFinancingActivities", metric_key: "repayments_of_borrowings_financing", label: "Repayments of borrowings (financing activities)", value_kind: "monetary", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "ResearchAndDevelopmentExpense", metric_key: "research_and_development_expense", label: "Research and development expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ReserveOfCashFlowHedges", metric_key: "reserve_of_cash_flow_hedges", label: "Reserve of cash flow hedges", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "ReserveOfExchangeDifferencesOnTranslation", metric_key: "fx_translation_reserve", label: "FX translation reserve", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "ReserveOfRemeasurementsOfDefinedBenefitPlans", metric_key: "reserve_defined_benefit_remeasurement", label: "Reserve of remeasurements of defined benefit plans", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "ReserveOfSharebasedPayments", metric_key: "reserve_of_sharebased_payments", label: "Reserve of sharebased payments", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "RestrictedCashAndCashEquivalents", metric_key: "restricted_cash", label: "Restricted cash and cash equivalents", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "RetainedEarnings", metric_key: "retained_earnings", label: "Retained earnings", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "RetainedEarningsExcludingProfitLossForReportingPeriod", metric_key: "retained_earnings_excl_current_period", label: "Retained earnings (excl. current period)", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "RetainedEarningsProfitLossForReportingPeriod", metric_key: "retained_earnings_current_period_profit", label: "Retained earnings — current period profit", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "Revenue", metric_key: "revenue", label: "Revenue", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "RevenueFromContractsWithCustomers", metric_key: "revenue", label: "Revenue from contracts with customers", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "RevenueFromRenderingOfServices", metric_key: "revenue_from_rendering_of_services", label: "Revenue from rendering of services", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "RevenueFromSaleOfGoods", metric_key: "revenue_from_sale_of_goods", label: "Revenue from sale of goods", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "RightofuseAssets", metric_key: "right_of_use_assets", label: "Right-of-use assets", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "RightofuseAssetsThatDoNotMeetDefinitionOfInvestmentProperty", metric_key: "rightofuse_assets_that_do_not_meet_definition_of_investment", label: "Rightofuse assets that do not meet definition of investment property", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "SaleOrIssueOfTreasuryShares", metric_key: "sale_or_issue_of_treasury_shares", label: "Sale or issue of treasury shares", value_kind: "count", statement_group: "cash_flow", period_nature: "duration" },
    CrosswalkEntry { concept: "SalesAndMarketingExpense", metric_key: "sales_and_marketing_expense", label: "Sales and marketing expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "SellingExpense", metric_key: "selling_expense", label: "Selling expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ServicesExpense", metric_key: "services_expense", label: "Services expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ShareOfOtherComprehensiveIncomeOfAssociatesAndJointVenturesAccountedForUsingEquityMethodThatWillBeReclassifiedToProfitOrLossNetOfTax", metric_key: "share_of_oci_of_associates_and_jvs_equity_method_reclassifiable", label: "Share of OCI of associates and joint ventures (equity method) (reclassifiable) net of tax", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ShareOfProfitLossOfAssociatesAndJointVenturesAccountedForUsingEquityMethod", metric_key: "share_of_profit_loss_of_associates_and_jvs_equity_method", label: "Share of profit loss of associates and joint ventures (equity method)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "ShareOfProfitLossOfJointVenturesAccountedForUsingEquityMethod", metric_key: "share_of_profit_loss_of_joint_ventures_equity_method", label: "Share of profit loss of joint ventures (equity method)", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "SharePremium", metric_key: "share_premium", label: "Share premium", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "ShorttermBorrowings", metric_key: "shortterm_borrowings", label: "Shortterm borrowings", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "StatutoryReserve", metric_key: "statutory_reserve", label: "Statutory reserve", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "TaxExpenseOtherThanIncomeTaxExpense", metric_key: "tax_expense_excl_income_tax_expense", label: "Tax expense excluding income tax expense", value_kind: "monetary", statement_group: "income", period_nature: "duration" },
    CrosswalkEntry { concept: "TradeAndOtherCurrentPayables", metric_key: "trade_and_other_current_payables", label: "Trade and other current payables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "TradeAndOtherCurrentPayablesToTradeSuppliers", metric_key: "trade_and_other_current_payables_to_trade_suppliers", label: "Trade and other current payables to trade suppliers", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "TradeAndOtherCurrentReceivables", metric_key: "trade_and_other_current_receivables", label: "Trade and other current receivables", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
    CrosswalkEntry { concept: "TradeAndOtherPayablesToTradeSuppliers", metric_key: "trade_and_other_payables_to_trade_suppliers", label: "Trade and other payables to trade suppliers", value_kind: "monetary", statement_group: "balance", period_nature: "instant" },
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
