//! Behavior tests for the #364 queue integration (plan tests 1–12): the two
//! generation-pinned kinds, the supersession invariant, id-authoritative
//! terminalization, and startup reconciliation. Real validator, real commit
//! atom, real worker dispatch — no seeded verdicts.

use super::*;
use crate::jobs::handlers::build_worker;
use crate::storage::{
    open_in_memory_database, AttentionEventListInput, NewKpiIngestRun, NewStagedObservation,
    TRIGGER_JOB_FAILED,
};

fn seed_company_and_document(connection: &rusqlite::Connection, company: &str, doc: &str) {
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES (?1, 'gpw', ?1, ?2, ?3)",
            rusqlite::params![company, format!("GPW:{company}"), format!("{company} SA")],
        )
        .expect("company");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status, title)
             VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched', ?4)",
            rusqlite::params![
                doc,
                company,
                format!("https://x/{doc}.pdf"),
                format!("Raport okresowy {company}")
            ],
        )
        .expect("document");
}

/// A staged observation that resolves cleanly against the seeded canonical
/// `revenue` definition (migration 0034): validation outcome `ready`.
fn clean_observation() -> NewStagedObservation {
    NewStagedObservation {
        raw_label: "Przychody ze sprzedaży".to_owned(),
        raw_value: "1 234,5".to_owned(),
        currency: Some("PLN".to_owned()),
        normalized_value: Some("1234.5".to_owned()),
        unit_scale: Some("ones".to_owned()),
        measure_window: Some("flow".to_owned()),
        metric_key_candidate: Some("revenue".to_owned()),
        mapping_status: Some("mapped".to_owned()),
        citation_page: Some(3),
        ..Default::default()
    }
}

/// No usable citation locator → `citation.missing` (flagged) → outcome `failed`.
fn uncited_observation() -> NewStagedObservation {
    NewStagedObservation {
        citation_page: None,
        ..clean_observation()
    }
}

fn create_run(state: &AppState, doc: &str, company: &str) -> KpiIngestRun {
    state
        .kpi_ingest_runs()
        .create_run_if_absent(&NewKpiIngestRun {
            report_document_id: doc.to_owned(),
            company_id: company.to_owned(),
            period_id: None,
            profile_version: "p1".to_owned(),
            scope: Some("standalone".to_owned()),
            data_quality: Some("final".to_owned()),
            period_fiscal_year: Some(2025),
            period_type: Some("FY".to_owned()),
        })
        .expect("create run")
}

/// Drives a fresh run to `staged` with the given observations, returning
/// `(state, run_id, revision)`.
fn staged_run(observations: Vec<NewStagedObservation>) -> (AppState, String, i64) {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    let state = AppState::new(connection);
    let run = create_run(&state, "doc1", "c1");
    let revision = stage(&state, &run.id, observations);
    (state, run.id, revision)
}

/// Claims, captures, extracts and stages one revision on an existing run.
/// The claim may be a no-op when agent-1's lease from an earlier step is
/// still live (a lease-holding run is not re-claimable).
fn stage(state: &AppState, run_id: &str, observations: Vec<NewStagedObservation>) -> i64 {
    let store = state.kpi_ingest_runs();
    let _ = store.claim_next("agent-1", 3600).expect("claim call");
    let run = store.get_run(run_id).expect("run").expect("exists");
    if run.status == KpiIngestRunState::Discovered {
        store
            .mark_source_captured(run_id, "agent-1", "hash1")
            .expect("capture");
        store
            .begin_extracting(run_id, "agent-1", "instr-1")
            .expect("begin extracting");
    }
    let (revision, _) = state
        .kpi_ingest_staging()
        .stage_observations(run_id, "agent-1", observations)
        .expect("stage");
    revision
}

fn run_status(state: &AppState, run_id: &str) -> KpiIngestRunState {
    state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("run")
        .expect("exists")
        .status
}

fn progress_step(state: &AppState, run_id: &str) -> Option<String> {
    let run = state
        .kpi_ingest_runs()
        .get_run(run_id)
        .expect("run")
        .expect("exists");
    let progress: serde_json::Value = serde_json::from_str(run.progress_json.as_deref()?).ok()?;
    progress
        .get("step")
        .and_then(|step| step.as_str())
        .map(str::to_owned)
}

fn attention_events(state: &AppState) -> Vec<crate::storage::AttentionEvent> {
    state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events")
        .into_iter()
        .filter(|event| event.trigger_type == TRIGGER_JOB_FAILED)
        .collect()
}

// ---------------------------------------------------------------------
// Plan test 1: happy path — validate chains commit, run completes.
// ---------------------------------------------------------------------

#[test]
fn a_staged_run_is_validated_and_committed_through_the_queue() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm validate");

    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate step"));

    // Chaining: the commit is its OWN pending generation row (separate budget).
    let run = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("run")
        .expect("exists");
    assert_eq!(run.status, KpiIngestRunState::ReadyToCommit);
    let hash = run.manifest_hash.clone().expect("frozen hash");
    let commit_id = commit_job_id(&run_id, revision, &hash);
    let commit_row = state
        .jobs()
        .status(&commit_id)
        .expect("status")
        .expect("commit row exists");
    assert_eq!(commit_row.status, "pending");

    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("commit step"));
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Complete);
    assert!(state
        .kpi_ingest_staging()
        .get_commit_receipt(&run_id)
        .expect("receipt read")
        .is_some());
    assert_eq!(progress_step(&state, &run_id).as_deref(), Some("committed"));

    let counts = state.jobs().counts().expect("counts");
    assert_eq!(counts.succeeded, 2, "both generation rows succeeded");
    assert_eq!(counts.failed, 0);
}

// ---------------------------------------------------------------------
// Plan test 2: validation outcome `failed` is job SUCCESS (agent worklist).
// ---------------------------------------------------------------------

#[test]
fn a_failed_validation_outcome_is_job_success_and_chains_nothing() {
    let (state, run_id, revision) = staged_run(vec![uncited_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm validate");

    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate step"));

    assert_eq!(
        run_status(&state, &run_id),
        KpiIngestRunState::ValidationFailed,
        "the run returned to the agent's worklist"
    );
    assert_eq!(
        progress_step(&state, &run_id).as_deref(),
        Some("validation_failed")
    );
    let counts = state.jobs().counts().expect("counts");
    assert_eq!(counts.succeeded, 1, "outcome=failed is queue success");
    assert_eq!(counts.pending, 0, "no commit row was chained");
    assert!(attention_events(&state).is_empty(), "no failure event");
}

// ---------------------------------------------------------------------
// Plan test 3: the lost-wakeup class — a re-stage while the old generation's
// row is still running arms a NEW row (generation-keyed ids).
// ---------------------------------------------------------------------

#[test]
fn a_restage_while_the_old_generation_runs_arms_a_new_row() {
    let (state, run_id, rev1) = staged_run(vec![uncited_observation()]);
    enqueue_validate(&state, &run_id, rev1).expect("arm rev1");

    // The rev1 row is claimed and RUNNING (not yet settled) — the exact window
    // where a stable job id lost the wakeup.
    let claimed = state
        .jobs()
        .claim_next_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("claim")
        .expect("row");
    assert_eq!(claimed.id, validate_job_id(&run_id, rev1));

    // Its work happens (validation fails), the agent repairs and re-stages.
    validate_kpi_ingest_run(&state, &run_id).expect("validate rev1");
    assert_eq!(
        run_status(&state, &run_id),
        KpiIngestRunState::ValidationFailed
    );
    let rev2 = stage(&state, &run_id, vec![clean_observation()]);
    assert!(rev2 > rev1);
    enqueue_validate(&state, &run_id, rev2).expect("arm rev2");

    // The fresh intent lives in its OWN pending row while rev1 is running.
    let rev2_row = state
        .jobs()
        .status(&validate_job_id(&run_id, rev2))
        .expect("status")
        .expect("row");
    assert_eq!(rev2_row.status, "pending");

    // rev1 settles (its handler would observe supersession and no-op).
    state.jobs().mark_succeeded(&claimed.id).expect("settle");

    // The rev2 row drives the run forward without any restart.
    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate rev2"));
    assert_eq!(
        run_status(&state, &run_id),
        KpiIngestRunState::ReadyToCommit
    );
}

// ---------------------------------------------------------------------
// Plan test 4: separate retry budgets by construction — the chained commit
// row starts with its own untouched budget.
// ---------------------------------------------------------------------

#[test]
fn the_chained_commit_row_owns_a_fresh_retry_budget() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm validate");

    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate step"));

    let validate_row = state
        .jobs()
        .status(&validate_job_id(&run_id, revision))
        .expect("status")
        .expect("row");
    assert_eq!(validate_row.attempts, 1, "validation consumed its attempt");

    let hash = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("run")
        .expect("exists")
        .manifest_hash
        .expect("hash");
    let commit_row = state
        .jobs()
        .status(&commit_job_id(&run_id, revision, &hash))
        .expect("status")
        .expect("row");
    assert_eq!(
        (commit_row.attempts, commit_row.max_attempts),
        (0, 3),
        "the commit generation is a separate row with its own full budget"
    );
}

// ---------------------------------------------------------------------
// Plan tests 5 + 6 (+12): exhaustion through the terminal hook; a
// non-terminal attempt fires nothing; a rolled-back commit leaves no
// `committed` progress snapshot.
// ---------------------------------------------------------------------

#[test]
fn validation_exhaustion_terminalizes_the_run_through_the_hook() {
    // A staged revision whose rows were destroyed underneath (raw corruption)
    // is a persistent validator `Conflict` while the run stays staged@N — the
    // retry path, driven to a terminal row.
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "DELETE FROM kpi_staged_observations WHERE run_id = ?1",
                [run_id.as_str()],
            )
            .expect("corrupt staged rows");
    }

    // max_attempts=1 makes the first failure terminal (the arm primitive's
    // budget of 3 is not under test here).
    let job_id = validate_job_id(&run_id, revision);
    let payload = serde_json::to_string(&ValidatePayload {
        job_id: job_id.clone(),
        run_id: run_id.clone(),
        revision,
    })
    .expect("payload");
    state
        .jobs()
        .enqueue(&job_id, KPI_INGEST_VALIDATE_KIND, &payload, 1)
        .expect("enqueue");

    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("terminal attempt"));

    let run = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("run")
        .expect("exists");
    assert_eq!(run.status, KpiIngestRunState::Failed);
    assert!(
        run.last_error.is_some(),
        "the run records the terminal error"
    );
    let events = attention_events(&state);
    assert_eq!(events.len(), 1, "exactly one job_failed event");
    assert_eq!(events[0].evidence_ref, job_id);
    assert_eq!(events[0].company_id.as_deref(), Some("c1"));
}

#[test]
fn a_nonterminal_commit_failure_fires_nothing_and_keeps_the_run_queue_owned() {
    // Vanishing pinned definition = a persistent commit error that is neither
    // contention nor supersession: the tuple is still live, so the handler
    // returns Err and the queue retries. Attempt 1 of 3 → no event, no domain
    // transition, and the rolled-back transaction left no `committed`
    // progress snapshot (plan test 12).
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm validate");
    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate step"));
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "DELETE FROM kpi_definitions WHERE metric_key = 'revenue'",
                [],
            )
            .expect("vanish definition");
    }

    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("commit attempt 1"));

    assert_eq!(
        run_status(&state, &run_id),
        KpiIngestRunState::ReadyToCommit,
        "the run stays queue-owned until the row is durably terminal"
    );
    assert_eq!(
        progress_step(&state, &run_id).as_deref(),
        Some("validation_ready"),
        "the rolled-back commit left no `committed` snapshot"
    );
    assert!(
        attention_events(&state).is_empty(),
        "no event before terminal"
    );
    let hash = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("run")
        .expect("exists")
        .manifest_hash
        .expect("hash");
    let row = state
        .jobs()
        .status(&commit_job_id(&run_id, revision, &hash))
        .expect("status")
        .expect("row");
    assert_eq!(row.status, "pending", "queued for retry");
}

#[test]
fn commit_exhaustion_terminalizes_the_run_through_the_hook() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm validate");
    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate step"));
    let hash = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("run")
        .expect("exists")
        .manifest_hash
        .expect("hash");
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "DELETE FROM kpi_definitions WHERE metric_key = 'revenue'",
                [],
            )
            .expect("vanish definition");
    }

    // Replace the chained 3-attempt row with a 1-attempt one: the first real
    // failure is terminal.
    let job_id = commit_job_id(&run_id, revision, &hash);
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE job_queue SET max_attempts = 1 WHERE id = ?1",
                [job_id.as_str()],
            )
            .expect("shrink budget");
    }
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("terminal attempt"));

    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Failed);
    let events = attention_events(&state);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].evidence_ref, job_id);
}

// ---------------------------------------------------------------------
// Plan test 7: id↔payload authority — malformed payloads terminalize
// identically live and after restart; a tampered payload never touches the
// run it names.
// ---------------------------------------------------------------------

#[test]
fn a_malformed_payload_terminalizes_the_id_named_run_live_and_after_restart() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    let job_id = validate_job_id(&run_id, revision);
    state
        .jobs()
        .enqueue(&job_id, KPI_INGEST_VALIDATE_KIND, "not-json", 1)
        .expect("enqueue");

    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("terminal attempt"));

    // Live: the id-authoritative hook already terminalized the run.
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Failed);
    let events = attention_events(&state);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].company_id.as_deref(),
        Some("c1"),
        "failure_context derives the company from the job id, not the payload"
    );
    assert_eq!(
        events[0].evidence_title.as_deref(),
        Some("Raport okresowy c1"),
        "the subject is the document's own title"
    );

    // Restart: reconciliation converges on the identical state — no second
    // event, no state change (the run already left the queue-owned states).
    state.jobs().reclaim_stale_running().expect("reclaim");
    reconcile_ingest_jobs(&state).expect("reconcile");
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Failed);
    assert_eq!(attention_events(&state).len(), 1);
}

#[test]
fn a_tampered_payload_naming_another_run_never_touches_it() {
    let connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "ca", "doca");
    seed_company_and_document(&connection, "cb", "docb");
    let state = AppState::new(connection);
    let run_a = create_run(&state, "doca", "ca");
    let rev_a = stage(&state, &run_a.id, vec![clean_observation()]);
    let run_b = create_run(&state, "docb", "cb");
    let rev_b = stage(&state, &run_b.id, vec![clean_observation()]);

    // The REAL row id names A; the payload coherently names B.
    let row_id = validate_job_id(&run_a.id, rev_a);
    let payload_for_b = serde_json::to_string(&ValidatePayload {
        job_id: validate_job_id(&run_b.id, rev_b),
        run_id: run_b.id.clone(),
        revision: rev_b,
    })
    .expect("payload");
    state
        .jobs()
        .enqueue(&row_id, KPI_INGEST_VALIDATE_KIND, &payload_for_b, 1)
        .expect("enqueue");

    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("terminal attempt"));

    // B untouched; A (the id authority) terminalized; the event references A.
    assert_eq!(run_status(&state, &run_b.id), KpiIngestRunState::Staged);
    assert_eq!(run_status(&state, &run_a.id), KpiIngestRunState::Failed);
    let events = attention_events(&state);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].evidence_ref, row_id);
    assert_eq!(events[0].company_id.as_deref(), Some("ca"));
}

// ---------------------------------------------------------------------
// Plan test 8: supersession arms the successor; replay is clean; the
// single-armer invariant is loud.
// ---------------------------------------------------------------------

#[test]
fn an_invalidation_cycle_rearms_validation_and_completes_without_restart() {
    // ready → invalidate (SAME revision) → the pending commit row observes
    // supersession, re-arms the succeeded validate row, and the drain reaches
    // `complete` without any restart. Identical staged content re-freezes the
    // identical manifest hash, so the commit generation legitimately reuses
    // its id — the r3 "reactivate a terminal row of the same generation" path.
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm validate");
    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate"));
    state
        .kpi_ingest_runs()
        .invalidate_manifest(&run_id)
        .expect("invalidate");
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Staged);

    // The stale commit row observes supersession → Ok + validate re-armed.
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("stale commit noop"));
    let validate_row = state
        .jobs()
        .status(&validate_job_id(&run_id, revision))
        .expect("status")
        .expect("row");
    assert_eq!(
        validate_row.status, "pending",
        "the succeeded validate row was re-armed for the same revision"
    );

    // Drain: revalidate → re-freeze → commit → complete, no restart.
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("revalidate"));
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("commit"));
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Complete);
}

#[test]
fn a_commit_replay_after_success_is_a_clean_noop() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm validate");
    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate"));
    let hash = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("run")
        .expect("exists")
        .manifest_hash
        .expect("hash");
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("commit"));
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Complete);

    // A duplicate/reclaimed commit row for the same tuple replays the stored
    // receipt (fast path) and succeeds without arming anything.
    let job_id = commit_job_id(&run_id, revision, &hash);
    let payload = serde_json::to_string(&CommitPayload {
        job_id: job_id.clone(),
        run_id: run_id.clone(),
        revision,
        manifest_hash: hash,
    })
    .expect("payload");
    state
        .jobs()
        .reschedule(&job_id, KPI_INGEST_COMMIT_KIND, &payload, 3)
        .expect("re-arm replay");
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("replay"));
    let counts = state.jobs().counts().expect("counts");
    assert_eq!(counts.failed, 0);
    assert_eq!(counts.pending, 0, "nothing was armed by the replay");
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Complete);
}

#[test]
fn arming_a_running_row_is_a_loud_typed_error_and_touches_nothing() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm validate");
    let claimed = state
        .jobs()
        .claim_next_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("claim")
        .expect("row");

    let result = enqueue_validate(&state, &run_id, revision);
    assert!(
        matches!(result, Err(ArmError::Running { .. })),
        "arming a running generation is the single-armer invariant violation"
    );
    let row = state
        .jobs()
        .status(&claimed.id)
        .expect("status")
        .expect("row");
    assert_eq!(row.status, "running", "the row was not touched");
    assert_eq!(row.attempts, 1, "attempts were not reset");
}

// ---------------------------------------------------------------------
// Plan test 9: startup reconciliation matrix (one focused test per case).
// ---------------------------------------------------------------------

#[test]
fn reconcile_arms_a_staged_run_with_no_job_row() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    // Crash between stage and enqueue: no row exists.
    reconcile_ingest_jobs(&state).expect("reconcile");
    let row = state
        .jobs()
        .status(&validate_job_id(&run_id, revision))
        .expect("status")
        .expect("row");
    assert_eq!(row.status, "pending");
}

#[test]
fn reconcile_rebuilds_the_commit_row_from_the_live_tuple() {
    // Crash between the validation atom and the chain-enqueue: the run is
    // ready_to_commit with NO commit row; reconciliation reconstructs the id
    // and payload (including the hash) from the live tuple.
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    validate_kpi_ingest_run(&state, &run_id).expect("validate directly");
    assert_eq!(
        run_status(&state, &run_id),
        KpiIngestRunState::ReadyToCommit
    );

    reconcile_ingest_jobs(&state).expect("reconcile");
    let hash = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("run")
        .expect("exists")
        .manifest_hash
        .expect("hash");
    let row = state
        .jobs()
        .status(&commit_job_id(&run_id, revision, &hash))
        .expect("status")
        .expect("row");
    assert_eq!(row.status, "pending");

    // The rebuilt row drives the run to completion.
    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("commit"));
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Complete);
}

#[test]
fn reconcile_rearms_an_inconsistent_succeeded_row_and_preserves_a_pending_one() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    let job_id = validate_job_id(&run_id, revision);

    // Inconsistent: the row succeeded but the run is still staged (e.g. a
    // crash rolled the domain work back after the queue settled).
    enqueue_validate(&state, &run_id, revision).expect("arm");
    let claimed = state
        .jobs()
        .claim_next_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("claim")
        .expect("row");
    state.jobs().mark_succeeded(&claimed.id).expect("settle");
    reconcile_ingest_jobs(&state).expect("reconcile");
    let row = state.jobs().status(&job_id).expect("status").expect("row");
    assert_eq!(row.status, "pending", "inconsistent succeeded row re-armed");

    // Pending rows keep their attempts/backoff: reconcile again and assert the
    // row (attempts already 1 from the claim above) is untouched.
    reconcile_ingest_jobs(&state).expect("reconcile again");
    let row = state.jobs().status(&job_id).expect("status").expect("row");
    assert_eq!(row.status, "pending");
    assert_eq!(row.attempts, 0, "reschedule reset, then left untouched");

    // A RUNNING row of the current generation is untouched too (reconcile
    // called without a prior reclaim — the branch a live worker would hit).
    let claimed = state
        .jobs()
        .claim_next_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("claim")
        .expect("row");
    reconcile_ingest_jobs(&state).expect("reconcile with running row");
    let row = state
        .jobs()
        .status(&claimed.id)
        .expect("status")
        .expect("row");
    assert_eq!(
        row.status, "running",
        "a running generation row is never touched"
    );
    assert_eq!(row.attempts, 1);
}

#[test]
fn reconcile_terminalizes_a_dead_lettered_current_generation() {
    // Plan test 10 (crash seam): the row was left `running` at its last
    // attempt (crash before settle), the run still staged. Startup: reclaim
    // dead-letters it → reconciliation fires ONE event and marks the run
    // failed, never resetting the exhausted row. A second pass converges.
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    let job_id = validate_job_id(&run_id, revision);
    let payload = serde_json::to_string(&ValidatePayload {
        job_id: job_id.clone(),
        run_id: run_id.clone(),
        revision,
    })
    .expect("payload");
    state
        .jobs()
        .enqueue(&job_id, KPI_INGEST_VALIDATE_KIND, &payload, 1)
        .expect("enqueue");
    let _claimed = state
        .jobs()
        .claim_next_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("claim")
        .expect("row");
    // Crash: never settled.

    state.jobs().reclaim_stale_running().expect("reclaim");

    // The crash-between-event-and-transition seam: a previous startup got as
    // far as recording the event, then died before `mark_failed`. The event
    // already exists (dedup key = the job id), the run is still queue-owned —
    // this reconciliation must complete the transition WITHOUT a second event.
    state
        .attention()
        .record_job_failure(&job_id, Some("c1"), None)
        .expect("pre-recorded event (crash seam)");
    reconcile_ingest_jobs(&state).expect("reconcile");

    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Failed);
    let row = state.jobs().status(&job_id).expect("status").expect("row");
    assert_eq!(row.status, "failed", "the exhausted row was NOT reset");
    let events = attention_events(&state);
    assert_eq!(events.len(), 1, "the pre-recorded event deduped, no second");
    assert_eq!(events[0].evidence_ref, job_id);

    // Idempotent repeat: a fully-converged state stays converged.
    state.jobs().reclaim_stale_running().expect("reclaim again");
    reconcile_ingest_jobs(&state).expect("reconcile again");
    assert_eq!(attention_events(&state).len(), 1, "no second event");
}

#[test]
fn reconcile_never_terminalizes_a_run_from_a_superseded_generations_row() {
    // A dead-lettered commit row of an OLD generation (different revision →
    // different id) must not fail the run; the live generation is armed.
    let (state, run_id, rev1) = staged_run(vec![uncited_observation()]);
    validate_kpi_ingest_run(&state, &run_id).expect("validate rev1 → failed");
    let rev2 = stage(&state, &run_id, vec![clean_observation()]);
    validate_kpi_ingest_run(&state, &run_id).expect("validate rev2 → ready");
    assert_eq!(
        run_status(&state, &run_id),
        KpiIngestRunState::ReadyToCommit
    );

    // Seed a dead-lettered row under the OLD generation's id.
    let stale_id = commit_job_id(&run_id, rev1, "stalehash");
    state
        .jobs()
        .enqueue(&stale_id, KPI_INGEST_COMMIT_KIND, "not-json", 1)
        .expect("enqueue stale");
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE job_queue SET status = 'failed', last_error = 'old failure'
                 WHERE id = ?1",
                [stale_id.as_str()],
            )
            .expect("dead-letter the stale row");
    }

    reconcile_ingest_jobs(&state).expect("reconcile");
    assert_eq!(
        run_status(&state, &run_id),
        KpiIngestRunState::ReadyToCommit,
        "a superseded generation's failure is inert bookkeeping"
    );
    let hash = state
        .kpi_ingest_runs()
        .get_run(&run_id)
        .expect("run")
        .expect("exists")
        .manifest_hash
        .expect("hash");
    let live_row = state
        .jobs()
        .status(&commit_job_id(&run_id, rev2, &hash))
        .expect("status")
        .expect("row");
    assert_eq!(live_row.status, "pending", "the live generation was armed");
    assert!(attention_events(&state).is_empty());
}

#[test]
fn reconcile_refuses_a_ready_run_with_a_null_hash() {
    let (state, run_id, _revision) = staged_run(vec![clean_observation()]);
    validate_kpi_ingest_run(&state, &run_id).expect("validate");
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET manifest_hash = NULL WHERE id = ?1",
                [run_id.as_str()],
            )
            .expect("corrupt hash");
    }
    let error = reconcile_ingest_jobs(&state).expect_err("raw corruption refused");
    assert!(
        error.contains("NULL manifest_hash"),
        "typed refusal: {error}"
    );
}

#[test]
fn reconcile_leaves_terminal_runs_untouched() {
    let (state, run_id, _revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, _revision).expect("arm");
    let worker = build_worker(state.clone());
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_VALIDATE_KIND])
        .expect("validate"));
    assert!(worker
        .process_one_for_kinds(&[KPI_INGEST_COMMIT_KIND])
        .expect("commit"));
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Complete);

    let before = state.jobs().counts().expect("counts");
    reconcile_ingest_jobs(&state).expect("reconcile");
    assert_eq!(state.jobs().counts().expect("counts"), before);
    assert_eq!(run_status(&state, &run_id), KpiIngestRunState::Complete);
}

// ---------------------------------------------------------------------
// Plan test 11: arming is idempotent over a pending row.
// ---------------------------------------------------------------------

#[test]
fn arming_the_same_generation_twice_is_one_row_with_untouched_attempts() {
    let (state, run_id, revision) = staged_run(vec![clean_observation()]);
    enqueue_validate(&state, &run_id, revision).expect("arm once");
    enqueue_validate(&state, &run_id, revision).expect("arm twice");
    let counts = state.jobs().counts().expect("counts");
    assert_eq!(counts.pending, 1, "one generation row");
    let row = state
        .jobs()
        .status(&validate_job_id(&run_id, revision))
        .expect("status")
        .expect("row");
    assert_eq!(row.attempts, 0);
}
