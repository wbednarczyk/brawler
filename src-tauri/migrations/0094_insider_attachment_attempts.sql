-- v0.57 Company Health — insider attachment-PDF tier (ADR 0083 Decision 6 + the
-- 2026-07-17 ground-truth amendment; plan v0.57 T4b; data-model § Company Health).
-- Forward-only / idempotent / self-healing (data-model migration rules).
--
-- The MAR art. 19 transaction figures (volume / price / currency / tx_date /
-- instrument / direction) live in the attached "Powiadomienie…" notification
-- document, not the Bankier cover note. The attachment documents are ALREADY
-- registered at ingest as `report_documents` (source_type='espi_attachment',
-- origin_ref=<feed_item_id>, fetch_status='metadata_only') — T4b needs NO new
-- attachment table: it fetches those rows on demand (the existing report-document
-- fetch infra) and deterministically parses them to FILL the NULL columns of
-- `insider_transactions` (migration 0090).
--
-- This migration adds only the **attempt-once marker** (mirrors the
-- `insider_espi_unparsed` idiom from migration 0090): a classified insider filing
-- whose attachment tier has been attempted terminally is recorded here once, so the
-- fetch+parse sweep attempts each filing exactly once and re-runs create zero new
-- rows / issue zero new fetches. A transient fetch failure is NOT recorded (the
-- report_documents row stays retryable), so it is retried on the next sweep.
--
-- The counts columns surface the merge diagnostics (filled / appended / conflicts)
-- per filing for the closure report — no separate diagnostics table.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS insider_attachment_attempts (
    -- One row per classified insider filing whose attachment tier reached a
    -- terminal outcome. PK = feed item (a filing is attempted once).
    feed_item_id TEXT PRIMARY KEY REFERENCES feed_items(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- Terminal outcome: parsed | no_attachment | no_text_layer | not_found.
    --   parsed        — a notification document parsed; NULLs filled / rows appended.
    --   no_attachment — the filing carries no fetchable notification document.
    --   no_text_layer — the document was fetched but is scanned/unreadable (parked
    --                   for the vision path; never guessed).
    --   not_found     — the document was read but held no recognizable MAR art. 19
    --                   notification form.
    outcome TEXT NOT NULL CHECK(outcome IN ('parsed', 'no_attachment', 'no_text_layer', 'not_found')),
    -- Merge diagnostics for the `parsed` outcome (0 otherwise): NULLs filled on
    -- matched units, rows appended, and CONFLICTs where the PDF disagreed with an
    -- existing non-NULL value (nothing was overwritten — the conflict is recorded).
    filled INTEGER NOT NULL DEFAULT 0,
    appended INTEGER NOT NULL DEFAULT 0,
    conflicts INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_insider_attachment_attempts_company
    ON insider_attachment_attempts(company_id);
