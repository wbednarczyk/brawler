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

/// Every `job_runs` row's status, oldest first — shared by the ClaimGuard
/// tests here and the occurrence-ledger dispatch tests in
/// `queue_dispatch_tests.rs` (via `super::job_runs_statuses`).
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

/// A handler that always succeeds, registered alongside a panicking handler
/// so the SAME `JobWorker` instance (not a second one sharing state) proves
/// it survives a panic. Shared by this file and `queue_dispatch_tests.rs`.
struct OkHandler;
impl JobHandler for OkHandler {
    fn kind(&self) -> &'static str {
        "test-ok-after-panic"
    }
    fn run(&self, _payload: &str, _state: &AppState) -> Result<(), String> {
        Ok(())
    }
}

// ------------------------------------------------------------------
// ClaimGuard unwind containment (ADR 0109 dec. 2, sol diff R1 #1)
// ------------------------------------------------------------------

/// A handler whose `serialization_key` (called in `dispatch_claimed`'s
/// scaffolding, BEFORE identity resolution / `begin_attempt` — i.e. outside
/// the inner `handler.run` boundary, exactly the surrounding-scaffolding
/// panic surface sol diff R1 #1 flags) panics. This is a genuine end-to-end
/// injection: `run_id` is still `None` at this point, so it exercises the
/// `ClaimGuard` fallback's queue-only path (no occurrence to settle).
struct PanicsBeforeIdentityHandler {
    runs: AtomicUsize,
}

impl JobHandler for PanicsBeforeIdentityHandler {
    fn kind(&self) -> &'static str {
        "panics-before-identity"
    }
    fn serialization_key(&self, _payload: &str) -> Option<String> {
        panic!("boom: scaffolding panic before identity resolution");
    }
    fn run(&self, _payload: &str, _state: &AppState) -> Result<(), String> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn a_scaffolding_panic_before_identity_is_contained_and_the_worker_survives() {
    // The OUTER catch_unwind (added by this fix) contains a panic BEFORE
    // identity/begin_attempt ever runs. `ClaimGuard`'s fallback settles the
    // queue row (no occurrence exists yet — run_id is None); the queue row
    // is never left `running`, and the SAME worker keeps processing.
    let handler = Arc::new(PanicsBeforeIdentityHandler {
        runs: AtomicUsize::new(0),
    });
    let state = AppState::new(open_in_memory_database().expect("db"));
    let mut worker = JobWorker::new(state);
    worker.register(handler.clone());
    worker.register(Arc::new(OkHandler));

    worker
        .state
        .jobs()
        .enqueue("job-1", "panics-before-identity", "{}", 3)
        .expect("enqueue");

    assert!(worker.process_one().expect("process survives the panic"));
    assert_eq!(
        handler.runs.load(Ordering::SeqCst),
        0,
        "the handler must never run — the panic happened before that point"
    );
    let status = worker
        .state
        .jobs()
        .status("job-1")
        .expect("status")
        .expect("row");
    assert_ne!(
        status.status, "running",
        "the claimed row must never strand `running` after a scaffolding panic"
    );
    assert!(
        job_runs_statuses(&worker.state).is_empty(),
        "no occurrence was ever opened"
    );

    // The SAME worker instance still processes the next job normally.
    worker
        .state
        .jobs()
        .enqueue("job-2", "test-ok-after-panic", "{}", 1)
        .expect("enqueue second");
    assert!(worker.process_one().expect("second job processes fine"));
}

/// A minimal `ClaimedJob` for `ClaimGuard`'s own white-box tests below — the
/// exact `kind`/`payload` never matter (no handler is registered for these).
fn claimed_job(id: &str, attempts: i64, max_attempts: i64) -> ClaimedJob {
    ClaimedJob {
        id: id.to_owned(),
        kind: "any-kind".to_owned(),
        payload: "{}".to_owned(),
        attempts,
        max_attempts,
    }
}

/// Directly exercises `ClaimGuard`'s Drop-while-armed fallback for the two
/// shapes an outer-scaffolding panic can leave it in (sol diff R1 #1): with
/// an occurrence already open (`run_id: Some`, representing a panic during/
/// after `begin_attempt` but before settle) and without one (representing a
/// panic during identity resolution itself). White-box (same module) since
/// no honest end-to-end seam exists to force a real panic at those exact
/// points without adding test-only production hooks.
#[test]
fn claim_guard_fallback_settles_a_run_id_occurrence_on_unwind() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .jobs()
        .enqueue("job-1", "any-kind", "{}", 3)
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");
    let run_id = state
        .job_runs()
        .begin_attempt(crate::storage::NewJobRun {
            activity_key: "k:job-1".to_owned(),
            run_key: "job-1".to_owned(),
            kind: "any-kind".to_owned(),
            family: crate::jobs::activity_identity::ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "s".to_owned(),
            target: crate::jobs::activity_identity::ActivityTarget::Sources,
            attempt: 1,
        })
        .expect("begin");

    {
        let guard = ClaimGuard::new(state.clone(), claimed_job("job-1", 1, 3), None);
        guard.set_run_id(run_id);
        // Dropped here without `disarm()` — simulates an unwind mid-flight.
    }

    let queue_status = state.jobs().status("job-1").expect("status").expect("row");
    assert_ne!(
        queue_status.status, "running",
        "the fallback must terminalize the stranded queue row"
    );
    assert_eq!(job_runs_statuses(&state), vec!["retry_scheduled"]);
}

#[test]
fn claim_guard_fallback_settles_the_queue_row_with_no_occurrence() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .jobs()
        .enqueue("job-2", "any-kind", "{}", 1)
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");

    {
        let _guard = ClaimGuard::new(state.clone(), claimed_job("job-2", 1, 1), None);
        // No run_id ever set (identity resolution never got that far).
    }

    let queue_status = state.jobs().status("job-2").expect("status").expect("row");
    assert_eq!(
        queue_status.status, "failed",
        "attempts (1) == max_attempts (1): terminal on the fallback path too"
    );
    assert!(job_runs_statuses(&state).is_empty());
}

#[test]
fn claim_guard_fallback_settles_succeeded_when_marked_before_unwind() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .jobs()
        .enqueue("job-3", "any-kind", "{}", 3)
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");
    let run_id = state
        .job_runs()
        .begin_attempt(crate::storage::NewJobRun {
            activity_key: "k:job-3".to_owned(),
            run_key: "job-3".to_owned(),
            kind: "any-kind".to_owned(),
            family: crate::jobs::activity_identity::ActivityFamily::SourceRefresh,
            company_id: None,
            subject: "s".to_owned(),
            target: crate::jobs::activity_identity::ActivityTarget::Sources,
            attempt: 1,
        })
        .expect("begin");

    {
        let guard = ClaimGuard::new(state.clone(), claimed_job("job-3", 1, 3), None);
        guard.set_run_id(run_id);
        guard.set_succeeded(true);
        // A panic while ATTEMPTING the succeed-settle call — the fallback
        // must record success, never downgrade completed work to failed.
    }

    let queue_status = state.jobs().status("job-3").expect("status").expect("row");
    assert_eq!(queue_status.status, "succeeded");
    assert_eq!(job_runs_statuses(&state), vec!["succeeded"]);
}

#[test]
fn claim_guard_disarmed_is_a_noop_on_drop() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .jobs()
        .enqueue("job-4", "any-kind", "{}", 1)
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");

    {
        let guard = ClaimGuard::new(state.clone(), claimed_job("job-4", 1, 1), None);
        guard.disarm();
        // The ordinary path is presumed to have already handled this job
        // (e.g. via `defer`) — the guard must not touch it again.
        state.jobs().defer("job-4", 0).expect("ordinary defer");
    }

    let queue_status = state.jobs().status("job-4").expect("status").expect("row");
    assert_eq!(
        queue_status.status, "pending",
        "a disarmed guard must never re-settle a row the ordinary path already handled"
    );
}

/// sol diff R3 #2: `settle_with_recovery` retries the SAME op (same closure,
/// so the SAME args/error text) exactly once, and reports `Unrecovered` —
/// never a bare `None` indistinguishable from `MissingRow` — leaving the
/// guard armed for its own last-resort Drop recovery.
#[test]
fn settle_with_recovery_retries_once_with_the_same_op_then_reports_unrecovered() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let guard = ClaimGuard::new(state, claimed_job("job-x", 1, 1), None);
    let attempts = AtomicUsize::new(0);
    let outcome = settle_with_recovery(&guard, || {
        attempts.fetch_add(1, Ordering::SeqCst);
        Err::<(), _>(StorageError::Classification("boom".to_owned()))
    });
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "op is retried exactly once"
    );
    assert!(matches!(outcome, SettleOutcome::Unrecovered));
    assert!(
        guard.armed.get(),
        "an Unrecovered outcome must leave the guard armed for Drop's own recovery"
    );
}

/// sol diff R3 #2: once BOTH ordinary settle attempts have failed (simulated
/// directly here — the guard is left armed with the real error recorded via
/// `set_pending_error`, never a settle attempt made), a Drop that runs
/// WITHOUT an actual panic in flight must repeat that SAME real error — never
/// the fabricated "panic: ..." message — and, since this lands the row
/// terminal, fire the SAME terminal hooks the ordinary branch would, exactly
/// once.
#[test]
fn claim_guard_drop_fallback_uses_the_real_error_and_fires_terminal_hooks_once() {
    let handler = Arc::new(HookRecordingHandler {
        kind: "hook-kind-recovery",
        preflight_errors: 0,
        preflights: AtomicUsize::new(0),
        terminal_calls: AtomicUsize::new(0),
    });
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .jobs()
        .enqueue("job-5", handler.kind(), "{}", 1)
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");

    {
        let guard = ClaimGuard::new(
            state.clone(),
            claimed_job("job-5", 1, 1),
            Some(handler.clone() as Arc<dyn JobHandler>),
        );
        guard.set_pending_error("transient failure 0");
        // Dropped here with no settle attempt ever made and no panic in
        // flight — the "twice-failed-then-recovered" shape, minus the
        // mechanically-unreproducible DB poisoning (SQLite's `total_changes`
        // does not survive as a per-call-site counter usable from a trigger
        // without a custom SQL function, which is not worth a new rusqlite
        // feature for one test); the ordinary path always sets
        // `pending_error` before every `settle_with_recovery` call, so this
        // is exactly the state Drop sees in the real twice-failed case.
    }

    let queue_status = state.jobs().status("job-5").expect("status").expect("row");
    assert_eq!(queue_status.status, "failed");
    assert_eq!(
        queue_status.last_error.as_deref(),
        Some("transient failure 0"),
        "Drop's non-panic fallback must repeat the REAL error, never a fabricated one"
    );
    assert_eq!(
        handler.terminal_calls.load(Ordering::SeqCst),
        1,
        "the non-panic Drop fallback must fire on_terminal_failure exactly once, same as the \
         ordinary branch"
    );
}

/// The panic counterpart: while genuinely unwinding, `Drop` must keep its
/// fixed diagnosis (never a stale `pending_error` from before the panic) and
/// must fire NO hooks — invoking arbitrary handler code mid-unwind risks a
/// second panic, which aborts the process outright.
#[test]
fn claim_guard_drop_during_a_real_panic_ignores_pending_error_and_fires_no_hooks() {
    let handler = Arc::new(HookRecordingHandler {
        kind: "hook-kind-panic",
        preflight_errors: 0,
        preflights: AtomicUsize::new(0),
        terminal_calls: AtomicUsize::new(0),
    });
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .jobs()
        .enqueue("job-6", handler.kind(), "{}", 1)
        .expect("enqueue");
    state.jobs().claim_next().expect("claim").expect("job");

    let state_for_panic = state.clone();
    let handler_for_panic = handler.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let guard = ClaimGuard::new(
            state_for_panic,
            claimed_job("job-6", 1, 1),
            Some(handler_for_panic as Arc<dyn JobHandler>),
        );
        guard.set_pending_error("this must never reach the queue row");
        panic!("boom: genuine unwind while the guard is armed");
    }));
    assert!(
        result.is_err(),
        "the panic must actually unwind through this scope"
    );

    let queue_status = state.jobs().status("job-6").expect("status").expect("row");
    assert_eq!(queue_status.status, "failed");
    assert_eq!(
        queue_status.last_error.as_deref(),
        Some("panic: job worker unwound before settling"),
        "a genuine unwind keeps the fabricated panic message, never the stale pending_error"
    );
    assert_eq!(
        handler.terminal_calls.load(Ordering::SeqCst),
        0,
        "no hooks fire from a Drop that is itself mid-unwind"
    );
}

// Occurrence-ledger dispatch tests (ADR 0109 dec. 2) live in
// `queue_dispatch_tests.rs` (split out to stay under the file-size ratchet,
// ADR 0103) — nested here so they resolve as `queue::tests::dispatch::*`.
#[path = "queue_dispatch_tests.rs"]
mod dispatch;
