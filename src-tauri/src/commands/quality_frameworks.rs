//! Tauri commands for quality frameworks (ADR 0046). Thin wrappers over the
//! storage facade; all access to frameworks/criteria/evaluations goes through
//! these typed commands (the UI↔Rust driving-adapter seam, ADR 0039).

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{app_state, jobs::qualitative_assessment::QUALITATIVE_ASSESSMENT_KIND, storage};

/// Retries for an enqueued assessment run: the job has no per-job status table
/// (the `job_queue` row IS its status), so a transient provider hiccup should
/// get a couple of retries before the run lands `failed` and surfaces via the
/// jobs read model. Deterministic failures (no provider, uncited response)
/// simply exhaust these fast — matching feed-analysis behavior (ADR 0075 T5).
const ASSESSMENT_MAX_ATTEMPTS: i64 = 3;

/// Suffix of the deterministic **follow-up** row a re-run parks in when the main
/// `company:framework` assessment job is already `running` (so a request arriving
/// mid-run is never dropped and no criterion is assessed twice). MUST match the
/// sibling-serialization guard in `storage::jobs` claim queries, which keeps a row
/// and its `…{FOLLOWUP_SUFFIX}` sibling from ever being claimed (and thus run)
/// concurrently — the ai worker lane has more than one worker (ADR 0059).
const FOLLOWUP_SUFFIX: &str = ":followup";

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
/// There is **one** primary job-queue row per `company:framework` so a full run
/// and a single-criterion re-run never race two concurrent jobs over the same
/// criterion (which would duplicate a paid AI request). `criterion_ids = None`
/// means "all qualitative criteria"; a re-run passes a subset. When a not-yet-
/// started job is already pending, the two requests are **merged** (all-criteria
/// wins over a subset; two subsets union) so no criterion is dropped and none is
/// assessed twice.
///
/// A request arriving while the primary row is `running` is **not dropped**: it is
/// parked in a deterministic follow-up row (`<id>{FOLLOWUP_SUFFIX}`) with the same
/// payload shape, merging criterion subsets the same way. The queue's sibling
/// guard (see [`FOLLOWUP_SUFFIX`] and the `storage::jobs` claim queries) keeps the
/// primary and follow-up rows from ever running concurrently, and the worker claims
/// the follow-up FIFO once the in-flight run finishes. Resolution order (never
/// leaves two pending sibling rows,
/// which the multi-worker ai lane could otherwise claim at once and double-run):
/// 1. A follow-up is already parked → merge into it and re-arm only that row.
/// 2. Else re-arm the primary row (merging any still-pending primary request);
///    `reschedule` succeeds unless the primary is running.
/// 3. Else (primary running) → park this request in the follow-up row.
pub(crate) fn enqueue_assessment(
    state: &app_state::AppState,
    company_id: &str,
    framework_id: &str,
    criterion_ids: Option<Vec<String>>,
) -> Result<(), String> {
    let jobs = state.jobs();
    let main_id = format!("{QUALITATIVE_ASSESSMENT_KIND}:{company_id}:{framework_id}");
    let followup_id = format!("{main_id}{FOLLOWUP_SUFFIX}");

    // (1) A follow-up is already parked behind a running primary → merge into it and
    // re-arm only that row. Never also touch the primary row: two pending sibling
    // rows could be claimed by the ai lane's two workers at once and double-run.
    if let Some(pending) = jobs
        .pending_payload(&followup_id)
        .map_err(|error| error.to_string())?
    {
        let merged = merge_criterion_ids(&pending, criterion_ids);
        return reschedule_assessment(&jobs, &followup_id, company_id, framework_id, &merged);
    }

    // (2) Re-arm the primary row, merging with any still-pending primary request so a
    // superseding enqueue never narrows coverage. `reschedule` returns false only if
    // the primary row is currently running.
    let main_merged = match jobs
        .pending_payload(&main_id)
        .map_err(|error| error.to_string())?
    {
        Some(pending) => merge_criterion_ids(&pending, criterion_ids.clone()),
        None => criterion_ids.clone(),
    };
    let payload = assessment_payload(company_id, framework_id, &main_merged);
    if jobs
        .reschedule(
            &main_id,
            QUALITATIVE_ASSESSMENT_KIND,
            &payload,
            ASSESSMENT_MAX_ATTEMPTS,
        )
        .map_err(|error| error.to_string())?
    {
        return Ok(());
    }

    // (3) The primary row is running → park this request in the follow-up row so it
    // is never dropped and never double-runs. The follow-up was empty (checked in
    // step 1), so its coverage is exactly this request.
    reschedule_assessment(
        &jobs,
        &followup_id,
        company_id,
        framework_id,
        &criterion_ids,
    )
}

/// Build the opaque JSON payload for an assessment job. `criterion_ids = None`
/// omits `criterionIds` (assess all qualitative criteria).
fn assessment_payload(
    company_id: &str,
    framework_id: &str,
    criterion_ids: &Option<Vec<String>>,
) -> String {
    let mut payload = json!({ "companyId": company_id, "frameworkId": framework_id });
    if let Some(ids) = criterion_ids {
        payload["criterionIds"] = json!(ids);
    }
    payload.to_string()
}

/// Re-arm `id` (primary or follow-up) with the given merged criterion set.
fn reschedule_assessment(
    jobs: &storage::JobQueueStore,
    id: &str,
    company_id: &str,
    framework_id: &str,
    criterion_ids: &Option<Vec<String>>,
) -> Result<(), String> {
    let payload = assessment_payload(company_id, framework_id, criterion_ids);
    jobs.reschedule(
        id,
        QUALITATIVE_ASSESSMENT_KIND,
        &payload,
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

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct GetQualitativeAssessmentStatusInput {
    pub company_id: String,
    pub framework_id: String,
}

/// Narrow lifecycle read for the durable `qualitative_assessment` job of one
/// company × framework (P1, owner T7). Lets the Quality panel surface a terminal
/// failure — the previous "queued" hint cleared silently even when the async job
/// erred (no provider, uncited response). `status` maps the queue row honestly:
/// `pending → queued`, `running`, `failed`, `succeeded`; a missing row is
/// `succeeded` when an assessment is already stored, else `idle`. `lastError` is
/// the queue's recorded failure message (only meaningful on `failed`).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct QualitativeAssessmentStatus {
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"idle\" | \"queued\" | \"running\" | \"failed\" | \"succeeded\"")
    )]
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
}

/// Map the durable queue row for `qualitative_assessment:<company>:<framework>`
/// into the panel's status shape. Pure (testable without Tauri) — the command is
/// a thin async wrapper. Never invents queue fields: it reads status/attempts/
/// last_error as the queue stores them.
pub(crate) fn qualitative_assessment_status(
    state: &app_state::AppState,
    company_id: &str,
    framework_id: &str,
) -> Result<QualitativeAssessmentStatus, String> {
    let job_id = format!("{QUALITATIVE_ASSESSMENT_KIND}:{company_id}:{framework_id}");
    if let Some(row) = state
        .jobs()
        .status(&job_id)
        .map_err(|error| error.to_string())?
    {
        let status = match row.status.as_str() {
            "pending" => "queued",
            "running" => "running",
            "failed" => "failed",
            "succeeded" => "succeeded",
            // Defensive: surface an unknown persisted status verbatim rather than
            // guess — masking a schema change would be the worse failure.
            other => other,
        };
        return Ok(QualitativeAssessmentStatus {
            status: status.to_owned(),
            attempts: row.attempts,
            last_error: row.last_error,
        });
    }

    // No queue row: a stored assessment means a prior run succeeded (its terminal
    // row since pruned/re-armed); nothing stored means the job never ran (idle).
    let assessed = !state
        .get_qualitative_assessment(framework_id, company_id)
        .map_err(|error| error.to_string())?
        .is_empty();
    Ok(QualitativeAssessmentStatus {
        status: if assessed { "succeeded" } else { "idle" }.to_owned(),
        attempts: 0,
        last_error: None,
    })
}

/// Read the durable assessment job's lifecycle status for the Quality panel poll
/// (ADR 0075, P1). Async + `spawn_blocking`: it does two DB reads off the UI
/// thread (the queue row, then a fallback current-state read).
#[tauri::command]
pub async fn get_qualitative_assessment_status(
    input: GetQualitativeAssessmentStatusInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<QualitativeAssessmentStatus, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        qualitative_assessment_status(&state, &input.company_id, &input.framework_id)
    })
    .await
    .map_err(|error| format!("qualitative assessment status task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::open_in_memory_database;

    fn job_id(company: &str, framework: &str) -> String {
        format!("{QUALITATIVE_ASSESSMENT_KIND}:{company}:{framework}")
    }

    fn followup_id(company: &str, framework: &str) -> String {
        format!("{}{FOLLOWUP_SUFFIX}", job_id(company, framework))
    }

    fn pending(state: &AppState, company: &str, framework: &str) -> String {
        state
            .jobs()
            .pending_payload(&job_id(company, framework))
            .expect("query")
            .expect("a pending assessment job")
    }

    fn pending_followup(state: &AppState, company: &str, framework: &str) -> String {
        state
            .jobs()
            .pending_payload(&followup_id(company, framework))
            .expect("query")
            .expect("a pending follow-up assessment job")
    }

    // Drive the main company:framework job to `running` (as an in-flight assessment
    // would be) so the next enqueue must park in the follow-up row.
    fn start_main_run(state: &AppState, company: &str, framework: &str) {
        let claimed = state
            .jobs()
            .claim_next()
            .expect("claim")
            .expect("a runnable main job");
        assert_eq!(claimed.id, job_id(company, framework));
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

    // ADR 0075 T6 (review): a request arriving while the main job is *running* must
    // not be silently dropped (the old `reschedule` returned false and the bool was
    // discarded). It is parked in the deterministic follow-up row, never disturbing
    // the in-flight run and never double-running the same criterion.
    #[test]
    fn nothing_running_uses_the_main_row_and_creates_no_followup() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", None).expect("full run");

        assert!(
            state
                .jobs()
                .pending_payload(&job_id("co", "fw"))
                .expect("query")
                .is_some(),
            "the main row is armed"
        );
        assert!(
            state
                .jobs()
                .pending_payload(&followup_id("co", "fw"))
                .expect("query")
                .is_none(),
            "no follow-up row is created when nothing is running"
        );
        assert_eq!(state.jobs().counts().expect("counts").pending, 1);
    }

    #[test]
    fn a_running_main_parks_the_request_in_a_followup_row() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", None).expect("full run");
        start_main_run(&state, "co", "fw"); // main now running

        // A re-run arriving mid-flight is parked, not dropped.
        enqueue_assessment(&state, "co", "fw", None).expect("re-run mid-flight");

        let followup = pending_followup(&state, "co", "fw");
        assert!(
            !followup.contains("criterionIds"),
            "the follow-up covers all criteria: {followup}"
        );
        // The main run is untouched (never double-run) and still the only running row.
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.running, 1);
        assert_eq!(counts.pending, 1, "exactly one parked follow-up");
    }

    #[test]
    fn mid_run_requests_merge_into_the_followup_row() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", Some(vec!["X".to_owned()])).expect("initial");
        start_main_run(&state, "co", "fw"); // main now running

        // Two subset re-runs arrive while the main runs → they union in the follow-up.
        enqueue_assessment(&state, "co", "fw", Some(vec!["X".to_owned()])).expect("mid-run X");
        enqueue_assessment(&state, "co", "fw", Some(vec!["Y".to_owned()])).expect("mid-run Y");

        let followup = pending_followup(&state, "co", "fw");
        assert!(
            followup.contains("\"X\"") && followup.contains("\"Y\""),
            "mid-run subsets union into the one follow-up row: {followup}"
        );
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(counts.running, 1, "the main run is undisturbed");
        assert_eq!(counts.pending, 1, "still a single follow-up row");
    }

    #[test]
    fn an_all_criteria_mid_run_request_absorbs_a_pending_followup_subset() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", None).expect("initial full run");
        start_main_run(&state, "co", "fw"); // main now running

        enqueue_assessment(&state, "co", "fw", Some(vec!["X".to_owned()])).expect("mid-run X");
        enqueue_assessment(&state, "co", "fw", None).expect("mid-run all");

        let followup = pending_followup(&state, "co", "fw");
        assert!(
            !followup.contains("criterionIds"),
            "an all-criteria request absorbs the subset in the follow-up: {followup}"
        );
    }

    // ---- get_qualitative_assessment_status (P1, owner T7) -------------------

    // Drive the main job to a terminal `failed` row, recording `error` as its
    // last error. `mark_failed` only lands terminal once attempts reach
    // max_attempts, so claim+fail until the queue reports it failed.
    fn fail_main_run_terminally(state: &AppState, company: &str, framework: &str, error: &str) {
        let jobs = state.jobs();
        let id = job_id(company, framework);
        for _ in 0..(ASSESSMENT_MAX_ATTEMPTS + 2) {
            let Some(claimed) = jobs.claim_next().expect("claim") else {
                break;
            };
            assert_eq!(claimed.id, id);
            jobs.mark_failed(&id, error, 0).expect("mark failed");
            if jobs.counts().expect("counts").failed == 1 {
                return;
            }
        }
        panic!("job did not reach a terminal failed state");
    }

    #[test]
    fn status_is_idle_when_never_enqueued_and_nothing_stored() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let status = qualitative_assessment_status(&state, "co", "fw").expect("status");
        assert_eq!(status.status, "idle");
        assert_eq!(status.attempts, 0);
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn status_is_queued_after_enqueue() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", None).expect("enqueue");

        let status = qualitative_assessment_status(&state, "co", "fw").expect("status");
        assert_eq!(status.status, "queued", "a pending row reads as queued");
        assert_eq!(status.attempts, 0);
        assert_eq!(status.last_error, None);
    }

    #[test]
    fn status_is_failed_with_the_backend_error_when_the_job_exhausts_retries() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", None).expect("enqueue");
        fail_main_run_terminally(&state, "co", "fw", "no usable AI provider for capability");

        let status = qualitative_assessment_status(&state, "co", "fw").expect("status");
        assert_eq!(status.status, "failed");
        assert_eq!(status.attempts, ASSESSMENT_MAX_ATTEMPTS);
        assert_eq!(
            status.last_error.as_deref(),
            Some("no usable AI provider for capability"),
            "the terminal failure surfaces the queue's recorded error"
        );
    }

    #[test]
    fn status_is_succeeded_when_the_queue_row_succeeded() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_assessment(&state, "co", "fw", None).expect("enqueue");
        let claimed = state.jobs().claim_next().expect("claim").expect("runnable");
        state.jobs().mark_succeeded(&claimed.id).expect("succeed");

        let status = qualitative_assessment_status(&state, "co", "fw").expect("status");
        assert_eq!(status.status, "succeeded");
        assert_eq!(status.last_error, None);
    }
}
