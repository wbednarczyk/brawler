use super::common::sample_bankier_company_items;
use super::*;

#[test]
fn creates_lists_and_completes_ai_analysis_jobs() {
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
            custom_question: None,
            provider_id: "provider_gemini".to_owned(),
            model: "gemini-2.5-flash".to_owned(),
            prompt_version: Some("m13.source_grounded.v1".to_owned()),
        })
        .expect("analysis job should be created");

    assert_eq!(job.feed_item_id, feed_item.id);
    assert_eq!(job.status, "queued");
    assert_eq!(job.prompt_preset_id, "default_summary");
    assert_eq!(job.provider_id, "provider_gemini");
    assert!(job.result.is_none());

    let running = state
        .mark_ai_analysis_job_running(&job.id)
        .expect("analysis job should become running");

    assert_eq!(running.status, "running");
    assert!(running.started_at.is_some());

    let completed = state
        .complete_ai_analysis_job(CompletedAiAnalysis {
            job_id: job.id.clone(),
            summary: "Revenue increased and management highlighted delivery timing.".to_owned(),
            significance: "medium".to_owned(),
            reasoning: "The report updates investor expectations using cited source text.".to_owned(),
            language: Some("en".to_owned()),
            tags: vec!["earnings".to_owned(), "guidance".to_owned()],
            source_references: vec![NewAiAnalysisSourceReference {
                source_url: "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
                    .to_owned(),
                label: Some("Bankier Company Komunikaty".to_owned()),
            }],
        })
        .expect("analysis job should complete");

    let result = completed
        .result
        .expect("completed job should include result");

    assert_eq!(completed.status, "succeeded");
    assert_eq!(result.provider_id, "provider_gemini");
    assert_eq!(result.model, "gemini-2.5-flash");
    assert_eq!(result.prompt_version, "m13.source_grounded.v1");
    assert_eq!(result.significance, "medium");
    assert_eq!(result.tags, vec!["earnings", "guidance"]);
    assert_eq!(result.source_references.len(), 1);

    let jobs = state
        .list_ai_analysis_jobs(&feed_item.id)
        .expect("analysis jobs should list");

    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].status, "succeeded");
    assert!(jobs[0].result.is_some());
}

#[test]
fn fails_ai_analysis_jobs_with_recoverable_error_state() {
    let state = state_with_feed_item();
    let feed_item = state
        .list_feed_items()
        .expect("feed items should list")
        .pop()
        .expect("feed item should exist");
    let job = state
        .create_ai_analysis_job(NewAiAnalysisJob {
            feed_item_id: feed_item.id,
            prompt_preset_id: None,
            custom_question: Some("What changed in this report?".to_owned()),
            provider_id: "provider_gemini".to_owned(),
            model: "gemini-2.5-flash".to_owned(),
            prompt_version: None,
        })
        .expect("analysis job should be created");

    let failed = state
        .mark_ai_analysis_job_failed(
            &job.id,
            "provider_not_configured",
            "Gemini general analysis credentials are not configured.",
        )
        .expect("analysis job should fail");

    assert_eq!(failed.status, "failed");
    assert_eq!(
        failed.error_code.as_deref(),
        Some("provider_not_configured")
    );
    assert!(failed.finished_at.is_some());
    assert_eq!(
        failed.custom_question.as_deref(),
        Some("What changed in this report?")
    );
}

#[test]
fn rejects_invalid_ai_analysis_result_significance() {
    let state = state_with_feed_item();
    let feed_item = state
        .list_feed_items()
        .expect("feed items should list")
        .pop()
        .expect("feed item should exist");
    let job = state
        .create_ai_analysis_job(NewAiAnalysisJob {
            feed_item_id: feed_item.id,
            prompt_preset_id: None,
            custom_question: None,
            provider_id: "provider_gemini".to_owned(),
            model: "gemini-2.5-flash".to_owned(),
            prompt_version: None,
        })
        .expect("analysis job should be created");

    let result = state.complete_ai_analysis_job(CompletedAiAnalysis {
        job_id: job.id,
        summary: "Summary".to_owned(),
        significance: "urgent".to_owned(),
        reasoning: "Reasoning".to_owned(),
        language: Some("en".to_owned()),
        tags: Vec::new(),
        source_references: Vec::new(),
    });

    assert!(result.is_err());
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
        .ingest_bankier_company_items(&sample_bankier_company_items(&company))
        .expect("feed item should ingest");

    state
}
