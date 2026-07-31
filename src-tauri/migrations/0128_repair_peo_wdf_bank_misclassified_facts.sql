-- Repair the two live-wrong PEO PSr H1/2026 bank-table facts the
-- `espi_cover_note` cover-note classifier produced before this card's fix
-- (epic #277 T2, card #279).
--
-- Root cause (fixed in the same change, `espi_cover_note.rs::classify`):
-- 1. `total_liabilities`: the classifier's bare "zobowiązania" stem fell
--    through to `total_liabilities` for ANY liabilities row lacking a
--    długo/krótkoterminowe qualifier. Bank WDF tables often carry no
--    "Zobowiązania razem" row at all — Pekao's PSr H1/2026 note only has
--    qualified sub-lines, and the interbank row ("Zobowiązania wobec innych
--    banków", 7 899 000 000 PLN) landed on the total slot, ~40x understated
--    (the real total liabilities are ~316.9bn).
-- 2. `wdf_equity_parent`: the "przypisan" trigger also matched the
--    non-controlling-interests row ("Kapitał własny przypisany udziałom
--    niedającym kontroli", 12 000 000 PLN) — the exact opposite of parent
--    equity — silently shadowing the real parent-equity row ("Kapitał
--    własny przypisany akcjonariuszom Banku", ~32.9bn) that follows it in
--    the same table.
--
-- Deletes the two facts by narrow predicate — ticker + period + metric +
-- extraction_method + the EXACT wrong value — so a re-extraction that lands
-- a correct value at the same slot (or a user's manual override) is never
-- touched. `financial_fact_provenance.fact_id` carries no FK/cascade (see
-- 0057), so its row is cleaned up explicitly first (0107 precedent).
-- Idempotent: once deleted, the value predicate matches nothing on a re-run.

DELETE FROM financial_fact_provenance
WHERE fact_id IN (
    SELECT f.id
      FROM financial_facts f
      JOIN companies c ON c.id = f.company_id
      JOIN financial_periods p ON p.id = f.period_id
      JOIN kpi_definitions kd ON kd.id = f.definition_id
     WHERE c.ticker = 'PEO'
       AND p.fiscal_year = 2026
       AND p.period_type = 'H1'
       AND f.extraction_method = 'espi_cover_note'
       AND (
            (kd.metric_key = 'total_liabilities' AND f.value_numeric = '7899000000')
         OR (kd.metric_key = 'wdf_equity_parent' AND f.value_numeric = '12000000')
       )
);

DELETE FROM financial_facts
WHERE id IN (
    SELECT f.id
      FROM financial_facts f
      JOIN companies c ON c.id = f.company_id
      JOIN financial_periods p ON p.id = f.period_id
      JOIN kpi_definitions kd ON kd.id = f.definition_id
     WHERE c.ticker = 'PEO'
       AND p.fiscal_year = 2026
       AND p.period_type = 'H1'
       AND f.extraction_method = 'espi_cover_note'
       AND (
            (kd.metric_key = 'total_liabilities' AND f.value_numeric = '7899000000')
         OR (kd.metric_key = 'wdf_equity_parent' AND f.value_numeric = '12000000')
       )
);
