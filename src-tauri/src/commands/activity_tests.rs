use super::*;
use crate::jobs::activity_identity::{ActivityFamily, ActivityTarget};
use crate::jobs::scheduler::SOURCE_REFRESH_KIND;
use crate::storage::{
    open_in_memory_database, CaptureReportDocumentInput, JobRunOutcome, NewCompany, NewJobRun,
    NewKpiIngestRun,
};

fn state() -> AppState {
    AppState::new(open_in_memory_database().expect("db"))
}

fn company(state: &AppState, ticker: &str) -> String {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company")
        .id
}

#[test]
fn pending_is_queued_not_active() {
    let state = state();
    state
        .jobs()
        .enqueue(
            "src:1",
            SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");

    let view = compute_activity(&state).expect("view");
    assert!(view.active.is_empty());
    assert_eq!(view.queued.len(), 1);
    assert_eq!(view.queued[0].status, "queued");
    assert_eq!(view.queued[0].family, ActivityFamily::SourceRefresh);
}

#[test]
fn retired_kind_rows_are_excluded() {
    let state = state();
    state
        .jobs()
        .enqueue("legacy:1", "qualitative_assessment", "{}", 1)
        .expect("enqueue");

    let view = compute_activity(&state).expect("view");
    assert!(
        view.queued.is_empty(),
        "an unregistered kind's row must not surface as an item"
    );
}

#[test]
fn detection_run_is_one_item_per_document_with_document_target() {
    let state = state();
    let cdr = company(&state, "CDR");
    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: cdr.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/r.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport Q2 2026".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    state
        .autopilot()
        .create_run_if_absent(
            &format!("run:{doc}"),
            &cdr,
            &doc,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    state
        .jobs()
        .enqueue(
            // The real production stage-job id format (`autopilot:{run_id}:{stage}`,
            // `jobs::autopilot::stage_job_id`) — required since the read model's
            // stalled-autopilot-run detection matches it exactly (sol diff R1 #12),
            // and the unified keyed task map (sol diff R1 #5) now surfaces any
            // mismatch as a real precedence conflict instead of silently
            // coexisting in two DTO sections.
            &format!("autopilot:run:{doc}:fetch"),
            crate::jobs::autopilot::AUTOPILOT_STAGE_KIND,
            &format!(r#"{{"run_id":"run:{doc}","stage":"fetch"}}"#),
            1,
        )
        .expect("enqueue");

    let view = compute_activity(&state).expect("view");
    assert_eq!(view.queued.len(), 1);
    let item = &view.queued[0];
    assert_eq!(item.family, ActivityFamily::ReportReading);
    assert_eq!(item.activity_key, format!("report-reading:run:{doc}"));
    match &item.target {
        ActivityTarget::Company { company_id, tool } => {
            assert_eq!(company_id, &cdr);
            assert!(matches!(
                tool,
                Some(crate::jobs::activity_identity::ActivityTool::Dokumenty { .. })
            ));
        }
        other => panic!("expected a company/dokumenty target, got {other:?}"),
    }
}

#[test]
fn sweep_child_run_collapses_into_the_sweep_activity_key() {
    // A stage job belonging to a sweep-triggered run resolves to the SAME
    // `report-sweep:<id>` activity_key as the sweep job itself, so the panel
    // shows one parent item, never a separate child row (ADR 0109 dec. 1).
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");
    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: cdr.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/r2.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport historyczny".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    state
        .autopilot()
        .create_run_if_absent(
            "run:child",
            &cdr,
            &doc,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");

    let connection = state.checkout_for_tests().expect("checkout");
    let identity = crate::jobs::activity_identity::identity_for_job(
        crate::jobs::autopilot::AUTOPILOT_STAGE_KIND,
        "autopilot:child:fetch",
        r#"{"run_id":"run:child","stage":"fetch"}"#,
        &connection,
    )
    .expect("identity");
    assert_eq!(identity.activity_key, format!("report-sweep:{}", sweep.id));
    assert_eq!(identity.family, ActivityFamily::ReportSweep);
}

#[test]
fn recent_cap_applies_after_the_collapse() {
    let state = state();
    // 41 distinct terminal occurrences, all "now" — cap must reduce to 40.
    for i in 0..41 {
        let run_id = state
            .job_runs()
            .begin_attempt(NewJobRun {
                activity_key: format!("source-refresh:adapter-{i}"),
                run_key: format!("job-{i}"),
                kind: SOURCE_REFRESH_KIND.to_owned(),
                family: ActivityFamily::SourceRefresh,
                company_id: None,
                subject: format!("Adapter {i}"),
                target: ActivityTarget::Sources,
                attempt: 1,
            })
            .expect("begin");
        state
            .job_runs()
            .settle(run_id, JobRunOutcome::Succeeded)
            .expect("settle");
    }
    let view = compute_activity(&state).expect("view");
    assert_eq!(view.recent.len(), 40, "cap applies AFTER the collapse");
}

#[test]
fn a_stage_plus_retries_collapses_to_one_recent_item() {
    // sol diff R1 #17: a task that took several ATTEMPTS (retries) before
    // finally terminating — the shape a multi-stage pipeline with retries
    // leaves in `job_runs` — must still collapse to exactly ONE `recent`
    // item, the LATEST attempt, not one row per attempt.
    let state = state();
    let key = "source-refresh:flaky-adapter".to_owned();

    let attempt_1 = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: key.clone(),
            run_key: "job-flaky:1".to_owned(),
            kind: SOURCE_REFRESH_KIND.to_owned(),
            family: ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "Flaky Adapter".to_owned(),
            target: ActivityTarget::Sources,
            attempt: 1,
        })
        .expect("begin attempt 1");
    // `retry_scheduled` is explicitly non-terminal (the job runs again) and
    // excluded from the `recent_occurrences` candidate set entirely — settle
    // this as `interrupted` instead (a genuinely TERMINAL row in storage,
    // the shape a crash mid-retry leaves) so TWO terminal rows really do
    // share this activity_key, actually exercising the collapse.
    state
        .job_runs()
        .settle(attempt_1, JobRunOutcome::Interrupted)
        .expect("settle attempt 1 as interrupted");

    let attempt_2 = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: key.clone(),
            run_key: "job-flaky:1".to_owned(),
            kind: SOURCE_REFRESH_KIND.to_owned(),
            family: ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "Flaky Adapter".to_owned(),
            target: ActivityTarget::Sources,
            attempt: 2,
        })
        .expect("begin attempt 2");
    state
        .job_runs()
        .settle(attempt_2, JobRunOutcome::Succeeded)
        .expect("settle attempt 2 as succeeded");

    let view = compute_activity(&state).expect("view");
    let matches: Vec<_> = view
        .recent
        .iter()
        .filter(|item| item.activity_key == key)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "every attempt of the same task collapses to ONE recent item, got {matches:?}"
    );
    assert_eq!(matches[0].status, "succeeded", "the LATEST attempt wins");
    assert_eq!(matches[0].attempt, 2);
}

#[test]
fn recent_window_excludes_older_than_7_days() {
    let state = state();
    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "INSERT INTO job_runs
                (activity_key, run_key, kind, family, subject, target_json, status,
                 attempt, started_at, finished_at)
             VALUES ('old', 'old', 'k', 'sourceRefresh', 's', '{\"kind\":\"sources\"}',
                 'succeeded', 1,
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-8 days'),
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-8 days'))",
            [],
        )
        .expect("seed old row");
    drop(connection);
    let view = compute_activity(&state).expect("view");
    assert!(
        view.recent.is_empty(),
        "an 8-day-old occurrence is outside the 7-day window"
    );
}

fn document(state: &AppState, company_id: &str, title: &str) -> String {
    state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company_id.to_owned(),
            source_type: "official_report".to_owned(),
            url: format!("https://example.test/{title}.pdf"),
            period_id: None,
            origin_ref: None,
            title: Some(title.to_owned()),
            attribution: None,
        })
        .expect("document")
        .id
}

#[test]
fn queued_transcript_job_is_queued() {
    // A freshly created transcript job (`create_video_transcript_job`) has no
    // `job_runs`/registry entry yet — the direct-activity registry is only
    // opened once `run_video_transcript_job` actually starts it (ADR 0109
    // dec. 3). It must still surface as `queued`, never be invisible.
    let state = state();
    state
        .create_transcript_job(crate::storage::NewTranscriptJob {
            company_id: None,
            provider_id: None,
            source_url: "https://youtube.test/watch?v=mock".to_owned(),
            source_label: Some("Earnings call Q2 2026".to_owned()),
            recognized_company_candidates: None,
        })
        .expect("create transcript job");

    let view = compute_activity(&state).expect("view");
    let transcripts: Vec<_> = view
        .queued
        .iter()
        .filter(|item| item.family == ActivityFamily::Transcript)
        .collect();
    assert_eq!(transcripts.len(), 1);
    assert_eq!(transcripts[0].subject, "Earnings call Q2 2026");
    assert_eq!(transcripts[0].target, ActivityTarget::Transcripts);
    assert!(
        view.active
            .iter()
            .all(|item| item.family != ActivityFamily::Transcript),
        "a not-yet-started transcript job must never appear in active"
    );
}

#[test]
fn kpi_run_without_live_lease_is_waiting() {
    // ADR 0109 dec. 4: KPI ingest never writes `job_runs` — its live lease IS
    // the activity signal. An unleased (never-claimed) non-terminal run is
    // `queued` ("waiting"), never `active`.
    let state = state();
    let cdr = company(&state, "CDR");
    let doc = document(&state, &cdr, "Raport roczny 2025");
    state
        .kpi_ingest_runs()
        .create_run_if_absent(&NewKpiIngestRun {
            report_document_id: doc,
            company_id: cdr,
            period_id: None,
            profile_version: "gpw_ifrs_annual@v1".to_owned(),
            scope: None,
            data_quality: None,
            period_fiscal_year: None,
            period_type: None,
        })
        .expect("kpi run");

    let view = compute_activity(&state).expect("view");
    assert!(
        view.active
            .iter()
            .all(|item| item.family != ActivityFamily::KpiIngest),
        "an unleased KPI run must never appear in active"
    );
    let queued: Vec<_> = view
        .queued
        .iter()
        .filter(|item| item.family == ActivityFamily::KpiIngest)
        .collect();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].status, "queued");
}

// The read-model tests from `kpi_run_with_a_live_lease_is_active` on live in a
// sibling file (file-size ratchet, ADR 0103); they share this module's helpers.
#[path = "activity_read_model_tests.rs"]
mod read_model;
