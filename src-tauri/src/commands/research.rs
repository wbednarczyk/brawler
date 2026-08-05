//! Research surface. Brief/digest **generation** is retired (ADR 0084
//! decision 1); `list_research_briefs` / `list_research_digests` reads
//! survive as already-stored user data (decision 5).

use crate::{app_state, storage};

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
