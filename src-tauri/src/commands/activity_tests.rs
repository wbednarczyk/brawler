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

#[test]
fn kpi_run_with_a_live_lease_is_active() {
    // The flip side: a claimed (leased) non-terminal run IS the activity
    // signal — `active`, never `queued`.
    let state = state();
    let cdr = company(&state, "CDR");
    let doc = document(&state, &cdr, "Raport roczny 2025");
    let run = state
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
    state
        .kpi_ingest_runs()
        .claim_run(&run.id, "agent-1", 300)
        .expect("claim");

    let view = compute_activity(&state).expect("view");
    let active: Vec<_> = view
        .active
        .iter()
        .filter(|item| item.family == ActivityFamily::KpiIngest)
        .collect();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].status, "running");
    assert!(
        view.queued
            .iter()
            .all(|item| item.family != ActivityFamily::KpiIngest),
        "a leased KPI run must never also appear in queued"
    );
}

#[test]
fn domain_row_without_backing_job_is_stalled() {
    // ADR 0109 dec. 4: a non-terminal domain row with no live backing job is
    // `stalled` — surfaced honestly by the READ model even before startup
    // reconciliation has had a chance to terminalize it (rare in practice).
    let state = state();
    let cdr = company(&state, "CDR");
    let doc = document(&state, &cdr, "Raport stranded");
    state
        .autopilot()
        .create_run_if_absent("run:stranded", &cdr, &doc, "detection", "autopilot", None)
        .expect("create run")
        .expect("created");
    // No stage job enqueued for this run: provably stranded.

    let view = compute_activity(&state).expect("view");
    let stalled: Vec<_> = view
        .active
        .iter()
        .filter(|item| item.family == ActivityFamily::ReportReading && item.status == "stalled")
        .collect();
    assert_eq!(stalled.len(), 1);
    assert_eq!(stalled[0].activity_key, "report-reading:run:stranded");
}

#[test]
fn running_row_without_open_occurrence_is_stalled() {
    // The panic-containment invariant (ADR 0109 dec. 2) means a `job_queue`
    // row literally `running` with no matching open `job_runs` occurrence
    // should not exist while the app is alive — but the read model states it
    // honestly as `stalled` rather than fabricating `active` or hiding it.
    let state = state();
    state
        .jobs()
        .enqueue(
            "src:degenerate",
            SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute(
            "UPDATE job_queue SET status = 'running' WHERE id = 'src:degenerate'",
            [],
        )
        .expect("force running, no occurrence");

    let view = compute_activity(&state).expect("view");
    let stalled: Vec<_> = view
        .active
        .iter()
        .filter(|item| item.status == "stalled")
        .collect();
    assert_eq!(stalled.len(), 1);
    assert_eq!(stalled[0].family, ActivityFamily::SourceRefresh);
}

#[test]
fn running_transcript_row_without_open_occurrence_is_stalled() {
    // sol diff R1 #6: the live read model used to project only `queued`
    // transcript rows — a `running` transcript row with no backing
    // occurrence (the finalizer's own best-effort ceiling, or crash residue
    // before a restart's reconcile runs) was simply invisible, not honestly
    // `stalled` like the analogous `job_queue` case above.
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
    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "UPDATE transcript_jobs SET status = 'running', started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE source_label = 'Earnings call Q2 2026'",
            [],
        )
        .expect("force running, no occurrence");
    drop(connection);

    let view = compute_activity(&state).expect("view");
    let stalled: Vec<_> = view
        .active
        .iter()
        .filter(|item| item.family == ActivityFamily::Transcript && item.status == "stalled")
        .collect();
    assert_eq!(stalled.len(), 1);
    assert_eq!(stalled[0].subject, "Earnings call Q2 2026");
}

#[test]
fn sweep_is_one_parent_with_progress_and_children_suppressed() {
    // ADR 0109 dec. 1: a sweep is ONE item with member progress; its child
    // `autopilot_stage` jobs (pending/running) resolve to the SAME
    // `activity_key` and must collapse into that one item, never render as
    // separate rows.
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");

    let done_doc = document(&state, &cdr, "Raport ukończony");
    let done_run = state
        .autopilot()
        .create_run_if_absent(
            "run:done",
            &cdr,
            &done_doc,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");

    let pending_doc = document(&state, &cdr, "Raport w toku");
    state
        .autopilot()
        .create_run_if_absent(
            "run:pending",
            &cdr,
            &pending_doc,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");
    state
        .jobs()
        .enqueue(
            "autopilot:pending:fetch",
            crate::jobs::autopilot::AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"run:pending","stage":"fetch"}"#,
            1,
        )
        .expect("enqueue stage");

    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "UPDATE autopilot_run SET status = 'succeeded' WHERE id = ?1",
            [&done_run.id],
        )
        .expect("mark done");
    // The sweep's OWN fan-out job finishes (and the row goes `completed`)
    // quickly, well before its member runs do — the sweep item's continued
    // liveness comes entirely from the collapsed child stage job below, not
    // from `history_sweeps.status` (mirrors `run_history_sweep_job`).
    connection
        .execute(
            "UPDATE history_sweeps
             SET status = 'completed', candidates_total = 2,
                 enqueued_run_ids_json = '[\"run:done\",\"run:pending\"]'
             WHERE id = ?1",
            [&sweep.id],
        )
        .expect("seed sweep progress");
    drop(connection);

    let view = compute_activity(&state).expect("view");
    let sweep_items: Vec<_> = view
        .active
        .iter()
        .chain(view.queued.iter())
        .filter(|item| item.activity_key == format!("report-sweep:{}", sweep.id))
        .collect();
    assert_eq!(
        sweep_items.len(),
        1,
        "the sweep parent and its pending child must collapse into ONE item"
    );
    let progress = sweep_items[0].progress.as_ref().expect("progress");
    assert_eq!(progress.done, 1);
    assert_eq!(progress.total, 2);
    assert_eq!(progress.failed, 0);
}

// ------------------------------------------------------------------
// Domain-status override (sol diff R1 #4)
// ------------------------------------------------------------------

#[test]
fn recent_succeeded_occurrence_with_a_failed_domain_row_shows_failed() {
    // `job_runs.status` alone records only whether the LEDGERED attempt
    // itself finished — never the underlying domain outcome. A report-
    // reading run that finalized `failed` must show `failed` in `recent`,
    // never the occurrence's own stale `succeeded`.
    let state = state();
    let cdr = company(&state, "CDR");
    let doc = document(&state, &cdr, "Raport z błędem");
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run:domain-failed",
            &cdr,
            &doc,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "UPDATE autopilot_run SET status = 'failed', last_error = 'diff stage exploded' WHERE id = ?1",
            [&run.id],
        )
        .expect("mark failed");
    drop(connection);

    let run_id = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: format!("report-reading:{}", run.id),
            run_key: format!("autopilot:{}:notify", run.id),
            kind: crate::jobs::autopilot::AUTOPILOT_STAGE_KIND.to_owned(),
            family: ActivityFamily::ReportReading,
            company_id: Some(cdr.clone()),
            subject: "Raport z błędem".to_owned(),
            target: ActivityTarget::Company {
                company_id: cdr,
                tool: None,
            },
            attempt: 1,
        })
        .expect("begin");
    state
        .job_runs()
        .settle(run_id, JobRunOutcome::Succeeded)
        .expect("settle succeeded");

    let view = compute_activity(&state).expect("view");
    let item = view
        .recent
        .iter()
        .find(|item| item.activity_key == format!("report-reading:{}", run.id))
        .expect("recent item");
    assert_eq!(item.status, "failed");
    assert_eq!(item.error.as_deref(), Some("diff stage exploded"));
}

#[test]
fn recent_occurrence_with_a_partial_domain_row_shows_partial() {
    // `job_runs.status` has no `partial` value at all (the schema's CHECK
    // constraint excludes it) — `partial` can ONLY come from the domain
    // override.
    let state = state();
    let cdr = company(&state, "CDR");
    let doc = document(&state, &cdr, "Raport częściowy");
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run:domain-partial",
            &cdr,
            &doc,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "UPDATE autopilot_run SET status = 'partial', last_error = 'notify failed, rest ok' WHERE id = ?1",
            [&run.id],
        )
        .expect("mark partial");
    drop(connection);

    let run_id = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: format!("report-reading:{}", run.id),
            run_key: format!("autopilot:{}:notify", run.id),
            kind: crate::jobs::autopilot::AUTOPILOT_STAGE_KIND.to_owned(),
            family: ActivityFamily::ReportReading,
            company_id: Some(run.company_id.clone()),
            subject: "Raport częściowy".to_owned(),
            target: ActivityTarget::Company {
                company_id: run.company_id.clone(),
                tool: None,
            },
            attempt: 1,
        })
        .expect("begin");
    state
        .job_runs()
        .settle(run_id, JobRunOutcome::Succeeded)
        .expect("settle succeeded");

    let view = compute_activity(&state).expect("view");
    let item = view
        .recent
        .iter()
        .find(|item| item.activity_key == format!("report-reading:{}", run.id))
        .expect("recent item");
    assert_eq!(item.status, "partial");
}

// ------------------------------------------------------------------
// Keyed task map + precedence (sol diff R1 #5)
// ------------------------------------------------------------------

#[test]
fn sweep_with_a_running_child_pending_children_and_a_completed_fan_out_is_one_running_item() {
    // Precedence `running > stalled > queued > recent` (sol diff R1 #5): the
    // sweep's OWN job finishes quickly (its own occurrence is a terminal,
    // `recent`-window row), one member's stage job is literally CLAIMED
    // (`running`), and another member is merely `pending` — the whole sweep
    // must collapse to exactly ONE item, `running` (the highest-precedence
    // source wins), never split across sections or shown `recent`/`stalled`.
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");

    let running_doc = document(&state, &cdr, "Raport w trakcie");
    state
        .autopilot()
        .create_run_if_absent(
            "run:running-child",
            &cdr,
            &running_doc,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");
    state
        .jobs()
        .enqueue(
            "autopilot:run:running-child:fetch",
            crate::jobs::autopilot::AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"run:running-child","stage":"fetch"}"#,
            1,
        )
        .expect("enqueue running child");
    // Claim it so the queue row is literally `running` (matches production:
    // the queue's dispatch seam opens the occurrence via `begin_attempt`).
    let connection = state.checkout_for_tests().expect("checkout");
    let identity = crate::jobs::activity_identity::identity_for_job(
        crate::jobs::autopilot::AUTOPILOT_STAGE_KIND,
        "autopilot:run:running-child:fetch",
        r#"{"run_id":"run:running-child","stage":"fetch"}"#,
        &connection,
    )
    .expect("identity");
    drop(connection);
    let claimed = state
        .jobs()
        .claim_next()
        .expect("claim")
        .expect("running child claimed");
    let run_id = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: identity.activity_key.clone(),
            run_key: claimed.id,
            kind: crate::jobs::autopilot::AUTOPILOT_STAGE_KIND.to_owned(),
            family: identity.family,
            company_id: identity.company_id.clone(),
            subject: identity.subject.clone(),
            target: identity.target.clone(),
            attempt: 1,
        })
        .expect("begin occurrence for the running child");
    let _ = run_id;

    let pending_doc = document(&state, &cdr, "Raport oczekujący");
    state
        .autopilot()
        .create_run_if_absent(
            "run:pending-child",
            &cdr,
            &pending_doc,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");
    state
        .jobs()
        .enqueue(
            "autopilot:run:pending-child:fetch",
            crate::jobs::autopilot::AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"run:pending-child","stage":"fetch"}"#,
            1,
        )
        .expect("enqueue pending child");

    // The sweep's own fan-out job already finished (terminal, `recent`).
    let sweep_run_id = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: format!("report-sweep:{}", sweep.id),
            run_key: format!("sweep-job:{}", sweep.id),
            kind: crate::jobs::history_sweep::HISTORY_SWEEP_KIND.to_owned(),
            family: ActivityFamily::ReportSweep,
            company_id: Some(cdr.clone()),
            subject: "GPW ESPI/EBI".to_owned(),
            target: ActivityTarget::Company {
                company_id: cdr,
                tool: None,
            },
            attempt: 1,
        })
        .expect("begin sweep occurrence");
    state
        .job_runs()
        .settle(sweep_run_id, JobRunOutcome::Succeeded)
        .expect("settle sweep occurrence");

    let view = compute_activity(&state).expect("view");
    let sweep_key = format!("report-sweep:{}", sweep.id);
    let matches: Vec<_> = view
        .active
        .iter()
        .chain(view.queued.iter())
        .chain(view.recent.iter())
        .filter(|item| item.activity_key == sweep_key)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "the sweep must collapse to exactly ONE item across active/queued/recent, got {matches:?}"
    );
    assert_eq!(
        matches[0].status, "running",
        "the running member wins the precedence"
    );

    // The summary's unique-key count must agree: exactly one ACTIVE key
    // (the sweep), never inflated by its member/occurrence duplicates.
    let summary = compute_activity_summary(&state).expect("summary");
    assert_eq!(
        summary.active, 1,
        "summary counts unique keys, not raw rows"
    );
    assert_eq!(summary.queued, 0);
}

#[test]
fn stalled_items_never_inflate_the_active_summary_count() {
    // sol diff R1 #5: `active` counts RUNNING keys only — a stalled item
    // stays in the panel's In-progress section (the `active` DTO array) but
    // must never drive the topbar spinner count.
    let state = state();
    state
        .jobs()
        .enqueue(
            "src:degenerate",
            SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute(
            "UPDATE job_queue SET status = 'running' WHERE id = 'src:degenerate'",
            [],
        )
        .expect("force running, no occurrence -> stalled");

    let view = compute_activity(&state).expect("view");
    assert_eq!(view.active.len(), 1);
    assert_eq!(view.active[0].status, "stalled");

    let summary = compute_activity_summary(&state).expect("summary");
    assert_eq!(summary.active, 0, "a stalled item must not count as active");
}

// ------------------------------------------------------------------
// No unwrap_or_default on storage reads (sol diff R1 #13)
// ------------------------------------------------------------------

#[test]
fn a_forced_storage_failure_surfaces_as_an_error_not_an_empty_view() {
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
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute("DROP TABLE job_queue", [])
        .expect("poison job_queue");

    let result = compute_activity(&state);
    assert!(
        result.is_err(),
        "a forced storage failure must propagate as an error, never a silently-empty view: {result:?}"
    );
}

#[test]
fn a_forced_storage_failure_surfaces_in_the_summary_too() {
    let state = state();
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute("DROP TABLE job_runs", [])
        .expect("poison job_runs");

    let result = compute_activity_summary(&state);
    assert!(
        result.is_err(),
        "the summary must propagate storage failures too: {result:?}"
    );
}

// ------------------------------------------------------------------
// Parent-task evidence (sol diff R1 #14)
// ------------------------------------------------------------------

#[test]
fn a_pending_job_carries_its_real_attempt_count() {
    let state = state();
    state
        .jobs()
        .enqueue(
            "src:flaky",
            SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            5,
        )
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");
    state
        .jobs()
        .mark_failed("src:flaky", "transient", 0)
        .expect("fail once, back to pending");

    let view = compute_activity(&state).expect("view");
    let item = view
        .queued
        .iter()
        .find(|item| item.family == ActivityFamily::SourceRefresh)
        .expect("queued item");
    assert_eq!(
        item.attempt, 1,
        "the real job_queue.attempts count, never hardcoded 0"
    );
}

#[test]
fn a_sweep_parent_carries_bounded_member_subjects() {
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");
    let doc_a = document(&state, &cdr, "Raport A");
    let run_a = state
        .autopilot()
        .create_run_if_absent(
            "run:member-a",
            &cdr,
            &doc_a,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");
    let doc_b = document(&state, &cdr, "Raport B");
    let run_b = state
        .autopilot()
        .create_run_if_absent(
            "run:member-b",
            &cdr,
            &doc_b,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");
    state
        .jobs()
        .enqueue(
            "autopilot:run:member-a:fetch",
            crate::jobs::autopilot::AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"run:member-a","stage":"fetch"}"#,
            1,
        )
        .expect("enqueue member a stage");

    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "UPDATE history_sweeps SET candidates_total = 2, enqueued_run_ids_json = ?2 WHERE id = ?1",
            rusqlite::params![
                sweep.id,
                serde_json::to_string(&[run_a.id.clone(), run_b.id.clone()]).unwrap(),
            ],
        )
        .expect("seed member run ids");
    drop(connection);

    let view = compute_activity(&state).expect("view");
    let sweep_key = format!("report-sweep:{}", sweep.id);
    let item = view
        .active
        .iter()
        .chain(view.queued.iter())
        .find(|item| item.activity_key == sweep_key)
        .expect("sweep item");
    let mut members = item.members.clone();
    members.sort();
    assert_eq!(members, vec!["Raport A".to_owned(), "Raport B".to_owned()]);
}

#[test]
fn list_activity_is_checkout_bounded() {
    let state = state();
    let cdr = company(&state, "CDR");
    state
        .jobs()
        .enqueue(
            "src:1",
            SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");
    let _ = cdr;

    let before = state.checkout_count();
    compute_activity(&state).expect("view");
    let delta = state.checkout_count() - before;
    assert_eq!(
        delta, 1,
        "compute_activity must check out exactly one connection"
    );
}

/// Advisory (not a CI gate): run once with `--ignored` against a COPY of the
/// owner's real snapshot and eyeball whether `compute_activity`'s grouping
/// reads as tasks (S1 handoff evidence).
#[test]
#[ignore]
fn dump_activity_on_real_snapshot() {
    let snapshot = std::path::Path::new(
        "/tmp/claude-1000/-home-wojtas-projects-brawler/d9ef921f-6b1b-4904-8b65-b5f67e25e394/scratchpad/realdb/snap.sqlite3",
    );
    if !snapshot.exists() {
        eprintln!("real snapshot not present in this sandbox, skipping");
        return;
    }
    let dir = std::env::temp_dir().join(format!("brawler-activity-dump-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let copy_path = dir.join("snap-copy.sqlite3");
    std::fs::copy(snapshot, &copy_path).expect("copy real snapshot");

    let connection = crate::storage::open_database(&copy_path).expect("open + migrate copy");
    let state = AppState::new(connection);

    let view = compute_activity(&state).expect("compute_activity");
    println!("=== active ({}) ===", view.active.len());
    for item in &view.active {
        println!(
            "  [{:?}] status={} subject={:?} key={} progress={:?}",
            item.family, item.status, item.subject, item.activity_key, item.progress
        );
    }
    println!("=== queued ({}) ===", view.queued.len());
    for item in &view.queued {
        println!(
            "  [{:?}] status={} subject={:?} key={}",
            item.family, item.status, item.subject, item.activity_key
        );
    }
    println!("=== recent ({}) ===", view.recent.len());
    for item in &view.recent {
        println!(
            "  [{:?}] status={} subject={:?} key={} finishedAt={:?}",
            item.family, item.status, item.subject, item.activity_key, item.finished_at
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
