-- Index for the qualitative current-state read (v0.50.0, ADR 0075). The Quality
-- panel's get_qualitative_assessment resolves, per criterion, the most-recent
-- agent-assessed row across all snapshots via a correlated lookup keyed on
-- (criterion_id, source='agent'). Without an index that filter scans every
-- criterion_results row, so the read grows quadratically with a company's
-- assessment history on a UI-load path. Append-only + idempotent (IF NOT EXISTS).
CREATE INDEX IF NOT EXISTS idx_criterion_results_agent
ON criterion_results(criterion_id, source);
