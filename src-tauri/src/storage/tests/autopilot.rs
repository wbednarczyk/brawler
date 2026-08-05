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
        .create_run_if_absent("run1", &company.id, "doc1", "detection", MODE_ASSIST, None)
        .expect("create");
    assert!(first.is_some(), "first create inserts a run");

    // A second create for the same (company, document) is a no-op (detection dedup).
    let second = state
        .autopilot()
        .create_run_if_absent(
            "run1-again",
            &company.id,
            "doc1",
            "detection",
            MODE_ASSIST,
            None,
        )
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
fn list_runs_carries_the_report_document_title() {
    // v0.60 D6: an autopilot run must name the report it processed, so the Today
    // row states WHICH report instead of a bare "New report processed."
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    let document_id = fetched_statement(
        &state,
        &company.id,
        "Skonsolidowany raport kwartalny Q2 2026",
        "https://example.test/cdr-q2-2026",
    );

    state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            &document_id,
            "manual",
            MODE_ASSIST,
            None,
        )
        .expect("create run");

    // Both read models (get + list) resolve the document title via the join.
    let run = state.autopilot().get_run("run1").expect("run");
    assert_eq!(
        run.report_document_title.as_deref(),
        Some("Skonsolidowany raport kwartalny Q2 2026"),
        "get_run resolves the processed report's document title"
    );

    let runs = state
        .autopilot()
        .list_runs(&ListAutopilotRunsInput::default())
        .expect("list");
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].report_document_title.as_deref(),
        Some("Skonsolidowany raport kwartalny Q2 2026"),
        "list_runs resolves the processed report's document title"
    );
}

#[test]
fn list_runs_report_document_title_is_none_without_a_document_row() {
    // Tolerant fallback: a run whose report document has no stored row (legacy /
    // pruned) reports no title — the frontend keeps its generic copy.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);

    state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            "orphan_doc",
            "manual",
            MODE_ASSIST,
            None,
        )
        .expect("create run");

    let runs = state
        .autopilot()
        .list_runs(&ListAutopilotRunsInput::default())
        .expect("list");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].report_document_title, None);
}

#[test]
fn failed_run_without_facts_is_recreated_on_next_detection() {
    // Self-heal (ADR 0059): a transient extraction failure finalizes the run as
    // `failed` with no produced facts. The next detection sweep must be able to
    // re-create and re-run it (same deterministic id), not dedup it away forever.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);

    state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            "doc1",
            "detection",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create");
    state
        .autopilot()
        .finalize_run(
            "run1",
            "failed",
            "extract",
            None,
            Some("Gemini unavailable"),
        )
        .expect("finalize failed");

    let recreated = state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            "doc1",
            "detection",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create");
    assert!(
        recreated.is_some(),
        "a failed run with no facts is re-created for retry"
    );
    let run = state.autopilot().get_run("run1").expect("run");
    assert_eq!(run.status, "pending", "the re-created run starts fresh");

    let runs = state
        .autopilot()
        .list_runs(&ListAutopilotRunsInput::default())
        .expect("list");
    assert_eq!(runs.len(), 1, "still exactly one run per report");
}

#[test]
fn failed_run_with_facts_is_preserved() {
    // A failed run that already committed facts (partial success) must not be
    // wiped and re-run — that would duplicate its facts. It still dedups.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);

    state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            "doc1",
            "detection",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create");
    state
        .autopilot()
        .add_produced_facts("run1", &["f1".to_owned()])
        .expect("add");
    state
        .autopilot()
        .finalize_run(
            "run1",
            "failed",
            "cross_reference",
            None,
            Some("later stage failed"),
        )
        .expect("finalize failed");

    let again = state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            "doc1",
            "detection",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create");
    assert!(
        again.is_none(),
        "a failed run that produced facts is preserved, not re-run"
    );
}

#[test]
fn succeeded_run_without_facts_is_preserved() {
    // A `succeeded` run (even with zero facts) is a completed decision and must
    // dedup — self-heal only retries `failed` runs, not empty successes.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);

    state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            "doc1",
            "detection",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create");
    state
        .autopilot()
        .finalize_run("run1", "succeeded", "notify", None, None)
        .expect("finalize succeeded");

    let again = state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            "doc1",
            "detection",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create");
    assert!(again.is_none(), "a succeeded run is never re-created");
}

#[test]
fn produced_facts_merge_and_clear() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    state
        .autopilot()
        .create_run_if_absent(
            "run1",
            &company.id,
            "doc1",
            "detection",
            MODE_AUTOPILOT,
            None,
        )
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

/// Pins the invariant `commands::autopilot::undo_autopilot_run` relies on: it
/// reverts a run's `produced_fact_ids` by id, unconditionally —
/// `delete_financial_fact` has no `confirmation_state` filter — so a
/// `confirmed` structured fact (ADR 0061 dec. 3/8/9: a validation-clean
/// structured set now auto-confirms outright) is undoable exactly like an
/// `auto_unreviewed`/`pending` one. Verified at the storage layer since the
/// command itself takes a `tauri::State` wrapper this test harness cannot
/// construct.
#[test]
fn undo_deletes_produced_facts_regardless_of_confirmation_state() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);

    let fact_id = state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: &company.id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-12-31"),
            report_document_id: "doc1",
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Przychody"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("record structured fact");
    let fact_id = match fact_id {
        crate::storage::kpi_extraction::StructuredFactCommit::Created(id) => id,
        other => panic!("revenue is a catalog metric; expected Created, got {other:?}"),
    };

    state
        .autopilot()
        .create_run_if_absent("run1", &company.id, "doc1", "manual", MODE_AUTOPILOT, None)
        .expect("create run");
    state
        .autopilot()
        .add_produced_facts("run1", std::slice::from_ref(&fact_id))
        .expect("add produced fact");

    // The undo command's primitive: delete each produced fact id, no
    // confirmation_state gate.
    state
        .delete_financial_fact(&fact_id)
        .expect("a confirmed fact must be deletable by id, same as auto_unreviewed/pending");
    assert!(
        state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company.id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list facts")
            .is_empty(),
        "the confirmed fact should be gone after the id-based delete"
    );

    state
        .autopilot()
        .clear_produced_facts("run1")
        .expect("clear produced facts");
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
        .create_run_if_absent("r1", &company.id, "d1", "detection", MODE_ASSIST, None)
        .expect("c");
    state
        .autopilot()
        .create_run_if_absent("r2", &company.id, "d2", "detection", MODE_ASSIST, None)
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
    // Genuinely offline: clear the default AI provider (migration 0036 seeds
    // `provider_gemini`) so the extract stage takes the "no_ai_provider" degrade
    // path instead of trying — and failing — to reach a real provider. Since
    // ADR 0059 a *failed* extraction correctly fails the run, so an offline drain
    // must exercise the graceful-degrade path, not a swallowed provider error.
    let connection = open_in_memory_database().expect("db");
    connection
        .execute(
            "UPDATE settings SET value = '' WHERE key = 'general_analysis_provider'",
            [],
        )
        .expect("clear default provider for offline drain");
    let state = AppState::new(connection);
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

#[test]
fn recreated_run_rearms_stale_terminal_stage_jobs() {
    // Bug dce9ce8: stage job ids are deterministic (`autopilot:{run_id}:{stage}`).
    // When a run row is deleted and later recreated under the SAME id (e.g. an
    // out-of-band recovery/reset), its stage jobs from the prior life are already
    // `succeeded` in `job_queue`; `enqueue_stage` must re-arm them rather than
    // silently no-op (`INSERT OR IGNORE`) against the existing row.
    let connection = open_in_memory_database().expect("db");
    connection
        .execute(
            "UPDATE settings SET value = '' WHERE key = 'general_analysis_provider'",
            [],
        )
        .expect("clear default provider for offline drain");
    let state = AppState::new(connection);
    let company = test_company(&state);
    // assist mode: no AI needed; the pipeline runs deterministically offline.
    state
        .autopilot()
        .set_mode(&company.id, MODE_ASSIST)
        .expect("set");
    let doc_id = fetched_statement(
        &state,
        &company.id,
        "CD PROJEKT 2026 Q1 SSF",
        "https://example.com/ssf.pdf",
    );
    let run_id = format!("autopilot_run:{}:{}", company.id, doc_id);

    // First life: create the run, drive it to a terminal state.
    state
        .autopilot()
        .create_run_if_absent(
            &run_id,
            &company.id,
            &doc_id,
            "detection",
            MODE_ASSIST,
            None,
        )
        .expect("create")
        .expect("first create inserts a run");
    jobs::autopilot::enqueue_first_stage(&state, &run_id);
    let worker = jobs::handlers::build_worker(state.clone());
    worker.run_until_idle().expect("drain first life");
    let first_life = state.autopilot().get_run(&run_id).expect("run");
    assert_eq!(
        first_life.status, "succeeded",
        "first life reaches a terminal state"
    );

    // Simulate the recreate/recovery path: the run row is deleted while its
    // job_queue rows — deterministic on the same run id — remain, still terminal
    // `succeeded`, from the prior life.
    let raw = state.checkout().expect("connection");
    raw.execute("DELETE FROM autopilot_run WHERE id = ?1", [&run_id])
        .expect("delete run row");
    drop(raw);

    let recreated = state
        .autopilot()
        .create_run_if_absent(
            &run_id,
            &company.id,
            &doc_id,
            "detection",
            MODE_ASSIST,
            None,
        )
        .expect("create")
        .expect("recreate inserts a fresh run row");
    assert_eq!(recreated.status, "pending");
    jobs::autopilot::enqueue_first_stage(&state, &run_id);

    // Drain again: the recreated run must actually re-arm and execute its stages,
    // not stick at pending/fetch behind stale-succeeded job rows.
    let worker = jobs::handlers::build_worker(state.clone());
    worker.run_until_idle().expect("drain second life");

    let second_life = state.autopilot().get_run(&run_id).expect("run");
    assert_eq!(
        second_life.status, "succeeded",
        "the recreated run reaches a terminal state instead of sticking at pending/fetch"
    );
    assert_eq!(second_life.stage, "notify");
}

#[test]
fn detection_sweep_rearms_an_existing_stuck_pending_run() {
    // Bug dce9ce8, the exact shape found in real-DB forensics: an `autopilot_run`
    // row already exists `pending`/`fetch` (an earlier sweep's very first
    // `enqueue_stage` call silently no-op'd against a stale `succeeded`
    // `job_queue` row left by an unrelated prior life of the same deterministic
    // stage id), with no live job ever driving it. Because the run row already
    // exists, a later sweep's `create_run_if_absent` dedups (`Ok(None)`) — so the
    // sweep must also re-arm a stuck non-terminal run it finds via dedup, not
    // only a freshly (re)created one.
    let connection = open_in_memory_database().expect("db");
    connection
        .execute(
            "UPDATE settings SET value = '' WHERE key = 'general_analysis_provider'",
            [],
        )
        .expect("clear default provider for offline drain");
    let state = AppState::new(connection);
    let company = test_company(&state);
    state
        .autopilot()
        .set_mode(&company.id, MODE_ASSIST)
        .expect("set");
    let doc_id = fetched_statement(
        &state,
        &company.id,
        "CD PROJEKT 2026 Q1 SSF",
        "https://example.com/ssf.pdf",
    );
    let run_id = format!("autopilot_run:{}:{}", company.id, doc_id);

    // Create the run WITHOUT enqueuing its first stage, then seed a stale
    // `succeeded` job_queue row under the exact deterministic stage id an
    // unrelated prior life would have left behind.
    state
        .autopilot()
        .create_run_if_absent(
            &run_id,
            &company.id,
            &doc_id,
            "detection",
            MODE_ASSIST,
            None,
        )
        .expect("create")
        .expect("first create inserts a run");
    let raw = state.checkout().expect("connection");
    raw.execute(
        "INSERT INTO job_queue (id, kind, payload, status, max_attempts)
         VALUES (?1, 'autopilot_stage', '{}', 'succeeded', 1)",
        [format!("autopilot:{run_id}:fetch")],
    )
    .expect("seed stale succeeded job row");
    drop(raw);

    let run = state.autopilot().get_run(&run_id).expect("run");
    assert_eq!(run.status, "pending");
    assert_eq!(run.stage, "fetch");

    // A later detection sweep must find and re-arm this stuck run, not just dedup
    // and leave it forever pending.
    jobs::autopilot::run_detection_sweep(&state);
    let worker = jobs::handlers::build_worker(state.clone());
    worker.run_until_idle().expect("drain");

    let run = state.autopilot().get_run(&run_id).expect("run");
    assert_eq!(
        run.status, "succeeded",
        "the sweep re-arms and drives the stuck run to a terminal state"
    );
    assert_eq!(run.stage, "notify");

    // Still exactly one run for this report — the sweep did not create a duplicate.
    assert_eq!(
        state
            .autopilot()
            .list_runs(&ListAutopilotRunsInput::default())
            .expect("list")
            .len(),
        1
    );
}

/// T3.1 (ADR 0077 §3): the shared `enqueue_extraction_run` — extracted from the
/// detection sweep so the future history sweep enqueues runs through the exact
/// same path — creates a run and arms its first stage on the first call.
#[test]
fn enqueue_extraction_run_creates_a_run_and_arms_its_first_stage() {
    use jobs::autopilot::{enqueue_extraction_run, EnqueueExtractionOutcome};

    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    let doc_id = fetched_statement(
        &state,
        &company.id,
        "CD PROJEKT 2026 Q1 SSF",
        "https://example.com/ssf.pdf",
    );

    let outcome =
        enqueue_extraction_run(&state, &company.id, &doc_id, "detection", MODE_ASSIST, None);
    assert_eq!(outcome, EnqueueExtractionOutcome::Created);

    let run_id = format!("autopilot_run:{}:{}", company.id, doc_id);
    let run = state.autopilot().get_run(&run_id).expect("run created");
    assert_eq!(run.status, "pending");
    assert_eq!(run.stage, "fetch");
    assert!(
        state
            .jobs()
            .pending_payload(&format!("autopilot:{run_id}:fetch"))
            .expect("query")
            .is_some(),
        "the first (fetch) stage job must be armed and pending"
    );
}

/// T3.1: on a second call the run dedups (`create_run_if_absent` → `Ok(None)`),
/// but a still-pending run whose stage job went terminal in a prior life must be
/// re-armed — the exact dce9ce8 stuck shape the detection sweep already guards,
/// now shared. Mirrors `detection_sweep_rearms_an_existing_stuck_pending_run`.
#[test]
fn enqueue_extraction_run_rearms_a_stuck_pending_run() {
    use jobs::autopilot::{enqueue_extraction_run, EnqueueExtractionOutcome};

    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    let doc_id = fetched_statement(
        &state,
        &company.id,
        "CD PROJEKT 2026 Q1 SSF",
        "https://example.com/ssf.pdf",
    );
    let run_id = format!("autopilot_run:{}:{}", company.id, doc_id);

    let first =
        enqueue_extraction_run(&state, &company.id, &doc_id, "detection", MODE_ASSIST, None);
    assert_eq!(first, EnqueueExtractionOutcome::Created);

    // Simulate the stuck shape: the fetch stage row goes terminal (`succeeded`)
    // while the run itself is still pending/fetch, so nothing is left to drive it.
    let raw = state.checkout().expect("connection");
    raw.execute(
        "UPDATE job_queue SET status = 'succeeded' WHERE id = ?1",
        [format!("autopilot:{run_id}:fetch")],
    )
    .expect("mark stage terminal");
    drop(raw);

    let second =
        enqueue_extraction_run(&state, &company.id, &doc_id, "detection", MODE_ASSIST, None);
    assert_eq!(second, EnqueueExtractionOutcome::Rearmed);
    assert!(
        state
            .jobs()
            .pending_payload(&format!("autopilot:{run_id}:fetch"))
            .expect("query")
            .is_some(),
        "the stuck fetch stage job must be re-armed back to pending"
    );
}

/// T3.1: a terminal existing run (already `succeeded`) dedups without arming any
/// stage job — re-running extraction on a finished report is a no-op.
#[test]
fn enqueue_extraction_run_dedups_a_terminal_run_without_arming_a_job() {
    use jobs::autopilot::{enqueue_extraction_run, EnqueueExtractionOutcome};

    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = test_company(&state);
    let doc_id = fetched_statement(
        &state,
        &company.id,
        "CD PROJEKT 2026 Q1 SSF",
        "https://example.com/ssf.pdf",
    );
    let run_id = format!("autopilot_run:{}:{}", company.id, doc_id);

    state
        .autopilot()
        .create_run_if_absent(
            &run_id,
            &company.id,
            &doc_id,
            "manual",
            MODE_AUTOPILOT,
            None,
        )
        .expect("create")
        .expect("run created");
    state
        .autopilot()
        .finalize_run(&run_id, "succeeded", "notify", Some("done"), None)
        .expect("finalize");

    let outcome =
        enqueue_extraction_run(&state, &company.id, &doc_id, "detection", MODE_ASSIST, None);
    assert_eq!(outcome, EnqueueExtractionOutcome::DedupedTerminal);
    assert_eq!(
        state.jobs().counts().expect("counts").pending,
        0,
        "no stage job should be armed for an already-terminal run"
    );
}

/// Real-data validation (CLAUDE.md standing rule): exercise the whole autopilot
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
            .create_run_if_absent(
                &run_id,
                &company,
                &report_doc,
                "manual",
                MODE_AUTOPILOT,
                None,
            )
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
        .create_run_if_absent("r1", &company.id, "d1", "detection", MODE_ASSIST, None)
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
