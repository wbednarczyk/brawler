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
