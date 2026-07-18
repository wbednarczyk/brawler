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
use crate::jobs::queue::{JobHandler, JobWorker, WorkerPool};

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
/// Job kind: automatic per-company report-history backfill (v0.57 catch-up, ADR
/// 0077 amendment). Payload `{companyId}`; the handler paginates the Bankier
/// company listing back N years and ingests periodic reports through the normal
/// path, then chains a history sweep. Drains on the **sources** lane and
/// serializes on the Bankier-company source lock. Defined with the job.
pub use crate::jobs::backfill::COMPANY_BACKFILL_KIND;
/// Job kind: run a history sweep (ADR 0077 §3). The payload carries `{sweepId}`;
/// the handler enqueues a full autopilot run for every canonical periodic report
/// whose period lacks accepted facts, through the shared `enqueue_extraction_run`.
pub use crate::jobs::history_sweep::HISTORY_SWEEP_KIND;
/// Job kind: extract the management-holdings section of one stored periodic report
/// into `management_holdings` + stamp founder/insiders (ADR 0083 D6, v0.57 T5). The
/// payload carries `{companyId, reportDocumentId}`; the handler runs the
/// deterministic parser and writes rows directly, or records a residual for the
/// AI/OCR path. A deterministic CPU parse chained from ingestion — assigned to the
/// **autopilot** lane, never a provider call. Defined with the job.
pub use crate::jobs::management_holdings_extraction::MANAGEMENT_EXTRACTION_KIND;
/// Job kind: compose a morning briefing (ADR 0068 decision 4, v0.54.0). The
/// payload carries `{force}`; the handler runs the deterministic composer + an
/// optional narrative and persists the briefing. Assigned to the **ai** lane
/// (the narrative is a provider call). Defined with the job.
pub use crate::jobs::morning_briefing::MORNING_BRIEFING_KIND;
/// Job kind: extract the shareholders table of one stored periodic report into
/// `ownership_stakes` (ADR 0072, v0.56.0 T3). The payload carries
/// `{companyId, reportDocumentId}`; the handler runs the deterministic parser and
/// writes stakes directly, or records a residual for the AI/OCR path. A
/// deterministic CPU parse chained from ingestion — assigned to the **autopilot**
/// lane (the history-sweep family), never a provider call. Defined with the job.
pub use crate::jobs::ownership_extraction::OWNERSHIP_EXTRACTION_KIND;
/// Job kind: assess qualitative quality-framework criteria (ADR 0075, v0.50.0).
/// The payload carries `{companyId, frameworkId, criterionIds?}`; the handler
/// gathers evidence, assesses each criterion through the capability pool, and
/// writes `source = agent` criterion results. Defined with the job.
pub use crate::jobs::qualitative_assessment::QUALITATIVE_ASSESSMENT_KIND;

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
/// KPI extraction departs from the `per_job_handler` shape (T5.1, ADR 0077
/// pacing fix): a transient provider failure (429 rate limit, temporary
/// unavailability, network error) propagates as `Err` so the queue's capped
/// backoff retry (2..64s, [`crate::jobs::queue::retry_backoff_seconds`])
/// engages instead of killing the job on the first 429 — the runner has
/// already left the domain row re-runnable and recorded a `retry_scheduled`
/// diagnostic. Every other failure keeps the prior semantics exactly: the
/// domain row is marked failed and the queue row completes (no retry).
/// [`enqueue_per_job`] gives this kind a retry budget > 1.
struct KpiExtractionHandler;

impl JobHandler for KpiExtractionHandler {
    fn kind(&self) -> &'static str {
        KPI_EXTRACTION_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        use crate::jobs::kpi_extraction::KpiExtractionJobError;
        match crate::jobs::kpi_extraction::run_kpi_extraction_job(state, payload) {
            Ok(_) => Ok(()),
            Err(KpiExtractionJobError::TransientRetryScheduled(message)) => Err(message),
            Err(KpiExtractionJobError::Internal(error)) => {
                let _ = state.mark_kpi_extraction_job_failed(payload, "unknown", &error);
                Ok(())
            }
        }
    }
}

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

/// Qualitative assessment (ADR 0075). Runs the job directly and returns its
/// Result: a provider/parse/citation failure returns `Err` so the queue retries
/// with backoff, and nothing is persisted unless every criterion produced a
/// valid, cited result. No per-job status table (the read model is the
/// evaluation snapshots it writes).
struct QualitativeAssessmentHandler;

impl JobHandler for QualitativeAssessmentHandler {
    fn kind(&self) -> &'static str {
        QUALITATIVE_ASSESSMENT_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::qualitative_assessment::run_qualitative_assessment_job(state, payload)
    }
}

/// A morning briefing (ADR 0068 decision 4). Runs the composer + optional
/// narrative and returns its Result: a storage-level failure returns `Err` (the
/// queue may retry), while a missing/failed narrative provider is NOT an error
/// (the briefing completes as a structured list) and returns `Ok`.
struct MorningBriefingHandler;

impl JobHandler for MorningBriefingHandler {
    fn kind(&self) -> &'static str {
        MORNING_BRIEFING_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::morning_briefing::run_morning_briefing_job(state, payload)
    }
}

/// A history sweep (ADR 0077 §3). Runs the sweep directly and returns its Result:
/// a storage-level abort returns `Err` so the queue retries with backoff, while
/// domain outcomes (off-mode skip, per-candidate counts including runs that could
/// not be enqueued) are recorded on the sweep row and return `Ok`.
struct HistorySweepHandler;

impl JobHandler for HistorySweepHandler {
    fn kind(&self) -> &'static str {
        HISTORY_SWEEP_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::history_sweep::run_history_sweep_job(state, payload)
    }
}

/// Ownership extraction (ADR 0072, v0.56 T3). Runs the deterministic parse and
/// returns its Result: a storage-level abort returns `Err` so the queue records
/// it, while domain outcomes (a residual recorded, a missing/unfetched document
/// skipped) return `Ok`. Deterministic — no per-job status table (its read model
/// is the stakes/residual rows it writes).
struct OwnershipExtractionHandler;

impl JobHandler for OwnershipExtractionHandler {
    fn kind(&self) -> &'static str {
        OWNERSHIP_EXTRACTION_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::ownership_extraction::run_ownership_extraction_job(state, payload)
    }
}

/// Management-holdings extraction (ADR 0083 D6, v0.57 T5). Deterministic sibling of
/// the ownership handler: writes holdings + stamps founders, or parks a residual.
struct ManagementExtractionHandler;

impl JobHandler for ManagementExtractionHandler {
    fn kind(&self) -> &'static str {
        MANAGEMENT_EXTRACTION_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::management_holdings_extraction::run_management_extraction_job(state, payload)
    }
}

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

/// Extract the `adapterId` from a source-refresh payload — the per-source
/// serialization key (ADR 0059). Both the scheduled full refresh and the
/// per-company refresh key on the same adapter id, so all work for one source
/// serializes (politeness / no duplicate concurrent work).
fn payload_adapter_id(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()?
        .get("adapterId")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
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

    fn serialization_key(&self, payload: &str) -> Option<String> {
        payload_adapter_id(payload)
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        let adapter_id =
            payload_adapter_id(payload).ok_or("scheduled source refresh missing adapterId")?;
        crate::jobs::source_refresh::refresh_source_for_trigger(
            state,
            &adapter_id,
            "scheduler",
            None,
        )
        .map(|_| ())
    }
}

/// A per-company refresh for a company-scoped source (ADR 0059). Planned by the
/// scheduled refresh (one job per tracked company) so a slow all-companies loop can
/// no longer monopolize the worker. Serializes on the adapter id, so at most one
/// company of the source refreshes at a time. Returns `Err` on failure to retry.
struct SourceCompanyRefreshHandler;

impl JobHandler for SourceCompanyRefreshHandler {
    fn kind(&self) -> &'static str {
        crate::jobs::source_refresh::SOURCE_COMPANY_REFRESH_KIND
    }

    fn serialization_key(&self, payload: &str) -> Option<String> {
        payload_adapter_id(payload)
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::source_refresh::run_source_company_refresh(state, payload)
    }
}

/// A full-history quote backfill for one company (ADR 0082, v0.53 T2). Runs
/// on company add / on `yahoo-eod` being enabled (planned via
/// `jobs::quote_backfill::enqueue_backfill_for_company`). Serializes on the
/// `yahoo-eod` source lock (ADR 0059) so it never races the scheduled daily
/// pull for the same source. Returns `Err` on failure so the queue retries
/// with backoff.
struct QuoteBackfillHandler;

impl JobHandler for QuoteBackfillHandler {
    fn kind(&self) -> &'static str {
        crate::jobs::quote_backfill::QUOTE_BACKFILL_KIND
    }

    fn serialization_key(&self, _payload: &str) -> Option<String> {
        Some(crate::jobs::quote_backfill::YAHOO_ADAPTER_ID.to_owned())
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::quote_backfill::run_quote_backfill_job(state, payload)
    }
}

/// An automatic per-company report-history backfill (v0.57 catch-up, ADR 0077
/// amendment). Runs [`crate::jobs::backfill::backfill_company_history`] for one
/// company through the durable queue instead of the manual IPC path, so a cold or
/// persisted-but-stale DB fills its report history without the user clicking.
/// Serializes on the Bankier-company source lock (ADR 0059) so it never races the
/// scheduled per-company refresh. Returns `Ok` on any domain outcome (the progress
/// row + chained sweep carry the result); a malformed payload returns `Err`.
struct CompanyBackfillHandler;

impl JobHandler for CompanyBackfillHandler {
    fn kind(&self) -> &'static str {
        COMPANY_BACKFILL_KIND
    }

    fn serialization_key(&self, _payload: &str) -> Option<String> {
        Some(crate::source_adapters::bankier_company::ADAPTER_ID.to_owned())
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::backfill::run_company_backfill_job(state, payload)
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
    worker.register(Arc::new(AiAnalysisHandler));
    worker.register(Arc::new(ClaimExtractionHandler));
    worker.register(Arc::new(KpiExtractionHandler));
    worker.register(Arc::new(ResearchBriefHandler));
    worker.register(Arc::new(ResearchDigestHandler));
    worker.register(Arc::new(QualitativeAssessmentHandler));
    worker.register(Arc::new(MorningBriefingHandler));
    worker.register(Arc::new(HistorySweepHandler));
    worker.register(Arc::new(OwnershipExtractionHandler));
    worker.register(Arc::new(ManagementExtractionHandler));
    worker.register(Arc::new(AutopilotStageHandler));
    worker.register(Arc::new(ScheduledSourceRefreshHandler));
    worker.register(Arc::new(SourceCompanyRefreshHandler));
    worker.register(Arc::new(ScheduledRegistryRefreshHandler));
    worker.register(Arc::new(QuoteBackfillHandler));
    worker.register(Arc::new(CompanyBackfillHandler));
    worker
}

/// The isolated worker-lane layout (ADR 0059): which job kinds each pool drains,
/// and how many threads it runs. Lanes keep a slow source refresh from starving
/// autopilot; the shared per-provider AI limit (not the thread count) is the real
/// ceiling on AI cost, so generous defaults are safe. Worker counts come from
/// `config` (settings-driven, tolerant defaults). Every registered kind must appear
/// in exactly one lane.
pub fn pool_layout(config: crate::storage::QueueConfig) -> Vec<WorkerPool> {
    use crate::jobs::scheduler::{REGISTRY_REFRESH_KIND, SOURCE_REFRESH_KIND};
    use crate::jobs::source_refresh::SOURCE_COMPANY_REFRESH_KIND;
    vec![
        WorkerPool {
            name: "sources",
            kinds: vec![
                SOURCE_REFRESH_KIND,
                SOURCE_COMPANY_REFRESH_KIND,
                REGISTRY_REFRESH_KIND,
                crate::jobs::quote_backfill::QUOTE_BACKFILL_KIND,
                COMPANY_BACKFILL_KIND,
            ],
            workers: config.sources_workers.max(1) as usize,
        },
        WorkerPool {
            name: "autopilot",
            kinds: vec![
                AUTOPILOT_STAGE_KIND,
                HISTORY_SWEEP_KIND,
                OWNERSHIP_EXTRACTION_KIND,
                MANAGEMENT_EXTRACTION_KIND,
            ],
            workers: config.autopilot_workers.max(1) as usize,
        },
        WorkerPool {
            name: "ai",
            kinds: vec![
                AI_ANALYSIS_KIND,
                KPI_EXTRACTION_KIND,
                CLAIM_EXTRACTION_KIND,
                RESEARCH_BRIEF_KIND,
                RESEARCH_DIGEST_KIND,
                QUALITATIVE_ASSESSMENT_KIND,
                MORNING_BRIEFING_KIND,
            ],
            workers: config.ai_workers.max(1) as usize,
        },
    ]
}

/// Enqueue a per-job-table job onto the durable queue, keyed by the job's own
/// id (attempts budget per kind — see [`per_job_max_attempts`]). Replaces the
/// prior fire-and-forget `spawn_blocking`; the
/// worker runs the handler, so a crash mid-run resumes. Logs and drops on
/// enqueue error (best-effort, matching the prior detached spawn).
///
/// Uses `reschedule`, not plain `enqueue`: every `per_job_handler` (ai_analysis,
/// claim_extraction, research_brief, research_digest) always returns `Ok` — and
/// [`KpiExtractionHandler`] does too except while a transient retry is pending —
/// so the `job_queue` row ends `succeeded` regardless of the *domain* outcome
/// recorded in the job's own table — that is precisely what
/// lets a domain job land `failed` while its `job_queue` row is already terminal.
/// The `retry_*` commands (`retry_kpi_extraction`, `retry_claim_extraction`,
/// `retry_ai_analysis`) then re-enqueue under the **same** `job_id`; plain
/// `enqueue` (`INSERT OR IGNORE`) would silently no-op against that already-
/// `succeeded` row and the retry would never actually run (bug class dce9ce8).
/// `reschedule` re-arms a terminal row to `pending` and leaves a `running` row
/// untouched (never double-run); a fresh `job_id` is inserted exactly as before.
pub fn enqueue_per_job(state: &AppState, kind: &'static str, job_id: &str) {
    if let Err(error) = state
        .jobs()
        .reschedule(job_id, kind, job_id, per_job_max_attempts(kind))
    {
        log::warn!("failed to enqueue {kind} job {job_id}: {error}");
    }
}

/// Queue attempts budget for kpi_extraction: 1 first run + 4 backoff retries
/// (2/4/8/16s waits) for transient provider failures — enough to ride out a
/// short 429 window without hammering a rate-limited provider (T5.1, ADR 0077).
const KPI_EXTRACTION_MAX_ATTEMPTS: i64 = 5;

/// Retry budget per per-job kind. Only kpi_extraction gets queue-level backoff
/// retries (its handler returns `Err` on transient provider failures); every
/// other per-job kind keeps the original single-attempt semantics — their
/// handlers always return `Ok`, so a larger budget would be dead config.
fn per_job_max_attempts(kind: &str) -> i64 {
    if kind == KPI_EXTRACTION_KIND {
        KPI_EXTRACTION_MAX_ATTEMPTS
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    #[test]
    fn enqueue_per_job_rearms_a_stale_terminal_row_under_the_same_id() {
        // Bug class dce9ce8: every `per_job_handler` always returns `Ok` (the
        // domain outcome lives in its own job table), so its `job_queue` row
        // always ends `succeeded` — even when the domain job itself failed. The
        // `retry_*` commands then re-enqueue under the SAME job id; if that used
        // plain `enqueue` (`INSERT OR IGNORE`), the already-`succeeded` row would
        // silently swallow the retry forever.
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_per_job(&state, KPI_EXTRACTION_KIND, "job-1");
        let worker = build_worker(state.clone());
        assert!(worker.process_one().expect("process one"));
        assert_eq!(state.jobs().counts().expect("counts").succeeded, 1);

        // Retry: same job id, same kind.
        enqueue_per_job(&state, KPI_EXTRACTION_KIND, "job-1");
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.pending, 1,
            "the retry re-arms the terminal row instead of silently no-opping"
        );
        assert_eq!(counts.succeeded, 0);

        assert!(
            worker.process_one().expect("process one"),
            "the re-armed job is actually dispatched again"
        );
    }

    #[test]
    fn every_registered_kind_is_in_exactly_one_lane() {
        // Guardrail (ADR 0059 invariant, made executable for T5): every job kind
        // registered in `build_worker` must be drained by exactly one lane in
        // `pool_layout`, and no lane may list a kind with no handler. Without this,
        // a new kind (like `morning_briefing`) could be registered but never
        // assigned to a lane — enqueued jobs would sit pending forever.
        let state = AppState::new(open_in_memory_database().expect("db"));
        let worker = build_worker(state.clone());
        let registered: std::collections::BTreeSet<&str> =
            worker.registered_kinds().into_iter().collect();

        let mut lane_kinds: Vec<&str> = Vec::new();
        for pool in pool_layout(state.queue_config()) {
            lane_kinds.extend(pool.kinds);
        }
        let unique_lane_kinds: std::collections::BTreeSet<&str> =
            lane_kinds.iter().copied().collect();

        assert_eq!(
            lane_kinds.len(),
            unique_lane_kinds.len(),
            "a job kind is assigned to more than one lane"
        );
        assert_eq!(
            registered, unique_lane_kinds,
            "every registered kind must appear in exactly one lane (and vice versa)"
        );
        assert!(
            unique_lane_kinds.contains(MORNING_BRIEFING_KIND),
            "morning_briefing must be assigned to a lane"
        );
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
