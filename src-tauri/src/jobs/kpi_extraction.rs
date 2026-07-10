//! Async KPI extraction job (v0.36.0, epic 9879941; rewired to tier-4 in ADR
//! 0077 §4 / T4.5).
//!
//! The manual "Extract KPIs" job is now **one implementation** with the tier-4
//! OCR path ([`crate::jobs::structured_extraction::run_tier4_extraction`]): the
//! LLM never reads numbers. A company with a confirmed OCR profile parses to
//! VALIDATED facts (committed directly, `source_tier='ai'`); a never-bootstrapped
//! company bootstraps the profile (labels only) and lands PROPOSALS for the
//! existing confirm flow. The job/proposal lifecycle shape the review UI expects
//! is preserved — a facts-emitting run completes with zero proposals and an
//! honest `committed_fact_count`.

use serde_json::json;

use crate::{app_state, storage};

/// How [`run_kpi_extraction_job`] failed (T5.1, ADR 0077 pacing fix).
///
/// The split is what lets the two call sites react differently: the queue
/// handler turns `TransientRetryScheduled` into an `Err` so the queue's capped
/// backoff retry (2..64s) engages, while `Internal` keeps the pre-existing
/// per-job semantics (mark the domain row failed, no queue retry). Autopilot's
/// inline call site never sees `TransientRetryScheduled` — a run without a
/// queue row for the job id has no retry budget, so transient failures there
/// take the terminal path exactly as before.
#[derive(Debug)]
pub enum KpiExtractionJobError {
    /// A transient provider failure (rate limit / unavailable / network) while
    /// the queue still has retry budget. The domain job row was left
    /// re-runnable (not terminally failed) and a `retry_scheduled` diagnostic
    /// was recorded; the queue handler must propagate an `Err` so the backoff
    /// retry is scheduled.
    TransientRetryScheduled(String),
    /// Any other runner failure (domain row lookup, result persistence).
    Internal(String),
}

impl std::fmt::Display for KpiExtractionJobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransientRetryScheduled(message) | Self::Internal(message) => {
                write!(f, "{message}")
            }
        }
    }
}

/// Provider error codes worth a queue retry: transient availability failures,
/// never client/config/content errors. Mirrors
/// [`crate::providers::analysis::AnalysisProviderError::is_availability_error`]
/// on the `code()` strings the extraction pipeline carries (pinned by test).
fn is_transient_error_code(code: &str) -> bool {
    matches!(
        code,
        "provider_limit" | "provider_unavailable" | "network_error"
    )
}

/// The durable-queue row currently executing this job, if any: per-job enqueues
/// key the queue row by the domain job id (`enqueue_per_job`), so a `running`
/// row under that id means this run was queue-dispatched and its
/// `attempts`/`max_attempts` are the retry budget. `None` for inline runs
/// (autopilot calls the runner directly on its own stage row).
fn claimed_queue_row(state: &app_state::AppState, job_id: &str) -> Option<storage::JobStatusRow> {
    state
        .jobs()
        .status(job_id)
        .ok()
        .flatten()
        .filter(|row| row.status == "running")
}

pub fn run_kpi_extraction_job(
    state: &app_state::AppState,
    job_id: &str,
) -> Result<storage::KpiExtractionJob, KpiExtractionJobError> {
    let job = state
        .get_kpi_extraction_job(job_id)
        .map_err(|error| KpiExtractionJobError::Internal(error.to_string()))?;
    if job.status == "succeeded" {
        return Ok(job);
    }

    log::info!(
        "module=kpi_extraction stage=running jobId={} companyId={} documentId={} providerId={} model={}",
        job.id,
        job.company_id,
        job.report_document_id,
        job.provider_id,
        job.model
    );
    record(state, &job, "running", "info", "KPI extraction started.");

    match extract(state, &job) {
        Ok(completed) => {
            let job = state
                .complete_kpi_extraction_job(completed)
                .map_err(|error| KpiExtractionJobError::Internal(error.to_string()))?;
            record(
                state,
                &job,
                "stored",
                "info",
                "KPI extraction proposals stored.",
            );
            Ok(job)
        }
        Err((error_code, error)) => {
            // T5.1 pacing fix: a transient provider failure (429 rate limit,
            // temporary unavailability, network error) with queue retry budget
            // left must NOT terminally fail the domain job — return the
            // transient error so the queue handler schedules the capped
            // backoff retry, and record a queue-visible diagnostic so the UI
            // never shows a silent stall. Terminal errors (bad config,
            // non-PDF document, parse/provider errors) and the final exhausted
            // attempt keep the fast-fail path below.
            let queue_row = if is_transient_error_code(error_code) {
                claimed_queue_row(state, &job.id)
            } else {
                None
            };
            if let Some(row) = queue_row
                .as_ref()
                .filter(|row| row.attempts < row.max_attempts)
            {
                log::warn!(
                    "module=kpi_extraction stage=retry_scheduled jobId={} errorCode={} attempt={}/{} error={}",
                    job.id,
                    error_code,
                    row.attempts,
                    row.max_attempts,
                    error
                );
                record_meta(
                    state,
                    &job,
                    "retry_scheduled",
                    "warning",
                    "KPI extraction hit a transient provider failure; the queue will retry with backoff.",
                    json!({
                        "errorCode": error_code,
                        "error": error,
                        "attempt": row.attempts,
                        "maxAttempts": row.max_attempts,
                    }),
                );
                return Err(KpiExtractionJobError::TransientRetryScheduled(format!(
                    "{error_code}: {error}"
                )));
            }
            // A transient error that exhausted its queue attempts gets an
            // explicit message so the terminal state is self-explanatory.
            // (Inline/autopilot runs have no queue row and keep their message.)
            let error = if let Some(row) = queue_row {
                format!(
                    "{error} (queue retries exhausted after {} attempts)",
                    row.attempts
                )
            } else {
                error
            };
            log::error!(
                "module=kpi_extraction stage=failed jobId={} errorCode={} error={}",
                job.id,
                error_code,
                error
            );
            let failed = state
                .mark_kpi_extraction_job_failed(job_id, error_code, &error)
                .map_err(|storage_error| {
                    KpiExtractionJobError::Internal(storage_error.to_string())
                })?;
            record_meta(
                state,
                &failed,
                "failed",
                "error",
                "KPI extraction failed.",
                json!({ "errorCode": error_code, "error": error }),
            );
            Ok(failed)
        }
    }
}

/// Runs the tier-4 OCR extraction for one job (T4.5). Derives the reporting
/// period deterministically, then hands off to the shared
/// [`crate::jobs::structured_extraction::run_tier4_extraction`] (LLM proposes the
/// profile MAP only; the parser reads the numbers). Facts (if any) are persisted
/// by tier-4; proposals ride back on this job. A "could not run" degradation
/// (no vision provider, non-PDF document, failed bootstrap) becomes a terminal
/// error so the job fails visibly rather than succeeding empty.
fn extract(
    state: &app_state::AppState,
    job: &storage::KpiExtractionJob,
) -> Result<storage::CompletedKpiExtraction, (&'static str, String)> {
    let document = state
        .get_report_document(&job.report_document_id)
        .map_err(|error| ("provider_error", error.to_string()))?;
    if document.fetch_status != "fetched" {
        return Err((
            "provider_error",
            "the report document has not been fetched yet".to_owned(),
        ));
    }
    // Tier-4 parses through a KNOWN reporting period — the LLM never reads the
    // period or the numbers. Derive it deterministically from the title/URL/ESEF,
    // exactly as the structured path does.
    let Some((fiscal_year, period_type, period_end)) =
        crate::jobs::structured_extraction::derive_report_period(state, &document)
    else {
        return Err((
            "no_period",
            "could not determine the reporting period for this document — tier-4 OCR \
             extraction needs a period derivable from the title/URL (or an ESEF instance)"
                .to_owned(),
        ));
    };

    state
        .mark_kpi_extraction_job_running(&job.id)
        .map_err(|error| ("unknown", error.to_string()))?;
    record_meta(
        state,
        job,
        "request_sent",
        "info",
        "Tier-4 OCR extraction started.",
        json!({ "fiscalYear": fiscal_year, "periodType": period_type, "periodEnd": period_end }),
    );

    // Manual extraction runs in the `assist` trust ladder: a validation-clean OCR
    // parse still auto-confirms (deterministic + validated), an unproven one lands
    // `pending`. `run_tier4_extraction` maps this exactly like the structured tiers.
    let outcome = crate::jobs::structured_extraction::run_tier4_extraction(
        state,
        &job.company_id,
        &job.report_document_id,
        fiscal_year,
        period_type,
        &period_end,
        storage::MODE_ASSIST,
    )?;

    // Terminal "could not run" degradations fail the job with an actionable
    // message (a manual click deserves a visible error, not an empty success).
    match outcome.reason.as_str() {
        "not_pdf" => {
            return Err((
                "non_pdf_document",
                "tier-4 OCR extraction only handles PDF reports; this document is a \
                 structured/ESEF filing (handled by the deterministic pipeline)"
                    .to_owned(),
            ))
        }
        "no_vision_provider" => {
            return Err((
                "provider_error",
                "no vision-extraction provider is configured. Add a document-native \
                 provider (Mistral) under Settings \u{2192} AI \u{2192} Vision extraction."
                    .to_owned(),
            ))
        }
        "no_stored_file" => {
            return Err((
                "provider_error",
                "the report document has no stored file".to_owned(),
            ))
        }
        "bootstrap_failed" => {
            return Err((
                "parse_error",
                "the OCR profile bootstrap did not return a usable layout".to_owned(),
            ))
        }
        // facts_emitted | bootstrap_proposals | proposals_flagged | empty → succeed.
        _ => {}
    }

    let committed = outcome.produced_fact_ids.len() as i64;
    record_meta(
        state,
        job,
        "parsed",
        "info",
        "Tier-4 OCR extraction parsed.",
        json!({
            "reason": outcome.reason,
            "committedFactCount": committed,
            "proposalCount": outcome.proposals.len(),
        }),
    );

    Ok(storage::CompletedKpiExtraction {
        job_id: job.id.clone(),
        detected_fiscal_year: outcome.detected_fiscal_year,
        detected_period_type: outcome.detected_period_type,
        detected_period_end_date: outcome.detected_period_end_date,
        detected_currency: outcome.detected_currency,
        detected_language: outcome.detected_language,
        committed_fact_count: committed,
        proposals: outcome.proposals,
    })
}

fn record(
    state: &app_state::AppState,
    job: &storage::KpiExtractionJob,
    stage: &str,
    severity: &str,
    message: &str,
) {
    record_meta(state, job, stage, severity, message, json!({}));
}

fn record_meta(
    state: &app_state::AppState,
    job: &storage::KpiExtractionJob,
    stage: &str,
    severity: &str,
    message: &str,
    extra: serde_json::Value,
) {
    let mut metadata = json!({
        "companyId": job.company_id,
        "documentId": job.report_document_id,
        "providerId": job.provider_id,
        "model": job.model
    });
    if let (Some(base), Some(extra)) = (metadata.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }
    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "kpi_extraction".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "kpi_extraction_job".to_owned(),
            id: Some(job.id.clone()),
        }),
        stage: stage.to_owned(),
        severity: severity.to_owned(),
        message: message.to_owned(),
        metadata: Some(metadata),
    });
}

#[cfg(test)]
mod tests {
    use super::run_kpi_extraction_job;
    use crate::{
        fundamentals::extraction::ocr::{OcrExtractionProfile, ValueColumnLayout},
        fundamentals::extraction::pdf::UnitScale,
        providers::analysis::{
            capabilities::AiCapability, TEST_SAMPLE_ANALYSIS_MODEL,
            TEST_SAMPLE_ANALYSIS_PROVIDER_ID, TEST_SAMPLE_FAIL_PROVIDER_ERROR_MARKER,
            TEST_SAMPLE_FAIL_PROVIDER_LIMIT_MARKER,
        },
        storage::{
            open_in_memory_database, AppState, CaptureReportDocumentInput, ConfirmKpiProposalInput,
            ListFinancialFactsInput, NewCompany, NewKpiExtractionJob,
        },
    };
    use std::collections::BTreeMap;

    /// A per-call-unique data dir: the pid-only dir collides across parallel test
    /// threads sharing this module (each writes/reads the same `report.pdf`).
    fn unique_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("brawler-kpi-extraction-{}-{n}", std::process::id()))
    }

    /// Seeds `capability_providers[vision_extraction] = test_sample` directly on
    /// the connection (bypassing `validate_capability_providers`, the same
    /// technique the pool tests use) so the rewired tier-4 job resolves the
    /// credential-less test-sample OCR provider.
    fn seed_vision_test_sample(connection: &rusqlite::Connection) {
        let json = serde_json::json!({
            AiCapability::VisionExtraction.key(): [
                { "provider": TEST_SAMPLE_ANALYSIS_PROVIDER_ID, "model": TEST_SAMPLE_ANALYSIS_MODEL },
            ]
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO settings (key, value, value_type) VALUES ('capability_providers', ?1, 'json') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [json],
            )
            .expect("seed vision capability provider");
    }

    /// A company + a fetched PDF report document whose title carries a derivable
    /// Q3 2025 period, with the vision (OCR) capability wired to the test-sample
    /// provider. `file_bytes` is the stored document the test-sample OCR "reads"
    /// (its content, or a failure marker).
    fn state_with_report_document_bytes(file_bytes: &[u8]) -> (AppState, String, String) {
        let dir = unique_dir();
        std::fs::create_dir_all(&dir).expect("temp data dir");
        let connection = open_in_memory_database().expect("database should initialize");
        seed_vision_test_sample(&connection);
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should be created");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/report-q3-2025.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("ACME 2025 Q3 report".to_owned()),
                attribution: Some("Investor relations".to_owned()),
            })
            .expect("report document should be captured");
        std::fs::write(dir.join("report.pdf"), file_bytes).expect("write sample pdf");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.pdf"),
                Some("application/pdf"),
                None,
                Some(file_bytes.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    /// The common case: a clean stored PDF the test-sample OCR turns into its
    /// canned Polish statement markdown.
    fn state_with_report_document() -> (AppState, String, String) {
        state_with_report_document_bytes(b"%PDF-1.4 sample")
    }

    /// Seeds a confirmed OCR profile matching `TEST_SAMPLE_OCR_MARKDOWN` so the
    /// tier-4 job parses to VALIDATED facts (not proposals).
    fn seed_ocr_profile(state: &AppState, company_id: &str) {
        let label_map = BTreeMap::from([
            ("przychody ze sprzedaży".to_string(), "revenue".to_string()),
            ("aktywa razem".to_string(), "total_assets".to_string()),
        ]);
        let bootstrap = OcrExtractionProfile::bootstrap(
            company_id,
            UnitScale::Thousands,
            label_map,
            ValueColumnLayout::CurrentPeriodFirst,
            Vec::new(),
            false,
        );
        // CONFIRMED (v2): the facts path requires a user-confirmed profile
        // (ADR 0077 §4 kickoff decision 3); a v1 bootstrap only yields proposals.
        let profile = bootstrap.clone().confirm(
            bootstrap.scale,
            bootstrap.label_map.clone(),
            bootstrap.value_column,
            bootstrap.skip_columns.clone(),
            bootstrap.strip_enumerators,
        );
        state
            .fundamentals_provenance()
            .upsert_ocr_profile(&profile)
            .expect("seed ocr profile");
    }

    /// T4.5 rewire: a never-bootstrapped company's manual extraction runs the
    /// tier-4 bootstrap (the text-LLM proposes the profile MAP, never numbers)
    /// and lands PROPOSALS only — no facts. The reporting period is derived
    /// deterministically from the document title, not the model.
    #[test]
    fn extraction_job_bootstraps_and_persists_proposals_without_writing_facts() {
        let (state, company_id, document_id) = state_with_report_document();
        let job = state
            .create_kpi_extraction_job(NewKpiExtractionJob {
                company_id: company_id.clone(),
                report_document_id: document_id,
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: None,
            })
            .expect("extraction job created");

        let completed = run_kpi_extraction_job(&state, &job.id).expect("job runs");

        assert_eq!(completed.status, "succeeded");
        assert_eq!(completed.detected_fiscal_year, Some(2025));
        assert_eq!(completed.detected_period_type.as_deref(), Some("Q3"));
        assert_eq!(
            completed.proposals.len(),
            2,
            "bootstrap maps przychody + aktywa razem"
        );
        assert!(completed.proposals.iter().all(|p| p.status == "pending"));
        assert_eq!(
            completed.committed_fact_count, 0,
            "a bootstrap run never writes facts"
        );

        // No facts committed by a bootstrap extraction.
        let facts = state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company_id),
                period_id: None,
                definition_id: None,
            })
            .expect("facts list");
        assert!(
            facts.is_empty(),
            "a bootstrap extraction must not write facts"
        );
    }

    /// T4.5 facts path: with a confirmed OCR profile the manual job parses to a
    /// VALIDATED set and commits facts directly (`source_tier='ai'`), completing
    /// with zero proposals and an honest `committed_fact_count`.
    #[test]
    fn extraction_job_with_a_profile_commits_validated_facts_not_proposals() {
        let (state, company_id, document_id) = state_with_report_document();
        seed_ocr_profile(&state, &company_id);
        let job = state
            .create_kpi_extraction_job(NewKpiExtractionJob {
                company_id: company_id.clone(),
                report_document_id: document_id,
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: None,
            })
            .expect("extraction job created");

        let completed = run_kpi_extraction_job(&state, &job.id).expect("job runs");

        assert_eq!(completed.status, "succeeded");
        assert!(
            completed.proposals.is_empty(),
            "a validated profile parse commits facts, not proposals"
        );
        assert_eq!(completed.committed_fact_count, 2);

        let facts = state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company_id),
                period_id: None,
                definition_id: None,
            })
            .expect("facts list");
        assert_eq!(facts.len(), 2, "tier-4 committed the validated facts");
        // The revenue value scaled from the OCR markdown (142 312 tys. → 142312000).
        assert!(
            facts.iter().any(|f| f.value_numeric.trim() == "142312000"),
            "facts: {facts:?}"
        );
        // Every committed fact carries the honest `ai` source tier.
        let provenance = state
            .fundamentals_provenance()
            .get_many(&facts.iter().map(|f| f.id.clone()).collect::<Vec<_>>())
            .expect("provenance");
        assert!(
            provenance.iter().all(|p| p.source_tier == "ai"),
            "tier-4 facts persist source_tier='ai': {provenance:?}"
        );
    }

    /// Pin (T5.1): the transient/terminal split must mirror
    /// `AnalysisProviderError::is_availability_error` on the carried `code()`
    /// strings — drift here would retry client errors or fast-fail 429s.
    #[test]
    fn transient_classification_mirrors_provider_availability_errors() {
        use crate::providers::analysis::AnalysisProviderError as E;
        let all = [
            E::ProviderNotConfigured,
            E::ProviderLimit,
            E::ProviderUnavailable("down".to_owned()),
            E::ProviderError("bad request".to_owned()),
            E::NetworkError("connection reset".to_owned()),
            E::ParseError("bad json".to_owned()),
            E::Unknown("x".to_owned()),
        ];
        for error in &all {
            assert_eq!(
                super::is_transient_error_code(error.code()),
                error.is_availability_error(),
                "classification drift for code {:?}",
                error.code()
            );
        }
        // Non-provider extraction codes are terminal too.
        assert!(!super::is_transient_error_code("non_pdf_document"));
        assert!(!super::is_transient_error_code("unknown"));
    }

    /// T5.1 (ADR 0077 pacing fix): a transient provider failure (429 rate
    /// limit) must engage the queue's capped backoff retry — the queue row goes
    /// back to `pending` with the attempt counted, and the domain job is NOT
    /// terminally failed while attempts remain (it must stay re-runnable).
    /// A queue-visible diagnostic records the transient failure so the UI does
    /// not show a silent stall.
    #[test]
    fn transient_provider_failure_reschedules_the_queue_job_with_backoff() {
        // The scripted OCR-failure marker rides in the stored document bytes the
        // test-sample OCR "reads" (tier-4 OCRs the file, not a text prompt).
        let (state, company_id, document_id) = state_with_report_document_bytes(
            format!("%PDF {TEST_SAMPLE_FAIL_PROVIDER_LIMIT_MARKER}").as_bytes(),
        );
        // Diagnostic events are only persisted in developer mode.
        state
            .set_developer_mode_enabled(true)
            .expect("enable developer mode");
        let job = state
            .create_kpi_extraction_job(NewKpiExtractionJob {
                company_id,
                report_document_id: document_id,
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: None,
            })
            .expect("extraction job created");

        crate::jobs::handlers::enqueue_per_job(
            &state,
            crate::jobs::handlers::KPI_EXTRACTION_KIND,
            &job.id,
        );
        let worker = crate::jobs::handlers::build_worker(state.clone());
        assert!(worker.process_one().expect("process"), "job was claimed");

        // Queue: retried with backoff — pending again, attempt counted, not
        // terminal in either direction.
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.pending, 1, "rescheduled for a backoff retry");
        assert_eq!(counts.failed, 0, "not dead-lettered on the first 429");
        assert_eq!(counts.succeeded, 0, "a transient failure is not a success");
        let row = state
            .jobs()
            .status(&job.id)
            .expect("status")
            .expect("queue row exists");
        assert_eq!(row.attempts, 1, "the failed attempt is counted");
        assert!(
            row.last_error
                .as_deref()
                .unwrap_or_default()
                .contains("provider_limit"),
            "the queue records the transient error: {:?}",
            row.last_error
        );

        // Backoff: the retry is scheduled in the future, not immediately runnable.
        assert!(
            !worker.process_one().expect("process"),
            "the retry waits out the backoff window"
        );

        // Domain job: not terminally failed while attempts remain.
        let domain = state.get_kpi_extraction_job(&job.id).expect("domain job");
        assert_ne!(
            domain.status, "failed",
            "the domain job stays re-runnable between retries"
        );

        // Queue-visible progress: the transient failure is recorded, not silent.
        let events = state
            .list_diagnostic_events(20)
            .expect("diagnostic events should list");
        assert!(
            events
                .iter()
                .any(|event| event.module == "kpi_extraction" && event.stage == "retry_scheduled"),
            "a retry_scheduled diagnostic records the transient failure"
        );
    }

    /// T5.1 regression pin: a terminal provider error keeps the fast-fail
    /// behavior exactly — domain job marked failed with its error code, queue
    /// row completes, no retry.
    #[test]
    fn terminal_provider_error_fast_fails_without_queue_retry() {
        let (state, company_id, document_id) = state_with_report_document_bytes(
            format!("%PDF {TEST_SAMPLE_FAIL_PROVIDER_ERROR_MARKER}").as_bytes(),
        );
        let job = state
            .create_kpi_extraction_job(NewKpiExtractionJob {
                company_id,
                report_document_id: document_id,
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: None,
            })
            .expect("extraction job created");

        crate::jobs::handlers::enqueue_per_job(
            &state,
            crate::jobs::handlers::KPI_EXTRACTION_KIND,
            &job.id,
        );
        let worker = crate::jobs::handlers::build_worker(state.clone());
        assert!(worker.process_one().expect("process"));

        let domain = state.get_kpi_extraction_job(&job.id).expect("domain job");
        assert_eq!(domain.status, "failed", "terminal errors fast-fail");
        assert_eq!(domain.error_code.as_deref(), Some("provider_error"));

        // The queue row is terminal (executed, domain outcome in its own table)
        // and nothing is left to retry.
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.pending, 0, "no retry for a terminal error");
        assert_eq!(counts.running, 0);
        assert!(!worker.process_one().expect("process"), "queue is idle");
    }

    /// T5.1: when the queue's retry budget is exhausted by transient failures,
    /// the domain job must end in a clear `failed` state with a message — never
    /// stuck `running`/`queued` in limbo.
    #[test]
    fn exhausted_transient_retries_end_in_a_failed_domain_job() {
        let (state, company_id, document_id) = state_with_report_document_bytes(
            format!("%PDF {TEST_SAMPLE_FAIL_PROVIDER_LIMIT_MARKER}").as_bytes(),
        );
        let job = state
            .create_kpi_extraction_job(NewKpiExtractionJob {
                company_id,
                report_document_id: document_id,
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: None,
            })
            .expect("extraction job created");

        // max_attempts = 1: this run is the last (only) attempt — the exhaustion
        // case, without waiting out real backoff windows.
        state
            .jobs()
            .enqueue(
                &job.id,
                crate::jobs::handlers::KPI_EXTRACTION_KIND,
                &job.id,
                1,
            )
            .expect("enqueue");
        let worker = crate::jobs::handlers::build_worker(state.clone());
        assert!(worker.process_one().expect("process"));

        let domain = state.get_kpi_extraction_job(&job.id).expect("domain job");
        assert_eq!(
            domain.status, "failed",
            "an exhausted transient failure must terminally fail the domain job, not leave limbo"
        );
        assert_eq!(domain.error_code.as_deref(), Some("provider_limit"));
        assert!(
            !domain.error.as_deref().unwrap_or_default().is_empty(),
            "the failure carries a clear message"
        );

        // Nothing lingers in the queue.
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.pending, 0);
        assert_eq!(counts.running, 0);
    }

    #[test]
    fn confirming_a_proposal_commits_a_fact_and_rejecting_does_not() {
        let (state, company_id, document_id) = state_with_report_document();
        let job = state
            .create_kpi_extraction_job(NewKpiExtractionJob {
                company_id: company_id.clone(),
                report_document_id: document_id,
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: None,
            })
            .expect("extraction job created");
        let job = run_kpi_extraction_job(&state, &job.id).expect("job runs");

        let revenue = job
            .proposals
            .iter()
            .find(|p| p.metric_key == "revenue")
            .expect("revenue proposal");
        let total_assets = job
            .proposals
            .iter()
            .find(|p| p.metric_key == "total_assets")
            .expect("total_assets proposal");
        assert!(!revenue.is_proposed_kpi);

        // Reject one bootstrap proposal.
        state
            .reject_kpi_proposal(&total_assets.id)
            .expect("reject proposal");

        // Confirm the known KPI -> commits one fact with the detected period.
        let fact = state
            .confirm_kpi_proposal(ConfirmKpiProposalInput {
                proposal_id: revenue.id.clone(),
                value_numeric: None,
                currency: None,
                fiscal_year: None,
                period_type: None,
                period_end_date: None,
                accept_as_new_kpi: false,
            })
            .expect("confirm proposal")
            .fact;
        assert_eq!(fact.value_numeric, "142312000");
        assert_eq!(fact.confirmation_state, "confirmed");
        assert_eq!(fact.extraction_method, "ai");

        let facts = state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company_id),
                period_id: None,
                definition_id: None,
            })
            .expect("facts list");
        assert_eq!(facts.len(), 1, "only the confirmed proposal becomes a fact");
    }
}
