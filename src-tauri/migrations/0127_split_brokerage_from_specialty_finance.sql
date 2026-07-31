-- Split `brokerage` out of `statement_type = 'specialty_finance'`
-- (owner decision 2026-07-31, ADR 0092 layer-2 follow-up).
--
-- Migration 0095 mapped 'giełdy i biura maklerskie' (exchanges and brokerage
-- houses) onto `specialty_finance`, and 0098 later mapped 'Wierzytelności'
-- (debt collection) onto the SAME type. That conflated two unrelated
-- businesses under one discriminator, and it cost real behaviour: the
-- `scope='sector'` KPI pack for `specialty_finance` is pure debt-collection
-- vocabulary — `recoveries`, `erc` (estimated remaining collections),
-- `cash_ebitda`, `portfolio_purchases` — none of which a brokerage house or a
-- stock exchange reports. ADR 0092 layer 2 therefore had to seed NOTHING for
-- the whole type, so KRUK got no statement pack either.
--
-- Splitting the type frees the pack. After this migration:
--   * `specialty_finance` means debt collection only, and 0126's allow-list
--     (widened in the same change) seeds it ALL FOUR pack keys — every one is
--     a headline figure in a debt collector's own periodic reporting, which is
--     exactly why the 0034 pack was written this way: `recoveries` (cash
--     actually collected — the top line of the business), `erc` (the forward
--     book value the whole equity story rests on), `cash_ebitda` (the sector's
--     standard cash-earnings measure), `portfolio_purchases` (the reinvestment
--     that drives future recoveries). Nothing here is a ratio or a
--     notes-only disclosure, so nothing is left out.
--   * `brokerage` has NO sector pack (migration 0034 never seeded one), so a
--     broker keeps the ADR 0092 core floor and nothing else. Seeding an
--     invented pack is deliberately out of scope: it would be guessing at
--     expectations rather than reading them off a curated catalog.
--
-- MANUAL-WINS. `statement_type` has no `_source` column (unlike `sector`, whose
-- `sector_source` distinguishes 'registry' from 'manual' — migrations
-- 0071/0073), so "was this value automatic?" can only be answered by
-- reconstructing 0095's own predicate. This migration therefore rewrites ONLY
-- rows that still look exactly like 0095's output — `statement_type` is still
-- `'specialty_finance'` AND `sector` is still the broker sector 0095 matched
-- on. Anything else is left alone:
--   * a hand-set value on a broker-sector company is not 'specialty_finance'
--     and is not touched;
--   * a 'specialty_finance' row whose sector is NULL, changed, or 'Wierzytelności'
--     has unknown or debt-collection provenance and is never guessed at.
-- 0095's allow-list had exactly one broker row ('giełdy i biura maklerskie'),
-- so that is the whole list here.
--
-- Measured on the maintainer's database (2026-07-31), statement_type before:
-- industrial 46, specialty_finance 3 (GPW, KRU, XTB), banking 2 (PEO, PKO),
-- insurance 1 (PZU). After: industrial 46, brokerage 2 (GPW, XTB),
-- specialty_finance 1 (KRU), banking 2, insurance 1 — 2 rows reclassified, 0
-- rows lost. KRU then gains its 4 pack rows from the re-run of 0126's
-- statement below, taking `kpi_relevance` from 269 to 273.
--
-- Forward, idempotent, self-healing: re-running matches nothing (the
-- reclassified rows are no longer 'specialty_finance'), and the pack re-seed is
-- the usual `INSERT OR IGNORE` + `NOT EXISTS`, so a curated row is never
-- overwritten and nothing is ever deleted (ADR 0092: automation widens, the
-- user narrows).

UPDATE companies
SET statement_type = 'brokerage'
WHERE statement_type = 'specialty_finance'
  AND sector = 'giełdy i biura maklerskie';

-- Re-run the layer-2 seed now that `specialty_finance` means debt collection
-- only. Identical in shape to 0126 (and to
-- `storage/financials.rs::seed_statement_pack_kpi_relevance`, whose allow-list
-- grew with this change) — one shape, one behaviour.
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
        'ffo',
        -- specialty_finance (debt collection) — the whole pack, freed by the split
        'recoveries',
        'erc',
        'cash_ebitda',
        'portfolio_purchases'
     )
WHERE NOT EXISTS (
    SELECT 1
    FROM kpi_relevance existing
    WHERE existing.company_id = c.id
      AND existing.definition_id = d.id
);
