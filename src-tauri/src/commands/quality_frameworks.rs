//! Tauri commands for quality frameworks (ADR 0046). Thin wrappers over the
//! storage facade; all access to frameworks/criteria/evaluations goes through
//! these typed commands (the UI↔Rust driving-adapter seam, ADR 0039).

use serde::Deserialize;

use crate::commands::error::{CommandError, CommandErrorCode};
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

// ---- Qualitative assessment (ADR 0075, as amended by ADR 0084) -------------
//
// Criterion verdicts are agent-written with provenance through the MCP
// write-tools (`set_qualitative_verdicts`, ADR 0088); the read below survives
// under ADR 0084 decision 5 as user data a previous version stored.

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct GetQualitativeAssessmentInput {
    pub company_id: String,
    pub framework_id: String,
}

// ---- set_qualitative_verdicts (MCP-first write, ADR 0088 M3 / ADR 0084 dec. 5)
//
// The qualitative-verdict WRITE path: verdicts arrive through the
// provenance-gated MCP `act` tier — every criterion result must carry a
// non-empty `citationsJson` evidence array (the registry classifies this
// command Act + `CitationsJson`). Headless / MCP-only: no UI entry point
// (verdicts are authored by a connected agent, not in-app). The MCP act
// handler and this typed command share [`build_persist_qualitative_input`]
// so their behavior can never diverge.

/// One agent-authored qualitative criterion verdict. `ordinal` and `label` are
/// resolved from the framework's criteria (not supplied by the caller);
/// `citationsJson` is the serialized typed-evidence array (provenance).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct QualitativeVerdictInput {
    pub criterion_id: String,
    /// `pass` | `partial` | `fail` | `insufficient_evidence`.
    pub verdict: String,
    pub reasoning: String,
    /// Serialized, non-empty typed-evidence array — the write's provenance.
    pub citations_json: String,
    /// `low` | `medium` | `high`.
    pub confidence: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SetQualitativeVerdictsInput {
    pub framework_id: String,
    pub company_id: String,
    pub results: Vec<QualitativeVerdictInput>,
}

/// Provenance marker written to each criterion result's `prompt_version` for
/// verdicts authored over the MCP act tier.
pub const MCP_VERDICT_PROMPT_VERSION: &str = "mcp";

/// Map a [`SetQualitativeVerdictsInput`] onto the storage
/// [`storage::PersistQualitativeAssessmentInput`], resolving each result's
/// `ordinal`/`label` from the framework criteria. Shared by the typed command
/// and the MCP act handler (single source of truth). An unknown `criterionId`
/// or an empty `results` list is `invalid_input`.
pub fn build_persist_qualitative_input(
    state: &app_state::AppState,
    input: SetQualitativeVerdictsInput,
) -> Result<storage::PersistQualitativeAssessmentInput, CommandError> {
    if input.results.is_empty() {
        return Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            "results must contain at least one criterion verdict",
        ));
    }
    let framework = state
        .get_quality_framework(&input.framework_id)
        .map_err(CommandError::from)?;
    let by_id: std::collections::HashMap<&str, (i64, &str)> = framework
        .criteria
        .iter()
        .map(|criterion| {
            (
                criterion.id.as_str(),
                (criterion.ordinal, criterion.label.as_str()),
            )
        })
        .collect();

    let mut results = Vec::with_capacity(input.results.len());
    for result in input.results {
        let (ordinal, label) = by_id.get(result.criterion_id.as_str()).ok_or_else(|| {
            CommandError::new(
                CommandErrorCode::InvalidInput,
                format!(
                    "criterion {} is not part of framework {}",
                    result.criterion_id, framework.id
                ),
            )
        })?;
        results.push(storage::QualitativeCriterionResult {
            criterion_id: result.criterion_id,
            ordinal: *ordinal,
            label: (*label).to_owned(),
            verdict: result.verdict,
            reasoning: result.reasoning,
            citations_json: result.citations_json,
            confidence: result.confidence,
            prompt_version: MCP_VERDICT_PROMPT_VERSION.to_owned(),
        });
    }
    Ok(storage::PersistQualitativeAssessmentInput {
        framework_id: input.framework_id,
        company_id: input.company_id,
        results,
    })
}

/// Persist a batch of agent-authored qualitative verdicts as one immutable
/// snapshot (ADR 0075). Async + `spawn_blocking` (DoD §C).
#[tauri::command]
pub async fn set_qualitative_verdicts(
    input: SetQualitativeVerdictsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FrameworkEvaluation, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let persist = build_persist_qualitative_input(&state, input)?;
        state
            .persist_qualitative_assessment(persist)
            .map_err(CommandError::from)
    })
    .await
    .map_err(|error| {
        CommandError::new(CommandErrorCode::Internal, format!("task failed: {error}"))
    })?
}
