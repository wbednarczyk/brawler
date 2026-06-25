//! Autonomous report pipeline tests (North Star, v0.49.0, ADR 0055): the
//! trust-ladder store, run records, and an offline end-to-end pipeline drain.

use super::*;
use crate::jobs;

fn test_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create")
}

/// Seed a report document that classifies as a financial statement (so detection
/// picks it up) and is already fetched (so the fetch stage is an offline no-op).
fn fetched_statement(state: &AppState, company_id: &str, title: &str, url: &str) -> String {
    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company_id.to_owned(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some(title.to_owned()),
            attribution: None,
        })
        .expect("report document should create");
    state
        .mark_report_document_fetched(
            &doc.id,
            Some("report_documents/seed.pdf"),
            Some("application/pdf"),
            None,
            Some(1),
        )
        .expect("mark fetched");
    doc.id
}

#[test]
fn mode_defaults_to_off_and_is_settable() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);

    // A company with no row reads as off.
    assert_eq!(
        state.autopilot().get_mode(&company.id).expect("mode"),
        MODE_OFF
    );
    assert!(state
        .autopilot()
        .opted_in_company_ids()
        .expect("ids")
        .is_empty());

    state
        .autopilot()
        .set_mode(&company.id, MODE_ASSIST)
        .expect("set");
    assert_eq!(
        state.autopilot().get_mode(&company.id).expect("mode"),
        MODE_ASSIST
    );

    state
        .autopilot()
        .set_mode(&company.id, MODE_AUTOPILOT)
        .expect("set");
    assert_eq!(
        state.autopilot().get_mode(&company.id).expect("mode"),
        MODE_AUTOPILOT
    );
    assert_eq!(
        state.autopilot().opted_in_company_ids().expect("ids"),
        vec![company.id.clone()]
    );

    // Back to off removes it from the opted-in set.
    state
        .autopilot()
        .set_mode(&company.id, MODE_OFF)
        .expect("set");
    assert!(state
        .autopilot()
        .opted_in_company_ids()
        .expect("ids")
        .is_empty());
}

#[test]
fn create_run_is_idempotent_per_company_and_document() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);

    let first = state
        .autopilot()
        .create_run_if_absent("run1", &company.id, "doc1", "detection", MODE_ASSIST)
        .expect("create");
    assert!(first.is_some(), "first create inserts a run");

    // A second create for the same (company, document) is a no-op (detection dedup).
    let second = state
        .autopilot()
        .create_run_if_absent("run1-again", &company.id, "doc1", "detection", MODE_ASSIST)
        .expect("create");
    assert!(
        second.is_none(),
        "duplicate (company, document) does not re-create"
    );

    let runs = state
        .autopilot()
        .list_runs(&ListAutopilotRunsInput::default())
        .expect("list");
    assert_eq!(runs.len(), 1);
}

#[test]
fn produced_facts_merge_and_clear() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    state
        .autopilot()
        .create_run_if_absent("run1", &company.id, "doc1", "detection", MODE_AUTOPILOT)
        .expect("create");

    state
        .autopilot()
        .add_produced_facts("run1", &["f1".to_owned(), "f2".to_owned()])
        .expect("add");
    // Re-adding an overlapping id de-duplicates rather than double-counting.
    state
        .autopilot()
        .add_produced_facts("run1", &["f2".to_owned(), "f3".to_owned()])
        .expect("add");

    let run = state.autopilot().get_run("run1").expect("run");
    assert_eq!(run.produced_fact_ids, vec!["f1", "f2", "f3"]);

    state
        .autopilot()
        .clear_produced_facts("run1")
        .expect("clear");
    assert!(state
        .autopilot()
        .get_run("run1")
        .expect("run")
        .produced_fact_ids
        .is_empty());
}

#[test]
fn bulk_set_and_list_modes() {
    // The per-company settings surface (ADR 0056): set many companies at once,
    // list only those with an explicit (non-off) mode.
    let state = AppState::new(open_in_memory_database().expect("db"));
    assert!(state.autopilot().list_modes().expect("list").is_empty());

    let count = state
        .autopilot()
        .set_modes(
            &["c1".to_owned(), "c2".to_owned(), "c3".to_owned()],
            MODE_ASSIST,
        )
        .expect("bulk set");
    assert_eq!(count, 3);

    let modes = state.autopilot().list_modes().expect("list");
    assert_eq!(modes.len(), 3);
    assert!(modes.iter().all(|m| m.mode == MODE_ASSIST));

    // Re-running on an overlapping set upserts (no duplicate rows).
    state
        .autopilot()
        .set_modes(&["c2".to_owned(), "c4".to_owned()], MODE_AUTOPILOT)
        .expect("bulk set");
    let modes = state.autopilot().list_modes().expect("list");
    assert_eq!(modes.len(), 4, "c2 updated in place, c4 added");
    assert_eq!(
        state.autopilot().get_mode("c2").expect("mode"),
        MODE_AUTOPILOT
    );
    assert_eq!(state.autopilot().get_mode("c1").expect("mode"), MODE_ASSIST);
}

#[test]
fn list_runs_filters_by_notification_state() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    state
        .autopilot()
        .create_run_if_absent("r1", &company.id, "d1", "detection", MODE_ASSIST)
        .expect("c");
    state
        .autopilot()
        .create_run_if_absent("r2", &company.id, "d2", "detection", MODE_ASSIST)
        .expect("c");
    state
        .autopilot()
        .set_notification_state("r2", "dismissed")
        .expect("state");

    let unread = state
        .autopilot()
        .list_runs(&ListAutopilotRunsInput {
            company_id: None,
            notification_state: Some("unread".to_owned()),
            limit: None,
        })
        .expect("list");
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].id, "r1");
}

#[test]
fn detection_and_pipeline_drain_offline_to_a_notification() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    // assist mode: no AI needed; the pipeline runs deterministically offline.
    state
        .autopilot()
        .set_mode(&company.id, MODE_ASSIST)
        .expect("set");
    fetched_statement(
        &state,
        &company.id,
        "CD PROJEKT 2026 Q1 SSF",
        "https://example.com/ssf.pdf",
    );

    // Detection rides refresh completion: a new periodic report starts a run.
    jobs::autopilot::run_detection_sweep(&state);
    let runs = state
        .autopilot()
        .list_runs(&ListAutopilotRunsInput::default())
        .expect("list");
    assert_eq!(runs.len(), 1, "one run created for the new statement");
    assert_eq!(runs[0].status, "pending");

    // Drain the durable queue: each stage chains the next to completion.
    let worker = jobs::handlers::build_worker(state.clone());
    worker.run_until_idle().expect("drain");

    let run = state.autopilot().get_run(&runs[0].id).expect("run");
    assert_eq!(run.status, "succeeded");
    assert_eq!(run.stage, "notify");
    assert_eq!(run.notification_state, "unread");
    assert!(
        run.summary_text.is_some(),
        "a notification summary is composed"
    );

    // Detection is idempotent: a second sweep makes no new run.
    jobs::autopilot::run_detection_sweep(&state);
    assert_eq!(
        state
            .autopilot()
            .list_runs(&ListAutopilotRunsInput::default())
            .expect("list")
            .len(),
        1
    );
}

/// Real-data validation (AGENTS.md standing rule): exercise the whole autopilot
/// pipeline against a throwaway copy of the maintainer's real DB. **Inert in CI** —
/// it skips unless `BRAWLER_REAL_DB` points at a DB copy, so `make check` never runs
/// it. Run it manually:
///
/// ```text
/// cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
/// BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 BRAWLER_CLEAR_AI=1 \
///   cargo test -p brawler --lib autopilot_real_data_validation -- --nocapture --ignored
/// ```
///
/// `BRAWLER_REAL_COMPANY` overrides the company (default a GPW issuer with several
/// extracted statements); `BRAWLER_CLEAR_AI=1` blanks the analysis provider so the
/// extract stage degrades deterministically (validating fetch/diff/cross-ref/notify
/// on real data without an AI key). Without it, the real provider is attempted.
#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB (a throwaway copy)"]
fn autopilot_real_data_validation() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP autopilot_real_data_validation: set BRAWLER_REAL_DB to a throwaway copy");
        return;
    };
    let company =
        std::env::var("BRAWLER_REAL_COMPANY").unwrap_or_else(|_| "company_gpw_cbf".to_owned());

    // open_database applies migrations — so this also validates migration 0055 on the
    // real schema.
    let connection = open_database(&db_path).expect("open real db");
    if std::env::var("BRAWLER_CLEAR_AI").is_ok() {
        connection
            .execute(
                "UPDATE settings SET value = '' WHERE key = 'general_analysis_provider'",
                [],
            )
            .expect("clear provider");
    }
    // Point at the real Tauri data dir so the extract stage can read the actual
    // report PDFs (BRAWLER_REAL_DATA_DIR); otherwise a temp dir (extraction degrades).
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, std::path::PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };

    eprintln!("== autopilot real-data validation: company={company} ==");
    state
        .autopilot()
        .set_mode(&company, MODE_AUTOPILOT)
        .expect("opt company into autopilot");

    // Target a specific report (BRAWLER_REAL_REPORT_DOC, a manual trigger on a doc
    // whose PDF is present) or fall back to detection's newest-per-type pick.
    if let Ok(report_doc) = std::env::var("BRAWLER_REAL_REPORT_DOC") {
        let run_id = format!("autopilot_run:{company}:{report_doc}");
        if state
            .autopilot()
            .create_run_if_absent(&run_id, &company, &report_doc, "manual", MODE_AUTOPILOT)
            .expect("create run")
            .is_some()
        {
            jobs::autopilot::enqueue_first_stage(&state, &run_id);
        }
    } else {
        // Detection rides refresh completion; invoke the sweep directly here.
        jobs::autopilot::run_detection_sweep(&state);
    }
    let detected = state
        .autopilot()
        .list_runs(&ListAutopilotRunsInput {
            company_id: Some(company.clone()),
            notification_state: None,
            limit: Some(50),
        })
        .expect("list runs");
    eprintln!("detection created {} run(s)", detected.len());
    assert!(
        !detected.is_empty(),
        "expected at least one autopilot run for a company with periodic reports"
    );

    // Drain the durable queue: every stage chains to completion.
    let worker = jobs::handlers::build_worker(state.clone());
    worker.run_until_idle().expect("drain queue");

    for run in &detected {
        let run = state.autopilot().get_run(&run.id).expect("get run");
        eprintln!(
            "RUN {}\n  status={} stage={} report_doc={}\n  summary={:?}\n  kpi_delta={:?}\n  report_diff_ref={:?}\n  cross_refs={:?}\n  produced_facts={} notification={}",
            run.id,
            run.status,
            run.stage,
            run.report_document_id,
            run.summary_text,
            run.kpi_delta_json,
            run.report_diff_ref,
            run.cross_refs_json,
            run.produced_fact_ids.len(),
            run.notification_state,
        );
        // Every run reaches a terminal state with a composed notification (even a
        // failed/partial run still notifies — no silent dead-end).
        assert!(
            matches!(run.status.as_str(), "succeeded" | "partial" | "failed"),
            "run must reach a terminal state, got {}",
            run.status
        );
        assert!(
            run.summary_text.is_some(),
            "run must compose a notification"
        );
        assert_eq!(
            run.stage, "notify",
            "a finished run ends at the notify stage"
        );
    }
}

#[test]
fn finalize_records_status_and_summary() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    state
        .autopilot()
        .create_run_if_absent("r1", &company.id, "d1", "detection", MODE_ASSIST)
        .expect("c");

    let finalized = state
        .autopilot()
        .finalize_run(
            "r1",
            "partial",
            "extract",
            Some("stopped early"),
            Some("boom"),
        )
        .expect("finalize");
    assert_eq!(finalized.status, "partial");
    assert_eq!(finalized.stage, "extract");
    assert_eq!(finalized.summary_text.as_deref(), Some("stopped early"));
    assert_eq!(finalized.last_error.as_deref(), Some("boom"));
}
