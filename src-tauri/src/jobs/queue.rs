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
use crate::storage::{ClaimedJob, StorageError, StorageResult};

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

/// THE single terminal-failure point (ADR 0091 dec. 1): the job has exhausted
/// its retries and will not run again. Kinds whose failure surface is
/// [`crate::jobs::failure_surface::FailureSurface::TodayAttention`] raise the
/// generic system `job_failed` attention event here; kinds with a richer
/// domain surface (Sources adapter health, the autopilot run card) keep it
/// EXCLUSIVELY, so nothing double-fires. An unclassified kind surfaces
/// nothing and is caught by the enumeration gate
/// (`jobs::failure_surface::tests`), never silently defaulted here.
///
/// A free function, not a `JobWorker` method (sol diff R3 #2): `ClaimGuard`'s
/// `Drop` fallback — which has no `&JobWorker`, only its own `AppState`
/// handle — must be able to invoke this SAME terminal-failure point when its
/// own last-resort settle lands terminal, exactly like the ordinary branch.
///
/// Best-effort: raising the event must never turn a recorded failure into a
/// worker error (the queue row is already the durable record).
fn surface_terminal_failure(state: &AppState, job: &ClaimedJob, handler: Option<&dyn JobHandler>) {
    use crate::jobs::failure_surface::{failure_surface, FailureSurface};

    if failure_surface(&job.kind) != Some(FailureSurface::TodayAttention) {
        return;
    }
    let (company, subject) = handler
        .map(|handler| handler.failure_context(job, state))
        .unwrap_or((None, None));
    if let Err(error) =
        state
            .attention()
            .record_job_failure(&job.id, company.as_deref(), subject.as_deref())
    {
        log::warn!(
            "job queue: could not raise the failure event for job {} ({}): {error}",
            job.id,
            job.kind
        );
    }
}

/// Run both terminal hooks — `surface_terminal_failure` (which itself calls
/// the handler's `failure_context`) and `handler.on_terminal_failure` — with
/// NO unwind past this function (sol diff R4 #3), each wrapped in its OWN
/// `catch_unwind` independently, so a handler whose `failure_context` OR
/// `on_terminal_failure` panics can never propagate out of here. Before this
/// fix, `ClaimGuard`'s non-panic `Drop` fallback (and the outer dispatch-level
/// panic arm) invoked both directly and uncontained: a panic in either would
/// propagate past the point `dispatch`'s own outer `catch_unwind` had already
/// returned from (the `Drop` case) or was already inside (the panic-arm
/// case), killing the worker thread — or, if it happened while the guard
/// itself was unwinding, aborting the process outright. Used identically from
/// the ordinary error branch, the outer panic arm, and the non-panicking
/// `Drop` fallback — callers keep the "never while `std::thread::panicking()`"
/// rule themselves; invoking arbitrary handler code mid-unwind is what this
/// helper does NOT protect against.
fn run_terminal_hooks_no_unwind(
    state: &AppState,
    job: &ClaimedJob,
    handler: Option<&dyn JobHandler>,
    error: &str,
) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        surface_terminal_failure(state, job, handler);
    }))
    .is_err()
    {
        log::error!(
            "job queue: surface_terminal_failure panicked for job {} ({}) — caught, worker \
             thread continues",
            job.id,
            job.kind
        );
    }
    if let Some(handler) = handler {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handler.on_terminal_failure(job, error, state);
        }))
        .is_err()
        {
            log::error!(
                "job queue: handler.on_terminal_failure panicked for job {} ({}) — caught, \
                 worker thread continues",
                job.id,
                job.kind
            );
        }
    }
}

/// The outcome of [`settle_with_recovery`] (sol diff R3 #2) — replaces an
/// untyped `Option<T>` that could not tell a caller "committed" apart from
/// "both attempts failed, the claim guard is now the last resort", which used
/// to read identically to `MissingRow`'s intentional `None`.
enum SettleOutcome<T> {
    /// The call committed (on the primary attempt or the retry) — `T` is its
    /// real return value (e.g. `mark_failed_with_run`'s `will_retry` bool).
    Settled(T),
    /// [`StorageError::JobQueueRowMissingDuringSettle`] — the settle
    /// transaction proved the occurrence already closed truthfully even
    /// though the queue-side transition did not apply. Nothing more to do.
    MissingRow,
    /// Both the primary attempt and the retry failed. `claim_guard` is left
    /// ARMED; its `Drop` fallback is the last-resort recovery.
    Unrecovered,
}

/// RAII unwind guard for one claimed job's post-claim lifecycle (ADR 0109
/// dec. 2, sol diff R1 #1). `dispatch_claimed` disarms it ONLY once a
/// defer/settle call is known to have actually committed (sol diff R2 #1) —
/// via [`settle_with_recovery`], never unconditionally before the call's
/// result is known. If both the ordinary attempt and its one recovery retry
/// fail — [`SettleOutcome::Unrecovered`] — the caller records the REAL
/// intended failure text via [`ClaimGuard::set_pending_error`] so `Drop`'s
/// own last-resort settle (below) repeats that SAME transition with that
/// SAME text, never a fabricated one (sol diff R3 #2): the "panic: job
/// worker unwound before settling" message is reserved for when `Drop`
/// itself fires while the thread is actually unwinding (`std::thread::
/// panicking()`) — e.g. identity checkout or `begin_attempt` panicking
/// before any settle attempt was even made, or a second panic escaping the
/// dispatch-level panic arm's own hook calls.
struct ClaimGuard {
    state: AppState,
    job: ClaimedJob,
    /// The registered handler, if any — kept so a non-panic `Drop` fallback
    /// that lands terminal can invoke the SAME terminal hooks
    /// (`surface_terminal_failure`, `on_terminal_failure`) the ordinary
    /// branch would (sol diff R3 #2).
    handler: Option<Arc<dyn JobHandler>>,
    run_id: Cell<Option<i64>>,
    /// `Some(true)` once the handler is known to have succeeded (set right
    /// before the ordinary succeed-settle call) — the fallback then tries
    /// `mark_succeeded_with_run` instead of failing a job that actually
    /// finished its work. `None`/`Some(false)` fall back to failed/retried.
    succeeded: Cell<Option<bool>>,
    /// The real text of whichever failure/defer transition is currently
    /// intended — set by the caller right before every `settle_with_recovery`
    /// call in the non-success path (sol diff R3 #2).
    pending_error: Cell<Option<String>>,
    armed: Cell<bool>,
}

impl ClaimGuard {
    fn new(state: AppState, job: ClaimedJob, handler: Option<Arc<dyn JobHandler>>) -> Self {
        Self {
            state,
            job,
            handler,
            run_id: Cell::new(None),
            succeeded: Cell::new(None),
            pending_error: Cell::new(None),
            armed: Cell::new(true),
        }
    }

    fn set_run_id(&self, run_id: i64) {
        self.run_id.set(Some(run_id));
    }

    fn set_succeeded(&self, succeeded: bool) {
        self.succeeded.set(Some(succeeded));
    }

    fn set_pending_error(&self, error: impl Into<String>) {
        self.pending_error.set(Some(error.into()));
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
        if self.succeeded.get() == Some(true) {
            if let Err(error) = self
                .state
                .jobs()
                .mark_succeeded_with_run(&self.job.id, run_id)
            {
                log::error!(
                    "job queue: claim guard could not settle job {} as succeeded: {error}",
                    self.job.id
                );
            }
            return;
        }

        // sol diff R3 #2: the fabricated panic message is used ONLY while
        // genuinely unwinding — every non-panic arrival here (both ordinary
        // settle attempts failed, or a defer/checkout/begin_attempt failure
        // repeatedly failed to even record) reuses the REAL intended error
        // text instead.
        let panicking = std::thread::panicking();
        let error_text = if panicking {
            "panic: job worker unwound before settling".to_owned()
        } else {
            self.pending_error.take().unwrap_or_else(|| {
                "job worker exited before settling and no error was recorded".to_owned()
            })
        };
        let backoff = retry_backoff_seconds(self.job.attempts);
        match self
            .state
            .jobs()
            .mark_failed_with_run(&self.job.id, &error_text, backoff, run_id)
        {
            Ok(will_retry) => {
                // Terminal hooks fire from this last-resort path too (sol
                // diff R3 #2) — but never while genuinely unwinding: invoking
                // arbitrary handler code mid-panic risks a second panic
                // during unwind, which aborts the process outright.
                if !will_retry && !panicking {
                    run_terminal_hooks_no_unwind(
                        &self.state,
                        &self.job,
                        self.handler.as_deref(),
                        &error_text,
                    );
                }
            }
            Err(StorageError::JobQueueRowMissingDuringSettle { .. }) => {}
            Err(error) => {
                log::error!(
                    "job queue: claim guard could not settle job {} after exhausting recovery: {error}",
                    self.job.id
                );
            }
        }
    }
}

/// Run a defer/settle storage call with ONE idempotent retry, disarming
/// `claim_guard` only once the call is PROVEN to have committed — an `Ok`, or
/// [`StorageError::JobQueueRowMissingDuringSettle`], which the settle
/// transaction guarantees already closed the occurrence before returning
/// (ADR 0109 dec. 2, sol diff R2 #1). If BOTH the primary attempt and the
/// retry return any other `Err`, `claim_guard` is left ARMED — its `Drop`
/// fallback is the last-resort recovery — and the failure is logged at error
/// level so a truly stuck row is never silent. `op` is called with the SAME
/// arguments both times (sol diff R3 #2) — callers close over the real
/// intended error text once, never re-derive it per attempt.
fn settle_with_recovery<T>(
    claim_guard: &ClaimGuard,
    mut op: impl FnMut() -> StorageResult<T>,
) -> SettleOutcome<T> {
    for attempt in 1..=2 {
        match op() {
            Ok(value) => {
                claim_guard.disarm();
                return SettleOutcome::Settled(value);
            }
            Err(StorageError::JobQueueRowMissingDuringSettle { .. }) => {
                claim_guard.disarm();
                return SettleOutcome::MissingRow;
            }
            Err(error) if attempt == 1 => {
                log::warn!(
                    "job queue: settle failed for job {}, retrying once: {error}",
                    claim_guard.job.id
                );
            }
            Err(error) => {
                log::error!(
                    "job queue: settle failed twice for job {}: {error}; leaving the claim \
                     guard armed so its Drop fallback terminalizes the row",
                    claim_guard.job.id
                );
            }
        }
    }
    SettleOutcome::Unrecovered
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
    /// thread survives. A handler panic specifically is caught one layer
    /// INSIDE this (see `run_validated`) so it still enters the ordinary
    /// retry/terminal-hook path with its real message — the outer guard
    /// exists for panics in the surrounding scaffolding (identity checkout,
    /// `begin_attempt`, the settle calls themselves), which used to escape
    /// uncontained and could strand a claimed row `running` forever while
    /// killing the worker thread.
    ///
    /// The panic arm below (sol diff R2 #2) makes recovery explicit: it
    /// decides retry vs terminal via the SAME `mark_failed_with_run`/
    /// `mark_succeeded_with_run` path (through [`settle_with_recovery`]) the
    /// ordinary error branch uses, and invokes the SAME terminal hooks
    /// (`surface_terminal_failure`, `handler.on_terminal_failure`) when
    /// terminal — never just `ClaimGuard`'s Drop, which never called them.
    /// Drop stays the last-resort fallback, for when this recovery settle
    /// itself also fails.
    fn dispatch(&self, job: ClaimedJob) -> Result<(), String> {
        let handler = self.handlers.get(job.kind.as_str()).cloned();
        let claim_guard = ClaimGuard::new(self.state.clone(), job.clone(), handler.clone());

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.dispatch_claimed(&job, handler.as_deref(), &claim_guard)
        }));

        match outcome {
            Ok(result) => result,
            Err(panic_payload) => {
                let message = panic_message(panic_payload);
                log::error!(
                    "job queue: job {} ({}) unwound past its handler while settling: {message}",
                    job.id,
                    job.kind
                );
                // If the ordinary path already disarmed the guard (e.g. the
                // panic happened inside a terminal hook AFTER a successful
                // settle), there is nothing left to redo here — settling
                // again would double-handle an already-terminal row.
                if claim_guard.armed.get() {
                    let run_id = claim_guard.run_id.get();
                    if claim_guard.succeeded.get() == Some(true) {
                        settle_with_recovery(&claim_guard, || {
                            self.state.jobs().mark_succeeded_with_run(&job.id, run_id)
                        });
                    } else {
                        let panic_error = format!("panic: {message}");
                        let backoff = retry_backoff_seconds(job.attempts);
                        claim_guard.set_pending_error(panic_error.clone());
                        let outcome = settle_with_recovery(&claim_guard, || {
                            self.state.jobs().mark_failed_with_run(
                                &job.id,
                                &panic_error,
                                backoff,
                                run_id,
                            )
                        });
                        if let SettleOutcome::Settled(false) = outcome {
                            run_terminal_hooks_no_unwind(
                                &self.state,
                                &job,
                                handler.as_deref(),
                                &panic_error,
                            );
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// The identity/begin_attempt/run/settle body `dispatch` wraps in unwind
    /// containment. Every early return routes its defer/settle call through
    /// [`settle_with_recovery`] (sol diff R2 #1) — `claim_guard` disarms only
    /// once that call is proven committed, never unconditionally before its
    /// result is known.
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
                        claim_guard.set_pending_error(format!(
                            "job queue: source lock busy for job {} and the deferral could not \
                             be recorded",
                            job.id
                        ));
                        settle_with_recovery(claim_guard, || {
                            store.defer(&job.id, SOURCE_BUSY_BACKOFF_SECONDS)
                        });
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
                claim_guard.set_pending_error(format!("identity checkout failed: {error}"));
                settle_with_recovery(claim_guard, || {
                    store.defer(&job.id, SOURCE_BUSY_BACKOFF_SECONDS)
                });
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
                        claim_guard.set_pending_error(format!("begin_attempt failed: {error}"));
                        settle_with_recovery(claim_guard, || {
                            store.defer(&job.id, SOURCE_BUSY_BACKOFF_SECONDS)
                        });
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
                settle_with_recovery(claim_guard, || {
                    store.mark_succeeded_with_run(&job.id, run_id)
                });
            }
            Err(error) => {
                let backoff = retry_backoff_seconds(job.attempts);
                // sol diff R3 #2: the REAL handler error, stored so a non-panic
                // Drop fallback (both attempts below fail too) repeats this
                // SAME text instead of a fabricated one.
                claim_guard.set_pending_error(error.clone());
                let outcome = settle_with_recovery(claim_guard, || {
                    store.mark_failed_with_run(&job.id, &error, backoff, run_id)
                });
                if let SettleOutcome::Settled(false) = outcome {
                    // Event first (deduped, migration 0118), domain transition
                    // second — a crash between the two converges at startup.
                    // The hook fires for EVERY kind, independent of the
                    // failure-surface classification inside
                    // `surface_terminal_failure`.
                    run_terminal_hooks_no_unwind(&self.state, job, handler, &error);
                }
            }
        }
        Ok(())
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
