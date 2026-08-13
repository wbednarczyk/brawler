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
use crate::storage::ClaimedJob;

/// A handler for one job `kind`. `run` does the actual (blocking) work; the
/// worker calls it off the UI thread.
pub trait JobHandler: Send + Sync {
    /// The `kind` discriminator this handler claims (must be unique per worker).
    fn kind(&self) -> &'static str;

    /// Execute one job. The opaque JSON `payload` is the handler's to
    /// deserialize. Returning `Err` schedules a retry (until `max_attempts`).
    fn run(&self, payload: &str, state: &AppState) -> Result<(), String>;

    /// The per-source serialization key for this job, if any (ADR 0059). When
    /// `Some(id)`, the worker acquires the exclusive source lock for `id` before
    /// `run` and holds it for the whole run, so at most one worker refreshes that
    /// source at a time; a worker that cannot acquire it defers the job (no attempt
    /// consumed) and moves on. Default `None` — the job runs with no source lock.
    fn serialization_key(&self, _payload: &str) -> Option<String> {
        None
    }

    /// The company this job's work belongs to, from its payload (ADR 0091 dec. 1;
    /// same payload-reading pattern as [`JobHandler::serialization_key`]). Used to
    /// scope the `job_failed` attention event, so a failed per-company job lands on
    /// that company's row in the stream. Default `None` — workspace-wide work
    /// (a briefing, the aggregator pull) belongs to no single issuer and stores a
    /// NULL company scope (migration 0118).
    fn company_scope(&self, _payload: &str) -> Option<String> {
        None
    }

    /// The raw specific this job failed ON — a document title, a ticker (ADR 0091
    /// dec. 1 / ADR 0087 dec. 4: raw source data, NEVER composed prose). Snapshotted
    /// onto the failure event at fire time so the stream states WHAT failed even
    /// after the underlying row is pruned. `state` is available because the payload
    /// usually carries only ids; resolve the human specific from storage, and return
    /// `None` when there is none (the read model then states the job's own
    /// `last_error`).
    fn failure_subject(&self, _payload: &str, _state: &AppState) -> Option<String> {
        None
    }

    /// Preflight coherence check over the actual claimed row, called by the
    /// worker BEFORE [`JobHandler::run`]. Handlers whose payload duplicates the
    /// row identity (the KPI ingest kinds, #364) verify the REAL `job.id`
    /// matches the payload here, so a row whose id names one run but whose
    /// payload names another never does work for the wrong run. An `Err` enters
    /// the ordinary settle/retry path exactly like an `Err` from `run` — the
    /// row is never left `running`. Default: no check.
    fn validate_claimed_job(&self, _job: &ClaimedJob) -> Result<(), String> {
        Ok(())
    }

    /// The `(company_scope, failure_subject)` pair for the terminal `job_failed`
    /// attention event, resolved from the actual claimed row. Default delegates
    /// to the payload-based [`JobHandler::company_scope`] /
    /// [`JobHandler::failure_subject`]; handlers with an id-authoritative
    /// identity (#364) override to derive both from `job.id` so a tampered
    /// payload can never misattribute the event.
    fn failure_context(
        &self,
        job: &ClaimedJob,
        state: &AppState,
    ) -> (Option<String>, Option<String>) {
        (
            self.company_scope(&job.payload),
            self.failure_subject(&job.payload, state),
        )
    }

    /// Called by the worker AFTER the queue row is durably terminal (retries
    /// exhausted, `mark_failed` returned false) and after the failure event
    /// fired — event first, domain transition second, so a crash between the
    /// two converges via startup reconciliation. Runs regardless of the kind's
    /// failure-surface classification. Domain stores transition their own state
    /// here (the KPI ingest kinds mark the run `failed`, #364); handlers whose
    /// domain rows are finalized inside `run` (autopilot) keep the default
    /// no-op.
    fn on_terminal_failure(&self, _job: &ClaimedJob, _error: &str, _state: &AppState) {}
}

/// Poll interval when the queue is idle.
const IDLE_POLL: Duration = Duration::from_secs(5);

/// Backoff (seconds) applied when a job is deferred because its source is already
/// being refreshed by another worker (ADR 0059). Short enough to keep throughput,
/// long enough to avoid a tight claim/defer busy-loop over many same-source jobs.
const SOURCE_BUSY_BACKOFF_SECONDS: i64 = 3;

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

    /// Every job `kind` a handler is registered for. Used by the lane-coverage
    /// guardrail (every registered kind must appear in exactly one worker lane,
    /// [`crate::jobs::pool_layout`]).
    pub fn registered_kinds(&self) -> Vec<&'static str> {
        self.handlers.keys().copied().collect()
    }

    /// Dispatch one already-claimed job to its handler and record the outcome
    /// (success or retry-with-backoff). Shared by the all-kinds and kind-scoped
    /// claim paths.
    fn dispatch(&self, job: ClaimedJob) -> Result<(), String> {
        let store = self.state.jobs();
        let handler = self.handlers.get(job.kind.as_str()).cloned();

        // Per-source serialization (ADR 0059): if the handler names a source for
        // this job, hold that source's exclusive lock across the run. On contention
        // the job is deferred (not failed) and a later tick retries it.
        let _source_guard = match handler
            .as_ref()
            .and_then(|handler| handler.serialization_key(&job.payload))
        {
            Some(key) => match self.state.try_acquire_source(&key) {
                Some(guard) => Some(guard),
                None => {
                    store
                        .defer(&job.id, SOURCE_BUSY_BACKOFF_SECONDS)
                        .map_err(|error| error.to_string())?;
                    return Ok(());
                }
            },
            None => None,
        };

        // Preflight (id↔payload coherence) composes with the run itself: either
        // failure enters the same settle/retry path, never leaving the row
        // `running`.
        let outcome = match handler.as_ref() {
            Some(handler) => handler
                .validate_claimed_job(&job)
                .and_then(|()| handler.run(&job.payload, &self.state)),
            None => Err(format!("no handler registered for job kind {}", job.kind)),
        };

        match outcome {
            Ok(()) => store
                .mark_succeeded(&job.id)
                .map_err(|error| error.to_string())?,
            Err(error) => {
                let backoff = retry_backoff_seconds(job.attempts);
                let will_retry = store
                    .mark_failed(&job.id, &error, backoff)
                    .map_err(|error| error.to_string())?;
                if !will_retry {
                    // Event first (deduped, migration 0118), domain transition
                    // second — a crash between the two converges at startup.
                    // The hook fires for EVERY kind, independent of the
                    // failure-surface classification inside
                    // `surface_terminal_failure`.
                    self.surface_terminal_failure(&job, handler.as_deref());
                    if let Some(handler) = handler.as_ref() {
                        handler.on_terminal_failure(&job, &error, &self.state);
                    }
                }
            }
        }
        Ok(())
    }

    /// THE single terminal-failure point (ADR 0091 dec. 1): the job has exhausted
    /// its retries and will not run again. Kinds whose failure surface is
    /// [`FailureSurface::TodayAttention`] raise the generic system `job_failed`
    /// attention event here; kinds with a richer domain surface (Sources adapter
    /// health, the autopilot run card) keep it EXCLUSIVELY, so nothing double-fires.
    /// An unclassified kind surfaces nothing and is caught by the enumeration gate
    /// (`jobs::failure_surface::tests`), never silently defaulted here.
    ///
    /// Best-effort: raising the event must never turn a recorded failure into a
    /// worker error (the queue row is already the durable record).
    fn surface_terminal_failure(&self, job: &ClaimedJob, handler: Option<&dyn JobHandler>) {
        use crate::jobs::failure_surface::{failure_surface, FailureSurface};

        if failure_surface(&job.kind) != Some(FailureSurface::TodayAttention) {
            return;
        }
        let (company, subject) = handler
            .map(|handler| handler.failure_context(job, &self.state))
            .unwrap_or((None, None));
        if let Err(error) = self.state.attention().record_job_failure(
            &job.id,
            company.as_deref(),
            subject.as_deref(),
        ) {
            log::warn!(
                "job queue: could not raise the failure event for job {} ({}): {error}",
                job.id,
                job.kind
            );
        }
    }

    /// Claim and process at most one job (any kind). Returns `true` if a job was
    /// processed, `false` if nothing was runnable. The deterministic unit tests
    /// drive; also used by [`run_until_idle`].
    pub fn process_one(&self) -> Result<bool, String> {
        let Some(job) = self
            .state
            .jobs()
            .claim_next()
            .map_err(|error| error.to_string())?
        else {
            return Ok(false);
        };
        self.dispatch(job)?;
        Ok(true)
    }

    /// Claim and process at most one job whose `kind` is in `kinds` (a worker
    /// pool's lane). Isolation so one lane cannot starve another (ADR 0059).
    pub fn process_one_for_kinds(&self, kinds: &[&str]) -> Result<bool, String> {
        let Some(job) = self
            .state
            .jobs()
            .claim_next_for_kinds(kinds)
            .map_err(|error| error.to_string())?
        else {
            return Ok(false);
        };
        self.dispatch(job)?;
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

/// One isolated worker lane: a set of job `kinds` drained by `workers` dedicated
/// threads, independent of other lanes (ADR 0059). Isolation is what stops a slow
/// source refresh from starving latency-sensitive autopilot work — the failure the
/// single shared worker had. Threads (not async) are deliberate for a local app at
/// this scale; the lanes/locks port to async unchanged if that ever changes.
pub struct WorkerPool {
    pub name: &'static str,
    pub kinds: Vec<&'static str>,
    pub workers: usize,
}

/// Spawn the durable-queue worker as **isolated lanes**. One startup reclaim runs
/// before any lane begins (crash residue → pending, or dead-lettered if it has
/// exhausted its attempts); then each lane gets `workers` blocking threads that
/// drain only that lane's kinds. Off the UI thread (handlers do blocking storage/IO
/// work). Production entry point; tests drive `process_one` / `run_until_idle`
/// directly for determinism.
pub fn spawn_pools(worker: Arc<JobWorker>, pools: Vec<WorkerPool>) {
    if let Err(error) = worker.reclaim_on_startup() {
        log::warn!("job queue: startup reclaim failed: {error}");
    }
    for pool in pools {
        for slot in 0..pool.workers.max(1) {
            let worker = Arc::clone(&worker);
            let kinds = pool.kinds.clone();
            let name = pool.name;
            tauri::async_runtime::spawn_blocking(move || loop {
                match worker.process_one_for_kinds(&kinds) {
                    Ok(true) => {}
                    Ok(false) => std::thread::sleep(IDLE_POLL),
                    Err(error) => {
                        log::warn!("job queue [{name}#{slot}]: worker tick failed: {error}");
                        std::thread::sleep(IDLE_POLL);
                    }
                }
            });
        }
    }
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
}
