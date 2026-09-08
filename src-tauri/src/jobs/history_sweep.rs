//! History sweep — durable job + candidate selector (ADR 0077 §3).
//!
//! The history sweep enqueues extraction for every **canonical periodic report
//! whose period lacks accepted facts** — the backfill/manual counterpart to the
//! refresh-time detection sweep, which only ever looks at the newest document per
//! type. [`history_sweep_candidates`] answers "which periods still need
//! extracting?"; [`run_history_sweep_job`] drives each through the shared
//! [`crate::jobs::autopilot::enqueue_extraction_run`] with `trigger='history_sweep'`
//! and records the counted outcome on the sweep row.
//!
//! The selector is a **projection of the coverage read model**
//! ([`compute_fundamentals_coverage`]), never a parallel query, so the sweep and
//! the Coverage panel can never disagree about what is missing: a period the map
//! shows as "has a report, no facts, nothing in review" is exactly a candidate.
//!
//! Trust ladder (ADR 0077 §3 amendment (c)): a sweep runs only for a company in
//! mode `assist` or `autopilot`; mode `off` ends the sweep with an explicit
//! `skipped_reason='automation_off'` and zero enqueues — never a silent skip.

use std::collections::BTreeMap;
use std::io::Read;

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::commands::fundamentals_coverage::{compute_fundamentals_coverage, document_period};
use crate::fundamentals::extraction::classify::DocKind;
use crate::fundamentals::extraction::container::Container;
use crate::jobs::autopilot::{
    enqueue_extraction_run, EnqueueExtractionOutcome, TRIGGER_HISTORY_SWEEP,
};
use crate::report_documents_container::resolved_container;
use crate::storage::{HistorySweep, HistorySweepOutcome, ReportDocument, MODE_OFF};

/// Durable-queue job kind for one history sweep.
pub const HISTORY_SWEEP_KIND: &str = "history_sweep";

/// A sweep is idempotent (`enqueue_extraction_run` dedups), so a single attempt
/// is enough; a storage-level abort marks the sweep row `failed` rather than
/// looping the queue.
const HISTORY_SWEEP_MAX_ATTEMPTS: i64 = 1;

/// Payload for a `history_sweep` job: which sweep row to drive.
#[derive(Debug, Serialize, Deserialize)]
pub struct HistorySweepPayload {
    pub sweep_id: String,
}

/// One period that needs extracting: its canonical report document plus the
/// period the sweep will attribute the run to. Newest period first.
pub(crate) struct HistorySweepCandidate {
    pub document_id: String,
    pub fiscal_year: i64,
    pub period_type: String,
}

/// The periods worth sweeping for `company_id` (ADR 0077 §3), newest first. A
/// period qualifies when its coverage row shows a canonical periodic **report**
/// that is **fetched** (a metadata-only report has no file to extract), with
/// **no accepted facts** (`facts.total == 0`) and **nothing in review**
/// (`review.pending_proposals == 0` — a proposal already in flight is not a gap
/// to re-attack). A projection of the coverage map so the two never drift.
///
/// The document actually attacked is the period's best **extractable** document,
/// not blindly the coverage canonical: the fallback fires whenever the canonical
/// is not extractable by [`document_is_extractable`] — a genuinely dead file
/// (unreadable or zero bytes), or a non-iXBRL markup render (the positional
/// tier is retired, ADR 0095) — and the period's next-best fetched periodic
/// document that IS extractable (a PDF, ZIP, or inline-XBRL `.xhtml`) is
/// chosen instead, preferring ssf over jsf then the newest. This is a
/// **sweep-layer fallback only** — the coverage canonical selection (ADR 0061
/// dec. 1b) is untouched. With no extractable sibling the canonical is still
/// emitted so the gap is enqueued and recorded honestly, never a silent drop.
pub(crate) fn history_sweep_candidates(
    state: &AppState,
    company_id: &str,
) -> Result<Vec<HistorySweepCandidate>, String> {
    let coverage = compute_fundamentals_coverage(state, company_id)?;
    // Every fetched periodic document grouped by its derived period, so a period
    // whose canonical is unextractable can fall back to its best extractable
    // sibling. Loaded once here, not re-queried per candidate.
    let by_period = fetched_periodic_documents_by_period(state, company_id)?;
    // Coverage rows are already newest-period-first; preserving iteration order
    // carries that straight into the candidate list.
    let candidates = coverage
        .periods
        .into_iter()
        .filter_map(|row| {
            let report = row.report?;
            // A metadata-only (link-only) report has no stored file to extract.
            if !report.fetched {
                return None;
            }
            // Already extracted — not a gap. The "review already in flight"
            // half of this gate went with the KPI staging ledger (ADR 0084
            // decision 5): with no proposals table, stored facts are the only
            // signal that a period is already covered.
            if row.facts.total != 0 {
                return None;
            }
            let document_id = select_sweep_document(
                state,
                &by_period,
                (row.fiscal_year, &row.period_type),
                &report.document_id,
            );
            Some(HistorySweepCandidate {
                document_id,
                fiscal_year: row.fiscal_year,
                period_type: row.period_type,
            })
        })
        .collect();
    Ok(candidates)
}

/// The maximum leading bytes read from an XHTML file to sniff inline XBRL: the
/// inline-XBRL namespace and its `ix:` prefix are declared in the root element
/// near the top of the file, so a multi-MB body is never read whole per candidate.
const IXBRL_SNIFF_BYTES: u64 = 64 * 1024;

/// Whether a fetched periodic document could be extracted by SOME tier: an
/// ESEF report package (a ZIP, unpacked to its inner instance), an iXBRL
/// instance (markup carrying `ix:` tags), or a real PDF (deliberately
/// constant-true — see the arm comment). Non-extractable: a document with no
/// stored file, one whose bytes cannot be read (a genuinely dead/empty file),
/// a non-iXBRL markup render (the positional tier is retired, ADR 0095 —
/// enqueueing it would only produce the router's deliberate no-outcome empty
/// result forever), or bytes sniffed as **no** container the pipeline can
/// act on.
///
/// Container truth decides the branch (epic #229 T2): the stored
/// `detected_container` beats the filename, so a `.pdf` holding garbage bytes is
/// honestly *not* extractable instead of "a PDF by construction", and a `.pdf`
/// holding markup takes the readable-markup branch.
///
/// `pub(crate)` so the shared `enqueue_extraction_run` re-arm gate (ADR 0077 §3,
/// 2026-07-10) reuses this exact "could SOME tier read it now?" test instead of
/// re-deriving it — a terminal couldn't-extract run is re-attacked iff the document
/// is now extractable by this same definition.
pub(crate) fn document_is_extractable(state: &AppState, document: &ReportDocument) -> bool {
    let Some(local_path) = document.local_path.as_deref() else {
        return false;
    };
    match resolved_container(document) {
        // An ESEF/eSprawozdanie report package is extractable by construction
        // (the package unpack), with no byte read. A real PDF stays
        // constant-true DELIBERATELY even though machine fact-reading of PDFs
        // is retired (ADR 0086 dec. 1): its run's period grouping survives,
        // and re-arm containment lives elsewhere — the `pdf_document` gap
        // reason never re-arms and the pipeline version gate bounds every
        // other legacy reason (see `jobs::autopilot::terminal_run_should_rearm`).
        Container::Pdf | Container::Zip => true,
        // Markup is extractable ONLY when it is inline-XBRL — judged by the
        // ESEF tier's own instance sniff (`is_inline_xbrl`, the same one the
        // router uses, so routing and extractability can never disagree).
        Container::Xml | Container::Html => {
            read_file_prefix(&state.data_dir().join(local_path), IXBRL_SNIFF_BYTES)
                .map(|prefix| crate::fundamentals::extraction::esef::is_inline_xbrl(&prefix))
                .unwrap_or(false)
        }
        // We read these bytes and recognised no container: no tier can extract
        // them.
        Container::Unknown => false,
    }
}

/// Read up to `limit` leading bytes of a file. An unreadable file surfaces as
/// `Err` at the call site, which treats it as not-extractable.
fn read_file_prefix(path: &std::path::Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    file.take(limit).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Fetched periodic documents grouped by their derived `(fiscal_year,
/// period_type)`, each carrying the doc kind for the ssf-over-jsf sweep tie-break.
type PeriodicDocumentsByPeriod = BTreeMap<(i64, String), Vec<(DocKind, ReportDocument)>>;

/// All fetched periodic (ssf/jsf) documents for a company, grouped by their
/// derived `(fiscal_year, period_type)` and carrying the doc kind for the
/// ssf-over-jsf sweep tie-break. Reuses the coverage read model's period
/// derivation ([`document_period`]) so a period key here matches the coverage
/// row's exactly.
fn fetched_periodic_documents_by_period(
    state: &AppState,
    company_id: &str,
) -> Result<PeriodicDocumentsByPeriod, String> {
    let documents = state
        .list_report_documents_by_company(company_id)
        .map_err(|e| e.to_string())?;
    let mut by_period: PeriodicDocumentsByPeriod = BTreeMap::new();
    for document in documents {
        let kind = match document.doc_kind.as_deref() {
            Some("periodic_ssf") => DocKind::PeriodicSsf,
            Some("periodic_jsf") => DocKind::PeriodicJsf,
            _ => continue,
        };
        if document.fetch_status != "fetched" {
            continue;
        }
        let Some((fiscal_year, period_type, _index)) = document_period(state, &document) else {
            continue;
        };
        by_period
            .entry((fiscal_year, period_type))
            .or_default()
            .push((kind, document));
    }
    Ok(by_period)
}

/// The document the sweep actually attacks for a period. The coverage canonical is
/// kept when extractable ([`document_is_extractable`]); otherwise — a genuinely
/// dead file (unreadable/zero-byte) or a non-iXBRL XHTML (the positional tier
/// is retired, ADR 0095) — the period's best extractable sibling — preferring
/// ssf over jsf, then the newest — is chosen instead. With no extractable
/// sibling the canonical is kept so the gap is still enqueued (the run
/// degrades honestly), never dropped.
fn select_sweep_document(
    state: &AppState,
    by_period: &PeriodicDocumentsByPeriod,
    period: (i64, &str),
    canonical_id: &str,
) -> String {
    let key = (period.0, period.1.to_owned());
    let Some(siblings) = by_period.get(&key) else {
        // The canonical derived to a different period than any grouped document
        // (should not happen) — keep it.
        return canonical_id.to_owned();
    };
    // The canonical is extractable → keep it (the common case).
    match siblings.iter().find(|(_, d)| d.id == canonical_id) {
        Some((_, canonical)) if document_is_extractable(state, canonical) => {
            return canonical_id.to_owned();
        }
        // The canonical is not among this period's grouped documents — keep it.
        None => return canonical_id.to_owned(),
        // Canonical present but unextractable — fall through to sibling selection.
        Some(_) => {}
    }
    siblings
        .iter()
        .filter(|(_, d)| d.id != canonical_id)
        .filter(|(_, d)| document_is_extractable(state, d))
        .min_by(|a, b| sweep_candidate_rank(a).cmp(&sweep_candidate_rank(b)))
        .map(|(_, d)| d.id.clone())
        .unwrap_or_else(|| canonical_id.to_owned())
}

/// Sweep sibling ordering (smaller is better): ssf before jsf, then the newest
/// `created_at` first — mirroring the canonical selection's kind-then-recency
/// preference.
fn sweep_candidate_rank(entry: &(DocKind, ReportDocument)) -> (u8, std::cmp::Reverse<String>) {
    let (kind, document) = entry;
    let kind_rank = match kind {
        DocKind::PeriodicSsf => 0,
        _ => 1,
    };
    (kind_rank, std::cmp::Reverse(document.created_at.clone()))
}

/// Create a queued sweep row and enqueue its durable job, keyed by the sweep id,
/// atomically (issue #458: the pre-fix create-then-enqueue pair ran as two
/// autocommit statements, so a crash/error between them could commit a
/// `queued` sweep with no job ever able to drive it — permanently stranded).
/// Shared by the backfill chain (best-effort) and the manual command, so the two
/// build a sweep identically. Returns the created sweep record.
pub fn enqueue_history_sweep(
    state: &AppState,
    company_id: &str,
    trigger: &str,
) -> Result<HistorySweep, String> {
    state
        .history_sweeps()
        .create_history_sweep_with_job(
            company_id,
            trigger,
            HISTORY_SWEEP_KIND,
            HISTORY_SWEEP_MAX_ATTEMPTS,
            |id| {
                serde_json::to_string(&HistorySweepPayload {
                    sweep_id: id.to_owned(),
                })
                .unwrap_or_else(|_| "{}".to_owned())
            },
        )
        .map_err(|error| error.to_string())
}

/// Run one history sweep (the `history_sweep` handler entry point). Loads the
/// sweep row, applies the trust-ladder gate, enqueues a full autopilot run for
/// every candidate through the shared [`enqueue_extraction_run`], and records the
/// counted outcome. Honest by construction:
/// - a missing sweep row is an `Err` (the queue should not silently succeed);
/// - a company in mode `off` completes the sweep with `skipped_reason='automation_off'`
///   and zero enqueues (ADR 0077 §3 amendment (c));
/// - a storage-level abort listing candidates fails the sweep with the error;
/// - per-candidate outcomes are counted (`runs_enqueued` for Created|Rearmed,
///   `skipped_existing` for DedupedTerminal, `runs_failed` for Failed) and the
///   sweep still `completed`s with `runs_failed > 0` — a partial failure is
///   recorded, not swallowed.
pub fn run_history_sweep_job(state: &AppState, payload: &str) -> Result<(), String> {
    let payload: HistorySweepPayload =
        serde_json::from_str(payload).map_err(|error| error.to_string())?;
    let sweep = state
        .history_sweeps()
        .get_history_sweep(&payload.sweep_id)
        .map_err(|error| error.to_string())?;

    state
        .history_sweeps()
        .mark_history_sweep_running(&sweep.id)
        .map_err(|error| error.to_string())?;

    // Trust-ladder gate (ADR 0077 §3 amendment (c)): a company in mode `off` ends
    // the sweep with an explicit reason, never a silent skip.
    let mode = state
        .autopilot()
        .get_mode(&sweep.company_id)
        .map_err(|error| error.to_string())?;
    if mode == MODE_OFF {
        let outcome = HistorySweepOutcome {
            skipped_reason: Some("automation_off".to_owned()),
            ..Default::default()
        };
        state
            .history_sweeps()
            .complete_history_sweep(&sweep.id, &outcome)
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    // A storage-level failure listing candidates aborts the whole sweep — the
    // sweep cannot honestly report what it did, so it fails with the error.
    let candidates = match history_sweep_candidates(state, &sweep.company_id) {
        Ok(candidates) => candidates,
        Err(error) => {
            let _ = state.history_sweeps().fail_history_sweep(&sweep.id, &error);
            return Err(error);
        }
    };

    let mut outcome = HistorySweepOutcome {
        candidates_total: candidates.len() as i64,
        ..Default::default()
    };
    for candidate in &candidates {
        log::info!(
            "history sweep {}: extracting {} {} (document {})",
            sweep.id,
            candidate.fiscal_year,
            candidate.period_type,
            candidate.document_id
        );
        let run_outcome = enqueue_extraction_run(
            state,
            &sweep.company_id,
            &candidate.document_id,
            TRIGGER_HISTORY_SWEEP,
            &mode,
            // The enqueued run charges this sweep's tier-4 budget (ADR 0077 §6).
            Some(&sweep.id),
        );
        match run_outcome {
            EnqueueExtractionOutcome::Created | EnqueueExtractionOutcome::Rearmed => {
                outcome.runs_enqueued += 1;
                // The run id is deterministic on `(company, document)`
                // (`enqueue_extraction_run`), so sweep progress can query each run's
                // status without a parallel lookup.
                outcome.enqueued_run_ids.push(format!(
                    "autopilot_run:{}:{}",
                    sweep.company_id, candidate.document_id
                ));
            }
            EnqueueExtractionOutcome::DedupedTerminal => outcome.skipped_existing += 1,
            EnqueueExtractionOutcome::Failed => outcome.runs_failed += 1,
        }
    }

    state
        .history_sweeps()
        .complete_history_sweep(&sweep.id, &outcome)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests;
