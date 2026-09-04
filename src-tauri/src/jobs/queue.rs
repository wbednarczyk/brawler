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

use std::cell::Cell;
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

/// Extract a human message from a `catch_unwind` payload — shared by the
/// dispatch-level unwind boundary and the handler-run boundary.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "job worker panicked".to_owned())
}

/// RAII unwind guard for one claimed job's post-claim lifecycle (ADR 0109
/// dec. 2, sol diff R1 #1). `dispatch_claimed` disarms it right after each
/// ordinary settle call returns (Ok OR Err — a normal error already told the
/// caller what happened, nothing left for the fallback to do). If the stack
/// unwinds through a panic BEFORE that — identity checkout, `begin_attempt`,
/// or the settle call itself panicking mid-flight — `Drop` fires while still
/// armed and best-effort re-settles the claim so it never strands `running`
/// and the worker thread keeps running the next job.
struct ClaimGuard {
    state: AppState,
    job_id: String,
    attempts: i64,
    run_id: Cell<Option<i64>>,
    /// `Some(true)` once the handler is known to have succeeded (set right
    /// before the ordinary succeed-settle call) — the fallback then tries
    /// `mark_succeeded_with_run` instead of failing a job that actually
    /// finished its work. `None`/`Some(false)` fall back to failed/retried.
    succeeded: Cell<Option<bool>>,
    armed: Cell<bool>,
}

impl ClaimGuard {
    fn new(state: AppState, job_id: String, attempts: i64) -> Self {
        Self {
            state,
            job_id,
            attempts,
            run_id: Cell::new(None),
            succeeded: Cell::new(None),
            armed: Cell::new(true),
        }
    }

    fn set_run_id(&self, run_id: i64) {
        self.run_id.set(Some(run_id));
    }

    fn set_succeeded(&self, succeeded: bool) {
        self.succeeded.set(Some(succeeded));
    }

    /// The ordinary settle path has taken over (Ok or Err — either way the
    /// situation is already handled) — the fallback must not also fire.
    fn disarm(&self) {
        self.armed.set(false);
    }
}

impl Drop for ClaimGuard {
    fn drop(&mut self) {
        if !self.armed.get() {
            return;
        }
        let run_id = self.run_id.get();
        let result = if self.succeeded.get() == Some(true) {
            self.state
                .jobs()
                .mark_succeeded_with_run(&self.job_id, run_id)
        } else {
            let backoff = retry_backoff_seconds(self.attempts);
            self.state
                .jobs()
                .mark_failed_with_run(
                    &self.job_id,
                    "panic: job worker unwound before settling",
                    backoff,
                    run_id,
                )
                .map(|_| ())
        };
        if let Err(error) = result {
            log::error!(
                "job queue: claim guard could not settle job {} after a panic: {error}",
                self.job_id
            );
        }
    }
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
    ///
    /// The whole post-claim lifecycle (identity resolution, `begin_attempt`,
    /// the handler run, and settle) runs under [`ClaimGuard`]'s unwind
    /// containment (ADR 0109 dec. 2, sol diff R1 #1): a panic ANYWHERE in that
    /// path — not just inside `handler.run` — is caught here so the worker
    /// thread survives, and [`ClaimGuard::drop`] best-effort terminalizes the
    /// claim if it unwound before the ordinary settle path disarmed it. A
    /// handler panic specifically is caught one layer INSIDE this (see
    /// `run_validated`) so it still enters the ordinary retry/terminal-hook
    /// path with its real message, exactly as before this fix — the outer
    /// guard exists for panics in the surrounding scaffolding (identity
    /// checkout, `begin_attempt`, the settle calls themselves), which used to
    /// escape uncontained and could strand a claimed row `running` forever
    /// while killing the worker thread.
    fn dispatch(&self, job: ClaimedJob) -> Result<(), String> {
        let handler = self.handlers.get(job.kind.as_str()).cloned();
        let claim_guard = ClaimGuard::new(self.state.clone(), job.id.clone(), job.attempts);

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.dispatch_claimed(&job, handler.as_deref(), &claim_guard)
        }));

        match outcome {
            Ok(result) => result,
            Err(panic_payload) => {
                // `claim_guard` already ran its Drop-during-unwind fallback
                // (best-effort settle) by the time we get here. The worker
                // thread survives: log and move on to the next tick.
                let message = panic_message(panic_payload);
                log::error!(
                    "job queue: job {} ({}) unwound past its handler while settling: {message}",
                    job.id,
                    job.kind
                );
                Ok(())
            }
        }
    }

    /// The identity/begin_attempt/run/settle body `dispatch` wraps in unwind
    /// containment. Every early return disarms `claim_guard` first — the
    /// guard's fallback exists only to protect a panic mid-flight, never to
    /// double-handle a normal exit this function already handled itself.
    fn dispatch_claimed(
        &self,
        job: &ClaimedJob,
        handler: Option<&dyn JobHandler>,
        claim_guard: &ClaimGuard,
    ) -> Result<(), String> {
        let store = self.state.jobs();

        // Per-source serialization (ADR 0059): if the handler names a source for
        // this job, hold that source's exclusive lock across the run. On contention
        // the job is deferred (not failed) and a later tick retries it.
        let _source_guard =
            match handler.and_then(|handler| handler.serialization_key(&job.payload)) {
                Some(key) => match self.state.try_acquire_source(&key) {
                    Some(guard) => Some(guard),
                    None => {
                        claim_guard.disarm();
                        store
                            .defer(&job.id, SOURCE_BUSY_BACKOFF_SECONDS)
                            .map_err(|error| error.to_string())?;
                        return Ok(());
                    }
                },
                None => None,
            };

        // Occurrence ledger (ADR 0109 dec. 2): begin the attempt AFTER the source
        // lock, before the handler runs. An insert failure (or a kind with no
        // resolved identity — a wiring gap, never blocking the job itself) skips
        // to `run_id: None`: the queue row still settles normally, just with no
        // occurrence to close.
        // Resolve identity on its own checkout, released before `begin_attempt`
        // takes its own below — never nested (a nested checkout on the
        // single-connection pool would deadlock).
        // A checkout failure defers exactly like a `begin_attempt` failure:
        // a registered kind must never run unrecorded (the read model would
        // call its running row `stalled`).
        let identity = match self.state.checkout() {
            Ok(connection) => crate::jobs::activity_identity::identity_for_job(
                &job.kind,
                &job.id,
                &job.payload,
                &connection,
            ),
            Err(error) => {
                log::warn!(
                    "job queue: identity checkout failed for job {} ({}): {error}; deferring",
                    job.id,
                    job.kind
                );
                claim_guard.disarm();
                store
                    .defer(&job.id, SOURCE_BUSY_BACKOFF_SECONDS)
                    .map_err(|error| error.to_string())?;
                return Ok(());
            }
        };
        let run_id = match identity {
            Some(identity) => {
                let new_run = crate::storage::NewJobRun {
                    activity_key: identity.activity_key,
                    run_key: job.id.clone(),
                    kind: job.kind.clone(),
                    family: identity.family,
                    company_id: identity.company_id,
                    subject: identity.subject,
                    target: identity.target,
                    attempt: job.attempts,
                };
                match self.state.job_runs().begin_attempt(new_run) {
                    Ok(run_id) => {
                        claim_guard.set_run_id(run_id);
                        Some(run_id)
                    }
                    Err(error) => {
                        log::warn!(
                            "job queue: begin_attempt failed for job {} ({}): {error}; deferring",
                            job.id,
                            job.kind
                        );
                        claim_guard.disarm();
                        store
                            .defer(&job.id, SOURCE_BUSY_BACKOFF_SECONDS)
                            .map_err(|error| error.to_string())?;
                        return Ok(());
                    }
                }
            }
            None => None,
        };

        // Preflight (id↔payload coherence) composes with the run itself: either
        // failure enters the same settle/retry path, never leaving the row
        // `running`. The handler run itself is contained by its OWN
        // `catch_unwind` (below) so a handler panic becomes an ordinary
        // retry/terminal failure with its real message — never the outer
        // guard's generic fallback.
        let run_outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match handler {
                Some(handler) => handler
                    .validate_claimed_job(job)
                    .and_then(|()| handler.run(&job.payload, &self.state)),
                None => Err(format!("no handler registered for job kind {}", job.kind)),
            }))
            .unwrap_or_else(|panic_payload| {
                Err(format!("panic: {}", panic_message(panic_payload)))
            });

        match run_outcome {
            Ok(()) => {
                claim_guard.set_succeeded(true);
                let result = store
                    .mark_succeeded_with_run(&job.id, run_id)
                    .map_err(|error| error.to_string());
                claim_guard.disarm();
                result?;
            }
            Err(error) => {
                let backoff = retry_backoff_seconds(job.attempts);
                let will_retry = store.mark_failed_with_run(&job.id, &error, backoff, run_id);
                claim_guard.disarm();
                let will_retry = will_retry.map_err(|error| error.to_string())?;
                if !will_retry {
                    // Event first (deduped, migration 0118), domain transition
                    // second — a crash between the two converges at startup.
                    // The hook fires for EVERY kind, independent of the
                    // failure-surface classification inside
                    // `surface_terminal_failure`.
                    self.surface_terminal_failure(job, handler);
                    if let Some(handler) = handler {
                        handler.on_terminal_failure(job, &error, &self.state);
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
#[path = "queue_tests.rs"]
mod tests;
