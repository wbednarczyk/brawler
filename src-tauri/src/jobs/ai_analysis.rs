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

pub fn run_ai_analysis_job(
    state: &app_state::AppState,
    job_id: &str,
) -> Result<storage::AiAnalysisJob, String> {
    let job = state
        .get_ai_analysis_job(job_id)
        .map_err(|error| error.to_string())?;

    if job.status == "succeeded" {
        return Ok(job);
    }

    let provider = provider_for_job(state, &job)?;
    let feed_item = state
        .get_feed_item(&job.feed_item_id)
        .map_err(|error| error.to_string())?;
    let request = AnalysisRequest {
        feed_item,
        prompt_preset_id: job.prompt_preset_id.clone(),
        custom_question: job.custom_question.clone(),
    };

    state
        .mark_ai_analysis_job_running(job_id)
        .map_err(|error| error.to_string())?;

    match provider.analyze(&request) {
        Ok(output) => state
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
            .map_err(|error| error.to_string()),
        Err(error) => state
            .mark_ai_analysis_job_failed(job_id, error.code(), &error.to_string())
            .map_err(|storage_error| storage_error.to_string()),
    }
}

fn provider_for_job(
    state: &app_state::AppState,
    job: &storage::AiAnalysisJob,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    match job.provider_id.as_str() {
        TEST_SAMPLE_ANALYSIS_PROVIDER_ID => Ok(Box::new(TestSampleAnalysisProvider)),
        "provider_gemini" => {
            let settings = state.get_settings().map_err(|error| error.to_string())?;
            let api_key = credentials::read_gemini_general_analysis_api_key().unwrap_or(None);
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
