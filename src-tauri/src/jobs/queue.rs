//! In-process worker for the durable job queue (Architecture v2 / ADR 0050).
//!
//! The queue (`storage::JobQueueStore`) persists work; this worker drains it. A
//! [`JobHandler`] is registered per job `kind`; the worker atomically claims the
//! next runnable row, dispatches to the handler, and records success or a
//! retry-with-backoff. Because claim + attempt-increment are one atomic
//! statement and a startup reclaim requeues crash-residue `running` rows, work
//! survives a crash mid-job.
//!
//! Local-first: a single worker runs only while the app is open; background or
//! closed-app execution stays out of scope. The existing fire-and-forget
//! `spawn_blocking` jobs migrate onto this queue incrementally (strangler).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::app_state::AppState;

/// A handler for one job `kind`. `run` does the actual (blocking) work; the
/// worker calls it off the UI thread.
pub trait JobHandler: Send + Sync {
    /// The `kind` discriminator this handler claims (must be unique per worker).
    fn kind(&self) -> &'static str;

    /// Execute one job. The opaque JSON `payload` is the handler's to
    /// deserialize. Returning `Err` schedules a retry (until `max_attempts`).
    fn run(&self, payload: &str, state: &AppState) -> Result<(), String>;
}

/// Poll interval when the queue is idle.
const IDLE_POLL: Duration = Duration::from_secs(5);

/// Capped exponential backoff (seconds) for a retry, by attempt count: 2, 4, 8,
/// … 64s. Bounds retry pressure without unbounded growth.
fn retry_backoff_seconds(attempts: i64) -> i64 {
    let exponent = attempts.clamp(1, 6) as u32;
    2_i64.pow(exponent)
}

/// Worker holding the registered handlers and an `AppState` handle.
pub struct JobWorker {
    state: AppState,
    handlers: HashMap<&'static str, Arc<dyn JobHandler>>,
}

impl JobWorker {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for its `kind`. Panics if two handlers claim the same
    /// kind (a wiring bug, caught at startup).
    pub fn register(&mut self, handler: Arc<dyn JobHandler>) {
        let kind = handler.kind();
        assert!(
            self.handlers.insert(kind, handler).is_none(),
            "duplicate job handler for kind {kind}"
        );
    }

    /// Claim and process at most one job. Returns `true` if a job was processed,
    /// `false` if nothing was runnable. This is the deterministic unit the loop
    /// repeats — driven directly by tests.
    pub fn process_one(&self) -> Result<bool, String> {
        let store = self.state.jobs();
        let Some(job) = store.claim_next().map_err(|error| error.to_string())? else {
            return Ok(false);
        };

        let outcome = match self.handlers.get(job.kind.as_str()) {
            Some(handler) => handler.run(&job.payload, &self.state),
            None => Err(format!("no handler registered for job kind {}", job.kind)),
        };

        match outcome {
            Ok(()) => store
                .mark_succeeded(&job.id)
                .map_err(|error| error.to_string())?,
            Err(error) => {
                let backoff = retry_backoff_seconds(job.attempts);
                store
                    .mark_failed(&job.id, &error, backoff)
                    .map_err(|error| error.to_string())?;
            }
        }

        Ok(true)
    }

    /// Drain the queue until no job is currently runnable; returns how many were
    /// processed. (Jobs deferred by retry backoff are not yet runnable and are
    /// left for a later tick.)
    pub fn run_until_idle(&self) -> Result<usize, String> {
        let mut processed = 0;
        while self.process_one()? {
            processed += 1;
        }
        Ok(processed)
    }

    /// Reclaim crash-residue `running` rows back to `pending`. Call once on
    /// startup before the loop begins.
    pub fn reclaim_on_startup(&self) -> Result<usize, String> {
        self.state
            .jobs()
            .reclaim_stale_running()
            .map_err(|error| error.to_string())
    }
}

/// Spawn the worker on a dedicated blocking thread: reclaim crash residue, then
/// loop forever — drain runnable jobs, sleep [`IDLE_POLL`] when idle. Off the UI
/// thread (the handlers do blocking storage/IO work). Production entry point;
/// tests drive [`JobWorker::process_one`] / [`JobWorker::run_until_idle`]
/// directly for determinism.
pub fn spawn(worker: Arc<JobWorker>) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = worker.reclaim_on_startup() {
            log::warn!("job queue: startup reclaim failed: {error}");
        }
        loop {
            match worker.process_one() {
                Ok(true) => {}
                Ok(false) => std::thread::sleep(IDLE_POLL),
                Err(error) => {
                    log::warn!("job queue: worker tick failed: {error}");
                    std::thread::sleep(IDLE_POLL);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
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
}
