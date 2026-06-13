-- Per-company investor-relations reports page URL (v0.36.0, ADR 0029).
-- Durable, user-editable. Used as a fallback source for report documents when an
-- ESPI/EBI filing carries no usable attachment; the AI-assisted resolver locates
-- the specific report on this page from the event context.
ALTER TABLE companies ADD COLUMN ir_reports_url TEXT;
