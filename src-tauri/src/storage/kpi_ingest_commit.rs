//! Atomic manifest commit — one transaction per report (ADR 0098 dec. 5, epic
//! #352, card #362). `commit_manifest` re-verifies the frozen `ready`
//! manifest's freshness, then writes the period, every accepted fact, its
//! provenance, the immutable commit receipt, and the run's terminal state in
//! ONE `Immediate` transaction on ONE connection handle — any failure rolls
//! back everything, including the period row (closes the empty-period-hole
//! bug in `jobs/record_financial_facts.rs`, where the period commits before
//! validation and each fact holds its own transaction). Composing the
//! PUBLIC, connection-checking-out store methods inside this transaction is
//! prohibited (ADR 0098 dec. 5) — every write below is a connection-level
//! `pub(super)` primitive from a sibling module. Headless-only: no `jobs`
//! wrapper exists yet (the MCP/#353 and queue/#364 callers add their own thin
//! wrapper once they exist, so this module has no caller with zero purpose).
//! Reach it via `AppState::kpi_ingest_commit()`.
//!
//! Idempotency + concurrency (#363, ADR 0098 dec. 5 second half): retrying a
//! committed `(run, hash, revision)` returns the stored receipt verbatim (a
//! fast path BEFORE attempt parsing); a stale tuple is a typed conflict,
//! never an overwrite; the race loser resolves to the winner's receipt inside
//! its own transaction. The decision's per-(company, period) lock is realized
//! by serialization — SQLite's single writer plus the `Immediate` transaction
//! spanning the whole commit — which is strictly stronger than a targeted
//! lock; `busy_timeout` exhaustion surfaces as the retryable
//! `CommitContention`, never a raw SQLite error.

use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::json;

use crate::fundamentals::kpi_manifest::{
    Citation, KpiIngestManifest, ManifestSealError, Outcome, SealedManifest, ValidationState,
};

use super::database::Database;
use super::kpi_extraction::{self, PinnedFactInput};
use super::kpi_ingest_runs::{self, KpiIngestRunsStore};
use super::kpi_ingest_staging::{self, NewCommitReceipt};
use super::{CommitReceipt, StorageError, StorageResult, StructuredFactCommit};

/// One entry in `kpi_ingest_commit_receipts.outcomes_json` (migration 0138
/// schema v1): `{observationId, revision, ordinal, metricKey, factId, outcome,
/// detail?}`. `factId` is `null` only for `divergent` (no write); `detail` is
/// present only for `divergent` (`{existingFactId}`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FactOutcomeEntry {
    observation_id: String,
    revision: i64,
    ordinal: i64,
    metric_key: String,
    fact_id: Option<String>,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<serde_json::Value>,
}

/// Canonical structural-locator JSON for `financial_fact_provenance.citation`
/// (#362 step 5, sol F5): serde over the manifest's own [`Citation`] shape —
/// explicit nulls, deterministic declaration-order fields (`page`, `table`,
/// `row`, `quote`) — so two commits of the same locator hash identically.
fn canonical_citation_json(citation: &Citation) -> String {
    serde_json::to_string(citation).expect("Citation serialization is total")
}

fn period_conflict(run_id: &str, reason: &'static str) -> StorageError {
    StorageError::CommitPeriodConflict {
        run: run_id.to_owned(),
        reason,
    }
}

/// The ONE classification of a stored receipt against its run and the
/// caller's requested tuple (#363, ADR 0098 dec. 5) — used by all three
/// receipt-returning paths (the step-0 fast path, the race loser after a
/// transition refusal, the post-BUSY recovery). One JOINed query so the
/// receipt and run are read atomically on one connection.
///
/// Order matters: receipt↔run coherence is judged BEFORE the requested tuple,
/// so a corrupt pair can never hide behind a `StaleManifestForCommit`.
fn classify_stored_receipt(
    connection: &rusqlite::Connection,
    run_id: &str,
    requested_hash: &str,
    requested_revision: i64,
) -> StorageResult<Option<CommitReceipt>> {
    let row: Option<(CommitReceipt, String, Option<String>, i64)> = connection
        .query_row(
            "SELECT rc.id, rc.run_id, rc.manifest_hash, rc.manifest_revision,
                    rc.terminal_status, rc.period_id, rc.accepted_count,
                    rc.outcomes_schema_version, rc.outcomes_json, rc.committed_at,
                    r.status, r.manifest_hash, r.manifest_revision
             FROM kpi_ingest_commit_receipts rc
             JOIN kpi_ingest_runs r ON r.id = rc.run_id
             WHERE rc.run_id = ?1",
            [run_id],
            |row| {
                Ok((
                    CommitReceipt {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        manifest_hash: row.get(2)?,
                        manifest_revision: row.get(3)?,
                        terminal_status: row.get(4)?,
                        period_id: row.get(5)?,
                        accepted_count: row.get(6)?,
                        outcomes_schema_version: row.get(7)?,
                        outcomes_json: row.get(8)?,
                        committed_at: row.get(9)?,
                    },
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                ))
            },
        )
        .optional()?;
    let Some((receipt, run_status, run_hash, run_revision)) = row else {
        return Ok(None);
    };
    let coherent = (run_status == "complete" || run_status == "partial")
        && run_status == receipt.terminal_status
        && run_hash.as_deref() == Some(receipt.manifest_hash.as_str())
        && run_revision == receipt.manifest_revision;
    if !coherent {
        return Err(StorageError::CommitReceiptRunMismatch {
            run: run_id.to_owned(),
        });
    }
    if receipt.manifest_hash != requested_hash || receipt.manifest_revision != requested_revision {
        return Err(StorageError::StaleManifestForCommit {
            id: run_id.to_owned(),
        });
    }
    Ok(Some(receipt))
}

#[derive(Clone)]
pub struct KpiIngestCommitStore {
    db: Database,
}

impl KpiIngestCommitStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Atomically commits a `ready` manifest (ADR 0098 dec. 5) — see the
    /// module doc for the full flow. Returns the immutable [`CommitReceipt`].
    pub fn commit_manifest(
        &self,
        run_id: &str,
        manifest_hash: &str,
        revision: i64,
    ) -> StorageResult<CommitReceipt> {
        // --- Step 0 (#363): the receipt fast path, BEFORE attempt parsing ---
        // Retrying a committed manifest returns the stored receipt verbatim —
        // never re-executes the write primitives (they would answer
        // `Reobserved` on replay, a different verdict than the original
        // `Created`; ADR 0098 dec. 5). Deliberately no attempt re-seal: the
        // receipt stays the stable answer even if the attempt bytes rot.
        {
            let connection = self.db.checkout()?;
            if let Some(receipt) =
                classify_stored_receipt(&connection, run_id, manifest_hash, revision)?
            {
                return Ok(receipt);
            }
        }

        // --- Steps 1-2: outside the transaction, own connections -----------
        // Validation attempts are immutable/append-only; `begin_committing`
        // re-verifies freshness inside the transaction below, so a stale read
        // here is never a correctness issue, only a slightly-early refusal.
        let runs_store = KpiIngestRunsStore::new(self.db.clone());
        let attempt = match runs_store.get_validation_attempt(run_id, revision, manifest_hash)? {
            Some(attempt) => attempt,
            None => {
                // sol F1: classify against the run's CURRENT tuple — a wrong
                // hash/revision could never have reached this guard any other
                // way (the query filters on both), so a tuple mismatch means
                // the CALLER's request is stale; a matching tuple with no
                // attempt row predates migration 0139.
                let run = runs_store.get_run(run_id)?.ok_or_else(|| {
                    StorageError::KpiIngestRunNotFound {
                        id: run_id.to_owned(),
                    }
                })?;
                if run.manifest_hash.as_deref() != Some(manifest_hash)
                    || run.manifest_revision != revision
                {
                    return Err(StorageError::StaleManifestForCommit {
                        id: run_id.to_owned(),
                    });
                }
                return Err(StorageError::MissingValidationAttempt {
                    run: run_id.to_owned(),
                    revision,
                });
            }
        };

        let manifest: KpiIngestManifest =
            serde_json::from_str(&attempt.manifest_json).map_err(|_| {
                StorageError::CorruptStoredManifest {
                    run: run_id.to_owned(),
                }
            })?;
        // sol F6: `SealedManifest::seal` re-verifies internal consistency
        // (outcome/counts/validationState derived from codes) and classifies
        // a version mismatch separately — valid history sealed by an
        // old/newer validator build, remedy is invalidate+re-validate, never
        // "corrupt".
        let sealed = SealedManifest::seal(manifest.clone()).map_err(|error| match error {
            ManifestSealError::VersionMismatch { .. } => StorageError::UnsupportedManifestVersion {
                run: run_id.to_owned(),
            },
            _ => StorageError::CorruptStoredManifest {
                run: run_id.to_owned(),
            },
        })?;
        if sealed.manifest_hash() != attempt.manifest_hash || sealed.revision() != attempt.revision
        {
            return Err(StorageError::CorruptStoredManifest {
                run: run_id.to_owned(),
            });
        }
        if sealed.outcome() != Outcome::Ready {
            return Err(StorageError::CorruptStoredManifest {
                run: run_id.to_owned(),
            });
        }
        // sol F4: a real validator always flags `mapping.unresolved` on an
        // unmapped observation, so a `ready` manifest can never carry a null
        // `definitionId` on a passed/unreviewed observation — reachable only
        // via a raw-tampered stored row (same "corrupt" bucket as above).
        // `no_definition` stays in the outcomes vocabulary purely as 0138
        // schema documentation; no branch below ever produces it.
        for obs in &manifest.observations {
            if matches!(
                obs.validation_state,
                ValidationState::Passed | ValidationState::Unreviewed
            ) && obs.definition_id.is_none()
            {
                return Err(StorageError::CorruptStoredManifest {
                    run: run_id.to_owned(),
                });
            }
        }

        // --- Steps 3-9: ONE Immediate transaction on ONE handle ------------
        // Deterministic rendezvous for the concurrency test — a no-op unless
        // the test installed a barrier in THIS thread.
        #[cfg(test)]
        tests::pre_transaction_test_barrier();

        let mut connection = self.db.checkout()?;
        // The whole transactional body lives in this labeled block so the
        // BUSY path can fall out of it with every borrow of `connection`
        // ended — the failed guard is then dropped BEFORE the recovery read,
        // so a small/saturated pool cannot starve (#363, sol r2).
        'transactional: {
            let tx = match connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            {
                Ok(tx) => tx,
                // BEGIN IMMEDIATE lost the busy_timeout race (#363, sol r1 F3).
                // Only DatabaseBusy is writer contention — SQLITE_LOCKED and every
                // other SQLite error stay untyped.
                Err(rusqlite::Error::SqliteFailure(sqlite_error, _))
                    if sqlite_error.code == rusqlite::ErrorCode::DatabaseBusy =>
                {
                    break 'transactional;
                }
                Err(error) => return Err(error.into()),
            };

            // The race loser (#363): a parallel commit won while we were sealing.
            // After BEGIN IMMEDIATE succeeds, every earlier writer has committed
            // or rolled back, so the in-transaction receipt read is decisive; a
            // missing receipt means the refusal was genuine, so it propagates.
            if let Err(error) =
                kpi_ingest_runs::begin_committing(&tx, run_id, manifest_hash, revision)
            {
                if matches!(error, StorageError::InvalidRunTransition { .. }) {
                    if let Some(receipt) =
                        classify_stored_receipt(&tx, run_id, manifest_hash, revision)?
                    {
                        return Ok(receipt);
                    }
                }
                return Err(error);
            }

            // Rebind the sealed manifest to the run's FULL live context inside the
            // transaction (luna PR #376): begin_committing pins status/hash/
            // revision, but a raw-tampered attempt row could still pair a valid
            // manifest of run A with run B — every identity field the validator
            // bound at seal time is re-compared against the live row here.
            let live: (
                String,
                String,
                Option<String>,
                String,
                String,
                Option<String>,
                Option<String>,
            ) = tx.query_row(
                "SELECT company_id, report_document_id, source_content_hash, scope, data_quality,
                        expected_kpis_json, period_id
                 FROM kpi_ingest_runs WHERE id = ?1",
                [run_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )?;
            let (
                live_company,
                live_document,
                live_source_hash,
                live_scope,
                live_data_quality,
                live_expected_kpis,
                live_period_id,
            ) = live;
            if sealed.run_id() != run_id
                || sealed.company_id() != live_company
                || sealed.report_document_id() != live_document
                || sealed.source_content_hash() != live_source_hash.as_deref()
                || sealed.scope() != live_scope
                || sealed.data_quality() != live_data_quality
                || sealed.expected_kpis_json() != live_expected_kpis.as_deref()
            {
                return Err(StorageError::CorruptStoredManifest {
                    run: run_id.to_owned(),
                });
            }

            // Step 4: period — 4-branch match on (manifest.periodId, LIVE
            // run.period_id) under the `committing` guard (sol F3 r2): the FK
            // `period_id ON DELETE SET NULL` (migration 0137) means a deleted
            // pinned period looks identical to "never had one" unless the two
            // sides are told apart explicitly — never silently re-created.

            let period_id: String = match (sealed.period_id(), live_period_id.as_deref()) {
                (Some(manifest_period), Some(live_period)) => {
                    if manifest_period != live_period {
                        return Err(period_conflict(
                            run_id,
                            "manifest periodId does not match the run's live period_id",
                        ));
                    }
                    let row: Option<(String, i64, String)> = tx
                    .query_row(
                        "SELECT company_id, fiscal_year, period_type FROM financial_periods WHERE id = ?1",
                        [live_period],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()?;
                    let Some((company_id, fiscal_year, period_type)) = row else {
                        return Err(period_conflict(
                            run_id,
                            "the run's pinned period row no longer exists",
                        ));
                    };
                    if company_id != sealed.company_id()
                        || fiscal_year != sealed.fiscal_year()
                        || period_type != sealed.period_type()
                    {
                        return Err(period_conflict(
                        run_id,
                        "the run's pinned period row no longer matches the manifest's company/fiscalYear/periodType",
                    ));
                    }
                    live_period.to_owned()
                }
                (Some(_), None) => {
                    return Err(period_conflict(
                        run_id,
                        "the manifest's pinned period was deleted after validation",
                    ))
                }
                (None, Some(_)) => {
                    return Err(period_conflict(
                        run_id,
                        "the run gained a period after validation",
                    ))
                }
                (None, None) => {
                    let new_period_id = kpi_extraction::ensure_period(
                        &tx,
                        sealed.company_id(),
                        sealed.fiscal_year(),
                        sealed.period_type(),
                        None,
                        sealed.report_document_id(),
                    )?;
                    kpi_ingest_runs::attach_period_on_connection(&tx, run_id, &new_period_id)?;
                    new_period_id
                }
            };

            // Step 5: facts, in ordinal order — sorted here rather than assumed
            // (luna PR #376): seal rejects duplicate ordinals but not a permuted
            // stored array, and the receipt ledger must be ordinal-ordered.
            let mut ordered_observations: Vec<_> = manifest.observations.iter().collect();
            ordered_observations.sort_by_key(|o| o.ordinal);
            let mut outcome_entries = Vec::with_capacity(manifest.observations.len());
            let mut accepted_count: i64 = 0;
            for obs in ordered_observations {
                debug_assert!(
                matches!(
                    obs.validation_state,
                    ValidationState::Passed | ValidationState::Unreviewed
                ),
                "a ready manifest never carries a flagged observation (SealedManifest::seal already verified this)"
            );
                let definition_id = obs
                    .definition_id
                    .as_deref()
                    .expect("a null definitionId on a ready manifest was already rejected above");
                let Some(normalized_value) = obs.normalized_value.as_deref() else {
                    return Err(StorageError::CorruptStoredManifest {
                        run: run_id.to_owned(),
                    });
                };
                let validation_status = match obs.validation_state {
                    ValidationState::Passed => "passed",
                    ValidationState::Unreviewed => "unreviewed",
                    ValidationState::Flagged => {
                        return Err(StorageError::CorruptStoredManifest {
                            run: run_id.to_owned(),
                        })
                    }
                };
                let statement_basis = obs.scope.as_deref().unwrap_or_else(|| sealed.scope());
                let citation_json = canonical_citation_json(&obs.citation);

                let commit = kpi_extraction::record_pinned_fact(
                    &tx,
                    PinnedFactInput {
                        run_id,
                        company_id: sealed.company_id(),
                        period_id: &period_id,
                        definition_id,
                        metric_key: &obs.metric_key,
                        value_numeric: normalized_value,
                        currency: obs.currency.as_deref(),
                        statement_basis,
                        attribution: &obs.attribution,
                        measure_window: obs.measure_window.as_deref(),
                        data_quality: sealed.data_quality(),
                        report_document_id: sealed.report_document_id(),
                        validation_status,
                        citation: Some(&citation_json),
                    },
                )?;

                let (fact_id, outcome, detail) = match commit {
                StructuredFactCommit::Created(id) => {
                    accepted_count += 1;
                    (Some(id), "created", None)
                }
                StructuredFactCommit::Reobserved(id) => {
                    accepted_count += 1;
                    (Some(id), "reobserved", None)
                }
                StructuredFactCommit::Upgraded { fact_id, .. } => {
                    accepted_count += 1;
                    (Some(fact_id), "upgraded", None)
                }
                StructuredFactCommit::Divergent { fact_id, .. } => (
                    None,
                    "divergent",
                    Some(json!({ "existingFactId": fact_id })),
                ),
                StructuredFactCommit::NoDefinition => unreachable!(
                    "record_pinned_fact never returns NoDefinition — the definition is validated present before any write"
                ),
            };
                outcome_entries.push(FactOutcomeEntry {
                    observation_id: obs.observation_id.clone(),
                    revision,
                    ordinal: obs.ordinal,
                    metric_key: obs.metric_key.clone(),
                    fact_id,
                    outcome,
                    detail,
                });
            }

            // Step 6: terminal status — a complete reasons ledger (#361
            // construction: `completeness.missing_without_reason` forces
            // `failed`, never reaching this manifest) means any non-empty
            // `missing` is an honest partial commit, never a default.
            let terminal_status = if manifest
                .completeness
                .as_ref()
                .map(|c| !c.missing.is_empty())
                .unwrap_or(false)
            {
                "partial"
            } else {
                "complete"
            };

            // Step 7: the immutable receipt.
            let outcomes_json = serde_json::to_string(&outcome_entries)?;
            let receipt = kpi_ingest_staging::record_commit_receipt(
                &tx,
                NewCommitReceipt {
                    run_id: run_id.to_owned(),
                    manifest_hash: manifest_hash.to_owned(),
                    manifest_revision: revision,
                    terminal_status: terminal_status.to_owned(),
                    period_id: Some(period_id),
                    accepted_count,
                    outcomes_schema_version: 1,
                    outcomes_json,
                },
            )?;

            // Step 8: flip the run to its terminal state.
            kpi_ingest_runs::finalize_committing(&tx, run_id)?;

            // Step 9: commit. Any earlier `?` already rolled back everything via
            // `tx`'s drop.
            tx.commit()?;
            return Ok(receipt);
        }

        // BUSY recovery (#363): the loser of the busy_timeout race releases
        // its guard, then classifies the (possibly just-committed) receipt on
        // a fresh connection; no receipt → an explicitly retryable conflict.
        drop(connection);
        let recovery = self.db.checkout()?;
        if let Some(receipt) = classify_stored_receipt(&recovery, run_id, manifest_hash, revision)?
        {
            return Ok(receipt);
        }
        Err(StorageError::CommitContention {
            run: run_id.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests;
