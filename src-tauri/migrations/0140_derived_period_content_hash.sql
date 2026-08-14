-- Bind the derived-period cache to the bytes it describes (#385). The cache
-- (migration 0109) is keyed by report_document_id only, while recapture may
-- replace the document's bytes without touching this table — a version-only
-- cache hit can then serve a period derived from DIFFERENT content (including
-- the A→B→A recapture cycle, which restores the original hash on the document
-- row while the cache still describes B). The derivation writer now stamps the
-- content hash of the exact ReportDocument snapshot it derived from, and the
-- cache-hit predicate requires it to match the document's current hash;
-- legacy NULL rows self-heal by re-deriving on their next pipeline read.
ALTER TABLE document_derived_periods ADD COLUMN content_hash TEXT NULL;
