use crate::{
    app_state,
    providers::analysis::{
        capabilities::{AiCapability, CAPABILITY_ROUTED_PROVIDER_ID},
        registry, AiAnalysisProvider, ResearchDigestRequest,
    },
    storage,
};

pub fn run_research_digest_job(
    state: &app_state::AppState,
    job_id: &str,
) -> Result<storage::ResearchDigestJob, String> {
    let job = state
        .get_research_digest_job(job_id)
        .map_err(|error| error.to_string())?;
    if job.status == "succeeded" {
        return Ok(job);
    }

    let provider = provider_for_job(state, &job)?;
    let context = state
        .collect_research_digest_evidence(job_id)
        .map_err(|error| error.to_string())?;
    if context.evidence_items.is_empty() {
        return state
            .mark_research_digest_job_failed(
                job_id,
                "empty_evidence",
                "No changed research evidence or open reminders are available for this digest.",
            )
            .map_err(|error| error.to_string());
    }

    state
        .mark_research_digest_job_running(job_id)
        .map_err(|error| error.to_string())?;

    let output = match tauri::async_runtime::block_on(provider.generate_research_digest(
        &ResearchDigestRequest {
            scope_type: context.scope_type,
            scope_id: context.scope_id,
            evidence_items: context.evidence_items.clone(),
        },
    )) {
        Ok(output) => output,
        Err(error) => {
            return state
                .mark_research_digest_job_failed(job_id, error.code(), &error.to_string())
                .map_err(|storage_error| storage_error.to_string());
        }
    };

    let completed =
        storage::completed_digest_from_provider_output(job_id, &context.evidence_items, output)?;
    state
        .complete_research_digest_job(completed)
        .map_err(|error| error.to_string())
}

fn provider_for_job(
    state: &app_state::AppState,
    job: &storage::ResearchDigestJob,
) -> Result<Box<dyn AiAnalysisProvider>, String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    if job.provider_id == CAPABILITY_ROUTED_PROVIDER_ID {
        return crate::jobs::build_capability_provider(
            state,
            AiCapability::ResearchDigest,
            settings.ai_providers.general_analysis_timeout_seconds,
        );
    }
    let api_key = registry::read_analysis_provider_api_key(&job.provider_id);
    crate::jobs::build_gated_analysis_provider(
        state,
        &job.provider_id,
        api_key,
        &job.model,
        settings.ai_providers.general_analysis_timeout_seconds,
    )
}
