//! Tauri commands for quality frameworks (ADR 0046). Thin wrappers over the
//! storage facade; all access to frameworks/criteria/evaluations goes through
//! these typed commands (the UI↔Rust driving-adapter seam, ADR 0039).

use serde::Deserialize;
use serde_json::json;

use crate::{app_state, jobs::qualitative_assessment::QUALITATIVE_ASSESSMENT_KIND, storage};

/// Retries for an enqueued assessment run: the job has no per-job status table
/// (the `job_queue` row IS its status), so a transient provider hiccup should
/// get a couple of retries before the run lands `failed` and surfaces via the
/// jobs read model. Deterministic failures (no provider, uncited response)
/// simply exhaust these fast — matching feed-analysis behavior (ADR 0075 T5).
const ASSESSMENT_MAX_ATTEMPTS: i64 = 3;

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

// ---- Qualitative assessment (ADR 0075) -------------------------------------

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RunQualitativeAssessmentInput {
    pub company_id: String,
    pub framework_id: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RerunQualitativeCriterionInput {
    pub company_id: String,
    pub framework_id: String,
    pub criterion_id: String,
}

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

/// Enqueue (or re-arm) the single durable assessment job for a company×framework.
/// There is **one** job-queue row per `company:framework` so a full run and a
/// single-criterion re-run never race two concurrent jobs over the same criterion
/// (which would duplicate a paid AI request). `criterion_ids = None` means "all
/// qualitative criteria"; a re-run passes a subset. When a not-yet-started job is
/// already pending, the two requests are **merged** (all-criteria wins over a
/// subset; two subsets union) so no criterion is dropped and none is assessed
/// twice. A `running` row is left untouched (never double-run); the request is
/// re-issued on the next enqueue.
pub(crate) fn enqueue_assessment(
    state: &app_state::AppState,
    company_id: &str,
    framework_id: &str,
    criterion_ids: Option<Vec<String>>,
) -> Result<(), String> {
    let id = format!("{QUALITATIVE_ASSESSMENT_KIND}:{company_id}:{framework_id}");

    // Merge with a still-pending prior request so a superseding enqueue never
    // narrows its coverage: `None` (all) absorbs any subset; two subsets union.
    let merged = match state
        .jobs()
        .pending_payload(&id)
        .map_err(|error| error.to_string())?
    {
        Some(pending) => merge_criterion_ids(&pending, criterion_ids),
        None => criterion_ids,
    };

    let mut payload = json!({ "companyId": company_id, "frameworkId": framework_id });
    if let Some(ids) = merged {
        payload["criterionIds"] = json!(ids);
    }
    state
        .jobs()
        .reschedule(
            &id,
            QUALITATIVE_ASSESSMENT_KIND,
            &payload.to_string(),
            ASSESSMENT_MAX_ATTEMPTS,
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Union a new request's criteria with a pending job's payload. A missing/`null`
/// `criterionIds` on either side means "all qualitative criteria" and wins.
fn merge_criterion_ids(
    pending_payload: &str,
    incoming: Option<Vec<String>>,
) -> Option<Vec<String>> {
    let pending: Option<Vec<String>> = serde_json::from_str::<serde_json::Value>(pending_payload)
        .ok()
        .and_then(|value| {
            value
                .get("criterionIds")
                .and_then(|ids| serde_json::from_value(ids.clone()).ok())
        });
    match (pending, incoming) {
        // Either side asking for "all" absorbs the other.
        (None, _) | (_, None) => None,
        (Some(mut a), Some(b)) => {
            for id in b {
                if !a.contains(&id) {
                    a.push(id);
                }
            }
            Some(a)
        }
    }
}

/// Enqueue the durable `qualitative_assessment` job over the framework's
/// qualitative criteria for a company (ADR 0075). Asynchronous — progress and
/// failure (e.g. no text-capable provider) surface via the jobs read model.
#[tauri::command]
pub fn run_qualitative_assessment(
    input: RunQualitativeAssessmentInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    enqueue_assessment(&state, &input.company_id, &input.framework_id, None)
}

/// Re-enqueue assessment for a single qualitative criterion (the panel's re-run
/// action, ADR 0075). Shares the one per-`company:framework` job row with the
/// full run, so a re-run and a full run never assess the same criterion twice.
#[tauri::command]
pub fn rerun_qualitative_criterion(
    input: RerunQualitativeCriterionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    enqueue_assessment(
        &state,
        &input.company_id,
        &input.framework_id,
        Some(vec![input.criterion_id]),
    )
}

/// Current-state qualitative read for the Quality panel (ADR 0075 Decision 5):
/// per qualitative criterion, the most-recent agent-assessed result across all
/// snapshots. A never-assessed criterion is absent (empty state).
#[tauri::command]
pub fn get_qualitative_assessment(
    input: GetQualitativeAssessmentInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::CriterionResult>, String> {
    state
        .get_qualitative_assessment(&input.framework_id, &input.company_id)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::open_in_memory_database;

    fn job_id(company: &str, framework: &str) -> String {
        format!("{QUALITATIVE_ASSESSMENT_KIND}:{company}:{framework}")
    }

    fn pending(state: &AppState, company: &str, framework: &str) -> String {
        state
            .jobs()
            .pending_payload(&job_id(company, framework))
            .expect("query")
            .expect("a pending assessment job")
    }

    // ADR 0075 T5 (review #1): a framework-wide run and a single-criterion re-run
    // must never race two jobs over the same criterion (duplicate paid AI). They
    // share one per-company:framework row; "all criteria" absorbs a subset.
    #[test]
    fn full_run_absorbs_a_pending_single_criterion_rerun() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", Some(vec!["X".to_owned()])).expect("rerun X");
        enqueue_assessment(&state, "co", "fw", None).expect("full run");

        let payload = pending(&state, "co", "fw");
        assert!(
            !payload.contains("criterionIds"),
            "a full run absorbs the subset (assess all): {payload}"
        );
    }

    #[test]
    fn a_pending_full_run_is_not_narrowed_by_a_later_rerun() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", None).expect("full run");
        enqueue_assessment(&state, "co", "fw", Some(vec!["X".to_owned()])).expect("rerun X");

        let payload = pending(&state, "co", "fw");
        assert!(
            !payload.contains("criterionIds"),
            "the pending full run keeps its all-criteria coverage: {payload}"
        );
    }

    #[test]
    fn two_pending_reruns_union_their_criteria() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", Some(vec!["X".to_owned()])).expect("rerun X");
        enqueue_assessment(&state, "co", "fw", Some(vec!["Y".to_owned()])).expect("rerun Y");

        let payload = pending(&state, "co", "fw");
        assert!(
            payload.contains("\"X\"") && payload.contains("\"Y\""),
            "two subset re-runs union into one job: {payload}"
        );
    }
}
