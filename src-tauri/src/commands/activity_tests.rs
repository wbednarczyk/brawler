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

/// One key's terminal occurrence, appended in insertion order (so its
/// `job_runs.id` — the DESC tie-break `recent_candidates_sql` orders by —
/// reflects call order).
fn settle_one_occurrence(state: &AppState, activity_key: &str, attempt: i64) {
    let run_id = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: activity_key.to_owned(),
            run_key: format!("{activity_key}:{attempt}"),
            kind: SOURCE_REFRESH_KIND.to_owned(),
            family: ActivityFamily::SourceRefresh,
            company_id: None,
            subject: activity_key.to_owned(),
            target: ActivityTarget::Sources,
            attempt,
        })
        .expect("begin");
    state
        .job_runs()
        .settle(run_id, JobRunOutcome::Succeeded)
        .expect("settle");
}

#[test]
fn recent_cap_applies_after_the_collapse_with_duplicate_heavy_input() {
    // sol diff R2 #8(a): the prior version of this test seeded 41 DISTINCT
    // keys only — nothing to collapse, so it could not tell "cap after the
    // per-key collapse" apart from "cap the raw candidate scan" (both give
    // 40 either way with no duplicates). Seed duplicate-HEAVY input instead:
    // 41 distinct single-occurrence keys FIRST (lower ids, so OLDER in the
    // `finished_at DESC, id DESC` scan), then 5 keys with 9 occurrences each
    // (45 rows, higher ids — NEWEST) — so the newest ~45 candidates the scan
    // sees are dominated by just 5 keys repeating. A cap-BEFORE-collapse bug
    // would exhaust the 40-row budget on those 5 keys' duplicates alone,
    // undercounting to ~5 items; the real "collapse then cap" rule instead
    // absorbs the 5 duplicate keys once each, then keeps filling from the
    // distinct keys — 40 items total, every key exactly once.
    let state = state();
    for i in 0..41 {
        settle_one_occurrence(&state, &format!("source-refresh:distinct-{i}"), 1);
    }
    for k in 0..5 {
        for attempt in 1..=9 {
            settle_one_occurrence(&state, &format!("source-refresh:dup-{k}"), attempt);
        }
    }

    let view = compute_activity(&state).expect("view");
    assert_eq!(
        view.recent.len(),
        40,
        "collapse-then-cap: 5 duplicate keys + 35 of the 41 distinct keys"
    );
    let keys: std::collections::HashSet<&str> = view
        .recent
        .iter()
        .map(|item| item.activity_key.as_str())
        .collect();
    assert_eq!(
        keys.len(),
        40,
        "every key exactly once — no duplicates leaked through"
    );
    for k in 0..5 {
        assert!(
            keys.contains(format!("source-refresh:dup-{k}").as_str()),
            "every duplicate-heavy key must survive the collapse, not just some of its rows"
        );
    }
    assert_eq!(
        keys.iter()
            .filter(|key| key.starts_with("source-refresh:distinct-"))
            .count(),
        35,
        "35 of the 41 distinct keys fill the remaining cap budget (the 6 OLDEST dropped)"
    );
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
fn five_real_autopilot_stages_plus_a_retry_collapse_to_one_recent_item() {
    // sol diff R2 #8(b): the collapse test above uses a SYNTHETIC key under
    // `SOURCE_REFRESH_KIND` — never the REAL stage-id shape
    // (`autopilot:<run>:fetch|extract|diff|cross_reference|notify`) a
    // genuine autopilot pipeline run actually leaves in `job_runs`. Seed all
    // FIVE real stages of one run (`extract` retried once) and assert they
    // all collapse to exactly ONE `recent` item under the run's
    // `report-reading:<run_id>` activity_key.
    let state = state();
    let cdr = company(&state, "CDR");
    let doc = document(&state, &cdr, "Raport pięcioetapowy");
    let run = state
        .autopilot()
        .create_run_if_absent("run:five-stage", &cdr, &doc, "detection", "autopilot", None)
        .expect("create run")
        .expect("created");
    let key = format!("report-reading:{}", run.id);

    let settle_stage = |stage: &str, attempt: i64, outcome: JobRunOutcome<'static>| {
        let run_id = state
            .job_runs()
            .begin_attempt(NewJobRun {
                activity_key: key.clone(),
                run_key: crate::jobs::autopilot::stage_job_id(&run.id, stage),
                kind: crate::jobs::autopilot::AUTOPILOT_STAGE_KIND.to_owned(),
                family: ActivityFamily::ReportReading,
                company_id: Some(cdr.clone()),
                subject: "Raport pięcioetapowy".to_owned(),
                target: ActivityTarget::Company {
                    company_id: cdr.clone(),
                    tool: None,
                },
                attempt,
            })
            .expect("begin");
        state.job_runs().settle(run_id, outcome).expect("settle");
    };

    settle_stage(
        crate::jobs::autopilot::STAGE_FETCH,
        1,
        JobRunOutcome::Succeeded,
    );
    // `extract` retried once before succeeding — a genuinely non-linear
    // attempt sequence within one stage, not just across distinct stages.
    settle_stage(
        crate::jobs::autopilot::STAGE_EXTRACT,
        1,
        JobRunOutcome::Interrupted,
    );
    settle_stage(
        crate::jobs::autopilot::STAGE_EXTRACT,
        2,
        JobRunOutcome::Succeeded,
    );
    settle_stage(
        crate::jobs::autopilot::STAGE_DIFF,
        1,
        JobRunOutcome::Succeeded,
    );
    settle_stage(
        crate::jobs::autopilot::STAGE_CROSS_REFERENCE,
        1,
        JobRunOutcome::Succeeded,
    );
    settle_stage(
        crate::jobs::autopilot::STAGE_NOTIFY,
        1,
        JobRunOutcome::Succeeded,
    );

    // The run's own domain row must go terminal too — otherwise
    // `stalled_autopilot_runs` (a non-terminal run with no live backing job)
    // claims this activity_key as `stalled` and excludes it from `recent`
    // entirely, unrelated to the collapse this test is about.
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute(
            "UPDATE autopilot_run SET status = 'succeeded' WHERE id = ?1",
            [&run.id],
        )
        .expect("mark run terminal");

    let view = compute_activity(&state).expect("view");
    let matches: Vec<_> = view
        .recent
        .iter()
        .filter(|item| item.activity_key == key)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "five real stages (one retried) of the same run collapse to ONE item, got {matches:?}"
    );
    assert_eq!(matches[0].status, "succeeded");
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
