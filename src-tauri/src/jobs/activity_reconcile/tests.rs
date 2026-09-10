use super::*;
use crate::jobs::autopilot::AUTOPILOT_STAGE_KIND;
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
fn startup_reconcile_reschedules_a_run_whose_stage_job_is_entirely_missing() {
    // Issue #458 (ADR 0109 amendment): a non-terminal run with no live stage
    // job is no longer failed outright — only a DEAD-LETTERED stage job fails
    // it (`startup_reconcile_fails_a_run_with_a_dead_lettered_stage_job`,
    // unchanged below). A run whose stage job never existed at all (this
    // test) is resumable: its last-started stage (the default `fetch`, on a
    // freshly created run) is rescheduled, exactly like the queue's own
    // crash-residue reclaim resumes a crashed `running` job.
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
    // No stage job at all for this run: provably stranded, but not dead-lettered.

    reconcile_on_startup(&state);

    let after = state.autopilot().get_run(&run.id).expect("run");
    assert_eq!(
        after.status, "pending",
        "reconcile never touches run.status on a reschedule"
    );
    let job = state
        .jobs()
        .status(&crate::jobs::autopilot::stage_job_id(&run.id, &after.stage))
        .expect("status")
        .expect("the missing stage job must have been rescheduled");
    assert_eq!(job.status, "pending");
}

#[test]
fn startup_reconcile_reschedules_the_last_started_stage_of_a_stranded_run_exactly_once() {
    // Issue #458 (ADR 0109 amendment): a run stuck `running` at a stage with
    // NO job under it at all (crash before the stage's own job was even
    // enqueued) is resumable — its last-started stage (`extract`, here) is
    // rescheduled once. A SECOND reconcile pass must be a no-op (idempotent):
    // no new row, `attempts` untouched.
    let state = state();
    let company_id = company(&state);
    let document_id = state
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/r5.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport 5".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run-5",
            &company_id,
            &document_id,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    state
        .autopilot()
        .set_run_stage(&run.id, "extract", "running")
        .expect("set stage");
    // No job row at all for `extract`.

    reconcile_on_startup(&state);

    let job_id = crate::jobs::autopilot::stage_job_id(&run.id, "extract");
    let job = state
        .jobs()
        .status(&job_id)
        .expect("status")
        .expect("the last-started stage must have been rescheduled");
    assert_eq!(job.status, "pending");
    assert_eq!(job.attempts, 0);

    reconcile_on_startup(&state);
    let job_again = state
        .jobs()
        .status(&job_id)
        .expect("status")
        .expect("job row");
    assert_eq!(
        job_again.attempts, 0,
        "a second reconcile pass must be a no-op — reschedule on an already-pending row does nothing"
    );
}

#[test]
fn startup_reconcile_reschedules_when_a_stale_succeeded_job_from_a_previous_generation_exists() {
    // Issue #458: a run can be RECREATED under the same deterministic id
    // (`create_run_if_absent`'s self-heal DELETE-then-INSERT path) while its
    // stage's `job_queue` row from a PRIOR life still sits there, terminally
    // `succeeded` — a stale row, not a dead letter. That must resume the same
    // as a missing job (reschedule), never fail the run outright.
    let state = state();
    let company_id = company(&state);
    let document_id = state
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/r6.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport 6".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run-6",
            &company_id,
            &document_id,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    state
        .autopilot()
        .set_run_stage(&run.id, "extract", "running")
        .expect("set stage");
    let job_id = crate::jobs::autopilot::stage_job_id(&run.id, "extract");
    state
        .jobs()
        .enqueue(
            &job_id,
            AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"run-6","stage":"extract"}"#,
            3,
        )
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");
    state.jobs().mark_succeeded(&job_id).expect("succeed");

    reconcile_on_startup(&state);

    let job = state
        .jobs()
        .status(&job_id)
        .expect("status")
        .expect("job row");
    assert_eq!(
        job.status, "pending",
        "a stale succeeded row from a previous generation must be rescheduled, not treated as a dead letter"
    );
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
fn startup_reconcile_is_not_fooled_by_a_like_false_positive_from_an_underscore_in_the_run_id() {
    // sol diff R1 #12: the previous `id LIKE 'autopilot:' || run_id || ':%'`
    // check treated `_` in `run_id` as a SQL LIKE wildcard (matches ANY
    // single character) — an UNRELATED job_queue row could then
    // false-positive-match, making a genuinely stranded run look "live" and
    // never get reconciled. `co_1` and a decoy row `autopilot:coX1:fetch`
    // (the `_` position substituted) would have collided under the old
    // check; the exact `IN` fix must not be fooled.
    let state = state();
    let company_id = company(&state);
    let document_id = state
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/underscore.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport podkreślnik".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    let run = state
        .autopilot()
        .create_run_if_absent(
            "co_1",
            &company_id,
            &document_id,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    // The decoy: NOT a real stage job for `co_1` (its run_id in the payload
    // deliberately does not match either), but its id would satisfy the OLD
    // LIKE pattern `autopilot:co_1:%` (`_` wildcards to `X`).
    state
        .jobs()
        .enqueue(
            "autopilot:coX1:fetch",
            AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"unrelated","stage":"fetch"}"#,
            3,
        )
        .expect("enqueue decoy");

    reconcile_on_startup(&state);

    // Issue #458 (ADR 0109 amendment): `co_1`'s own stage job is MISSING
    // (only the decoy under a different id exists), so reconcile reschedules
    // it rather than failing the run — the decoy must not have been mistaken
    // for `co_1`'s job either way (the point of this test, unchanged): a
    // false "live" read would have left `co_1` untouched (still `pending`
    // with NO real job), while the correct exact-`IN` read sees no live job
    // and reschedules `co_1`'s real stage job.
    let run = state.autopilot().get_run(&run.id).expect("run");
    assert_eq!(
        run.status, "pending",
        "the decoy row must not falsely keep `co_1` looking live"
    );
    let real_job = state
        .jobs()
        .status(&crate::jobs::autopilot::stage_job_id(&run.id, &run.stage))
        .expect("status")
        .expect("co_1's own stage job must have been rescheduled");
    assert_eq!(real_job.status, "pending");
    let decoy = state
        .jobs()
        .status("autopilot:coX1:fetch")
        .expect("status")
        .expect("decoy row untouched");
    assert_eq!(
        decoy.attempts, 0,
        "the decoy must never have been claimed or otherwise touched by reconcile"
    );
}

#[test]
fn startup_reconcile_reschedules_a_run_whose_last_stage_succeeded_with_no_successor_and_run_until_idle_finalizes_it(
) {
    // sol diff R1 #17 + issue #458 (ADR 0109 amendment, 2026-09-08): `notify`
    // is the LAST stage — it has no successor to enqueue at all (`next_stage`
    // returns `None` for it). A crash between the notify job settling
    // `succeeded` and `autopilot_run.status` itself being finalized leaves
    // the run `pending`/`running` with NO live stage job. Under the queue's
    // own crash contract this is NOT a dead letter (the notify job's own
    // terminal state is `succeeded`, not `failed`) — it is resumable: the
    // run's last-started stage (`notify`) is rescheduled ONCE and re-runs;
    // `finalize_notify` is idempotent (unconditionally re-finalizes as
    // `succeeded`), so the re-run settles the run instead of stranding it
    // `failed` forever (the pre-fix behavior this test used to pin).
    let state = state();
    let company_id = company(&state);
    let document_id = state
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "official_report".to_owned(),
            url: "https://example.test/r4.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport 4".to_owned()),
            attribution: None,
        })
        .expect("document")
        .id;
    let run = state
        .autopilot()
        .create_run_if_absent(
            "run-4",
            &company_id,
            &document_id,
            "detection",
            "autopilot",
            None,
        )
        .expect("create run")
        .expect("created");
    // The run genuinely reached the notify stage before the simulated crash
    // (`run_stage` stamps stage/status -> running BEFORE running the stage
    // body) — realistic fidelity for the "reschedule the last-started stage"
    // rule, which reads `run.stage`.
    state
        .autopilot()
        .set_run_stage(&run.id, "notify", "running")
        .expect("set stage");
    // The notify stage's job ran to a genuine terminal success...
    state
        .jobs()
        .enqueue(
            "autopilot:run-4:notify",
            AUTOPILOT_STAGE_KIND,
            r#"{"run_id":"run-4","stage":"notify"}"#,
            1,
        )
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");
    state
        .jobs()
        .mark_succeeded("autopilot:run-4:notify")
        .expect("succeed");
    // ...but the run's own finalization (autopilot_run.status -> terminal)
    // never happened — simulating a crash in that exact window.

    reconcile_on_startup(&state);

    let after_reconcile = state.autopilot().get_run(&run.id).expect("run");
    assert_eq!(
        after_reconcile.status, "running",
        "reconcile never touches run.status on a reschedule"
    );
    let job = state
        .jobs()
        .status("autopilot:run-4:notify")
        .expect("status")
        .expect("job row");
    assert_eq!(
        job.status, "pending",
        "the notify job must be re-armed once, not left succeeded forever"
    );

    // Idempotent: a second reconcile pass must not re-arm it again (still
    // `pending`, `attempts` untouched — `reschedule` on an already-pending
    // row is a no-op).
    reconcile_on_startup(&state);
    let job_again = state
        .jobs()
        .status("autopilot:run-4:notify")
        .expect("status")
        .expect("job row");
    assert_eq!(job_again.attempts, job.attempts);

    // Driving the queue to idle re-runs the notify stage; it finalizes the
    // run instead of leaving it dangling.
    crate::jobs::handlers::build_worker(state.clone())
        .run_until_idle()
        .expect("drain the queue");

    let finalized = state.autopilot().get_run(&run.id).expect("run");
    assert_eq!(
        finalized.status, "succeeded",
        "the re-armed notify stage must finalize the run, not strand it"
    );
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
fn startup_reconcile_interrupt_is_atomic_across_all_open_occurrences() {
    // sol diff R1 #12: the interrupt pass now runs in ONE `BEGIN IMMEDIATE`
    // transaction — a fault partway must leave EVERY open occurrence
    // untouched (still `running`), never a split where some settled and
    // others didn't.
    let state = state();
    let run_id_a = state
        .job_runs()
        .begin_attempt(crate::storage::NewJobRun {
            activity_key: "source-refresh:a".to_owned(),
            run_key: "job-a".to_owned(),
            kind: "scheduled_source_refresh".to_owned(),
            family: crate::jobs::activity_identity::ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "a".to_owned(),
            target: crate::jobs::activity_identity::ActivityTarget::Sources,
            attempt: 1,
        })
        .expect("begin a");
    let run_id_b = state
        .job_runs()
        .begin_attempt(crate::storage::NewJobRun {
            activity_key: "source-refresh:b".to_owned(),
            run_key: "job-b".to_owned(),
            kind: "scheduled_source_refresh".to_owned(),
            family: crate::jobs::activity_identity::ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "b".to_owned(),
            target: crate::jobs::activity_identity::ActivityTarget::Sources,
            attempt: 1,
        })
        .expect("begin b");

    // Poison the settle UPDATE so the tx aborts partway through the loop.
    state
        .checkout_for_tests()
        .expect("checkout")
        .execute_batch(
            "CREATE TRIGGER poison_interrupt BEFORE UPDATE ON job_runs
             BEGIN SELECT RAISE(ABORT, 'interrupt poisoned for test'); END;",
        )
        .expect("install poison trigger");

    // Best-effort: reconcile_on_startup swallows the error and continues —
    // assert the DB state directly, not a return value.
    reconcile_on_startup(&state);

    let connection = state.checkout_for_tests().expect("checkout");
    for run_id in [run_id_a, run_id_b] {
        let status: String = connection
            .query_row(
                "SELECT status FROM job_runs WHERE id = ?1",
                [run_id],
                |row| row.get(0),
            )
            .expect("status");
        assert_eq!(
            status, "running",
            "a poisoned tx must leave EVERY occurrence untouched, never a partial split"
        );
    }
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
    reconcile_on_startup(&state); // second call must not error or re-flip anything

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
fn startup_reconcile_prunes_finished_rows_to_the_500_retention_cap() {
    // sol diff R2 #8(c): retention (`job_runs::prune`, ADR 0109 dec. 2 — keep
    // the newest 500 FINISHED rows) runs in the SAME transaction as this
    // pass's open-occurrence interrupt. Seed 500 already-finished rows PLUS
    // one open ("running") occurrence that reconcile itself terminalizes to
    // `interrupted` — 501 finished rows once reconcile runs — and assert the
    // table settles back to the 500-row cap, with the interrupted row
    // (freshest by `finished_at`/id) surviving and the single oldest
    // pre-existing row the one pruned.
    let state = state();
    {
        let mut connection = state.checkout_for_tests().expect("checkout");
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("tx");
        for i in 0..500 {
            tx.execute(
                "INSERT INTO job_runs
                    (activity_key, run_key, kind, family, subject, target_json, status,
                     attempt, started_at, finished_at)
                 VALUES (?1, ?1, 'k', 'sourceRefresh', 's', '{\"kind\":\"sources\"}',
                     'succeeded', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                [format!("pre-existing:{i}")],
            )
            .expect("seed finished row");
        }
        tx.commit().expect("commit");
    }
    let oldest_id: i64 = state
        .checkout_for_tests()
        .expect("checkout")
        .query_row("SELECT MIN(id) FROM job_runs", [], |row| row.get(0))
        .expect("oldest id");

    // One genuinely OPEN occurrence — reconcile must interrupt this AND
    // still enforce retention in the same pass.
    state
        .job_runs()
        .begin_attempt(crate::storage::NewJobRun {
            activity_key: "source-refresh:open".to_owned(),
            run_key: "job-open".to_owned(),
            kind: "scheduled_source_refresh".to_owned(),
            family: crate::jobs::activity_identity::ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "open".to_owned(),
            target: crate::jobs::activity_identity::ActivityTarget::Sources,
            attempt: 1,
        })
        .expect("begin the open occurrence");

    reconcile_on_startup(&state);

    let connection = state.checkout_for_tests().expect("checkout");
    let total: i64 = connection
        .query_row("SELECT COUNT(*) FROM job_runs", [], |row| row.get(0))
        .expect("count");
    assert_eq!(
        total, 500,
        "retention caps at 500 rows even right after reconcile terminalizes an open one"
    );

    let oldest_survives: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM job_runs WHERE id = ?1",
            [oldest_id],
            |row| row.get(0),
        )
        .expect("check oldest");
    assert_eq!(
        oldest_survives, 0,
        "the single oldest row (501 finished, cap 500) must be the one pruned"
    );

    let open_status: String = connection
        .query_row(
            "SELECT status FROM job_runs WHERE activity_key = 'source-refresh:open'",
            [],
            |row| row.get(0),
        )
        .expect("open row status");
    assert_eq!(
        open_status, "interrupted",
        "the open occurrence must still be interrupted, not swallowed by retention"
    );
}
