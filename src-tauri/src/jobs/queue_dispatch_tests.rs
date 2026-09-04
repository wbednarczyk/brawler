//! Occurrence-ledger dispatch tests (ADR 0109 dec. 2) — split out of
//! `queue_tests.rs` to stay under the file-size ratchet (ADR 0103).

use super::*;

/// A handler under a REGISTERED kind (so identity resolution succeeds),
/// controllable to succeed, fail, or panic.
struct ActivityHandler {
    fail_first: usize,
    panics: bool,
    runs: AtomicUsize,
}

impl JobHandler for ActivityHandler {
    fn kind(&self) -> &'static str {
        crate::jobs::scheduler::SOURCE_REFRESH_KIND
    }
    fn serialization_key(&self, payload: &str) -> Option<String> {
        serde_json::from_str::<serde_json::Value>(payload)
            .ok()?
            .get("adapterId")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
    }
    fn run(&self, _payload: &str, _state: &AppState) -> Result<(), String> {
        let prior = self.runs.fetch_add(1, Ordering::SeqCst);
        if self.panics {
            panic!("boom");
        }
        if prior < self.fail_first {
            Err(format!("transient failure {prior}"))
        } else {
            Ok(())
        }
    }
}

fn activity_worker(handler: Arc<ActivityHandler>) -> JobWorker {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let mut worker = JobWorker::new(state);
    worker.register(handler);
    worker
}

fn job_runs_run_keys(state: &AppState) -> Vec<String> {
    let connection = state.checkout_for_tests().expect("checkout");
    let mut statement = connection
        .prepare("SELECT run_key FROM job_runs ORDER BY id")
        .expect("prepare");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .map(|row| row.expect("row"))
        .collect()
}

#[test]
fn queue_handler_core_call_is_not_double_counted() {
    // ADR 0109 dec. 3: a scheduled run's occurrence carries the QUEUE job
    // id as its `run_key` (written by the dispatch seam's `begin_attempt`)
    // — never a `direct:` one (that prefix belongs to the direct-activity
    // registry's awaited-path wrapper, never reached by a queue handler,
    // which calls the unwrapped core) — and exactly one row per attempt.
    let handler = Arc::new(ActivityHandler {
        fail_first: 0,
        panics: false,
        runs: AtomicUsize::new(0),
    });
    let worker = activity_worker(handler);
    worker
        .state
        .jobs()
        .enqueue(
            "src:scheduled-1",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");
    assert!(worker.process_one().expect("process"));

    let run_keys = job_runs_run_keys(&worker.state);
    assert_eq!(
        run_keys,
        vec!["src:scheduled-1".to_owned()],
        "the run_key is the queue job id, never a direct: prefix"
    );
}

#[test]
fn dispatch_writes_one_job_run_per_attempt() {
    // Success: one row, terminal `succeeded`.
    let handler = Arc::new(ActivityHandler {
        fail_first: 0,
        panics: false,
        runs: AtomicUsize::new(0),
    });
    let worker = activity_worker(handler.clone());
    worker
        .state
        .jobs()
        .enqueue(
            "src:1",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");
    assert!(worker.process_one().expect("process"));
    assert_eq!(job_runs_statuses(&worker.state), vec!["succeeded"]);

    // Terminal failure (max_attempts = 1): one row, terminal `failed`.
    let handler = Arc::new(ActivityHandler {
        fail_first: usize::MAX,
        panics: false,
        runs: AtomicUsize::new(0),
    });
    let worker = activity_worker(handler.clone());
    worker
        .state
        .jobs()
        .enqueue(
            "src:2",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");
    assert!(worker.process_one().expect("process"));
    assert_eq!(job_runs_statuses(&worker.state), vec!["failed"]);

    // Retry scheduled (attempts left): one row, `retry_scheduled`.
    let handler = Arc::new(ActivityHandler {
        fail_first: usize::MAX,
        panics: false,
        runs: AtomicUsize::new(0),
    });
    let worker = activity_worker(handler.clone());
    worker
        .state
        .jobs()
        .enqueue(
            "src:3",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            3,
        )
        .expect("enqueue");
    assert!(worker.process_one().expect("process"));
    assert_eq!(job_runs_statuses(&worker.state), vec!["retry_scheduled"]);

    // A deferred job (source lock contention) writes NO occurrence.
    let handler = Arc::new(ActivityHandler {
        fail_first: 0,
        panics: false,
        runs: AtomicUsize::new(0),
    });
    let worker = activity_worker(handler.clone());
    worker
        .state
        .jobs()
        .enqueue(
            "src:4",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");
    let held = worker
        .state
        .try_acquire_source("gpw-espi-ebi")
        .expect("hold");
    assert!(worker.process_one().expect("process"), "claimed + deferred");
    assert!(job_runs_statuses(&worker.state).is_empty());
    drop(held);
}

#[test]
fn begin_attempt_failure_skips_run_and_defers() {
    // A poisoned job_runs table makes `begin_attempt` fail: the handler must
    // NOT run, and the claim is deferred (not failed) rather than losing the
    // job entirely.
    let handler = Arc::new(ActivityHandler {
        fail_first: 0,
        panics: false,
        runs: AtomicUsize::new(0),
    });
    let worker = activity_worker(handler.clone());
    worker
        .state
        .jobs()
        .enqueue(
            "src:1",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            3,
        )
        .expect("enqueue");
    worker
        .state
        .checkout_for_tests()
        .expect("checkout")
        .execute("DROP TABLE job_runs", [])
        .expect("poison job_runs");

    assert!(worker.process_one().expect("process"), "claimed + deferred");
    assert_eq!(
        handler.runs.load(Ordering::SeqCst),
        0,
        "the handler must not run when begin_attempt fails"
    );
    let counts = worker.state.jobs().counts().expect("counts");
    assert_eq!(counts.pending, 1, "deferred back to pending, not failed");
    assert_eq!(counts.failed, 0);
}

#[test]
fn settle_failure_recovers_and_leaves_no_row_running() {
    // sol diff R2 #1: the prior version of this test asserted the LEAK
    // itself (both sides stay `running` forever after a settle failure —
    // exactly the stranded-claim bug ADR 0109 dec. 2 exists to prevent).
    // Fixed: `dispatch_claimed` retries the SAME settle call once
    // (`settle_with_recovery`) before giving up; poison the trigger keyed on
    // the handler's REAL failure text, so both the primary attempt and the
    // retry — which use the identical error string — fail identically, and
    // `ClaimGuard`'s Drop fallback (a DIFFERENT, fixed message) is let
    // through as the last resort. No row remains `running` on either side.
    let handler = Arc::new(ActivityHandler {
        fail_first: usize::MAX,
        panics: false,
        runs: AtomicUsize::new(0),
    });
    let worker = activity_worker(handler.clone());
    worker
        .state
        .jobs()
        .enqueue(
            "src:1",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            1,
        )
        .expect("enqueue");

    // A trigger on UPDATE only — `begin_attempt`'s INSERT still succeeds, so
    // identity resolution proceeds normally; only the settle UPDATE fails,
    // and only for the handler's exact error text (`ActivityHandler`'s first
    // failure message) — `ClaimGuard`'s Drop fallback settles with its own
    // fixed "panic: job worker unwound before settling" message, which does
    // not match and is let through.
    worker
        .state
        .checkout_for_tests()
        .expect("checkout")
        .execute_batch(
            "CREATE TRIGGER poison_job_runs_settle BEFORE UPDATE ON job_runs
             WHEN NEW.error = 'transient failure 0'
             BEGIN SELECT RAISE(ABORT, 'settle poisoned for test'); END;",
        )
        .expect("install poison trigger");

    assert!(
        worker
            .process_one()
            .expect("dispatch survives the settle failure"),
        "a job was claimed and processed"
    );

    let queue_status = worker
        .state
        .jobs()
        .status("src:1")
        .expect("status")
        .expect("row");
    assert_eq!(
        queue_status.status, "failed",
        "the queue row must never strand `running` after a settle failure — the Drop \
         fallback terminalizes it once the ordinary retry also fails"
    );
    assert_eq!(
        job_runs_statuses(&worker.state),
        vec!["failed"],
        "the occurrence must never strand `running` either — same fallback settle"
    );
}

#[test]
fn defer_failure_recovers_and_leaves_no_row_running() {
    // sol diff R2 #1 (defer counterpart): a `defer()` failure (source-lock
    // contention) must not silently strand the claimed row `running` either.
    // `defer()`'s UPDATE sets `status = 'pending'` and never touches
    // `last_error`, so a trigger keyed on that exact shape poisons both the
    // primary attempt and the one recovery retry; `ClaimGuard`'s Drop
    // fallback goes through the ordinary failure path instead
    // (`mark_failed_with_run`, which sets a non-null `last_error` and, with
    // `max_attempts` exhausted, `status = 'failed'`), so its attempt is let
    // through.
    let handler = Arc::new(SerializingHandler {
        key: "bankier",
        runs: AtomicUsize::new(0),
    });
    let state = AppState::new(open_in_memory_database().expect("db"));
    let mut worker = JobWorker::new(state);
    worker.register(handler.clone());
    worker
        .state
        .jobs()
        .enqueue("job", "serialized-kind", "{}", 1)
        .expect("enqueue");

    // Another worker holds the source lock, so `dispatch_claimed` takes the
    // source-busy defer branch.
    let held = worker.state.try_acquire_source("bankier").expect("hold");
    worker
        .state
        .checkout_for_tests()
        .expect("checkout")
        .execute_batch(
            "CREATE TRIGGER poison_job_queue_defer BEFORE UPDATE ON job_queue
             WHEN NEW.status = 'pending' AND NEW.last_error IS NULL
             BEGIN SELECT RAISE(ABORT, 'defer poisoned for test'); END;",
        )
        .expect("install poison trigger");

    assert!(
        worker
            .process_one()
            .expect("dispatch survives the defer failure"),
        "a job was claimed"
    );
    assert_eq!(
        handler.runs.load(Ordering::SeqCst),
        0,
        "the handler must never run while the source is locked"
    );

    let queue_status = worker
        .state
        .jobs()
        .status("job")
        .expect("status")
        .expect("row");
    assert_ne!(
        queue_status.status, "running",
        "a repeatedly failing defer must never strand the claimed row `running` — the \
         Drop fallback's ordinary-failure settle is let through once the retry also fails"
    );
    drop(held);
}

#[test]
fn handler_panic_takes_the_retry_path() {
    // A handler panic is contained (`catch_unwind`): the queue row goes back
    // to `pending` with attempts+1, the occurrence settles `retry_scheduled`
    // (never left open), and the SAME worker instance survives to process the
    // next job normally (sol diff R1 #17: the prior version of this test
    // built a SECOND worker sharing state, proving state survival but never
    // that the WORKER THREAD/instance itself keeps running).
    let handler = Arc::new(ActivityHandler {
        fail_first: 0,
        panics: true,
        runs: AtomicUsize::new(0),
    });
    let state = AppState::new(open_in_memory_database().expect("db"));
    let mut worker = JobWorker::new(state);
    worker.register(handler.clone());
    worker.register(Arc::new(OkHandler));

    worker
        .state
        .jobs()
        .enqueue(
            "src:1",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            r#"{"adapterId":"gpw-espi-ebi"}"#,
            3,
        )
        .expect("enqueue");

    assert!(worker.process_one().expect("process survives the panic"));

    let status = worker
        .state
        .jobs()
        .status("src:1")
        .expect("status")
        .expect("row");
    assert_eq!(status.status, "pending");
    assert_eq!(status.attempts, 1);
    assert_eq!(job_runs_statuses(&worker.state), vec!["retry_scheduled"]);

    // Worker continues to process the NEXT job normally after a panic — the
    // SAME `worker` instance, never a second one.
    worker
        .state
        .jobs()
        .enqueue("other-ok", "test-ok-after-panic", "{}", 1)
        .expect("enqueue second");
    assert!(worker.process_one().expect("second job processes fine"));
    assert_eq!(
        worker
            .state
            .jobs()
            .status("other-ok")
            .expect("status")
            .expect("row")
            .status,
        "succeeded"
    );
}

/// A handler that panics in `serialization_key` — top-of-scaffolding, OUTSIDE
/// the inner `run_outcome` boundary, so only the OUTER `catch_unwind` around
/// `dispatch_claimed` (in `dispatch`) can catch it — and counts
/// `on_terminal_failure` calls.
struct PanicsInScaffoldingWithHooksHandler {
    terminal_calls: AtomicUsize,
}

impl JobHandler for PanicsInScaffoldingWithHooksHandler {
    fn kind(&self) -> &'static str {
        "panics-in-scaffolding-with-hooks"
    }
    fn serialization_key(&self, _payload: &str) -> Option<String> {
        panic!("boom: scaffolding panic on the last attempt");
    }
    fn run(&self, _payload: &str, _state: &AppState) -> Result<(), String> {
        Ok(())
    }
    fn on_terminal_failure(&self, _job: &ClaimedJob, _error: &str, _state: &AppState) {
        self.terminal_calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn a_max_attempt_scaffolding_panic_still_runs_terminal_hooks() {
    // sol diff R2 #2: the OUTER catch_unwind's panic arm must decide retry
    // vs terminal via the ordinary settle path and invoke the SAME terminal
    // hooks (`on_terminal_failure`) the normal error branch would — the
    // prior version relied solely on `ClaimGuard`'s Drop, which never called
    // them, so a last-attempt scaffolding panic terminalized the row
    // silently.
    let handler = Arc::new(PanicsInScaffoldingWithHooksHandler {
        terminal_calls: AtomicUsize::new(0),
    });
    let state = AppState::new(open_in_memory_database().expect("db"));
    let mut worker = JobWorker::new(state);
    worker.register(handler.clone());

    worker
        .state
        .jobs()
        // max_attempts = 1: this claim IS the last attempt, so a panic here
        // must be terminal, not a retry.
        .enqueue("job-1", handler.kind(), "{}", 1)
        .expect("enqueue");

    assert!(
        worker.process_one().expect("process survives the panic"),
        "the panic is contained; the worker keeps going"
    );

    let status = worker
        .state
        .jobs()
        .status("job-1")
        .expect("status")
        .expect("row");
    assert_eq!(
        status.status, "failed",
        "attempts (1) == max_attempts (1): terminal on this attempt"
    );
    assert_eq!(
        handler.terminal_calls.load(Ordering::SeqCst),
        1,
        "the outer scaffolding-panic arm must invoke on_terminal_failure exactly like \
         the ordinary error branch"
    );
}
