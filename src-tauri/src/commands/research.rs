use crate::{app_state, storage};

#[tauri::command]
pub fn list_research_evidence(
    input: storage::ResearchEvidenceInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ResearchEvidenceItem>, String> {
    state
        .list_research_evidence(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_company_timeline(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ResearchEvidenceItem>, String> {
    state
        .list_research_evidence(storage::ResearchEvidenceInput {
            company_id: Some(company_id),
            watchlist_id: None,
            limit: None,
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_watchlist_timeline(
    watchlist_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ResearchEvidenceItem>, String> {
    state
        .list_research_evidence(storage::ResearchEvidenceInput {
            company_id: None,
            watchlist_id: Some(watchlist_id),
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
pub fn create_evidence_link(
    input: storage::NewEvidenceLink,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::EvidenceLink, String> {
    state
        .create_evidence_link(input)
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
