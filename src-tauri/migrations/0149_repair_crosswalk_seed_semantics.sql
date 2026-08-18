-- Curation repairs for 0143-era seeds already applied to real databases
-- (luna review findings 1/4, epic #398 review round). 0148's own rows were
-- fixed in place (it had not been applied anywhere); these two keys shipped
-- in 0143, so the repair must be a forward migration.
--
-- 1) `dividends_paid` was seeded under statement_group 'other'. Dividends
--    paid is a cash-flow amount (IAS 7) even when the taxonomy concept name
--    omits the explicit financing classification — its two financing-scoped
--    siblings were already seeded 'cash_flow'.
--
-- 2) `cash_flows_from_operations` (concept CashFlowsFromUsedInOperations)
--    was labeled "(before working capital)", which is a DIFFERENT taxonomy
--    concept (CashFlowsFromUsedInOperationsBeforeChangesInWorkingCapital) —
--    that one now has its own key, so this label claimed a distinction it
--    does not have.
--
-- Forward, idempotent, self-healing, no-repaint: each UPDATE matches the
-- exact seeded value, so an owner-edited row is never rewritten, and a
-- re-run matches nothing.

UPDATE kpi_definitions
SET statement_group = 'cash_flow'
WHERE id = 'kpidef_dividends_paid'
  AND scope = 'canonical'
  AND statement_group = 'other';

UPDATE kpi_definitions
SET label = 'Cash flows from operations'
WHERE id = 'kpidef_cash_flows_from_operations'
  AND scope = 'canonical'
  AND label = 'Cash flows from operations (before working capital)';

-- 3) OCI and comprehensive-income keys move from statement_group 'income'
--    to 'other' (owner decision 2026-08-18, luna review finding 1): IAS 1
--    separates profit or loss from other comprehensive income, and in this
--    five-group catalog `other` is the only honest bucket for OCI components
--    and comprehensive-income totals. Guarded on the seeded value, so an
--    owner-edited row is never rewritten; keys seeded by 0148 already carry
--    'other' at insert, so this matches only the 0143-era rows.
UPDATE kpi_definitions
SET statement_group = 'other'
WHERE scope = 'canonical'
  AND statement_group = 'income'
  AND metric_key IN (
    'total_comprehensive_income',
    'comprehensive_income_attributable_nci',
    'comprehensive_income_attributable_parent',
    'gains_losses_on_cash_flow_hedges_before_tax',
    'gains_losses_on_exchange_differences_on_translation_net_of_tax',
    'income_tax_relating_to_cash_flow_hedges_of_oci',
    'income_tax_relating_to_components_of_oci_reclassifiable',
    'income_tax_relating_to_components_of_oci_non_reclassifiable',
    'income_tax_oci_remeasurement_defined_benefit_plans',
    'other_comprehensive_income',
    'oci_before_tax',
    'oci_before_tax_cash_flow_hedges',
    'oci_before_tax_exchange_differences_on_translation',
    'oci_before_tax_gains_losses_equity_investments',
    'oci_remeasurement_defined_benefit_plans_before_tax',
    'oci_net_of_tax_cash_flow_hedges',
    'oci_fx_translation_net_of_tax',
    'oci_net_of_tax_gains_losses_equity_investments',
    'oci_net_of_tax_gains_losses_defined_benefit_remeasurement',
    'oci_net_of_tax_gains_losses_on_revaluation',
    'oci_reclassifiable_before_tax',
    'oci_reclassifiable_net_of_tax',
    'oci_non_reclassifiable_before_tax',
    'oci_non_reclassifiable_net_of_tax',
    'share_of_oci_of_associates_jvs_reclassifiable_net_of_tax'
  );
