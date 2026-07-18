-- Repair the two mis-scaled CDR Q3 2023 facts the v0.57 health-facts backfill
-- persisted (card e6ebda3, 2026-07-18).
--
-- Root cause (fixed in the same change): `pdf::detect_unit_scale` triggered the
-- `Millions` scale on a BARE `mln zł` token that appears in NARRATIVE prose
-- ("koszty … około 95 mln zł") inside the CD PROJEKT Q3 2023 Polish
-- `Skonsolidowane sprawozdanie`, whose statements are DECLARED in thousands
-- ("wszystkie kwoty … w tys. zł"). Every value parsed off that document
-- over-scaled ×1000. The backfill SURFACED the disagreement for the seven
-- concepts that already had a correct value from the English condensed statement
-- (never overwritten), but `current_assets` / `current_liabilities` had no prior
-- value, so the mis-scaled figure was CREATED as an `auto_unreviewed` fact.
--
-- The parser is now fixed, so a re-extraction yields the correct thousands-scale
-- value — but these two already-persisted facts must be corrected. Per the
-- data-model repair rules this ships as a forward, idempotent, self-healing
-- migration (never a direct DB write):
--   * Exact-value guards make it idempotent — once corrected, the mis-scaled
--     source value no longer matches, so a re-run is a no-op.
--   * The signature (metric + CDR + FY2023/Q3 period + the Polish source document
--     + `html_positional` method) is precise: it can only match these two facts.
--   * Only the machine-created `auto_unreviewed` values are touched; a value a
--     user has since confirmed/edited (different magnitude) is never rewritten.
-- Correct values verified against the source document (printed "1 137 807" and
-- "164 335" in thousands → 1,137,807,000 and 164,335,000 PLN).

UPDATE financial_facts
   SET value_numeric = '1137807000',
       updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
 WHERE value_numeric = '1137807000000'
   AND extraction_method = 'html_positional'
   AND source_document_ref LIKE '%skonsolidowane_sprawozdanie_finansowe_grupy_cd_projekt_za_3q_2023%'
   AND definition_id IN (SELECT id FROM kpi_definitions WHERE metric_key = 'current_assets')
   AND period_id IN (
        SELECT p.id FROM financial_periods p JOIN companies c ON c.id = p.company_id
         WHERE c.ticker = 'CDR' AND p.fiscal_year = 2023 AND p.period_type = 'Q3');

UPDATE financial_facts
   SET value_numeric = '164335000',
       updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
 WHERE value_numeric = '164335000000'
   AND extraction_method = 'html_positional'
   AND source_document_ref LIKE '%skonsolidowane_sprawozdanie_finansowe_grupy_cd_projekt_za_3q_2023%'
   AND definition_id IN (SELECT id FROM kpi_definitions WHERE metric_key = 'current_liabilities')
   AND period_id IN (
        SELECT p.id FROM financial_periods p JOIN companies c ON c.id = p.company_id
         WHERE c.ticker = 'CDR' AND p.fiscal_year = 2023 AND p.period_type = 'Q3');
