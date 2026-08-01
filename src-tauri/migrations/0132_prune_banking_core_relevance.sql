-- Issue #284 (spun out of epic #277 T5): the ADR 0092 layer-1 core floor expects
-- `revenue` and `operating_profit` from EVERY company, but a bank under IFRS
-- files neither as a comparable statement line — so the completeness gate's
-- denominator counted two rows that can never be filled, capping bank recall at
-- an artificial ceiling and rendering permanently-red Coverage cells.
--
-- Measured on the maintainer's database (2026-08-01 copy), both tracked banks:
--   * `revenue` — ZERO facts at PEO and PKO, from any tier. PKO's FY2025 ESEF
--     instance carries no `ifrs-full:Revenue` tag and BiznesRadar's bank income
--     layout has no such row; `net_interest_income` + `net_fee_commission_income`
--     (already seeded by ADR 0092 layer 2) are the structural replacement.
--   * `operating_profit` — ONE fact, PKO FY2025 = 30 343 000 000 from the ESEF
--     concept `ProfitLossFromOperatingActivities`. That figure is the bank's
--     total operating INCOME (net interest 24 223 + net fee 5 243 + other), not
--     an operating profit: PKO's FY2025 net profit is 10 682. So the key has a
--     carrier but not a comparable meaning — expecting it is the same dishonest
--     denominator, and the mis-mapped concept itself is tracked separately.
-- Every other financial statement_type reports both keys normally on the same
-- database (insurance 17/17, specialty_finance 17/17, brokerage 33/33), so this
-- prune is deliberately banking-only rather than a general financial-issuer rule.
--
-- Archive, never delete: `status='archived'` is the lifecycle vocabulary, keeps
-- the row's history, and is exactly what `expected_primary_metric_keys` filters
-- on. Guarded to `source='core'`, so a user- or agent-curated expectation for the
-- same metric at a bank stays untouched — ADR 0092 layer 4 always wins. Forward,
-- idempotent (a re-run matches nothing once the rows are archived) and
-- self-healing (it keys off live `statement_type`, not a hard-coded ticker list).
--
-- The create-time twin lives in `seed_core_kpi_relevance` (storage/financials.rs):
-- without it a newly tracked bank would be seeded with the same two dead
-- expectations the moment this migration finished healing the existing ones.

UPDATE kpi_relevance
SET status = 'archived',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE source = 'core'
  AND status = 'active'
  AND company_id IN (SELECT id FROM companies WHERE statement_type = 'banking')
  AND definition_id IN (
      SELECT id FROM kpi_definitions
      WHERE scope = 'canonical' AND metric_key IN ('revenue', 'operating_profit')
  );
