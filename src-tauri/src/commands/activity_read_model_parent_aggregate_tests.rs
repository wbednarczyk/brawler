//! Sweep/batch parent aggregate + fail-closed domain lookups (sol diff R3
//! #1/#3) — split out of `activity_read_model_tests.rs` to stay under the
//! file-size ratchet (ADR 0103).

use super::*;

// ------------------------------------------------------------------
// Parent aggregate from durable counters + member states (sol diff R3 #1)
// ------------------------------------------------------------------

/// Gives a sweep its own terminal `job_runs` occurrence — the shape a
/// `recent`-window item needs so `domain_status_override` runs at all.
fn seed_sweep_occurrence(state: &AppState, sweep_id: &str, cdr: &str, outcome: JobRunOutcome<'_>) {
    let run_id = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: format!("report-sweep:{sweep_id}"),
            run_key: format!("history_sweep:{sweep_id}"),
            kind: crate::jobs::history_sweep::HISTORY_SWEEP_KIND.to_owned(),
            family: ActivityFamily::ReportSweep,
            company_id: Some(cdr.to_owned()),
            subject: "GPW ESPI/EBI".to_owned(),
            target: ActivityTarget::Company {
                company_id: cdr.to_owned(),
                tool: Some(crate::jobs::activity_identity::ActivityTool::Pokrycie),
            },
            attempt: 1,
        })
        .expect("begin sweep occurrence");
    state
        .job_runs()
        .settle(run_id, outcome)
        .expect("settle sweep occurrence");
}

#[test]
fn failed_zero_member_sweep_stays_failed() {
    // sol diff R3 #1 case 1: a storage-level abort before any candidate was
    // even attempted (`fail_history_sweep`) — zero members, zero candidates.
    // The old unconditional "no member failures -> succeeded" fallback used
    // to override this honest `failed` row to `succeeded`.
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");
    state
        .history_sweeps()
        .fail_history_sweep(&sweep.id, "could not list candidates")
        .expect("fail sweep");
    seed_sweep_occurrence(
        &state,
        &sweep.id,
        &cdr,
        JobRunOutcome::Failed {
            error: "could not list candidates",
        },
    );

    let view = compute_activity(&state).expect("view");
    let item = view
        .recent
        .iter()
        .find(|item| item.activity_key == format!("report-sweep:{}", sweep.id))
        .expect("recent item");
    assert_eq!(item.status, "failed");
    assert_eq!(item.error.as_deref(), Some("could not list candidates"));
}

#[test]
fn completed_sweep_with_only_runs_failed_shows_failed() {
    // sol diff R3 #1 case 2: a completed sweep whose every candidate failed
    // to ENQUEUE (`runs_failed`, no successes, no members at all) — the row's
    // own `completed` status and the (previously ignored) `runs_failed`
    // counter disagree; the aggregate must side with `runs_failed`.
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute(
            "UPDATE history_sweeps
             SET status = 'completed', candidates_total = 3, runs_failed = 3,
                 enqueued_run_ids_json = '[]'
             WHERE id = ?1",
            [&sweep.id],
        )
        .expect("seed runs_failed-only sweep");
    seed_sweep_occurrence(&state, &sweep.id, &cdr, JobRunOutcome::Succeeded);

    let view = compute_activity(&state).expect("view");
    let item = view
        .recent
        .iter()
        .find(|item| item.activity_key == format!("report-sweep:{}", sweep.id))
        .expect("recent item");
    assert_eq!(item.status, "failed");
}

#[test]
fn completed_sweep_with_successes_and_enqueue_failures_shows_partial() {
    // sol diff R3 #1 case 3.
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");
    let doc = document(&state, &cdr, "Raport ukończony");
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run:ok",
            &cdr,
            &doc,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");
    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "UPDATE autopilot_run SET status = 'succeeded' WHERE id = ?1",
            [&run.id],
        )
        .expect("mark succeeded");
    connection
        .execute(
            "UPDATE history_sweeps
             SET status = 'completed', candidates_total = 2, runs_failed = 1,
                 enqueued_run_ids_json = ?2
             WHERE id = ?1",
            rusqlite::params![sweep.id, format!(r#"["{}"]"#, run.id)],
        )
        .expect("seed mixed sweep");
    drop(connection);
    seed_sweep_occurrence(&state, &sweep.id, &cdr, JobRunOutcome::Succeeded);

    let view = compute_activity(&state).expect("view");
    let item = view
        .recent
        .iter()
        .find(|item| item.activity_key == format!("report-sweep:{}", sweep.id))
        .expect("recent item");
    assert_eq!(item.status, "partial");
}

#[test]
fn skipped_candidates_never_count_as_in_flight() {
    // sol diff R3 #1 case 4: candidates already extracted before the sweep
    // ran (`skipped_existing`) were never enqueued at all — once every
    // ENQUEUED member is terminal, `inFlight` must be 0, never inflated by
    // counting skipped candidates as still-running work.
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");
    let doc = document(&state, &cdr, "Raport ukończony");
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run:ok",
            &cdr,
            &doc,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");
    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "UPDATE autopilot_run SET status = 'succeeded' WHERE id = ?1",
            [&run.id],
        )
        .expect("mark succeeded");
    connection
        .execute(
            "UPDATE history_sweeps
             SET status = 'completed', candidates_total = 3, skipped_existing = 2,
                 enqueued_run_ids_json = ?2
             WHERE id = ?1",
            rusqlite::params![sweep.id, format!(r#"["{}"]"#, run.id)],
        )
        .expect("seed skip-heavy sweep");
    drop(connection);
    seed_sweep_occurrence(&state, &sweep.id, &cdr, JobRunOutcome::Succeeded);

    let view = compute_activity(&state).expect("view");
    let item = view
        .recent
        .iter()
        .find(|item| item.activity_key == format!("report-sweep:{}", sweep.id))
        .expect("recent item");
    assert_eq!(item.status, "succeeded");
    assert_eq!(
        item.in_flight,
        Some(0),
        "skipped candidates are never in flight"
    );
    let progress = item.progress.as_ref().expect("progress");
    assert_eq!(
        progress.done, 3,
        "1 succeeded member + 2 skipped candidates both count as done"
    );
    assert_eq!(progress.total, 3);
    assert_eq!(progress.failed, 0);
}

#[test]
fn sweep_with_only_a_partial_child_is_partial() {
    // sol diff R3 #1 case 5: a parent whose only member is `partial` (never
    // `succeeded`/`failed`) must resolve `partial` itself.
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");
    let doc = document(&state, &cdr, "Raport częściowy");
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run:partial-only",
            &cdr,
            &doc,
            "history_sweep",
            "autopilot",
            Some(&sweep.id),
        )
        .expect("create run")
        .expect("created");
    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute(
            "UPDATE autopilot_run SET status = 'partial' WHERE id = ?1",
            [&run.id],
        )
        .expect("mark partial");
    connection
        .execute(
            "UPDATE history_sweeps
             SET status = 'completed', candidates_total = 1, enqueued_run_ids_json = ?2
             WHERE id = ?1",
            rusqlite::params![sweep.id, format!(r#"["{}"]"#, run.id)],
        )
        .expect("seed partial-only sweep");
    drop(connection);
    seed_sweep_occurrence(&state, &sweep.id, &cdr, JobRunOutcome::Succeeded);

    let view = compute_activity(&state).expect("view");
    let item = view
        .recent
        .iter()
        .find(|item| item.activity_key == format!("report-sweep:{}", sweep.id))
        .expect("recent item");
    assert_eq!(item.status, "partial");
}

#[test]
fn malformed_member_json_surfaces_as_error() {
    // sol diff R3 #1 case 6: a malformed `enqueued_run_ids_json` is a
    // `StorageError`, never a silently-empty member list.
    let state = state();
    let cdr = company(&state, "CDR");
    let sweep = state
        .history_sweeps()
        .create_history_sweep(&cdr, "manual")
        .expect("sweep");
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute(
            "UPDATE history_sweeps
             SET status = 'completed', candidates_total = 1, enqueued_run_ids_json = 'not json'
             WHERE id = ?1",
            [&sweep.id],
        )
        .expect("seed malformed json");
    seed_sweep_occurrence(&state, &sweep.id, &cdr, JobRunOutcome::Succeeded);

    let result = compute_activity(&state);
    assert!(
        result.is_err(),
        "a malformed enqueued_run_ids_json must surface as an error: {result:?}"
    );
}

// ------------------------------------------------------------------
// Domain lookups fail closed (sol diff R3 #3)
// ------------------------------------------------------------------

#[test]
fn a_forced_report_reading_domain_lookup_failure_surfaces_from_compute_activity() {
    // `autopilot_run_status` used to collapse ANY query failure (not just a
    // genuinely absent row) into the same `None` the caller reads as "leave
    // the ledger status as is" — a report-reading recent item could silently
    // keep showing a stale `succeeded` while the domain lookup had actually
    // failed.
    let state = state();
    let cdr = company(&state, "CDR");
    let doc = document(&state, &cdr, "Raport z uszkodzoną bazą");
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run:domain-lookup-broken",
            &cdr,
            &doc,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    let run_id = state
        .job_runs()
        .begin_attempt(NewJobRun {
            activity_key: format!("report-reading:{}", run.id),
            run_key: format!("autopilot:{}:notify", run.id),
            kind: crate::jobs::autopilot::AUTOPILOT_STAGE_KIND.to_owned(),
            family: ActivityFamily::ReportReading,
            company_id: Some(cdr.clone()),
            subject: "Raport z uszkodzoną bazą".to_owned(),
            target: ActivityTarget::Company {
                company_id: cdr.clone(),
                tool: None,
            },
            attempt: 1,
        })
        .expect("begin");
    state
        .job_runs()
        .settle(run_id, JobRunOutcome::Succeeded)
        .expect("settle succeeded");

    // Dropping the WHOLE table would also break the pre-existing (already
    // fail-closed) `stalled_autopilot_runs` scan, so ANY code — fixed or not
    // — would error, proving nothing about THIS lookup specifically. Corrupt
    // just this one row's `status` into a BLOB instead (`PRAGMA
    // ignore_check_constraints` bypasses the CHECK that would otherwise
    // refuse it): `autopilot_run_status`'s plain `id = ?1` lookup must then
    // decode it and fail; `stalled_autopilot_runs`'s `status IN
    // ('pending','running')` filter never matches a BLOB, so it stays clean.
    let connection = state.checkout_for_tests().expect("checkout");
    connection
        .execute_batch("PRAGMA ignore_check_constraints = 1;")
        .expect("disable check constraints for this connection");
    connection
        .execute(
            "UPDATE autopilot_run SET status = X'ff' WHERE id = ?1",
            [&run.id],
        )
        .expect("corrupt status to force a real decode failure on this row");
    drop(connection);

    let result = compute_activity(&state);
    assert!(
        result.is_err(),
        "a forced report-reading domain lookup failure must propagate, never leave the \
         ledger's stale status in place: {result:?}"
    );
}
