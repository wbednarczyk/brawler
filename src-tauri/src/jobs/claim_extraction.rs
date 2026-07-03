//! Async claim extraction job (v0.42.0, epic cbf6999, ADR 0040).
//!
//! Loads a stored report document or a transcript, sends it to the selected AI
//! provider through the analysis boundary, parses the model output, and persists
//! the results as PROPOSALS. No claim is written here — materialising a claim is an
//! explicit user confirmation step (`storage::confirm_claim_proposal`). Mirrors the
//! KPI extraction job.

use crate::{
    app_state,
    providers::analysis::{
        capabilities::{AiCapability, CAPABILITY_ROUTED_PROVIDER_ID},
        claim_extraction_prompt, parse_claim_extraction_output, registry, AiAnalysisProvider,
        AnalysisDocument, ClaimExtractionRequest, DocumentSupport,
    },
    storage,
};

pub fn run_claim_extraction_job(
    state: &app_state::AppState,
    job_id: &str,
) -> Result<storage::ClaimExtractionJob, String> {
    let job = state
        .get_claim_extraction_job(job_id)
        .map_err(|error| error.to_string())?;
    if job.status == "succeeded" {
        return Ok(job);
    }

    log::info!(
        "module=claim_extraction stage=running jobId={} companyId={} sourceType={} sourceId={} providerId={} model={}",
        job.id,
        job.company_id,
        job.source_type,
        job.source_id,
        job.provider_id,
        job.model
    );

    match extract(state, &job) {
        Ok(completed) => state
            .complete_claim_extraction_job(completed)
            .map_err(|error| error.to_string()),
        Err((error_code, error)) => {
            log::error!(
                "module=claim_extraction stage=failed jobId={} errorCode={} error={}",
                job.id,
                error_code,
                error
            );
            state
                .mark_claim_extraction_job_failed(job_id, error_code, &error)
                .map_err(|storage_error| storage_error.to_string())
        }
    }
}

fn extract(
    state: &app_state::AppState,
    job: &storage::ClaimExtractionJob,
) -> Result<storage::CompletedClaimExtraction, (&'static str, String)> {
    let provider = provider_for_job(state, job).map_err(|error| ("provider_unavailable", error))?;
    if provider.document_support() == DocumentSupport::None {
        return Err((
            "provider_unavailable",
            "the selected AI provider cannot read documents".to_owned(),
        ));
    }

    let document = load_source(state, job)?;
    let request = ClaimExtractionRequest {
        company_name: company_name(state, job),
        source_kind: match job.source_type.as_str() {
            "transcript" => "transcript".to_owned(),
            _ => "report document".to_owned(),
        },
    };
    let prompt = claim_extraction_prompt(&request);

    state
        .mark_claim_extraction_job_running(&job.id)
        .map_err(|error| ("extraction_failed", error.to_string()))?;

    let text = tauri::async_runtime::block_on(provider.complete_document(&prompt, &document))
        .map_err(|error| (error.code(), error.to_string()))?;

    let output = parse_claim_extraction_output(&text, provider.provider_id())
        .map_err(|error| (error.code(), error.to_string()))?;

    // A transcript-sourced claim is attributed to the transcript (job-level), a
    // report-sourced claim to the report document.
    let (source_evidence_type, source_evidence_id) = match job.source_type.as_str() {
        "transcript" => ("transcript", job.source_id.clone()),
        _ => ("report_document", job.source_id.clone()),
    };

    let proposals = output
        .claims
        .into_iter()
        .map(|claim| storage::NewClaimProposal {
            statement: claim.statement,
            due_fiscal_year: claim.due_fiscal_year,
            due_period_type: claim.due_period_type,
            target_metric_key: claim.target_metric_key,
            target_comparator: claim.target_comparator,
            target_value_numeric: claim.target_value_numeric,
            target_unit: claim.target_unit,
            confidence: claim.confidence,
            source_snippet: claim.source_snippet,
            source_evidence_type: Some(source_evidence_type.to_owned()),
            source_evidence_id: Some(source_evidence_id.clone()),
        })
        .collect();

    Ok(storage::CompletedClaimExtraction {
        job_id: job.id.clone(),
        proposals,
    })
}

fn load_source(
    state: &app_state::AppState,
    job: &storage::ClaimExtractionJob,
) -> Result<AnalysisDocument, (&'static str, String)> {
    match job.source_type.as_str() {
        "transcript" => {
            let segments = state
                .list_transcript_segments(&job.source_id)
                .map_err(|error| ("source_not_found", error.to_string()))?;
            if segments.is_empty() {
                return Err((
                    "source_not_found",
                    "the transcript has no segments to extract from".to_owned(),
                ));
            }
            let text = segments
                .iter()
                .map(|segment| segment.text.trim())
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            Ok(AnalysisDocument::Text { text })
        }
        _ => {
            let document = state
                .get_report_document(&job.source_id)
                .map_err(|error| ("source_not_found", error.to_string()))?;
            if document.fetch_status != "fetched" {
                return Err((
                    "source_not_found",
                    "the report document has not been fetched yet".to_owned(),
                ));
            }
            let local_path = document.local_path.ok_or_else(|| {
                (
                    "source_not_found",
                    "the report document has no stored file".to_owned(),
                )
            })?;
            let path = state.data_dir().join(&local_path);
            let data = std::fs::read(&path).map_err(|error| {
                (
                    "source_not_found",
                    format!("failed to read report document file: {error}"),
                )
            })?;
            let mime_type = document
                .content_type
                .map(|value| value.split(';').next().unwrap_or("").trim().to_owned())
                .filter(|value| !value.is_empty() && value != "application/octet-stream")
                .unwrap_or_else(|| "application/pdf".to_owned());
            Ok(AnalysisDocument::Native { mime_type, data })
        }
    }
}

fn company_name(state: &app_state::AppState, job: &storage::ClaimExtractionJob) -> String {
    state
        .list_companies()
        .ok()
        .and_then(|companies| {
            companies
                .into_iter()
                .find(|company| company.id == job.company_id)
                .map(|company| company.display_name)
        })
        .unwrap_or_else(|| job.company_id.clone())
}

fn provider_for_job(
    state: &app_state::AppState,
    job: &storage::ClaimExtractionJob,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    let timeout_seconds = settings.ai_providers.general_analysis_timeout_seconds;
    if job.provider_id == CAPABILITY_ROUTED_PROVIDER_ID {
        return crate::jobs::build_capability_provider(
            state,
            AiCapability::ClaimExtraction,
            timeout_seconds,
        );
    }
    let api_key = registry::read_analysis_provider_api_key(&job.provider_id);
    crate::jobs::build_gated_analysis_provider(
        state,
        &job.provider_id,
        api_key,
        &job.model,
        timeout_seconds,
    )
}

#[cfg(test)]
mod tests {
    use super::run_claim_extraction_job;
    use crate::{
        providers::analysis::{TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID},
        storage::{
            open_in_memory_database, AppState, CaptureReportDocumentInput,
            ConfirmClaimProposalInput, NewClaimExtractionJob, NewCompany, NewTranscriptJob,
            NewTranscriptSegment,
        },
    };

    fn state_with_company() -> (AppState, String) {
        let dir =
            std::env::temp_dir().join(format!("brawler-claim-extraction-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp data dir");
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::with_data_dir(connection, dir);
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
        (state, company.id)
    }

    fn report_document(state: &AppState, company_id: &str) -> String {
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.to_owned(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/report.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Q4 report".to_owned()),
                attribution: Some("IR".to_owned()),
            })
            .expect("report document captured");
        std::fs::write(
            state.data_dir().join("claim-report.pdf"),
            b"%PDF-1.4 sample",
        )
        .expect("write sample pdf");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("claim-report.pdf"),
                Some("application/pdf"),
                None,
                Some(15),
            )
            .expect("mark fetched");
        document.id
    }

    fn transcript(state: &AppState, company_id: &str) -> String {
        let job = state
            .create_transcript_job(NewTranscriptJob {
                company_id: Some(company_id.to_owned()),
                provider_id: Some("provider_gemini".to_owned()),
                source_url: "https://youtu.be/sample".to_owned(),
                source_label: Some("Earnings call".to_owned()),
                recognized_company_candidates: None,
            })
            .expect("transcript job created");
        state
            .create_transcript_segment(NewTranscriptSegment {
                transcript_job_id: job.id.clone(),
                company_id: Some(company_id.to_owned()),
                start_seconds: Some(0),
                end_seconds: Some(30),
                speaker: Some("CEO".to_owned()),
                text: "We expect net revenue of at least 1 mln by the fourth quarter.".to_owned(),
                language: Some("en".to_owned()),
            })
            .expect("segment created");
        job.id
    }

    fn run_for_source(source_type: &str) {
        let (state, company_id) = state_with_company();
        let real_source_id = match source_type {
            "transcript" => transcript(&state, &company_id),
            _ => report_document(&state, &company_id),
        };
        let job = state
            .create_claim_extraction_job(NewClaimExtractionJob {
                company_id: company_id.clone(),
                source_type: source_type.to_owned(),
                source_id: real_source_id,
                provider_id: TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
                model: TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
                prompt_version: "claim-extraction.v1".to_owned(),
            })
            .expect("extraction job created");

        let completed = run_claim_extraction_job(&state, &job.id).expect("job runs");
        assert_eq!(completed.status, "succeeded");
        assert_eq!(completed.proposals.len(), 2, "two sample claims proposed");
        assert!(completed.proposals.iter().all(|p| p.status == "pending"));

        // Extraction writes no claims.
        assert!(state
            .list_management_claims(&company_id)
            .expect("claims list")
            .is_empty());

        // Confirming one proposal materialises exactly one claim with its provenance.
        let quantitative = completed
            .proposals
            .iter()
            .find(|p| p.target_metric_key.as_deref() == Some("net_revenue"))
            .expect("quantitative proposal");
        let claim = state
            .confirm_claim_proposal(ConfirmClaimProposalInput {
                proposal_id: quantitative.id.clone(),
                ..Default::default()
            })
            .expect("confirm proposal");
        assert_eq!(claim.status, "pending");
        assert_eq!(claim.due_fiscal_year, Some(2026));
        assert_eq!(claim.due_period_type.as_deref(), Some("Q4"));
        assert_eq!(
            claim.extraction_proposal_id.as_deref(),
            Some(quantitative.id.as_str())
        );

        // Reject the other proposal; it stays rejected (suppresses re-proposal).
        let qualitative = completed
            .proposals
            .iter()
            .find(|p| p.target_metric_key.is_none())
            .expect("qualitative proposal");
        let job = state
            .reject_claim_proposal(&qualitative.id)
            .expect("reject proposal");
        assert!(job
            .proposals
            .iter()
            .any(|p| p.id == qualitative.id && p.status == "rejected"));

        let claims = state
            .list_management_claims(&company_id)
            .expect("claims list");
        assert_eq!(
            claims.len(),
            1,
            "only the confirmed proposal becomes a claim"
        );
    }

    #[test]
    fn extracts_and_confirms_claims_from_a_report_document() {
        run_for_source("report_document");
    }

    #[test]
    fn extracts_and_confirms_claims_from_a_transcript() {
        run_for_source("transcript");
    }
}
