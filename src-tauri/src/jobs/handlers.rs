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
/// Job kind: a version-aware re-extraction batch (epic #398 Item B). The
/// payload carries `{batchId}`; the handler re-arms the company's successful
/// ESEF-tier runs whose stored pipeline version is stale, through the same
/// `rearm_run` + stage-queue primitives the history sweep's own re-arm uses —
/// a SEPARATE selector and durable record, so the sweep's "never re-arm an
/// emitted run" rule stays untouched. Defined with the job.
pub use crate::jobs::pipeline_reextraction::PIPELINE_REEXTRACTION_KIND;

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

/// Extract `companyId` from a job payload — the company a failed job's attention
/// event is scoped to (ADR 0091 dec. 1). Mirrors [`payload_adapter_id`]: the
/// payload is the handler's own contract, so reading one field from it needs no
/// storage round-trip.
fn payload_company_id(payload: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()?
        .get("companyId")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

/// The stored report document a failed extraction job was working on, by title —
/// the raw specific that makes the failure event state WHICH report failed
/// (ADR 0087 dec. 4: raw source data, never prose). `None` when the payload names
/// no document or the document is gone.
fn payload_report_document_title(payload: &str, state: &AppState) -> Option<String> {
    let document_id = serde_json::from_str::<serde_json::Value>(payload)
        .ok()?
        .get("reportDocumentId")
        .and_then(|value| value.as_str())?
        .to_owned();
    let title = state.get_report_document(&document_id).ok()?.title?;
    let title = title.trim();
    (!title.is_empty()).then(|| title.to_owned())
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

    fn company_scope(&self, payload: &str) -> Option<String> {
        payload_company_id(payload)
    }

    fn failure_subject(&self, payload: &str, state: &AppState) -> Option<String> {
        payload_report_document_title(payload, state)
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

    fn company_scope(&self, payload: &str) -> Option<String> {
        payload_company_id(payload)
    }

    fn failure_subject(&self, payload: &str, state: &AppState) -> Option<String> {
        payload_report_document_title(payload, state)
    }
}

/// A version-aware re-extraction batch (epic #398 Item B). Runs the batch
/// directly and returns its Result: a storage-level abort returns `Err` (the
/// batch row already recorded the failure, so no user-visible state is lost),
/// while a normal completion (including zero candidates — a settled company)
/// returns `Ok`.
struct PipelineReextractionHandler;

impl JobHandler for PipelineReextractionHandler {
    fn kind(&self) -> &'static str {
        PIPELINE_REEXTRACTION_KIND
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::pipeline_reextraction::run_pipeline_reextraction_job(state, payload)
    }
}

/// One stage of an autopilot run. The handler runs the stage (reusing existing
/// services) and chains the next on success; a fatal stage failure finalizes the
/// run inside [`run_stage`] (still notified), so the handler returns Ok and the
/// job is not retried-looped. A **transient** fetch failure returns `Err` so the
/// queue retries it with backoff (#189, ADR 0055 dec. 2); the last allowed
/// attempt finalizes instead, so a run never dangles over a dead job.
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

    fn company_scope(&self, payload: &str) -> Option<String> {
        payload_company_id(payload)
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

    fn company_scope(&self, payload: &str) -> Option<String> {
        payload_company_id(payload)
    }
}

/// NBP FX daily pull / full-history backfill (ADR 0089 dec. 2). Drains on the
/// **sources** (market-data) lane and serializes on the `nbp-fx` source lock
/// (ADR 0059) so it never races itself. Returns `Err` on a hard fetch failure so
/// the queue retries with backoff; a 404 window (no publication day) is not an
/// error.
struct FxDailyPullHandler;

impl JobHandler for FxDailyPullHandler {
    fn kind(&self) -> &'static str {
        crate::jobs::fx_daily_pull::FX_DAILY_PULL_KIND
    }

    fn serialization_key(&self, _payload: &str) -> Option<String> {
        Some(crate::source_adapters::nbp_fx::ADAPTER_ID.to_owned())
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        crate::jobs::fx_daily_pull::run_fx_daily_pull(state, payload)
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
    worker.register(Arc::new(MorningBriefingHandler));
    worker.register(Arc::new(HistorySweepHandler));
    worker.register(Arc::new(PipelineReextractionHandler));
    worker.register(Arc::new(OwnershipExtractionHandler));
    worker.register(Arc::new(ManagementExtractionHandler));
    worker.register(Arc::new(AutopilotStageHandler));
    worker.register(Arc::new(ScheduledSourceRefreshHandler));
    worker.register(Arc::new(SourceCompanyRefreshHandler));
    worker.register(Arc::new(ScheduledRegistryRefreshHandler));
    worker.register(Arc::new(QuoteBackfillHandler));
    worker.register(Arc::new(CompanyBackfillHandler));
    worker.register(Arc::new(FxDailyPullHandler));
    worker.register(Arc::new(
        crate::jobs::aggregator_fundamentals_pull::AggregatorFundamentalsPullHandler,
    ));
    for handler in crate::jobs::kpi_ingest_queue::handlers() {
        worker.register(handler);
    }
    worker
}

/// The isolated worker-lane layout (ADR 0059): which job kinds each pool drains,
/// and how many threads it runs. Lanes keep a slow source refresh from starving
/// autopilot. Worker counts come from `config` (settings-driven, tolerant
/// defaults). Every registered kind must appear in exactly one lane.
///
/// The morning briefing runs on the autopilot lane (ADR 0084 amending ADR 0059).
///
/// `include_kpi_ingest` gates the `kpi_ingest` lane (#364, fail-closed):
/// startup passes `true` only after the explicit queue reclaim AND the ingest
/// reconciliation both succeeded — on failure the lane stays off until the
/// next launch (pending ingest jobs wait, invisible to every worker; the other
/// lanes run normally). The lane runs a CONSTANT 1 worker (the ADR 0059
/// "indexing stays 1" precedent): both steps are short SQLite-writing work
/// globally serialized by the single writer anyway, so more workers would only
/// manufacture BUSY contention.
pub fn pool_layout(
    config: crate::storage::QueueConfig,
    include_kpi_ingest: bool,
) -> Vec<WorkerPool> {
    use crate::jobs::scheduler::{REGISTRY_REFRESH_KIND, SOURCE_REFRESH_KIND};
    use crate::jobs::source_refresh::SOURCE_COMPANY_REFRESH_KIND;
    let mut pools = vec![
        WorkerPool {
            name: "sources",
            kinds: vec![
                SOURCE_REFRESH_KIND,
                SOURCE_COMPANY_REFRESH_KIND,
                REGISTRY_REFRESH_KIND,
                crate::jobs::quote_backfill::QUOTE_BACKFILL_KIND,
                COMPANY_BACKFILL_KIND,
                // BiznesRadar-primary fundamentals pull: shares the BiznesRadar
                // host politeness posture with the other source fetches; serializes
                // on its own adapter id (ADR 0059 / ADR 0086).
                crate::jobs::aggregator_fundamentals_pull::AGGREGATOR_FUNDAMENTALS_PULL_KIND,
                // NBP FX pull/backfill: a small latency-tolerant external pull on
                // the same market-data lane (ADR 0089 dec. 2 / ADR 0059);
                // serializes on its own `nbp-fx` adapter id.
                crate::jobs::fx_daily_pull::FX_DAILY_PULL_KIND,
            ],
            workers: config.sources_workers.max(1) as usize,
        },
        WorkerPool {
            name: "autopilot",
            kinds: vec![
                AUTOPILOT_STAGE_KIND,
                HISTORY_SWEEP_KIND,
                PIPELINE_REEXTRACTION_KIND,
                OWNERSHIP_EXTRACTION_KIND,
                MANAGEMENT_EXTRACTION_KIND,
                MORNING_BRIEFING_KIND,
            ],
            workers: config.autopilot_workers.max(1) as usize,
        },
    ];
    if include_kpi_ingest {
        pools.push(WorkerPool {
            name: "kpi_ingest",
            kinds: vec![
                crate::jobs::kpi_ingest_queue::KPI_INGEST_VALIDATE_KIND,
                crate::jobs::kpi_ingest_queue::KPI_INGEST_COMMIT_KIND,
            ],
            workers: 1,
        });
    }
    pools
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

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
        for pool in pool_layout(state.queue_config(), true) {
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

        // Fail-closed mode (#364): with the kpi_ingest lane gated off, exactly
        // the two ingest kinds are missing — no other lane silently absorbs
        // them, and no other kind is lost.
        let mut disabled_kinds: std::collections::BTreeSet<&str> = Default::default();
        for pool in pool_layout(state.queue_config(), false) {
            disabled_kinds.extend(pool.kinds);
        }
        let missing: Vec<&str> = registered.difference(&disabled_kinds).copied().collect();
        assert_eq!(
            missing,
            vec![
                crate::jobs::kpi_ingest_queue::KPI_INGEST_COMMIT_KIND,
                crate::jobs::kpi_ingest_queue::KPI_INGEST_VALIDATE_KIND,
            ],
            "disabling the kpi_ingest lane must remove exactly the two ingest kinds"
        );
    }

    #[test]
    fn extraction_handlers_name_the_document_and_company_a_failure_was_about() {
        // ADR 0091 dec. 1: a failure event states RAW specifics. The extraction
        // handlers' payload carries only ids, so `failure_subject` resolves the
        // stored document's own title (never composed prose) and `company_scope`
        // reads the payload's company — the two data the Today row renders as
        // "<ticker> — <report title>".
        use crate::storage::{CaptureReportDocumentInput, NewCompany};

        let state = AppState::new(open_in_memory_database().expect("db"));
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "official_report".to_owned(),
                url: "https://example.test/raport-q2-2026.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Skonsolidowany raport kwartalny Q2 2026".to_owned()),
                attribution: None,
            })
            .expect("document");
        let payload = format!(
            "{{\"companyId\":\"{}\",\"reportDocumentId\":\"{}\"}}",
            company.id, document.id
        );

        for handler in [
            Box::new(OwnershipExtractionHandler) as Box<dyn JobHandler>,
            Box::new(ManagementExtractionHandler) as Box<dyn JobHandler>,
        ] {
            assert_eq!(
                handler.company_scope(&payload).as_deref(),
                Some(company.id.as_str()),
                "{} scopes its failure to the payload's company",
                handler.kind()
            );
            assert_eq!(
                handler.failure_subject(&payload, &state).as_deref(),
                Some("Skonsolidowany raport kwartalny Q2 2026"),
                "{} names the report it failed on",
                handler.kind()
            );
            // A payload with no document (or a pruned one) degrades to no subject —
            // the read model then states the job's own last_error.
            assert_eq!(
                handler.failure_subject("{\"companyId\":\"c1\"}", &state),
                None
            );
        }
    }

    #[test]
    fn every_registered_kind_dispatches_without_unknown_handler() {
        // A migrated fire-and-forget job has a handler, so an enqueued job is
        // claimed + dispatched (and its handler returns Ok after handling a
        // missing-row failure on the bogus id) — never left pending or terminally
        // failed as an unknown kind. Uses the deterministic morning briefing (the
        // AI per-job kinds are retired, ADR 0084).
        let state = AppState::new(open_in_memory_database().expect("db"));
        let kind = MORNING_BRIEFING_KIND;
        state
            .jobs()
            .enqueue(&format!("{kind}:bogus"), kind, "{\"force\":true}", 1)
            .expect("enqueue");

        let worker = build_worker(state.clone());
        let processed = worker.run_until_idle().expect("drain");

        assert_eq!(processed, 1);
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.pending, 0);
        assert_eq!(counts.failed, 0, "handler found and ran (no unknown-kind)");
        assert_eq!(counts.succeeded, 1);
    }
}
