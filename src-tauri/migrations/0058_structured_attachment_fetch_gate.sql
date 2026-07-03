-- ADR 0061 decision 1b: a structured ESEF/iXBRL (.xhtml) report attachment is
-- now always a fetch candidate, decided per-attachment rather than gated
-- behind the periodic-report text classifier -- a filing can be xhtml-only
-- under the EU ESEF mandate and still miss the Polish-language
-- periodic-report heuristic. This forward migration flips already-registered
-- structured attachments that were stuck `metadata_only` under the old
-- filing-level gate back to `pending`, so the next attachment-fetch pass
-- (source refresh / on-track backfill) picks them up.
--
-- Append-only, idempotent, self-healing: the WHERE clause only ever matches
-- rows still `metadata_only` with no stored file, so re-running this
-- migration (or applying it to a database in any prior state) converges
-- without re-touching an already-fetched or already-pending document.
-- Digital-signature (.xades) attachments are untouched -- they stay
-- `metadata_only` permanently (no financial data to fetch).
UPDATE report_documents
SET fetch_status = 'pending',
    fetch_error = NULL,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE source_type = 'espi_attachment'
  AND fetch_status = 'metadata_only'
  AND local_path IS NULL
  AND (lower(url) LIKE '%.xhtml' OR lower(url) LIKE '%.xhtml?%');
