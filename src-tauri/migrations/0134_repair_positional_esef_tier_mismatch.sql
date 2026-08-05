-- Bug #324: `financial_fact_provenance.source_tier` and
-- `financial_facts.extraction_method` disagreeing (7 rows on the maintainer's
-- DB: tier says `esef`, method says `html_positional`). The positional tier
-- always persists under `source_tier='pdf'` (83/90 html_positional facts do);
-- these 7 were left mislabeled by `record_structured_fact`'s tier-precedence
-- UPGRADE path (an issuer tier taking over an already-`pdf`/`html_positional`
-- slot rewrote the provenance row's `source_tier` but never synced
-- `financial_facts.extraction_method`, so it stayed `html_positional` under a
-- now-`esef` tier stamp). Fixed at the write path in the same change:
-- `write_fact_provenance_fields` (`storage/kpi_extraction.rs`) syncs
-- `financial_facts.extraction_method` and the provenance row in ONE call,
-- with a `SourceTier::matches_extraction_method` coherence guard refusing
-- any incoherent pair on every provenance write.
--
-- Repair: `extraction_method` is the honest signal (it names the parser that
-- actually produced the fact — `html_positional` never lies about its own
-- route), so the tier is corrected to match it, not the other way round.
-- Forward, idempotent, self-healing: a DB with no such row runs this as a
-- no-op.

UPDATE financial_fact_provenance
SET source_tier = 'pdf',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE source_tier = 'esef'
  AND fact_id IN (
        SELECT id FROM financial_facts WHERE extraction_method = 'html_positional'
    );
