-- Purge queued work for the retired in-app AI job kinds (ADR 0084).
--
-- The in-app AI analysis layer is removed, so no handler is registered for these
-- kinds any more. A durable queue carried over from a pre-removal install still
-- holds `pending`/`running` rows of them — work nothing can ever claim, which
-- would sit in the queue forever and skew the queue counts the diagnostics
-- surface reads.
--
-- Scope is deliberately narrow, per ADR 0084 decision 5 (stored AI output is
-- user data and is never destroyed):
--   * only UNCLAIMABLE statuses (`pending`, `running`) are deleted — terminal
--     rows (`succeeded`, `failed`) are execution history and stay;
--   * only the retired kinds are touched — every surviving kind is untouched;
--   * no domain table is dropped and no domain row is deleted. The per-job
--     status tables (`ai_analysis_jobs`, `kpi_extraction_jobs`, …) and their
--     outputs remain fully readable.
--
-- Forward, idempotent and self-healing: re-running deletes nothing the first run
-- already removed, and a row re-inserted by a stale binary is purged the next
-- time this migration's content runs.
DELETE FROM job_queue
WHERE status IN ('pending', 'running')
  AND kind IN (
    'ai_analysis',
    'claim_extraction',
    'kpi_extraction',
    'research_brief',
    'research_digest',
    'qualitative_assessment'
  );
