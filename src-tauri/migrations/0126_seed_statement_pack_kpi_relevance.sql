-- ADR 0092 layer 2: seed the statement-pack `kpi_relevance` additions for the
-- companies already stored (issue #273).
--
-- The `scope='sector'` KPI packs have existed since migration 0034 (ADR 0027)
-- and NOTHING has ever read them at runtime. Layer 2 turns
-- `companies.statement_type` into a conservative additive selection over them,
-- on top of the `source='core'` floor migrations 0106/0124 seeded. The forward
-- path is `storage/financials.rs::seed_statement_pack_kpi_relevance`, called
-- from `create_company` and from the daily aggregator pull (which is also what
-- makes a later `statement_type` change converge); this migration is the
-- backfill for what is already there.
--
-- CONSERVATIVE SUBSET — the ADR asks for keys genuinely universal within the
-- statement type, not every key a pack lists, because an expectation nobody
-- reports inflates the recall denominator without making the completeness gate
-- smarter. Each pick and each omission:
--
--   * banking (pack has 12): net_interest_income, net_fee_commission_income,
--     total_loans, total_deposits — the four primary-statement lines every
--     bank's periodic report carries. Omitted: operating_income /
--     operating_expenses (aggregates whose composition varies by presentation);
--     nim / cost_income_ratio / npl_ratio / cost_of_risk / cet1 / tcr (ratios
--     and capital measures reported in the commentary or the notes, not the
--     statements).
--   * insurance (pack has 7): gross_insurance_revenue only — the IFRS 17 top
--     line, mandatory for every EU insurer since 2023. Omitted:
--     gross_written_premium / net_earned_premium (pre-IFRS-17, now
--     supplementary), claims_ratio / combined_ratio (non-life only),
--     technical_result / investment_result (presentation varies).
--   * reit (pack has 7): ffo only — the NAREIT-standard headline. The rest are
--     property-type specific or derived.
--   * specialty_finance: NOTHING. The pack (recoveries, erc, cash_ebitda,
--     portfolio_purchases) is debt-collector vocabulary, but migration 0095
--     also maps exchanges and brokerage houses onto this same statement_type.
--     No key is universal across that mix and the ADR's rule is to leave out
--     when unsure. Splitting the type is a separate decision.
--
-- Measured on the maintainer's database (2026-07-31): 52 companies — 46
-- industrial, 2 banking (PEO, PKO), 1 insurance (PZU), 3 specialty_finance
-- (GPW, KRU, XTB). This migration therefore adds 2 x 4 = 8 banking rows and
-- 1 x 1 = 1 insurance row: 9 rows, growing kpi_relevance from 260 to 269. The
-- 46 industrial companies gain nothing — their pack is `scope='canonical'` and
-- the core floor already covers it.
--
-- Forward, idempotent, self-healing, additive:
--   * deterministic `kpirel_sector_<company>_<metric_key>` ids, so re-applying
--     converges instead of accumulating;
--   * `INSERT OR IGNORE` against `UNIQUE(company_id, definition_id)` plus the
--     `NOT EXISTS` guard, so a `core`/`user`/`agent` row for the same metric is
--     never overwritten, re-ranked or duplicated;
--   * nothing is ever deleted — a reclassified company keeps its old rows
--     (ADR 0092 layer 2: automation widens, the user narrows);
--   * a statement_type with no matching pack seeds nothing rather than failing.
--
-- The metric keys are globally unique across the packs, so the flat allow-list
-- plus the `d.sector = c.statement_type` join selects exactly the company's own
-- pack. (`statement_type` and the pack `sector` share one vocabulary —
-- 'banking' / 'insurance' / 'specialty_finance' / 'reit' — as migration 0095
-- established when it wrote those values.)

INSERT OR IGNORE INTO kpi_relevance
    (id, company_id, definition_id, status, source, rank)
SELECT
    'kpirel_sector_' || c.id || '_' || d.metric_key,
    c.id,
    d.id,
    'active',
    'sector',
    'primary'
FROM companies c
JOIN kpi_definitions d
  ON d.scope = 'sector'
 AND d.sector = c.statement_type
 AND d.metric_key IN (
        -- banking
        'net_interest_income',
        'net_fee_commission_income',
        'total_loans',
        'total_deposits',
        -- insurance
        'gross_insurance_revenue',
        -- reit
        'ffo'
     )
WHERE NOT EXISTS (
    SELECT 1
    FROM kpi_relevance existing
    WHERE existing.company_id = c.id
      AND existing.definition_id = d.id
);
