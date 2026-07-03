use crate::{
    app_state, jobs,
    providers::analysis::{
        capabilities::{AiCapability, CAPABILITY_ROUTED_PROVIDER_ID},
        TEST_SAMPLE_ANALYSIS_MODEL, TEST_SAMPLE_ANALYSIS_PROVIDER_ID,
    },
    storage,
};

#[tauri::command]
pub fn list_research_evidence(
    input: storage::ResearchEvidenceInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchTimelineResult, String> {
    state
        .list_research_evidence(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_company_timeline(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchTimelineResult, String> {
    state
        .list_research_evidence(storage::ResearchEvidenceInput {
            company_id: Some(company_id),
            watchlist_id: None,
            evidence_types: None,
            changed_since_review_only: None,
            limit: None,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_watchlist_timeline(
    watchlist_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchTimelineResult, String> {
    state
        .list_research_evidence(storage::ResearchEvidenceInput {
            company_id: None,
            watchlist_id: Some(watchlist_id),
            evidence_types: None,
            changed_since_review_only: None,
            limit: None,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn mark_research_scope_reviewed(
    input: storage::ResearchReviewCheckpointInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchReviewCheckpoint, String> {
    state
        .mark_research_scope_reviewed(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_research_review_state(
    input: storage::ResearchReviewCheckpointInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<storage::ResearchReviewCheckpoint>, String> {
    state
        .list_research_review_state(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_research_questions(
    input: storage::ResearchQuestionListInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ResearchQuestion>, String> {
    state
        .list_research_questions(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_research_question(
    input: storage::NewResearchQuestion,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchQuestion, String> {
    state
        .create_research_question(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_research_question(
    input: storage::ResearchQuestionUpdate,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchQuestion, String> {
    state
        .update_research_question(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_research_question(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_research_question(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_evidence_link(
    input: storage::NewEvidenceLink,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::EvidenceLink, String> {
    state
        .create_evidence_link(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_evidence_links(
    input: storage::EvidenceLinkListInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::EvidenceLink>, String> {
    state
        .list_evidence_links(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_evidence_link(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_evidence_link(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn start_research_brief(
    input: storage::ResearchBriefScopeInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchBriefJob, String> {
    // No explicit override input exists for this command: defer to capability
    // routing at run time when anything is actually configured; otherwise keep
    // the legacy test-sample default so brief generation still runs out of the
    // box (ADR 0060 as amended — mirrors the KPI/claim enqueue pin).
    let members = jobs::resolve_capability_members(&state, AiCapability::ResearchBrief)
        .map_err(|error| error.to_string())?;
    let (provider_id, model) = if members.is_empty() {
        (
            TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
            TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
        )
    } else {
        (
            CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
            CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
        )
    };
    let job = state
        .create_research_brief_job(storage::NewResearchBriefJob {
            scope_type: input.scope_type,
            scope_id: input.scope_id,
            provider_id,
            model,
        })
        .map_err(|error| error.to_string())?;

    spawn_research_brief_job(state.inner().clone(), job.id.clone());

    Ok(job)
}

#[tauri::command]
pub fn list_research_briefs(
    input: storage::ResearchBriefScopeInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ResearchBriefJob>, String> {
    state
        .list_research_brief_jobs(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_research_reminders(
    input: storage::ResearchReminderListInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ResearchReminder>, String> {
    state
        .list_research_reminders(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_research_reminder(
    input: storage::NewResearchReminder,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchReminder, String> {
    state
        .create_research_reminder(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_research_reminder(
    input: storage::ResearchReminderUpdate,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchReminder, String> {
    state
        .update_research_reminder(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_research_reminder(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_research_reminder(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn start_research_digest(
    input: storage::ResearchDigestScopeInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ResearchDigestJob, String> {
    // No explicit override input exists for this command: defer to capability
    // routing at run time when anything is actually configured; otherwise keep
    // the legacy test-sample default so digest generation still runs out of the
    // box (ADR 0060 as amended — mirrors the KPI/claim enqueue pin).
    let members = jobs::resolve_capability_members(&state, AiCapability::ResearchDigest)
        .map_err(|error| error.to_string())?;
    let (provider_id, model) = if members.is_empty() {
        (
            TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
            TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
        )
    } else {
        (
            CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
            CAPABILITY_ROUTED_PROVIDER_ID.to_owned(),
        )
    };
    let job = state
        .create_research_digest_job(storage::NewResearchDigestJob {
            scope_type: input.scope_type,
            scope_id: input.scope_id,
            provider_id,
            model,
        })
        .map_err(|error| error.to_string())?;

    spawn_research_digest_job(state.inner().clone(), job.id.clone());

    Ok(job)
}

#[tauri::command]
pub fn list_research_digests(
    input: storage::ResearchDigestScopeInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ResearchDigestJob>, String> {
    state
        .list_research_digest_jobs(input)
        .map_err(|error| error.to_string())
}

fn spawn_research_brief_job(state: app_state::AppState, job_id: String) {
    jobs::handlers::enqueue_per_job(&state, jobs::handlers::RESEARCH_BRIEF_KIND, &job_id);
}

fn spawn_research_digest_job(state: app_state::AppState, job_id: String) {
    jobs::handlers::enqueue_per_job(&state, jobs::handlers::RESEARCH_DIGEST_KIND, &job_id);
}
