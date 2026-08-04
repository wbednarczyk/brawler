-- Fix #318: durable attachment-unit provenance makes attachment merges
-- independent of how parsed documents are grouped into calls.
--
-- This migration is additive and append-only. The migration runner applies each
-- version at most once; the index is additionally guarded so schema repair can
-- safely re-run its index statement.
ALTER TABLE insider_transactions ADD COLUMN source_document_id TEXT;
ALTER TABLE insider_transactions ADD COLUMN source_unit_ord INTEGER;

CREATE UNIQUE INDEX IF NOT EXISTS idx_insider_tx_provenance
    ON insider_transactions(feed_item_id, source_document_id, source_unit_ord)
    WHERE source_document_id IS NOT NULL;
