//! Tauri commands for quality frameworks (ADR 0046). Thin wrappers over the
//! storage facade; all access to frameworks/criteria/evaluations goes through
//! these typed commands (the UI↔Rust driving-adapter seam, ADR 0039).

use crate::{app_state, storage};

#[tauri::command]
pub fn list_quality_frameworks(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::QualityFramework>, String> {
    state
        .list_quality_frameworks()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_quality_framework(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::QualityFramework, String> {
    state
        .get_quality_framework(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_quality_framework(
    input: storage::NewQualityFramework,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::QualityFramework, String> {
    state
        .create_quality_framework(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_quality_framework(
    input: storage::UpdateQualityFramework,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::QualityFramework, String> {
    state
        .update_quality_framework(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_quality_framework(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_quality_framework(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clone_framework(
    input: storage::CloneFrameworkInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::QualityFramework, String> {
    state
        .clone_framework(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reset_framework_to_template(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::QualityFramework, String> {
    state
        .reset_framework_to_template(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_framework_criterion(
    input: storage::NewFrameworkCriterion,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FrameworkCriterion, String> {
    state
        .create_framework_criterion(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_framework_criterion(
    input: storage::UpdateFrameworkCriterion,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FrameworkCriterion, String> {
    state
        .update_framework_criterion(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_framework_criterion(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_framework_criterion(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn validate_criterion_expression(
    expression: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ValidateCriterionResult, String> {
    Ok(state.validate_criterion_expression(&expression))
}

#[tauri::command]
pub fn evaluate_framework(
    input: storage::EvaluateFrameworkInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FrameworkEvaluation, String> {
    state
        .evaluate_framework(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_framework_evaluations(
    input: storage::ListFrameworkEvaluationsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::FrameworkEvaluation>, String> {
    state
        .list_framework_evaluations(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_framework_evaluation(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FrameworkEvaluation, String> {
    state
        .get_framework_evaluation(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_framework_evaluation(
    id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_framework_evaluation(&id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_available_metric_keys(
    company_id: Option<String>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::MetricKeyInfo>, String> {
    state
        .list_available_metric_keys(company_id.as_deref())
        .map_err(|error| error.to_string())
}
