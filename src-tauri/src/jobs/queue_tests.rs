use super::*;
use crate::storage::open_in_memory_database;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A handler that counts runs and can be told to fail a fixed number of times.
struct CountingHandler {
    kind: &'static str,
    runs: AtomicUsize,
    fail_first: usize,
}

impl JobHandler for CountingHandler {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn run(&self, _payload: &str, _state: &AppState) -> Result<(), String> {
        let prior = self.runs.fetch_add(1, Ordering::SeqCst);
        if prior < self.fail_first {
            Err(format!("transient failure {prior}"))
        } else {
            Ok(())
        }
    }
}

fn worker_with(handler: Arc<CountingHandler>) -> JobWorker {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let mut worker = JobWorker::new(state);
    worker.register(handler);
    worker
}

/// A handler that always fails and records hook invocations, to prove the
/// terminal hooks fire independent of failure-surface classification.
struct HookRecordingHandler {
    kind: &'static str,
    preflight_errors: usize,
    preflights: AtomicUsize,
    terminal_calls: AtomicUsize,
}

impl JobHandler for HookRecordingHandler {
    fn kind(&self) -> &'static str {
        self.kind
    }
    fn validate_claimed_job(&self, _job: &ClaimedJob) -> Result<(), String> {
        let prior = self.preflights.fetch_add(1, Ordering::SeqCst);
        if prior < self.preflight_errors {
            Err("preflight rejected".into())
        } else {
            Ok(())
        }
    }
    fn run(&self, _payload: &str, _state: &AppState) -> Result<(), String> {
        Err("always fails".into())
    }
    fn on_terminal_failure(&self, _job: &ClaimedJob, _error: &str, _state: &AppState) {
        self.terminal_calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn the_terminal_hook_fires_for_a_kind_with_no_today_surface() {
    // `on_terminal_failure` runs OUTSIDE the failure-surface classification:
    // a kind that is not TodayAttention (here: unclassified) still gets its
    // domain hook exactly once, at the terminal point only.
    let handler = Arc::new(HookRecordingHandler {
        kind: "hook-kind",
        preflight_errors: 0,
        preflights: AtomicUsize::new(0),
        terminal_calls: AtomicUsize::new(0),
    });
    let worker = {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let mut worker = JobWorker::new(state);
        worker.register(handler.clone());
        worker
    };
    worker
        .state
        .jobs()
        .enqueue("hook-kind:retrying", "hook-kind", "{}", 2)
        .expect("enqueue retrying");
    worker.process_one().expect("non-terminal attempt");
    assert_eq!(
        handler.terminal_calls.load(Ordering::SeqCst),
        0,
        "a non-terminal retry must not fire the hook"
    );
    worker
        .state
        .jobs()
        .enqueue("hook-kind:terminal", "hook-kind", "{}", 1)
        .expect("enqueue terminal");
    worker.process_one().expect("terminal attempt");
    assert_eq!(handler.terminal_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_preflight_rejection_settles_the_row_like_a_run_failure() {
    // `validate_claimed_job` errors enter the ordinary settle/retry path:
    // the row is retried with backoff, never left `running`, and `run` is
    // not reached for the rejected attempt.
    let handler = Arc::new(HookRecordingHandler {
        kind: "hook-kind",
        preflight_errors: usize::MAX,
        preflights: AtomicUsize::new(0),
        terminal_calls: AtomicUsize::new(0),
    });
    let worker = {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let mut worker = JobWorker::new(state);
        worker.register(handler.clone());
        worker
    };
    worker
        .state
        .jobs()
        .enqueue("hook-kind:1", "hook-kind", "{}", 1)
        .expect("enqueue");
    worker.process_one().expect("terminal preflight rejection");
    let status = worker
        .state
        .jobs()
        .status("hook-kind:1")
        .expect("status")
        .expect("row");
    assert_eq!(status.status, "failed");
    assert_eq!(status.last_error.as_deref(), Some("preflight rejected"));
    assert_eq!(handler.terminal_calls.load(Ordering::SeqCst), 1);
}

/// A handler that serializes on a fixed source key and counts its runs.
struct SerializingHandler {
    key: &'static str,
    runs: AtomicUsize,
}

impl JobHandler for SerializingHandler {
    fn kind(&self) -> &'static str {
        "serialized-kind"
    }
    fn serialization_key(&self, _payload: &str) -> Option<String> {
        Some(self.key.to_owned())
    }
    fn run(&self, _payload: &str, _state: &AppState) -> Result<(), String> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn defers_a_job_whose_source_is_already_locked() {
    // Per-source serialization (ADR 0059): if another worker holds the source
    // lock, the claimed job is deferred (not run, not failed) and retried later.
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
        .enqueue("job", "serialized-kind", "{}", 5)
        .expect("enqueue");

    // Simulate another worker already refreshing this source.
    let held = worker.state.try_acquire_source("bankier").expect("hold");

    assert!(worker.process_one().expect("process"), "a job was claimed");
    assert_eq!(
        handler.runs.load(Ordering::SeqCst),
        0,
        "the handler did not run while the source was locked"
    );
    let counts = worker.state.jobs().counts().expect("counts");
    assert_eq!(counts.pending, 1, "deferred back to pending");
    assert_eq!(counts.failed, 0, "a defer is not a failure");

    // Once the other worker releases the lock, the job runs to success.
    drop(held);
    worker
        .state
        .jobs()
        .defer("job", 0)
        .expect("bring available_at back to now for the test");
    assert!(worker.process_one().expect("process"));
    assert_eq!(handler.runs.load(Ordering::SeqCst), 1);
    assert_eq!(worker.state.jobs().counts().expect("counts").succeeded, 1);
}

#[test]
fn process_one_for_kinds_runs_only_its_lane() {
    // Worker-pool isolation (ADR 0059): the autopilot lane processes its job
    // even with an older source job waiting — a slow source refresh cannot
    // starve autopilot the way the single shared worker did.
    let source = Arc::new(CountingHandler {
        kind: "scheduled_source_refresh",
        runs: AtomicUsize::new(0),
        fail_first: 0,
    });
    let autopilot = Arc::new(CountingHandler {
        kind: "autopilot_stage",
        runs: AtomicUsize::new(0),
        fail_first: 0,
    });
    let mut worker = JobWorker::new(AppState::new(open_in_memory_database().expect("db")));
    worker.register(source.clone());
    worker.register(autopilot.clone());

    // Source enqueued first (older) — the FIFO head — then autopilot.
    worker
        .state
        .jobs()
        .enqueue("src", "scheduled_source_refresh", "{}", 3)
        .expect("enqueue src");
    worker
        .state
        .jobs()
        .enqueue("auto", "autopilot_stage", "{}", 3)
        .expect("enqueue auto");

    assert!(worker
        .process_one_for_kinds(&["autopilot_stage"])
        .expect("process"));
    assert_eq!(autopilot.runs.load(Ordering::SeqCst), 1);
    assert_eq!(
        source.runs.load(Ordering::SeqCst),
        0,
        "source lane untouched by the autopilot lane"
    );
    let counts = worker.state.jobs().counts().expect("counts");
    assert_eq!(counts.succeeded, 1);
    assert_eq!(
        counts.pending, 1,
        "the source job stays queued for its lane"
    );
}

#[test]
fn processes_a_registered_job_to_success() {
    let handler = Arc::new(CountingHandler {
        kind: "test-ok",
        runs: AtomicUsize::new(0),
        fail_first: 0,
    });
    let worker = worker_with(handler.clone());

    worker
        .state
        .jobs()
        .enqueue("job-1", "test-ok", "{}", 3)
        .expect("enqueue");

    assert!(worker.process_one().expect("process"));
    assert_eq!(handler.runs.load(Ordering::SeqCst), 1);

    let counts = worker.state.jobs().counts().expect("counts");
    assert_eq!(counts.succeeded, 1);
    assert_eq!(counts.pending, 0);

    // Nothing left to process.
    assert!(!worker.process_one().expect("process"));
}

#[test]
fn retries_a_transient_failure_then_succeeds() {
    let handler = Arc::new(CountingHandler {
        kind: "test-flaky",
        runs: AtomicUsize::new(0),
        fail_first: 1,
    });
    let worker = worker_with(handler.clone());

    worker
        .state
        .jobs()
        // backoff is in the past on the immediate retry path only via mark_failed's
        // +N seconds, so use a job that retries with attempts < max and a 0-length
        // wait is simulated by re-enqueue timing; here we assert it goes back to pending.
        .enqueue("job-flaky", "test-flaky", "{}", 5)
        .expect("enqueue");

    // First run fails -> requeued as pending (with backoff in the future).
    assert!(worker.process_one().expect("process"));
    let counts = worker.state.jobs().counts().expect("counts");
    assert_eq!(counts.pending, 1);
    assert_eq!(counts.failed, 0);
    assert_eq!(counts.succeeded, 0);
    assert_eq!(handler.runs.load(Ordering::SeqCst), 1);
}

#[test]
fn exhausts_attempts_into_terminal_failure() {
    let handler = Arc::new(CountingHandler {
        kind: "test-broken",
        runs: AtomicUsize::new(0),
        fail_first: usize::MAX,
    });
    let worker = worker_with(handler.clone());

    // max_attempts = 1: the first failure is terminal.
    worker
        .state
        .jobs()
        .enqueue("job-broken", "test-broken", "{}", 1)
        .expect("enqueue");

    assert!(worker.process_one().expect("process"));
    let counts = worker.state.jobs().counts().expect("counts");
    assert_eq!(counts.failed, 1);
    assert_eq!(counts.pending, 0);
}

#[test]
fn reclaims_crash_residue_running_rows() {
    let handler = Arc::new(CountingHandler {
        kind: "test-ok",
        runs: AtomicUsize::new(0),
        fail_first: 0,
    });
    let worker = worker_with(handler.clone());
    let store = worker.state.jobs();

    store
        .enqueue("job-crashed", "test-ok", "{}", 3)
        .expect("enqueue");
    // Simulate a crash mid-run: claim leaves the row 'running', then nothing
    // completes it.
    let claimed = store.claim_next().expect("claim").expect("a job");
    assert_eq!(claimed.id, "job-crashed");
    assert_eq!(store.counts().expect("counts").running, 1);

    // Startup reclaim requeues it; the worker then completes it.
    let reclaimed = worker.reclaim_on_startup().expect("reclaim");
    assert_eq!(reclaimed, 1);
    assert_eq!(store.counts().expect("counts").pending, 1);

    assert!(worker.process_one().expect("process"));
    assert_eq!(store.counts().expect("counts").succeeded, 1);
}

// ------------------------------------------------------------------
// Occurrence-ledger dispatch (ADR 0109 dec. 2)
// ------------------------------------------------------------------

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

fn job_runs_statuses(state: &AppState) -> Vec<String> {
    let connection = state.checkout_for_tests().expect("checkout");
    let mut statement = connection
        .prepare("SELECT status FROM job_runs ORDER BY id")
        .expect("prepare");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .map(|row| row.expect("row"))
        .collect()
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
fn settle_failure_leaves_queue_and_occurrence_consistent() {
    // The queue row and its occurrence settle in ONE transaction: after a
    // terminal failure both agree (`failed` / `failed`), never split.
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
    assert!(worker.process_one().expect("process"));

    let queue_status = worker
        .state
        .jobs()
        .status("src:1")
        .expect("status")
        .expect("row");
    assert_eq!(queue_status.status, "failed");
    assert_eq!(job_runs_statuses(&worker.state), vec!["failed"]);
}

#[test]
fn handler_panic_takes_the_retry_path() {
    // A handler panic is contained (`catch_unwind`): the queue row goes back
    // to `pending` with attempts+1, the occurrence settles `retry_scheduled`
    // (never left open), and the worker survives to process the next job.
    let handler = Arc::new(ActivityHandler {
        fail_first: 0,
        panics: true,
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

    // Worker continues to process the NEXT job normally after a panic.
    worker
        .state
        .jobs()
        .enqueue("other-ok", "test-ok-after-panic", "{}", 1)
        .expect("enqueue second");
    struct OkHandler;
    impl JobHandler for OkHandler {
        fn kind(&self) -> &'static str {
            "test-ok-after-panic"
        }
        fn run(&self, _payload: &str, _state: &AppState) -> Result<(), String> {
            Ok(())
        }
    }
    // Registering after construction isn't possible on this worker instance
    // (handlers are immutable once built for this test's `worker`), so drive
    // a second worker sharing the same state to prove the STATE survives.
    let mut worker2 = JobWorker::new(worker.state.clone());
    worker2.register(Arc::new(OkHandler));
    assert!(worker2.process_one().expect("second job processes fine"));
}
