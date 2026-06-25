//! Job handlers + worker assembly for the durable queue (Architecture v2 / ADR 0050).
//!
//! Each [`JobHandler`] adapts an existing job's logic to the queue: it claims one
//! `kind`, deserializes the payload, and runs the work. [`build_worker`] is the
//! single place handlers are registered, so app startup just builds the worker,
//! reclaims crash residue, and spawns the loop. Fire-and-forget `spawn_blocking`
//! jobs migrate onto the queue by adding a handler here + enqueuing instead of
//! spawning.

use std::sync::Arc;

use crate::app_state::AppState;
use crate::jobs::queue::{JobHandler, JobWorker};

/// Job kind: refresh the disposable embedding index (ADR 0035). Idempotent —
/// skips unchanged content — so retry/resume is safe.
pub const CONTENT_EMBEDDING_KIND: &str = "content_embedding";
/// Job kind: run an AI analysis for a feed item (status tracked in `ai_analysis_jobs`).
pub const AI_ANALYSIS_KIND: &str = "ai_analysis";
/// Job kind: extract management claims for a source (status in `claim_extraction_jobs`).
pub const CLAIM_EXTRACTION_KIND: &str = "claim_extraction";
/// Job kind: extract KPI proposals for a document (status in `kpi_extraction_jobs`).
pub const KPI_EXTRACTION_KIND: &str = "kpi_extraction";
/// Job kind: generate a research brief (status in `research_brief_jobs`).
pub const RESEARCH_BRIEF_KIND: &str = "research_brief";
/// Job kind: generate a research digest (status in `research_digest_jobs`).
pub const RESEARCH_DIGEST_KIND: &str = "research_digest";
/// Job kind: one stage of an autopilot run (North Star, v0.49.0 / ADR 0055). The
/// payload carries `{run_id, stage}`; the handler runs that stage and chains the
/// next, so a crash mid-stage resumes that stage only.
pub use crate::jobs::autopilot::AUTOPILOT_STAGE_KIND;

struct ContentEmbeddingHandler;

impl JobHandler for ContentEmbeddingHandler {
    fn kind(&self) -> &'static str {
        CONTENT_EMBEDDING_KIND
    }

    fn run(&self, _payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::content_embedding::run_content_embedding_job(state).map(|_outcome| ())
    }
}

/// Handlers for the user-initiated AI jobs. Each preserves the prior
/// fire-and-forget behavior exactly: run the job (which updates its own
/// per-job status table that the UI polls), and on error mark that table
/// failed. Returning `Ok` means "the job executed" — the domain outcome lives
/// in the per-job table, and there is no queue-level retry (single attempt, as
/// before); the gain is crash-resumability (a job left running is re-run).
macro_rules! per_job_handler {
    ($name:ident, $kind:expr, $run:path, $mark_failed:ident, $reason:expr) => {
        struct $name;
        impl JobHandler for $name {
            fn kind(&self) -> &'static str {
                $kind
            }
            fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
                if let Err(error) = $run(state, payload) {
                    let _ = state.$mark_failed(payload, $reason, &error);
                }
                Ok(())
            }
        }
    };
}

per_job_handler!(
    AiAnalysisHandler,
    AI_ANALYSIS_KIND,
    crate::jobs::ai_analysis::run_ai_analysis_job,
    mark_ai_analysis_job_failed,
    "unknown"
);
per_job_handler!(
    ClaimExtractionHandler,
    CLAIM_EXTRACTION_KIND,
    crate::jobs::claim_extraction::run_claim_extraction_job,
    mark_claim_extraction_job_failed,
    "extraction_failed"
);
per_job_handler!(
    KpiExtractionHandler,
    KPI_EXTRACTION_KIND,
    crate::jobs::kpi_extraction::run_kpi_extraction_job,
    mark_kpi_extraction_job_failed,
    "unknown"
);
per_job_handler!(
    ResearchBriefHandler,
    RESEARCH_BRIEF_KIND,
    crate::jobs::research_briefs::run_research_brief_job,
    mark_research_brief_job_failed,
    "unknown"
);
per_job_handler!(
    ResearchDigestHandler,
    RESEARCH_DIGEST_KIND,
    crate::jobs::research_digests::run_research_digest_job,
    mark_research_digest_job_failed,
    "unknown"
);

/// One stage of an autopilot run. The handler runs the stage (reusing existing
/// services) and chains the next on success; a fatal stage failure finalizes the
/// run inside [`run_stage`] (still notified), so the handler returns Ok and the
/// job is not retried-looped.
struct AutopilotStageHandler;

impl JobHandler for AutopilotStageHandler {
    fn kind(&self) -> &'static str {
        AUTOPILOT_STAGE_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::autopilot::run_stage(state, payload)
    }
}

/// A scheduled source-adapter refresh (Rust-side scheduler, ADR 0055 / AV5). The
/// scheduler re-arms this job per the poll interval; the worker executes the
/// refresh (detection rides its completion). Returns `Err` on failure so the queue
/// retries with backoff.
struct ScheduledSourceRefreshHandler;

impl JobHandler for ScheduledSourceRefreshHandler {
    fn kind(&self) -> &'static str {
        crate::jobs::scheduler::SOURCE_REFRESH_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        let parsed: serde_json::Value =
            serde_json::from_str(payload).map_err(|error| error.to_string())?;
        let adapter_id = parsed
            .get("adapterId")
            .and_then(|value| value.as_str())
            .ok_or("scheduled source refresh missing adapterId")?;
        crate::jobs::source_refresh::refresh_source_for_trigger(
            state,
            adapter_id,
            "scheduler",
            None,
        )
        .map(|_| ())
    }
}

/// A scheduled company-registry refresh-if-stale check (Rust-side scheduler).
struct ScheduledRegistryRefreshHandler;

impl JobHandler for ScheduledRegistryRefreshHandler {
    fn kind(&self) -> &'static str {
        crate::jobs::scheduler::REGISTRY_REFRESH_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        let stale_after_seconds = serde_json::from_str::<serde_json::Value>(payload)
            .ok()
            .and_then(|value| value.get("staleAfterSeconds").and_then(|v| v.as_i64()))
            .unwrap_or(86_400);
        if state
            .company_directories_are_stale(stale_after_seconds)
            .map_err(|error| error.to_string())?
        {
            crate::jobs::source_refresh::refresh_company_directories_for_trigger(
                state,
                "scheduler",
            )
            .map(|_| ())?;
        }
        Ok(())
    }
}

/// Build the durable-queue worker with every registered handler. Startup calls
/// this, then [`crate::jobs::queue::spawn`] to reclaim residue and run the loop.
pub fn build_worker(state: AppState) -> JobWorker {
    let mut worker = JobWorker::new(state);
    worker.register(Arc::new(ContentEmbeddingHandler));
    worker.register(Arc::new(AiAnalysisHandler));
    worker.register(Arc::new(ClaimExtractionHandler));
    worker.register(Arc::new(KpiExtractionHandler));
    worker.register(Arc::new(ResearchBriefHandler));
    worker.register(Arc::new(ResearchDigestHandler));
    worker.register(Arc::new(AutopilotStageHandler));
    worker.register(Arc::new(ScheduledSourceRefreshHandler));
    worker.register(Arc::new(ScheduledRegistryRefreshHandler));
    worker
}

/// Enqueue a per-job-table job onto the durable queue (single attempt, dedup by
/// the job's own id). Replaces the prior fire-and-forget `spawn_blocking`; the
/// worker runs the handler, so a crash mid-run resumes. Logs and drops on
/// enqueue error (best-effort, matching the prior detached spawn).
pub fn enqueue_per_job(state: &AppState, kind: &'static str, job_id: &str) {
    if let Err(error) = state.jobs().enqueue(job_id, kind, job_id, 1) {
        log::warn!("failed to enqueue {kind} job {job_id}: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    #[test]
    fn worker_runs_the_registered_content_embedding_handler() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        // Default similarity strategy is static, so the embedding job is a no-op
        // success — enough to prove the handler is registered and dispatched (not
        // an "unknown kind" terminal failure).
        state
            .jobs()
            .enqueue(CONTENT_EMBEDDING_KIND, CONTENT_EMBEDDING_KIND, "{}", 3)
            .expect("enqueue");

        let worker = build_worker(state.clone());
        assert!(worker.process_one().expect("process one"));
        assert_eq!(state.jobs().counts().expect("counts").succeeded, 1);
    }

    #[test]
    fn every_registered_kind_dispatches_without_unknown_handler() {
        // Each migrated fire-and-forget job has a handler, so an enqueued job is
        // claimed + dispatched (and its handler returns Ok after handling a
        // missing-row failure on the bogus id) — never left pending or terminally
        // failed as an unknown kind.
        let state = AppState::new(open_in_memory_database().expect("db"));
        for kind in [
            AI_ANALYSIS_KIND,
            CLAIM_EXTRACTION_KIND,
            KPI_EXTRACTION_KIND,
            RESEARCH_BRIEF_KIND,
            RESEARCH_DIGEST_KIND,
        ] {
            state
                .jobs()
                .enqueue(&format!("{kind}:bogus"), kind, "bogus", 1)
                .expect("enqueue");
        }

        let worker = build_worker(state.clone());
        let processed = worker.run_until_idle().expect("drain");

        assert_eq!(processed, 5);
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.pending, 0);
        assert_eq!(counts.failed, 0, "handlers found and ran (no unknown-kind)");
        assert_eq!(counts.succeeded, 5);
    }
}
