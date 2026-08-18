-- Quick ratio never computed, for any company, since it was seeded.
--
-- Migration 0048 seeded BOTH `inventory` (its own canonical row, 0 facts ever)
-- and the derived `quick_ratio` whose formula reads that key:
--   (current_assets - inventory) / current_liabilities
-- Migration 0084 later seeded `inventories`, which is where every extractor
-- and the aggregator actually write (771 facts / 44 companies on the owner's
-- database). So the formula's middle term resolved to nothing on every period,
-- the expression evaluated to unavailable, and the metric simply never
-- appeared — indistinguishable from "the issuer did not report it".
--
-- This is the catalog-fragmentation class ADR 0100 exists to close, caught by
-- reading the seeds rather than by any test: `inventory` is now a curated
-- alias of `inventories` (`fundamentals::kpi_aliases`, decision 12), so
-- writers land on the live key, and the formula is repointed here so readers
-- do too. The guardrail that keeps it closed is
-- `no_derived_formula_references_an_alias_source` — a derived formula may
-- never reference a key declared dead.
--
-- Forward, idempotent, self-healing: matches on the exact seeded formula
-- text, so a database whose row was already repaired (or hand-edited to
-- something else) is left untouched. Only the canonical row is touched — a
-- user- or company-scoped `quick_ratio` is the owner's own definition and is
-- never rewritten (ADR 0077 decision 8, no repaint).

UPDATE kpi_definitions
SET formula = '(current_assets - inventories) / current_liabilities'
WHERE id = 'kpidef_quick_ratio'
  AND scope = 'canonical'
  AND computation = 'derived'
  AND formula = '(current_assets - inventory) / current_liabilities';
