//! Async KPI extraction job (v0.36.0, epic 9879941).
//!
//! Loads a stored report document, sends it natively to the selected AI provider
//! through the document-input boundary, parses the model output, and persists the
//! results as PROPOSALS. No financial fact is written here — committing a value is
//! an explicit user confirmation step (see `storage::confirm_kpi_proposal`).

use serde_json::json;

use crate::{
    app_state,
    providers::analysis::{
        kpi_extraction_prompt, parse_kpi_extraction_output, registry, AiAnalysisProvider,
        AnalysisDocument, DocumentSupport, KpiCatalogEntry, KpiExtractionRequest,
    },
    storage,
};

pub fn run_kpi_extraction_job(
    state: &app_state::AppState,
    job_id: &str,
) -> Result<storage::KpiExtractionJob, String> {
    let job = state
        .get_kpi_extraction_job(job_id)
        .map_err(|error| error.to_string())?;
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
                .map_err(|error| error.to_string())?;
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
            log::error!(
                "module=kpi_extraction stage=failed jobId={} errorCode={} error={}",
                job.id,
                error_code,
                error
            );
            let failed = state
                .mark_kpi_extraction_job_failed(job_id, error_code, &error)
                .map_err(|storage_error| storage_error.to_string())?;
            record(state, &failed, "failed", "error", "KPI extraction failed.");
            Ok(failed)
        }
    }
}

fn extract(
    state: &app_state::AppState,
    job: &storage::KpiExtractionJob,
) -> Result<storage::CompletedKpiExtraction, (&'static str, String)> {
    let provider = provider_for_job(state, job).map_err(|error| ("provider_error", error))?;
    if provider.document_support() == DocumentSupport::None {
        return Err((
            "provider_error",
            "the selected AI provider cannot read documents".to_owned(),
        ));
    }

    let document = load_document(state, job)?;
    let request = build_request(state, job).map_err(|error| ("provider_error", error))?;
    let prompt = kpi_extraction_prompt(&request);

    state
        .mark_kpi_extraction_job_running(&job.id)
        .map_err(|error| ("unknown", error.to_string()))?;

    let text = tauri::async_runtime::block_on(provider.complete_document(&prompt, &document))
        .map_err(|error| (error.code(), error.to_string()))?;
    record(
        state,
        job,
        "response_received",
        "info",
        "KPI extraction provider response received.",
    );

    let output = parse_kpi_extraction_output(&text, provider.provider_id())
        .map_err(|error| (error.code(), error.to_string()))?;

    let (detected_fiscal_year, detected_period_type, detected_period_end_date) = match output.period
    {
        Some(period) => (
            Some(period.fiscal_year),
            Some(period.period_type),
            period.period_end_date,
        ),
        None => (None, None, None),
    };

    let proposals = output
        .facts
        .into_iter()
        .map(|fact| storage::NewKpiProposal {
            metric_key: fact.metric_key,
            label: fact.label,
            value_numeric: fact.value_numeric,
            unit: fact.unit,
            currency: fact.currency,
            as_reported_value: fact.as_reported_value,
            as_reported_scale: fact.as_reported_scale,
            measure_window: fact.measure_window,
            confidence: fact.confidence,
            source_snippet: fact.source_snippet,
            is_proposed_kpi: fact.is_proposed_kpi,
        })
        .collect();

    Ok(storage::CompletedKpiExtraction {
        job_id: job.id.clone(),
        detected_fiscal_year,
        detected_period_type,
        detected_period_end_date,
        detected_currency: output.currency,
        detected_language: output.language,
        proposals,
    })
}

fn load_document(
    state: &app_state::AppState,
    job: &storage::KpiExtractionJob,
) -> Result<AnalysisDocument, (&'static str, String)> {
    let document = state
        .get_report_document(&job.report_document_id)
        .map_err(|error| ("provider_error", error.to_string()))?;
    if document.fetch_status != "fetched" {
        return Err((
            "provider_error",
            "the report document has not been fetched yet".to_owned(),
        ));
    }
    let local_path = document.local_path.ok_or_else(|| {
        (
            "provider_error",
            "the report document has no stored file".to_owned(),
        )
    })?;
    let path = state.data_dir().join(&local_path);
    let data = std::fs::read(&path).map_err(|error| {
        (
            "provider_error",
            format!("failed to read report document file: {error}"),
        )
    })?;
    let mime_type = document
        .content_type
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "application/pdf".to_owned());

    Ok(AnalysisDocument::Native { mime_type, data })
}

fn build_request(
    state: &app_state::AppState,
    job: &storage::KpiExtractionJob,
) -> Result<KpiExtractionRequest, String> {
    let company_name = state
        .list_companies()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|company| company.id == job.company_id)
        .map(|company| company.display_name)
        .unwrap_or_else(|| job.company_id.clone());

    // Canonical packs apply to every company; company-scoped custom KPIs are added.
    // Sector relevance refinement is a later milestone (v0.37); the model can still
    // surface sector metrics as proposed extras.
    let known_kpis = state
        .list_kpi_definitions(storage::ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|definition| {
            definition.scope == "canonical"
                || (definition.scope == "company"
                    && definition.company_id.as_deref() == Some(job.company_id.as_str()))
        })
        .map(|definition| KpiCatalogEntry {
            metric_key: definition.metric_key,
            label: definition.label,
            value_kind: definition.value_kind,
            unit: definition.unit,
        })
        .collect();

    Ok(KpiExtractionRequest {
        company_name,
        statement_type: None,
        known_kpis,
        period_hint: job.period_hint.clone(),
    })
}

fn provider_for_job(
    state: &app_state::AppState,
    job: &storage::KpiExtractionJob,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let timeout_seconds = settings.ai_providers.general_analysis_timeout_seconds;
    let api_key = registry::read_analysis_provider_api_key(&job.provider_id);
    registry::build_analysis_provider(&job.provider_id, api_key, &job.model, timeout_seconds)
}

fn record(
    state: &app_state::AppState,
    job: &storage::KpiExtractionJob,
    stage: &str,
    severity: &str,
    message: &str,
) {
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
        metadata: Some(json!({
            "companyId": job.company_id,
            "documentId": job.report_document_id,
            "providerId": job.provider_id,
            "model": job.model
        })),
    });
}

#[cfg(test)]
mod tests {
    use super::run_kpi_extraction_job;
    use crate::{
        providers::analysis::{TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID},
        storage::{
            open_in_memory_database, AppState, CaptureReportDocumentInput, ConfirmKpiProposalInput,
            ListFinancialFactsInput, NewCompany, NewKpiExtractionJob,
        },
    };

    fn state_with_report_document() -> (AppState, String, String) {
        let dir =
            std::env::temp_dir().join(format!("brawler-kpi-extraction-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp data dir");
        let connection = open_in_memory_database().expect("database should initialize");
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
                title: Some("Q3 2025 report".to_owned()),
                attribution: Some("Investor relations".to_owned()),
            })
            .expect("report document should be captured");

        // Simulate a fetched file on disk.
        std::fs::write(dir.join("report.pdf"), b"%PDF-1.4 sample").expect("write sample pdf");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.pdf"),
                Some("application/pdf"),
                None,
                Some(15),
            )
            .expect("mark fetched");

        (state, company.id, document.id)
    }

    #[test]
    fn extraction_job_persists_proposals_without_writing_facts() {
        let (state, company_id, document_id) = state_with_report_document();
        let job = state
            .create_kpi_extraction_job(NewKpiExtractionJob {
                company_id: company_id.clone(),
                report_document_id: document_id,
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: Some("Q3 2025".to_owned()),
            })
            .expect("extraction job created");

        let completed = run_kpi_extraction_job(&state, &job.id).expect("job runs");

        assert_eq!(completed.status, "succeeded");
        assert_eq!(completed.detected_fiscal_year, Some(2025));
        assert_eq!(completed.detected_period_type.as_deref(), Some("Q3"));
        assert_eq!(completed.proposals.len(), 2);
        assert!(completed.proposals.iter().all(|p| p.status == "pending"));

        // No facts committed by extraction alone.
        let facts = state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company_id),
                period_id: None,
                definition_id: None,
            })
            .expect("facts list");
        assert!(facts.is_empty(), "extraction must not write facts");
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
        let backlog = job
            .proposals
            .iter()
            .find(|p| p.metric_key == "backlog")
            .expect("backlog proposal");
        assert!(!revenue.is_proposed_kpi);
        assert!(backlog.is_proposed_kpi);

        // Reject the out-of-taxonomy suggestion.
        state
            .reject_kpi_proposal(&backlog.id)
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
            .expect("confirm proposal");
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
