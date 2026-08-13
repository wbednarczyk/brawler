//! Queue integration for KPI ingest runs (#364, ADR 0098 dec. 8 model A).
//!
//! The durable queue executes only the two SHORT DETERMINISTIC steps of the
//! ingest lifecycle — validation (`staged`) and commit (`ready_to_commit`) —
//! and never holds a `running` row while any agent works. Each job row is
//! pinned to one GENERATION of work and its id carries the full generation
//! identity: `kpi_ingest_validate:{run}:rev{N}` /
//! `kpi_ingest_commit:{run}:rev{N}:{manifest_hash}`. A re-stage bumps the
//! revision (new validate generation); invalidate + revalidate of the same
//! revision freezes a NEW hash (new commit generation), so two tuples never
//! share one attention-evidence identity.
//!
//! Headless-only in #364: production producers arrive with the MCP workflow
//! tools (#353) and the acquisition planner (#354). Single-armer invariant:
//! with the one-worker `kpi_ingest` lane, reconciliation running before the
//! pools, and no external armers yet, [`arm`] can never observe a `running`
//! row of the generation it arms — #353 (the first concurrent armer) must
//! revisit this invariant when it lands its invalidate/stage tools. The same
//! topology makes [`terminalize_ingest_run`]'s check-then-`mark_failed`
//! race-free today (nothing can invalidate/re-stage between the generation
//! guard and the transition); #353 must make that pair atomic (a
//! generation-guarded `mark_failed`) alongside the arming revisit.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::jobs::kpi_ingest_validation::validate_kpi_ingest_run;
use crate::jobs::queue::JobHandler;
use crate::storage::{ClaimedJob, KpiIngestRun, KpiIngestRunState, StorageError};

pub const KPI_INGEST_VALIDATE_KIND: &str = "kpi_ingest_validate";
pub const KPI_INGEST_COMMIT_KIND: &str = "kpi_ingest_commit";

/// Retry budget per generation row. Validation and commit are separate kinds
/// with separate rows, so each phase owns its full budget (the "validator's"
/// vs the "commit job's" exhausted retries in the data-model transition table).
const MAX_ATTEMPTS: i64 = 3;

/// Arming a generation failed. `Running` is the #364 single-armer invariant
/// violation — never converted to success; every caller propagates it (a
/// handler's own row retries, reconciliation refuses to start the lane).
#[derive(Debug)]
pub enum ArmError {
    Running { job_id: String },
    Storage(String),
}

impl std::fmt::Display for ArmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running { job_id } => write!(
                f,
                "job {job_id} is running; arming its generation again violates \
                 the #364 single-armer invariant"
            ),
            Self::Storage(error) => write!(f, "{error}"),
        }
    }
}

fn validate_job_id(run_id: &str, revision: i64) -> String {
    format!("{KPI_INGEST_VALIDATE_KIND}:{run_id}:rev{revision}")
}

fn commit_job_id(run_id: &str, revision: i64, manifest_hash: &str) -> String {
    format!("{KPI_INGEST_COMMIT_KIND}:{run_id}:rev{revision}:{manifest_hash}")
}

/// The generation identity parsed back from a job id — the SINGLE authority
/// for terminalization and the failure event (live path and startup path see
/// the same identity; a tampered payload cannot redirect either). Run ids
/// (`kpiing_<hex>`) and manifest hashes contain no `:`, so the split is
/// unambiguous.
enum IngestJobIdentity {
    Validate {
        run_id: String,
        revision: i64,
    },
    Commit {
        run_id: String,
        revision: i64,
        manifest_hash: String,
    },
}

fn parse_job_id(job_id: &str) -> Option<IngestJobIdentity> {
    let parts: Vec<&str> = job_id.split(':').collect();
    let revision_of = |part: &str| part.strip_prefix("rev")?.parse::<i64>().ok();
    match parts.as_slice() {
        [KPI_INGEST_VALIDATE_KIND, run_id, revision] => Some(IngestJobIdentity::Validate {
            run_id: (*run_id).to_owned(),
            revision: revision_of(revision)?,
        }),
        [KPI_INGEST_COMMIT_KIND, run_id, revision, manifest_hash] => {
            Some(IngestJobIdentity::Commit {
                run_id: (*run_id).to_owned(),
                revision: revision_of(revision)?,
                manifest_hash: (*manifest_hash).to_owned(),
            })
        }
        _ => None,
    }
}

/// Both payloads duplicate the row identity (`job_id`) so the
/// [`JobHandler::validate_claimed_job`] preflight can verify the REAL claimed
/// row id, the payload's own claim, and the canonical id derived from the
/// payload tuple all agree before any domain work.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidatePayload {
    job_id: String,
    run_id: String,
    revision: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitPayload {
    job_id: String,
    run_id: String,
    revision: i64,
    manifest_hash: String,
}

/// Arms one generation row: `pending` is already armed (attempts/backoff
/// preserved), `running` is the invariant violation, anything else (missing or
/// terminal) is `reschedule`d — arming is always a FRESH intent (re-stage,
/// revalidation after invalidation, reconcile-missing), never a restart loop.
fn arm(state: &AppState, job_id: &str, kind: &'static str, payload: &str) -> Result<(), ArmError> {
    let jobs = state.jobs();
    match jobs
        .status(job_id)
        .map_err(|error| ArmError::Storage(error.to_string()))?
    {
        Some(row) if row.status == "pending" => return Ok(()),
        Some(row) if row.status == "running" => {
            return Err(ArmError::Running {
                job_id: job_id.to_owned(),
            })
        }
        _ => {}
    }
    let rearmed = jobs
        .reschedule(job_id, kind, payload, MAX_ATTEMPTS)
        .map_err(|error| ArmError::Storage(error.to_string()))?;
    if !rearmed {
        // TOCTOU: the row started running between the status read and the
        // reschedule guard — the same invariant violation, reported loudly.
        return Err(ArmError::Running {
            job_id: job_id.to_owned(),
        });
    }
    Ok(())
}

/// Arms the validation job for one staged revision. The production trigger is
/// #353's stage tool (after `stage_observations`); today tests drive it, and
/// startup reconciliation heals a crash between the stage and this call.
pub fn enqueue_validate(state: &AppState, run_id: &str, revision: i64) -> Result<(), ArmError> {
    let job_id = validate_job_id(run_id, revision);
    let payload = serde_json::to_string(&ValidatePayload {
        job_id: job_id.clone(),
        run_id: run_id.to_owned(),
        revision,
    })
    .map_err(|error| ArmError::Storage(error.to_string()))?;
    arm(state, &job_id, KPI_INGEST_VALIDATE_KIND, &payload)
}

fn enqueue_commit(
    state: &AppState,
    run_id: &str,
    revision: i64,
    manifest_hash: &str,
) -> Result<(), ArmError> {
    let job_id = commit_job_id(run_id, revision, manifest_hash);
    let payload = serde_json::to_string(&CommitPayload {
        job_id: job_id.clone(),
        run_id: run_id.to_owned(),
        revision,
        manifest_hash: manifest_hash.to_owned(),
    })
    .map_err(|error| ArmError::Storage(error.to_string()))?;
    arm(state, &job_id, KPI_INGEST_COMMIT_KIND, &payload)
}

/// The supersession invariant (#364): a "superseded" noop that leaves the run
/// in a queue-owned state must arm the CURRENT generation before returning Ok
/// — a noop is never "do nothing", it hands the baton to the live tuple.
fn arm_current_generation(state: &AppState, run: &KpiIngestRun) -> Result<(), String> {
    match run.status {
        KpiIngestRunState::Staged => {
            enqueue_validate(state, &run.id, run.manifest_revision).map_err(|e| e.to_string())
        }
        KpiIngestRunState::ReadyToCommit => match run.manifest_hash.as_deref() {
            Some(hash) => enqueue_commit(state, &run.id, run.manifest_revision, hash)
                .map_err(|e| e.to_string()),
            // A ready_to_commit row always freezes its hash atomically; NULL
            // here is raw corruption — refuse rather than fabricate an id.
            None => Err(format!(
                "run {} is ready_to_commit with a NULL manifest_hash (raw corruption)",
                run.id
            )),
        },
        // Agent-claimable, committing, or terminal states: the queue owns no
        // work for this run right now.
        _ => Ok(()),
    }
}

/// Preflight shared by both handlers: the REAL claimed row id, the payload's
/// `jobId`, and the canonical id derived from the payload tuple must agree
/// (sol r6 F2 — two payload fields alone would just be attacker-consistent).
fn verify_claimed_identity(
    job: &ClaimedJob,
    payload_job_id: &str,
    canonical: &str,
) -> Result<(), String> {
    if job.id != payload_job_id || payload_job_id != canonical {
        return Err(format!(
            "claimed row {} disagrees with its payload identity {} (canonical {})",
            job.id, payload_job_id, canonical
        ));
    }
    Ok(())
}

/// Id-authoritative `(company_scope, subject)` for the failure event: derive
/// the run from the REAL job id — a tampered payload can never misattribute
/// the event. Subject: the document's own title (raw specific), falling back
/// to the run id.
fn ingest_failure_context(state: &AppState, job_id: &str) -> (Option<String>, Option<String>) {
    let run_id = match parse_job_id(job_id) {
        Some(IngestJobIdentity::Validate { run_id, .. })
        | Some(IngestJobIdentity::Commit { run_id, .. }) => run_id,
        None => return (None, None),
    };
    let Ok(Some(run)) = state.kpi_ingest_runs().get_run(&run_id) else {
        return (None, None);
    };
    let title = state
        .get_report_document(&run.report_document_id)
        .ok()
        .and_then(|document| document.title)
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty());
    (Some(run.company_id), Some(title.unwrap_or(run_id)))
}

/// Id-authoritative domain terminalization, shared by the in-process
/// [`JobHandler::on_terminal_failure`] hook and the startup reconciliation
/// `failed` branch: parse the generation from the job id, and mark the run
/// `failed` ONLY if the live run still sits in this generation's queue-owned
/// state (a receipt-confirmed terminal run is never touched; a superseded
/// generation's failure is inert bookkeeping). Best-effort: the queue row is
/// already the durable record.
fn terminalize_ingest_run(state: &AppState, job_id: &str, error: &str) {
    let Some(identity) = parse_job_id(job_id) else {
        log::warn!("kpi ingest queue: unparseable terminal job id {job_id}");
        return;
    };
    let (run_id, matches) = match &identity {
        IngestJobIdentity::Validate { run_id, revision } => {
            let Ok(Some(run)) = state.kpi_ingest_runs().get_run(run_id) else {
                return;
            };
            (
                run_id.clone(),
                run.status == KpiIngestRunState::Staged && run.manifest_revision == *revision,
            )
        }
        IngestJobIdentity::Commit {
            run_id,
            revision,
            manifest_hash,
        } => {
            let Ok(Some(run)) = state.kpi_ingest_runs().get_run(run_id) else {
                return;
            };
            (
                run_id.clone(),
                run.status == KpiIngestRunState::ReadyToCommit
                    && run.manifest_revision == *revision
                    && run.manifest_hash.as_deref() == Some(manifest_hash.as_str()),
            )
        }
    };
    if !matches {
        return;
    }
    if let Err(error) = state.kpi_ingest_runs().mark_failed(&run_id, error) {
        log::warn!("kpi ingest queue: could not mark run {run_id} failed: {error}");
    }
}

/// The `kpi_ingest_validate` handler: a pure state-machine advance for one
/// staged generation. Validation completing with outcome `failed` is job
/// SUCCESS (the run returned to the agent's worklist); a proven-superseded
/// generation hands the baton to the live tuple; only infrastructure errors
/// on the still-current generation retry.
pub struct KpiIngestValidateHandler;

impl JobHandler for KpiIngestValidateHandler {
    fn kind(&self) -> &'static str {
        KPI_INGEST_VALIDATE_KIND
    }

    fn validate_claimed_job(&self, job: &ClaimedJob) -> Result<(), String> {
        let payload: ValidatePayload =
            serde_json::from_str(&job.payload).map_err(|error| error.to_string())?;
        let canonical = validate_job_id(&payload.run_id, payload.revision);
        verify_claimed_identity(job, &payload.job_id, &canonical)
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        let payload: ValidatePayload =
            serde_json::from_str(payload).map_err(|error| error.to_string())?;
        let Some(run) = state
            .kpi_ingest_runs()
            .get_run(&payload.run_id)
            .map_err(|error| error.to_string())?
        else {
            return Ok(());
        };
        if run.status != KpiIngestRunState::Staged || run.manifest_revision != payload.revision {
            return arm_current_generation(state, &run);
        }
        match validate_kpi_ingest_run(state, &payload.run_id) {
            Ok(outcome) if outcome.outcome == "ready" => {
                // Chain the commit as its OWN generation row (separate retry
                // budget); the hash was frozen by the validation atom.
                enqueue_commit(
                    state,
                    &payload.run_id,
                    payload.revision,
                    &outcome.manifest_hash,
                )
                .map_err(|error| error.to_string())
            }
            Ok(_) => Ok(()),
            Err(error) => {
                let run = state
                    .kpi_ingest_runs()
                    .get_run(&payload.run_id)
                    .map_err(|error| error.to_string())?;
                match run {
                    Some(run)
                        if run.status == KpiIngestRunState::Staged
                            && run.manifest_revision == payload.revision =>
                    {
                        // Still our generation — a real failure, retried by the
                        // queue until this row's budget is exhausted.
                        Err(error.message)
                    }
                    Some(run) => arm_current_generation(state, &run),
                    None => Ok(()),
                }
            }
        }
    }

    fn failure_context(
        &self,
        job: &ClaimedJob,
        state: &AppState,
    ) -> (Option<String>, Option<String>) {
        ingest_failure_context(state, &job.id)
    }

    fn on_terminal_failure(&self, job: &ClaimedJob, error: &str, state: &AppState) {
        terminalize_ingest_run(state, &job.id, error);
    }
}

/// The `kpi_ingest_commit` handler: drives `commit_manifest` for one frozen
/// `(run, revision, manifest_hash)` tuple. Replay is idempotent (the stored
/// receipt is the stable answer), contention retries, and a superseded tuple
/// hands the baton to the live generation.
pub struct KpiIngestCommitHandler;

impl JobHandler for KpiIngestCommitHandler {
    fn kind(&self) -> &'static str {
        KPI_INGEST_COMMIT_KIND
    }

    fn validate_claimed_job(&self, job: &ClaimedJob) -> Result<(), String> {
        let payload: CommitPayload =
            serde_json::from_str(&job.payload).map_err(|error| error.to_string())?;
        let canonical = commit_job_id(&payload.run_id, payload.revision, &payload.manifest_hash);
        verify_claimed_identity(job, &payload.job_id, &canonical)
    }

    fn run(&self, payload: &str, state: &AppState) -> Result<(), String> {
        let payload: CommitPayload =
            serde_json::from_str(payload).map_err(|error| error.to_string())?;
        if state
            .kpi_ingest_runs()
            .get_run(&payload.run_id)
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Ok(());
        }
        match state.kpi_ingest_commit().commit_manifest(
            &payload.run_id,
            &payload.manifest_hash,
            payload.revision,
        ) {
            Ok(_receipt) => Ok(()),
            Err(StorageError::CommitContention { .. }) => {
                Err(format!("commit contention for run {}", payload.run_id))
            }
            Err(
                error @ (StorageError::StaleManifestForCommit { .. }
                | StorageError::InvalidRunTransition { .. }),
            ) => {
                let run = state
                    .kpi_ingest_runs()
                    .get_run(&payload.run_id)
                    .map_err(|error| error.to_string())?;
                match run {
                    Some(run)
                        if run.status == KpiIngestRunState::ReadyToCommit
                            && run.manifest_revision == payload.revision
                            && run.manifest_hash.as_deref()
                                == Some(payload.manifest_hash.as_str()) =>
                    {
                        // The live tuple is still ours yet the commit refused —
                        // a real failure, retried by the queue.
                        Err(error.to_string())
                    }
                    Some(run) => arm_current_generation(state, &run),
                    None => Ok(()),
                }
            }
            Err(error) => Err(error.to_string()),
        }
    }

    fn failure_context(
        &self,
        job: &ClaimedJob,
        state: &AppState,
    ) -> (Option<String>, Option<String>) {
        ingest_failure_context(state, &job.id)
    }

    fn on_terminal_failure(&self, job: &ClaimedJob, error: &str, state: &AppState) {
        terminalize_ingest_run(state, &job.id, error);
    }
}

/// Startup reconciliation of queue-owned runs with their generation rows
/// (#364; ordered AFTER the run-level reclaim (#360) and AFTER an explicit
/// `reclaim_stale_running`, BEFORE `spawn_pools`). For each `staged` /
/// `ready_to_commit` run, the CURRENT generation's (kind, id, payload) is
/// derived from the live tuple — the id carries the full identity, so a
/// `failed` row found under that id is tuple-matched by construction; failed
/// rows of superseded generations are inert bookkeeping. Matrix: `failed` →
/// event FIRST (deduped, 0118) then domain `mark_failed`; `pending`/`running`
/// → untouched; missing/`succeeded` → arm. An error (including the
/// single-armer invariant and raw corruption) aborts reconciliation — the
/// caller then must NOT start the `kpi_ingest` lane (fail-closed).
pub fn reconcile_ingest_jobs(state: &AppState) -> Result<(), String> {
    let runs_store = state.kpi_ingest_runs();
    let mut queue_owned: Vec<KpiIngestRun> = runs_store
        .list_runs(Some(KpiIngestRunState::Staged), None)
        .map_err(|error| error.to_string())?;
    queue_owned.extend(
        runs_store
            .list_runs(Some(KpiIngestRunState::ReadyToCommit), None)
            .map_err(|error| error.to_string())?,
    );

    for run in queue_owned {
        let (kind, job_id, payload) = match run.status {
            KpiIngestRunState::Staged => {
                let job_id = validate_job_id(&run.id, run.manifest_revision);
                let payload = serde_json::to_string(&ValidatePayload {
                    job_id: job_id.clone(),
                    run_id: run.id.clone(),
                    revision: run.manifest_revision,
                })
                .map_err(|error| error.to_string())?;
                (KPI_INGEST_VALIDATE_KIND, job_id, payload)
            }
            KpiIngestRunState::ReadyToCommit => {
                let Some(hash) = run.manifest_hash.as_deref() else {
                    return Err(format!(
                        "run {} is ready_to_commit with a NULL manifest_hash (raw corruption)",
                        run.id
                    ));
                };
                let job_id = commit_job_id(&run.id, run.manifest_revision, hash);
                let payload = serde_json::to_string(&CommitPayload {
                    job_id: job_id.clone(),
                    run_id: run.id.clone(),
                    revision: run.manifest_revision,
                    manifest_hash: hash.to_owned(),
                })
                .map_err(|error| error.to_string())?;
                (KPI_INGEST_COMMIT_KIND, job_id, payload)
            }
            _ => unreachable!("only queue-owned states are listed"),
        };

        let row = state
            .jobs()
            .status(&job_id)
            .map_err(|error| error.to_string())?;
        match row {
            Some(row) if row.status == "failed" => {
                // Event first (deduped by the 0118 partial unique index), the
                // domain transition second: a crash between the two converges
                // on the next startup because the run stays queue-owned.
                let (company, subject) = ingest_failure_context(state, &job_id);
                state
                    .attention()
                    .record_job_failure(&job_id, company.as_deref(), subject.as_deref())
                    .map_err(|error| error.to_string())?;
                let error = row
                    .last_error
                    .unwrap_or_else(|| "job dead-lettered on reclaim".to_owned());
                runs_store
                    .mark_failed(&run.id, &error)
                    .map_err(|error| error.to_string())?;
            }
            Some(row) if row.status == "pending" || row.status == "running" => {}
            _ => arm(state, &job_id, kind, &payload).map_err(|error| error.to_string())?,
        }
    }
    Ok(())
}

/// Registers both handlers (used by `jobs::handlers::build_worker`).
pub fn handlers() -> Vec<Arc<dyn JobHandler>> {
    vec![
        Arc::new(KpiIngestValidateHandler),
        Arc::new(KpiIngestCommitHandler),
    ]
}

#[cfg(test)]
#[path = "kpi_ingest_queue/tests.rs"]
mod tests;
