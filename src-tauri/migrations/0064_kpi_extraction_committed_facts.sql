-- Committed-fact count on a KPI-extraction job (ADR 0077 §4, T4.5).
--
-- The manual "Extract KPIs" job is rewired to the tier-4 OCR path: when a
-- confirmed OCR profile parses to a VALIDATED set, the job commits facts
-- directly (not proposals) and completes with zero proposals. Without an honest
-- count the review panel would read "0 KPI values extracted" for a run that
-- actually committed facts. This column records how many validated facts the
-- run committed so the UI can render the outcome truthfully.
--
-- Append-only: the version-tracked runner applies each migration exactly once,
-- so a plain ADD COLUMN is safe; existing rows default to 0 (the historical
-- proposals-only path committed no facts), and reads tolerate the default.
ALTER TABLE kpi_extraction_jobs
    ADD COLUMN committed_fact_count INTEGER NOT NULL DEFAULT 0;
