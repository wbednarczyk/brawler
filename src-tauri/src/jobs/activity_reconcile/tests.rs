use super::*;
use crate::storage::{open_in_memory_database, NewCompany};

fn state() -> AppState {
    AppState::new(open_in_memory_database().expect("db"))
}

fn company(state: &AppState) -> String {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company")
        .id
}

fn seed_running_transcript(state: &AppState, company_id: &str) -> String {
    let connection = state.checkout_for_tests().expect("checkout");
    let id = "transcript-1".to_owned();
    connection
        .execute(
            "INSERT INTO transcript_jobs
                (id, company_id, provider_id, source_type, source_url, status, started_at)
             VALUES (?1, ?2, 'youtube', 'video', 'https://example.test/v', 'running',
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            rusqlite::params![id, company_id],
        )
        .expect("seed transcript");
    id
}

#[test]
fn startup_reconcile_interrupted_transcript() {
    let state = state();
    let company_id = company(&state);
    let job_id = seed_running_transcript(&state, &company_id);

    reconcile_on_startup(&state);

    let connection = state.checkout_for_tests().expect("checkout");
    let (status, error_code): (String, Option<String>) = connection
        .query_row(
            "SELECT status, error_code FROM transcript_jobs WHERE id = ?1",
            [&job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("row");
    assert_eq!(status, "failed");
    assert_eq!(error_code.as_deref(), Some("interrupted"));
}

#[test]
fn startup_reconcile_leaves_a_run_with_a_stage_in_retry_backoff_alone() {
    let state = state();
    let company_id = company(&state);
    let document_id = state
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/r.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run-1",
            &company_id,
            &document_id,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    // A stage job in retry backoff: `pending`, `available_at` in the future.
    state
        .jobs()
        .enqueue(
            "autopilot:run-1:fetch",
            AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"run-1","stage":"fetch"}"#,
            3,
        )
        .expect("enqueue");

    reconcile_on_startup(&state);

    let status = state.autopilot().get_run(&run.id).expect("run").status;
    assert_eq!(
        status, "pending",
        "a run with a live stage job is left alone"
    );
}

#[test]
fn startup_reconcile_fails_a_run_whose_stage_job_is_gone() {
    let state = state();
    let company_id = company(&state);
    let document_id = state
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/r2.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport 2".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run-2",
            &company_id,
            &document_id,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    // No stage job at all for this run: provably stranded.

    reconcile_on_startup(&state);

    let status = state.autopilot().get_run(&run.id).expect("run").status;
    assert_eq!(status, "failed");
}

#[test]
fn startup_reconcile_fails_a_run_with_a_dead_lettered_stage_job() {
    // A stage job present in `job_queue` but terminally `failed` (dead-lettered,
    // ADR 0059) is functionally the same as "no live stage job" — the run
    // cannot resume. Distinct seed shape from the "stage job is gone" test:
    // here the row EXISTS, just not pending/running.
    let state = state();
    let company_id = company(&state);
    let document_id = state
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/r3.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport 3".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run-3",
            &company_id,
            &document_id,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    state
        .jobs()
        .enqueue(
            "autopilot:run-3:fetch",
            AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"run-3","stage":"fetch"}"#,
            1,
        )
        .expect("enqueue");
    // Dead-letter it directly: terminal `failed`, attempts exhausted.
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute(
            "UPDATE job_queue SET status = 'failed', attempts = max_attempts \
             WHERE id = 'autopilot:run-3:fetch'",
            [],
        )
        .expect("dead-letter");

    reconcile_on_startup(&state);

    let status = state.autopilot().get_run(&run.id).expect("run").status;
    assert_eq!(status, "failed");
}

#[test]
fn startup_reconcile_never_terminalizes_kpi_runs() {
    // KPI ingest runs are NEVER terminalized here (ADR 0109 dec. 4) — their
    // own reclaim owns `committing`; reconcile must leave a `committing` run
    // with an EXPIRED lease untouched (it reads as waiting in the read model,
    // never as a stranded run reconcile itself flips).
    let state = state();
    let company_id = company(&state);
    let document_id = state
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/kpi.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport KPI".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    let run = state
        .kpi_ingest_runs()
        .create_run_if_absent(&crate::storage::NewKpiIngestRun {
            report_document_id: document_id,
            company_id,
            period_id: None,
            profile_version: "gpw_ifrs_annual@v1".to_owned(),
            scope: None,
            data_quality: None,
            period_fiscal_year: None,
            period_type: None,
        })
        .expect("kpi run");
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute(
            "UPDATE kpi_ingest_runs SET status = 'committing', lease_holder = 'agent-x', \
             lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour'), \
             last_heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour') \
             WHERE id = ?1",
            [&run.id],
        )
        .expect("seed committing + expired lease");

    reconcile_on_startup(&state);

    let status: String = state
        .checkout_for_tests()
        .expect("checkout")
        .query_row(
            "SELECT status FROM kpi_ingest_runs WHERE id = ?1",
            [&run.id],
            |row| row.get(0),
        )
        .expect("status");
    assert_eq!(
        status, "committing",
        "reconcile must never touch a KPI run's status"
    );
}

#[test]
fn startup_reconcile_fails_an_orphaned_sweep() {
    let state = state();
    let company_id = company(&state);
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&company_id, "manual")
        .expect("sweep");
    // No parent job row for this sweep id: stranded.

    reconcile_on_startup(&state);

    let status = state
        .history_sweeps()
        .get_history_sweep(&sweep.id)
        .expect("sweep")
        .status;
    assert_eq!(status, "failed");
}

#[test]
fn startup_reconcile_fails_an_orphaned_batch() {
    let state = state();
    let company_id = company(&state);
    let batch = state
        .pipeline_reextraction()
        .create_batch(&company_id)
        .expect("batch");

    reconcile_on_startup(&state);

    let batch = state
        .pipeline_reextraction()
        .get_batch(&batch.id)
        .expect("batch");
    assert_eq!(batch.status, "failed");
}

#[test]
fn startup_reconcile_interrupts_open_occurrences() {
    let state = state();
    let run_id = state
        .job_runs()
        .begin_attempt(crate::storage::NewJobRun {
            activity_key: "source-refresh:x".to_owned(),
            run_key: "job-1".to_owned(),
            kind: "scheduled_source_refresh".to_owned(),
            family: crate::jobs::activity_identity::ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "x".to_owned(),
            target: crate::jobs::activity_identity::ActivityTarget::Sources,
            attempt: 1,
        })
        .expect("begin");

    reconcile_on_startup(&state);

    let connection = state.checkout_for_tests().expect("checkout");
    let status: String = connection
        .query_row(
            "SELECT status FROM job_runs WHERE id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .expect("status");
    assert_eq!(status, "interrupted");
}

#[test]
fn startup_reconcile_is_idempotent() {
    let state = state();
    let company_id = company(&state);
    seed_running_transcript(&state, &company_id);

    reconcile_on_startup(&state);
    reconcile_on_startup(&state); // second call must not error or re-flip anything

    let connection = state.checkout_for_tests().expect("checkout");
    let status: String = connection
        .query_row(
            "SELECT status FROM transcript_jobs WHERE id = 'transcript-1'",
            [],
            |row| row.get(0),
        )
        .expect("status");
    assert_eq!(status, "failed");
}
