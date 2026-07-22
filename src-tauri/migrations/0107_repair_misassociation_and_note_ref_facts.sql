-- v0.59 targeted data repairs (cards 45fcece / c7e354d and 22ac70c / 40281b3).
--
-- Two independent, evidence-backed repairs of facts/documents that the extraction
-- fixes shipping in the same release now read correctly, but which were already
-- persisted wrong. Forward, idempotent and self-healing per the data-model
-- migration rules: every statement is a guarded predicate, re-applying converges
-- (a clean database matches nothing), and the runner applies this file exactly
-- once (version-tracked in `schema_migrations`). Covered by the app's
-- pre-migration snapshot + rotating backups (ADR 0032). No file on disk is
-- touched — DB rows only.
--
-- ===========================================================================
-- PART A — remove the Energa filings mis-associated onto cyber_Folks (CBF).
--
-- The OCR/AI path read four scanned Energa Q3-2024 attachments that Bankier
-- served under cyber_Folks's own komunikat and stamped onto `company_gpw_cbf`.
-- Two are `periodic_ssf`/`periodic_jsf`, so they WIN cyber_Folks's Q3-2024
-- canonical slot (ADR 0077 §1) and injected Energa-scale figures (revenue
-- ~483.9 bn, cash ~148.7 bn) into cyber_Folks. Those `ai`-tier facts already
-- die with migration 0102's clean cut; what remains is the four documents.
--
-- Why the predicate is scoped to `company_gpw_cbf` and not a global
-- `url LIKE '%energa%'`: Bankier reuses the same "Grupy-Energa-za-III-kw-2024"
-- URL slug across unrelated issuers filing that day. On the maintainer's
-- database that slug also appears on Vercom (`vrc`), Orlen (`pkn`) and PKO
-- (`pko`) documents whose CONTENT is those issuers' own reports (verified: the
-- `vrc` "Energa" attachment is 98× "Vercom", 0× "Energa") — a global predicate
-- would delete correct filings. The mis-association is real only where a
-- foreign issuer's periodic report landed on a company that would never file it;
-- cyber_Folks is a hosting company and never files Energa's utility statements,
-- and none of cyber_Folks's own attachments carry "energa" in the URL, so the
-- company-scoped predicate hits exactly the four contaminating rows and nothing
-- else. On any database, it only matches a cyber_Folks espi attachment whose
-- Bankier URL names Energa — always a mis-association, never a cyber_Folks
-- filing.

-- Detach any surviving soft reference into these documents before deleting them
-- (the poisoning `ai` facts are already gone via 0102; this guards a
-- deterministic fact that happened to cite one).
UPDATE financial_facts
SET source_document_ref = NULL
WHERE source_document_ref IN (
    SELECT id FROM report_documents
    WHERE company_id = 'company_gpw_cbf'
      AND source_type = 'espi_attachment'
      AND lower(url) LIKE '%energa%'
);

-- The derived section index is a disposable, cascade-cleaned derivation; remove
-- it explicitly so the outcome does not depend on FK enforcement being on.
DELETE FROM report_document_sections
WHERE report_document_id IN (
    SELECT id FROM report_documents
    WHERE company_id = 'company_gpw_cbf'
      AND source_type = 'espi_attachment'
      AND lower(url) LIKE '%energa%'
);

DELETE FROM report_documents
WHERE company_id = 'company_gpw_cbf'
  AND source_type = 'espi_attachment'
  AND lower(url) LIKE '%energa%';

-- ===========================================================================
-- PART B — drop note-reference-misparsed pdf-tier cash facts for re-extraction.
--
-- The tier-2 PDF parser used to read a leading NOTE REFERENCE as the cash value
-- when a balance-sheet line's columns were all small ungrouped integers
-- ("Środki pieniężne i ich ekwiwalenty  19  179  66" → stored 19 000, ~1000×
-- low), and to strand a note ref when a document grouped thousands with a dot.
-- The fixed parser (same release) reads these correctly. These facts are
-- `auto_unreviewed` (autopilot-written, never user-confirmed) and pdf-tier, so
-- deleting them lets the corrected parser re-create them on the next extraction
-- pass — the delete-and-refill pattern. The signature is the misparse's own
-- footprint: a whole-number cash value that is an exact multiple of 1000 and no
-- larger than 60 000 (a note number ×1000). It is universally safe: on a clean
-- database cash is scaled to millions and matches nothing, and even were it to
-- match a genuinely tiny cash position, re-extraction re-writes the identical
-- value — the fact is derived, not canonical.

-- Detach soft references to the target facts first (an autopilot run's produced
-- id list, a manual claim's verification link, a superseding pointer), so undo
-- and claim reads never act on ids about to vanish.
UPDATE autopilot_run
SET produced_fact_ids_json = COALESCE((
        SELECT json_group_array(value)
        FROM json_each(autopilot_run.produced_fact_ids_json)
        WHERE value NOT IN (
            SELECT id FROM financial_facts
            WHERE definition_id = 'kpidef_cash'
              AND confirmation_state = 'auto_unreviewed'
              AND CAST(value_numeric AS INTEGER) = CAST(value_numeric AS REAL)
              AND CAST(value_numeric AS INTEGER) % 1000 = 0
              AND CAST(value_numeric AS INTEGER) BETWEEN 1000 AND 60000
              AND id IN (SELECT fact_id FROM financial_fact_provenance WHERE source_tier = 'pdf')
        )
    ), '[]')
WHERE json_valid(produced_fact_ids_json)
  AND EXISTS (
        SELECT 1 FROM json_each(autopilot_run.produced_fact_ids_json)
        WHERE value IN (
            SELECT id FROM financial_facts
            WHERE definition_id = 'kpidef_cash'
              AND confirmation_state = 'auto_unreviewed'
              AND CAST(value_numeric AS INTEGER) = CAST(value_numeric AS REAL)
              AND CAST(value_numeric AS INTEGER) % 1000 = 0
              AND CAST(value_numeric AS INTEGER) BETWEEN 1000 AND 60000
              AND id IN (SELECT fact_id FROM financial_fact_provenance WHERE source_tier = 'pdf')
        )
  );

UPDATE management_claims
SET verifying_fact_id = NULL
WHERE verifying_fact_id IN (
    SELECT id FROM financial_facts
    WHERE definition_id = 'kpidef_cash'
      AND confirmation_state = 'auto_unreviewed'
      AND CAST(value_numeric AS INTEGER) = CAST(value_numeric AS REAL)
      AND CAST(value_numeric AS INTEGER) % 1000 = 0
      AND CAST(value_numeric AS INTEGER) BETWEEN 1000 AND 60000
      AND id IN (SELECT fact_id FROM financial_fact_provenance WHERE source_tier = 'pdf')
);

UPDATE financial_facts
SET supersedes_id = NULL
WHERE supersedes_id IN (
    SELECT id FROM financial_facts
    WHERE definition_id = 'kpidef_cash'
      AND confirmation_state = 'auto_unreviewed'
      AND CAST(value_numeric AS INTEGER) = CAST(value_numeric AS REAL)
      AND CAST(value_numeric AS INTEGER) % 1000 = 0
      AND CAST(value_numeric AS INTEGER) BETWEEN 1000 AND 60000
      AND id IN (SELECT fact_id FROM financial_fact_provenance WHERE source_tier = 'pdf')
);

-- Delete the facts. Order matches migration 0102: the FACT goes first so no
-- fact is ever left without its provenance row (the invariant read models
-- depend on); the now-orphaned provenance row is removed immediately after.
DELETE FROM financial_facts
WHERE definition_id = 'kpidef_cash'
  AND confirmation_state = 'auto_unreviewed'
  AND CAST(value_numeric AS INTEGER) = CAST(value_numeric AS REAL)
  AND CAST(value_numeric AS INTEGER) % 1000 = 0
  AND CAST(value_numeric AS INTEGER) BETWEEN 1000 AND 60000
  AND id IN (SELECT fact_id FROM financial_fact_provenance WHERE source_tier = 'pdf');

DELETE FROM financial_fact_provenance
WHERE fact_id NOT IN (SELECT id FROM financial_facts);
