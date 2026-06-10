use crate::{
    app_state,
    providers::{
        analysis::{
            AiAnalysisProvider, AnalysisRequest, GeminiAnalysisProvider,
            TestSampleAnalysisProvider, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
        },
        credentials,
    },
    storage,
};
use serde_json::json;

pub fn run_ai_analysis_job(
    state: &app_state::AppState,
    job_id: &str,
) -> Result<storage::AiAnalysisJob, String> {
    let started_at = std::time::Instant::now();
    let job = state
        .get_ai_analysis_job(job_id)
        .map_err(|error| error.to_string())?;

    if job.status == "succeeded" {
        return Ok(job);
    }

    log::info!(
        "module=ai_analysis stage=running jobId={} feedItemId={} providerId={} model={} hasCustomQuestion={}",
        job.id,
        job.feed_item_id,
        job.provider_id,
        job.model,
        job.custom_question.is_some()
    );
    record_ai_analysis_diagnostic(
        state,
        &job,
        "running",
        "info",
        "AI analysis job started.",
        json!({
            "feedItemId": job.feed_item_id,
            "providerId": job.provider_id,
            "model": job.model,
            "promptPresetId": job.prompt_preset_id,
            "promptVersion": job.prompt_version,
            "hasCustomQuestion": job.custom_question.is_some()
        }),
    );
    let provider = match provider_for_job(state, &job) {
        Ok(provider) => provider,
        Err(error) => {
            log::error!(
                "module=ai_analysis stage=failed jobId={} feedItemId={} providerId={} model={} errorClass=provider_error error={}",
                job.id,
                job.feed_item_id,
                job.provider_id,
                job.model,
                error
            );
            let failed = state
                .mark_ai_analysis_job_failed(job_id, "provider_error", &error)
                .map_err(|storage_error| storage_error.to_string())?;
            record_ai_analysis_metrics(state, &failed, "failed", started_at);
            record_ai_analysis_diagnostic(
                state,
                &failed,
                "failed",
                "error",
                "AI analysis provider resolution failed.",
                json!({
                    "feedItemId": failed.feed_item_id,
                    "providerId": failed.provider_id,
                    "model": failed.model,
                    "errorCode": "provider_error"
                }),
            );

            return Ok(failed);
        }
    };
    let feed_item = state
        .get_feed_item(&job.feed_item_id)
        .map_err(|error| error.to_string())?;
    record_ai_analysis_diagnostic(
        state,
        &job,
        "context_loaded",
        "info",
        "AI analysis context loaded.",
        json!({
            "feedItemId": job.feed_item_id,
            "providerId": job.provider_id,
            "model": job.model,
            "hasBodyText": !feed_item.body_text.trim().is_empty(),
            "attachmentCount": feed_item.attachments.len()
        }),
    );
    let request = AnalysisRequest {
        feed_item,
        prompt_preset_id: job.prompt_preset_id.clone(),
        custom_question: job.custom_question.clone(),
    };

    state
        .mark_ai_analysis_job_running(job_id)
        .map_err(|error| error.to_string())?;
    log::info!(
        "module=ai_analysis stage=request_sent jobId={} providerId={} model={} hasCustomQuestion={}",
        job.id,
        provider.provider_id(),
        provider.model(),
        request.custom_question.is_some()
    );
    record_ai_analysis_diagnostic(
        state,
        &job,
        "request_sent",
        "info",
        "AI analysis provider request sent.",
        json!({
            "providerId": provider.provider_id(),
            "model": provider.model(),
            "promptPresetId": request.prompt_preset_id.clone(),
            "hasCustomQuestion": request.custom_question.is_some()
        }),
    );

    match provider.analyze(&request) {
        Ok(output) => {
            log::info!(
                "module=ai_analysis stage=response_received jobId={} providerId={} model={}",
                job.id,
                provider.provider_id(),
                provider.model()
            );
            record_ai_analysis_diagnostic(
                state,
                &job,
                "response_received",
                "info",
                "AI analysis provider response received.",
                json!({
                    "providerId": provider.provider_id(),
                    "model": provider.model()
                }),
            );
            record_ai_analysis_diagnostic(
                state,
                &job,
                "parsed",
                "info",
                "AI analysis response parsed.",
                json!({
                    "tagCount": output.tags.len(),
                    "sourceReferenceCount": output.source_references.len(),
                    "significance": output.significance.clone(),
                    "language": output.language.clone()
                }),
            );

            let completed = state
                .complete_ai_analysis_job(storage::CompletedAiAnalysis {
                    job_id: job_id.to_owned(),
                    summary: output.summary,
                    significance: output.significance,
                    reasoning: output.reasoning,
                    language: output.language,
                    tags: output.tags,
                    source_references: output
                        .source_references
                        .into_iter()
                        .map(|reference| storage::NewAiAnalysisSourceReference {
                            source_url: reference.source_url,
                            label: reference.label,
                        })
                        .collect(),
                })
                .map_err(|error| error.to_string())?;
            record_ai_analysis_metrics(state, &completed, "succeeded", started_at);
            record_ai_analysis_diagnostic(
                state,
                &completed,
                "stored",
                "info",
                "AI analysis result stored.",
                json!({
                    "feedItemId": completed.feed_item_id,
                    "providerId": completed.provider_id,
                    "model": completed.model,
                    "status": completed.status,
                    "hasResult": completed.result.is_some()
                }),
            );

            Ok(completed)
        }
        Err(error) => {
            let error_code = error.code();
            log::error!(
                "module=ai_analysis stage=failed jobId={} feedItemId={} providerId={} model={} errorCode={}",
                job.id,
                job.feed_item_id,
                job.provider_id,
                job.model,
                error_code
            );
            let failed = state
                .mark_ai_analysis_job_failed(job_id, error_code, &error.to_string())
                .map_err(|storage_error| storage_error.to_string())?;
            record_ai_analysis_metrics(state, &failed, "failed", started_at);
            record_ai_analysis_diagnostic(
                state,
                &failed,
                "failed",
                "error",
                "AI analysis job failed.",
                json!({
                    "feedItemId": failed.feed_item_id,
                    "providerId": failed.provider_id,
                    "model": failed.model,
                    "errorCode": error_code
                }),
            );

            Ok(failed)
        }
    }
}

fn record_ai_analysis_metrics(
    state: &app_state::AppState,
    job: &storage::AiAnalysisJob,
    status: &str,
    started_at: std::time::Instant,
) {
    state.increment_runtime_counter(
        "brawler_ai_analysis_runs_total",
        &[
            ("provider_id", job.provider_id.as_str()),
            ("model", job.model.as_str()),
            ("status", status),
        ],
    );
    state.observe_runtime_duration_seconds(
        "brawler_ai_analysis_duration_seconds",
        &[
            ("provider_id", job.provider_id.as_str()),
            ("model", job.model.as_str()),
            ("status", status),
        ],
        started_at.elapsed().as_secs_f64(),
    );
}

fn provider_for_job(
    state: &app_state::AppState,
    job: &storage::AiAnalysisJob,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    match job.provider_id.as_str() {
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID => {
            record_ai_analysis_diagnostic(
                state,
                job,
                "credential_checked",
                "info",
                "AI analysis provider credential checked.",
                json!({
                    "providerId": job.provider_id,
                    "credentialRequired": false,
                    "credentialConfigured": true
                }),
            );
            record_ai_analysis_diagnostic(
                state,
                job,
                "provider_resolved",
                "info",
                "AI analysis provider resolved.",
                json!({
                    "providerId": job.provider_id,
                    "model": job.model
                }),
            );
            Ok(Box::new(TestSampleAnalysisProvider))
        }
        "provider_gemini" => {
            let settings = state.get_settings().map_err(|error| error.to_string())?;
            let api_key = credentials::read_gemini_general_analysis_api_key().unwrap_or(None);
            record_ai_analysis_diagnostic(
                state,
                job,
                "credential_checked",
                if api_key.is_some() { "info" } else { "warning" },
                "AI analysis provider credential checked.",
                json!({
                    "providerId": job.provider_id,
                    "credentialConfigured": api_key.is_some()
                }),
            );
            record_ai_analysis_diagnostic(
                state,
                job,
                "provider_resolved",
                "info",
                "AI analysis provider resolved.",
                json!({
                    "providerId": job.provider_id,
                    "model": job.model,
                    "timeoutSeconds": settings.ai_providers.general_analysis_timeout_seconds
                }),
            );
            Ok(Box::new(
                GeminiAnalysisProvider::live(
                    api_key,
                    job.model.clone(),
                    settings.ai_providers.general_analysis_timeout_seconds,
                )
                .map_err(|error| error.to_string())?,
            ))
        }
        other => Err(format!("Unknown AI analysis provider: {other}")),
    }
}

fn record_ai_analysis_diagnostic(
    state: &app_state::AppState,
    job: &storage::AiAnalysisJob,
    stage: &str,
    severity: &str,
    message: &str,
    metadata: serde_json::Value,
) {
    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "ai_analysis".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "ai_analysis_job".to_owned(),
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
    use super::run_ai_analysis_job;
    use crate::{
        providers::analysis::{TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID},
        source_adapters::bankier_company::BankierCompanyItem,
        storage::{open_in_memory_database, AppState, NewAiAnalysisJob, NewCompany},
    };

    #[test]
    fn deterministic_analysis_job_completes_with_result() {
        let state = state_with_feed_item();
        let feed_item = state
            .list_feed_items()
            .expect("feed items should list")
            .pop()
            .expect("feed item should exist");
        let job = state
            .create_ai_analysis_job(NewAiAnalysisJob {
                feed_item_id: feed_item.id.clone(),
                prompt_preset_id: Some("default_summary".to_owned()),
                custom_question: Some("What changed?".to_owned()),
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: Some("m13.source_grounded.v1".to_owned()),
            })
            .expect("analysis job should be created");

        let completed = run_ai_analysis_job(&state, &job.id).expect("analysis job should complete");

        assert_eq!(completed.status, "succeeded");
        assert_eq!(completed.provider_id, TEST_SAMPLE_ANALYSIS_PROVIDER_ID);
        assert!(completed.started_at.is_some());
        assert!(completed.finished_at.is_some());
        assert!(completed.result.is_some());
    }

    #[test]
    fn deterministic_analysis_job_records_diagnostic_lifecycle_when_enabled() {
        let state = state_with_feed_item();
        state
            .set_developer_mode_enabled(true)
            .expect("developer mode should enable");
        let feed_item = state
            .list_feed_items()
            .expect("feed items should list")
            .pop()
            .expect("feed item should exist");
        let job = state
            .create_ai_analysis_job(NewAiAnalysisJob {
                feed_item_id: feed_item.id.clone(),
                prompt_preset_id: Some("default_summary".to_owned()),
                custom_question: Some("What changed?".to_owned()),
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: Some("m13.source_grounded.v1".to_owned()),
            })
            .expect("analysis job should be created");

        run_ai_analysis_job(&state, &job.id).expect("analysis job should complete");

        let events = state
            .list_diagnostic_events(20)
            .expect("diagnostic events should list");
        let stages = events
            .iter()
            .map(|event| event.stage.as_str())
            .collect::<std::collections::HashSet<_>>();

        for expected_stage in [
            "running",
            "credential_checked",
            "provider_resolved",
            "context_loaded",
            "request_sent",
            "response_received",
            "parsed",
            "stored",
        ] {
            assert!(
                stages.contains(expected_stage),
                "missing diagnostic stage {expected_stage}"
            );
        }
        assert!(events.iter().all(|event| event.module == "ai_analysis"));
        assert!(events.iter().all(|event| event
            .scope
            .as_ref()
            .is_some_and(|scope| scope.scope_type == "ai_analysis_job")));
        assert!(events
            .iter()
            .all(|event| !event.metadata.to_string().contains("What changed?")));
    }

    fn state_with_feed_item() -> AppState {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
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

        state
            .ingest_bankier_company_items(&[BankierCompanyItem {
                company_id: company.id,
                qualified_ticker: "GPW:CDR".to_owned(),
                title: "Wyniki finansowe QSr 1/2026".to_owned(),
                link: "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
                    .to_owned(),
                summary: "raporty okresowe".to_owned(),
                published_at: Some("2026-05-28T17:33:09".to_owned()),
                fetched_at: "2026-05-31T10:00:00Z".to_owned(),
                article_id: "9141553".to_owned(),
                pub_id: 3,
                dedupe_key: "bankier-company-komunikaty:article:9141553".to_owned(),
                duplicate_signature:
                    "official-secondary:GPW:CDR:wyniki-finansowe-qsr-1-2026:9141553".to_owned(),
                body_text: Some("Official Bankier report body from the article page.".to_owned()),
                attachments: Vec::new(),
                detail_fetch_attempted: true,
            }])
            .expect("feed item should ingest");

        state
    }
}
