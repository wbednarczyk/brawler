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
/// not blindly the coverage canonical (T-A2, amended T-B2): a non-iXBRL XHTML — a
/// pdf2htmlEX render — is now itself extractable via the tier-3b positional parser,
/// so the fallback fires only when the canonical is a **genuinely dead file**
/// (unreadable or zero bytes); the period's next-best fetched periodic document
/// that IS extractable (a PDF, or an iXBRL/positional `.xhtml`/`.xbri`/`.zip`) is
/// chosen instead, preferring ssf over jsf then the newest. This is a **sweep-layer
/// fallback only** — the coverage canonical selection (ADR 0061 dec. 1b) is
/// untouched. With no extractable sibling the canonical is still emitted so the
/// gap is enqueued and recorded honestly, never a silent drop.
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

/// Whether a fetched periodic document could be extracted by SOME tier: a PDF
/// (tier-4-eligible), an ESEF report package (a ZIP, unpacked to its inner
/// instance), an iXBRL instance (markup carrying `ix:` tags), **or** a non-iXBRL
/// XHTML — a pdf2htmlEX render — now that the tier-3b positional parser reads
/// those (ADR 0077 T-B2). Non-extractable: a document with no stored file, one
/// whose bytes cannot be read (a genuinely dead/empty file), or one whose bytes
/// were sniffed and recognised as **no** container the pipeline can act on.
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
        // A real PDF and an ESEF/eSprawozdanie report package are extractable by
        // construction — the PDF tier and the package unpack respectively — with
        // no byte read.
        Container::Pdf | Container::Zip => true,
        // Markup is extractable via ESEF (iXBRL) OR the positional tier
        // (non-iXBRL). Either way, a *readable, non-empty* file is extractable;
        // only an unreadable or zero-byte file (a genuinely dead document) is not
        // — the deliberate T-B2 contract change from T-A2, where a non-iXBRL XHTML
        // was itself treated as not-extractable (see the sibling-fallback tests).
        Container::Xml | Container::Html => {
            read_file_prefix(&state.data_dir().join(local_path), IXBRL_SNIFF_BYTES)
                .map(|prefix| !prefix.is_empty())
                .unwrap_or(false)
        }
        // We read these bytes and recognised no container: no tier can extract
        // them. Before T2 such a file passed as "a PDF by construction" purely
        // because its name ended `.pdf`, and every re-arm handed it back to the
        // PDF reader forever.
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
/// kept when extractable; when it is a genuinely dead file (unreadable/zero-byte —
/// a non-iXBRL XHTML is now extractable via the positional tier, T-B2), the
/// period's best extractable sibling — preferring ssf over jsf, then the newest —
/// is chosen instead. With no extractable sibling the canonical is kept so the gap
/// is still enqueued (the run degrades honestly), never dropped.
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

/// Create a queued sweep row and enqueue its durable job, keyed by the sweep id.
/// Shared by the backfill chain (best-effort) and the manual command, so the two
/// build a sweep identically. Returns the created sweep record.
pub fn enqueue_history_sweep(
    state: &AppState,
    company_id: &str,
    trigger: &str,
) -> Result<HistorySweep, String> {
    let sweep = state
        .history_sweeps()
        .create_history_sweep(company_id, trigger)
        .map_err(|error| error.to_string())?;
    let payload = serde_json::to_string(&HistorySweepPayload {
        sweep_id: sweep.id.clone(),
    })
    .map_err(|error| error.to_string())?;
    state
        .jobs()
        .enqueue(
            &sweep.id,
            HISTORY_SWEEP_KIND,
            &payload,
            HISTORY_SWEEP_MAX_ATTEMPTS,
        )
        .map_err(|error| error.to_string())?;
    Ok(sweep)
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
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, ListAutopilotRunsInput,
        NewCompany, NewFinancialFact, NewFinancialPeriod,
    };

    const NET_PROFIT: &str = "kpidef_net_profit";

    /// Real-data validation for the sweep selector (testing.md: every selector-
    /// like feature is validated against the maintainer's real database before
    /// trusting it). Prints each coverage row next to the selector's verdict so a
    /// human can see exactly which periods the sweep would attack and why the
    /// rest are skipped. Run it manually:
    ///
    /// ```text
    /// BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
    ///   cargo test -p brawler --lib history_sweep_candidates_real_data_validation -- --nocapture --ignored
    /// ```
    #[test]
    #[ignore = "real-data validation; needs BRAWLER_REAL_DB (a throwaway copy)"]
    fn history_sweep_candidates_real_data_validation() {
        let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
            eprintln!("SKIP: set BRAWLER_REAL_DB to a throwaway copy");
            return;
        };
        let company =
            std::env::var("BRAWLER_REAL_COMPANY").unwrap_or_else(|_| "company_gpw_cbf".to_owned());
        let connection = crate::storage::open_database(&db_path).expect("open real db");
        let state = AppState::new(connection);

        let coverage = compute_fundamentals_coverage(&state, &company).expect("coverage");
        let candidates = history_sweep_candidates(&state, &company).expect("candidates");
        let candidate_docs: std::collections::BTreeSet<&str> =
            candidates.iter().map(|c| c.document_id.as_str()).collect();

        eprintln!("== history sweep selector vs coverage: company={company} ==");
        for row in &coverage.periods {
            let (doc, fetched, title) = match &row.report {
                Some(report) => {
                    let title = state
                        .get_report_document(&report.document_id)
                        .ok()
                        .and_then(|d| d.title)
                        .unwrap_or_default();
                    (report.document_id.clone(), report.fetched, title)
                }
                None => (String::new(), false, String::new()),
            };
            let verdict = if candidate_docs.contains(doc.as_str()) {
                "CANDIDATE"
            } else {
                "skip"
            };
            eprintln!(
                "{:>9} | {} {} | facts={} fetched={} | {}",
                verdict, row.fiscal_year, row.period_type, row.facts.total, fetched, title
            );
        }
        eprintln!("candidates: {}", candidates.len());
    }

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Test S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    /// A stored periodic report. `fetched` controls whether it carries a file:
    /// `true` → a fetched PDF; `false` → a link-only metadata-only document.
    fn report(state: &AppState, company_id: &str, title: &str, url: &str, fetched: bool) -> String {
        let doc = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.to_owned(),
                source_type: "user_url".to_owned(),
                url: url.to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some(title.to_owned()),
                attribution: None,
            })
            .expect("report document");
        if fetched {
            state
                .mark_report_document_fetched(
                    &doc.id,
                    Some("reports/x.pdf"),
                    Some("application/pdf"),
                    Some("hash"),
                    Some(1024),
                )
                .expect("mark fetched");
        } else {
            state
                .mark_report_document_metadata_only(&doc.id)
                .expect("mark metadata_only");
        }
        doc.id
    }

    /// Epic #229 T2: `document_is_extractable` is the gate the sweep and the
    /// `enqueue_extraction_run` re-arm share. Before container truth it answered
    /// "extractable by construction" for anything whose name did not end `.xhtml`
    /// — so a `.pdf` holding garbage bytes was re-armed forever and handed to the
    /// PDF reader every time. The sniffed container gives each class its honest
    /// answer.
    #[test]
    fn extractability_follows_the_sniffed_container_not_the_pdf_name() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "brawler-hs-container-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let state = AppState::with_data_dir(open_in_memory_database().expect("db"), dir.clone());
        let company_id = company(&state);

        let seed = |file: &str, bytes: &[u8], container: &str| -> ReportDocument {
            std::fs::write(dir.join(file), bytes).expect("write file");
            let doc = state
                .create_or_find_pending_report_document(CaptureReportDocumentInput {
                    company_id: company_id.clone(),
                    source_type: "user_url".to_owned(),
                    url: format!("https://example.com/{file}"),
                    period_id: None,
                    origin_ref: None,
                    title: Some(format!("Raport {file}")),
                    attribution: None,
                })
                .expect("document");
            state
                .mark_report_document_fetched(
                    &doc.id,
                    Some(file),
                    Some("application/pdf"),
                    Some("hash"),
                    Some(bytes.len() as i64),
                )
                .expect("mark fetched");
            state
                .set_report_document_detected_container(&doc.id, container)
                .expect("stamp container");
            state.get_report_document(&doc.id).expect("reload")
        };

        // Garbage bytes under a `.pdf` name: no tier can read them. This is the
        // class the old name-based gate called extractable-by-construction.
        assert!(
            !document_is_extractable(&state, &seed("junk.pdf", b"\x00\x01\x02junk", "unknown")),
            "bytes we sniffed and did not recognise are NOT extractable"
        );
        // Markup under a `.pdf` name: readable by the ESEF/positional tiers.
        assert!(document_is_extractable(
            &state,
            &seed("markup.pdf", b"<?xml version=\"1.0\"?><html/>", "xml")
        ));
        // A ZIP package under a `.pdf` name: the structured path unpacks it.
        assert!(document_is_extractable(
            &state,
            &seed("package.pdf", b"PK\x03\x04\x14\x00", "zip")
        ));
        // A genuine PDF is unchanged.
        assert!(document_is_extractable(
            &state,
            &seed("real.pdf", b"%PDF-1.7\nbody", "pdf")
        ));
    }

    fn period(state: &AppState, company_id: &str, fiscal_year: i64, period_type: &str) -> String {
        state
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: company_id.to_owned(),
                fiscal_year,
                period_type: period_type.to_owned(),
                period_end_date: None,
                report_evidence_ref: None,
            })
            .expect("period")
            .id
    }

    fn fact(state: &AppState, company_id: &str, period_id: &str) {
        state
            .financials()
            .create_financial_fact(NewFinancialFact {
                company_id: company_id.to_owned(),
                period_id: period_id.to_owned(),
                definition_id: NET_PROFIT.to_owned(),
                value_numeric: "1000".to_owned(),
                currency: Some("PLN".to_owned()),
                statement_basis: None,
                attribution: None,
                variant: None,
                measure_window: None,
                data_quality: None,
                as_reported_value: None,
                as_reported_scale: None,
                reporting_standard: None,
                extraction_method: None,
                confidence: None,
                confirmation_state: None,
                supersedes_id: None,
                source_document_ref: None,
                annotation: None,
            })
            .expect("fact");
    }

    /// Seed a succeeded extraction job with one pending proposal detected at
    /// `(fiscal_year, period_type)`.
    /// (a) A fetched canonical report whose period has no facts and nothing in
    /// review is exactly a sweep candidate.
    #[test]
    fn fetched_report_without_facts_is_a_candidate() {
        let s = state();
        let c = company(&s);
        let doc = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].document_id, doc);
        assert_eq!(candidates[0].fiscal_year, 2025);
        assert_eq!(candidates[0].period_type, "FY");
    }

    /// (b) A period that already carries accepted facts is done, not a candidate.
    #[test]
    fn period_with_facts_is_not_a_candidate() {
        let s = state();
        let c = company(&s);
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        let p = period(&s, &c, 2025, "FY");
        fact(&s, &c, &p);

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        assert!(
            candidates.is_empty(),
            "a period with facts must not be swept"
        );
    }

    /// (d) A metadata-only (link-only) report has no file to extract — not a
    /// candidate even though its period is otherwise empty.
    #[test]
    fn metadata_only_report_is_not_a_candidate() {
        let s = state();
        let c = company(&s);
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://x/emitent/2026-03/ssf-2025.pdf",
            false,
        );

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        assert!(
            candidates.is_empty(),
            "a metadata-only report has no file to extract"
        );
    }

    /// (e) A period with facts but no canonical report cannot be swept — there is
    /// no document to extract from.
    #[test]
    fn facts_only_period_without_report_is_not_a_candidate() {
        let s = state();
        let c = company(&s);
        let p = period(&s, &c, 2024, "FY");
        fact(&s, &c, &p);

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        assert!(
            candidates.is_empty(),
            "a report-less period is not a sweep candidate"
        );
    }

    /// (f) Candidates come back newest-period first (the coverage map's order).
    #[test]
    fn candidates_are_ordered_newest_first() {
        let s = state();
        let c = company(&s);
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2024 SSF",
            "x/ssf-2024.pdf",
            true,
        );
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        let years: Vec<i64> = candidates
            .iter()
            .map(|candidate| candidate.fiscal_year)
            .collect();
        assert_eq!(years, vec![2025, 2024], "newest period first");
    }

    // ---- best-extractable-document fallback (T-A2) -------------------------

    fn unique_sweep_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("brawler-sweep-{}-{n}", std::process::id()))
    }

    /// A state backed by a real on-disk data dir (needed for the non-iXBRL sniff,
    /// which reads a file prefix).
    fn state_with_dir() -> AppState {
        let dir = unique_sweep_dir();
        std::fs::create_dir_all(&dir).expect("temp dir");
        AppState::with_data_dir(open_in_memory_database().expect("in-memory db"), dir)
    }

    /// A fetched periodic report backed by a real stored file. `file_name` must be
    /// unique within the state's data dir. Returns the document id.
    fn fetched_file_report(
        state: &AppState,
        company_id: &str,
        title: &str,
        url: &str,
        file_name: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> String {
        let doc = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.to_owned(),
                source_type: "user_url".to_owned(),
                url: url.to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some(title.to_owned()),
                attribution: None,
            })
            .expect("report document");
        std::fs::write(state.data_dir().join(file_name), bytes).expect("write file");
        state
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some(content_type),
                Some("hash"),
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        doc.id
    }

    /// A pdf2htmlEX render of an interim report — an XHTML with NO `ix:` tags.
    const NON_IXBRL_XHTML: &[u8] =
        b"<html><head><title>Raport</title></head><body><h1>Raport Q3 2024</h1></body></html>";

    /// A valid iXBRL instance (declares the inline-XBRL namespace, carries `ix:`
    /// facts at 2024-09-30).
    const IXBRL_XHTML: &[u8] = br#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2024-09-30</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
    </html>"#;

    /// (a) **T-B2 contract change (deliberate, was T-A2).** A period whose coverage
    /// canonical is a non-iXBRL XHTML (a pdf2htmlEX render) is now KEPT — the tier-3b
    /// positional parser reads it, so it is extractable and the sweep no longer
    /// falls back to a PDF sibling. Under T-A2 this same fixture attacked the PDF;
    /// the positional tier reverses that.
    #[test]
    fn non_ixbrl_xhtml_canonical_is_kept_now_extractable_via_positional_tier() {
        let s = state_with_dir();
        let c = company(&s);
        // ssf beats jsf → the ssf XHTML is the coverage canonical; a pdf2htmlEX
        // render with no `ix:` tags — now extractable via the positional tier.
        let xhtml = fetched_file_report(
            &s,
            &c,
            "Skonsolidowane sprawozdanie finansowe Q3 2024 SSF",
            "https://example.com/ssf_q3_2024.xhtml",
            "canonical.xhtml",
            NON_IXBRL_XHTML,
            "application/xhtml+xml",
        );
        fetched_file_report(
            &s,
            &c,
            "Jednostkowe sprawozdanie finansowe Q3 2024 JSF",
            "https://example.com/jsf_q3_2024.pdf",
            "sibling.pdf",
            b"%PDF-1.4 minimal placeholder",
            "application/pdf",
        );

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        assert_eq!(candidates.len(), 1, "one Q3 2024 period");
        assert_eq!(
            candidates[0].document_id, xhtml,
            "T-B2: the non-iXBRL XHTML canonical is now extractable (positional tier), so it is kept over the PDF sibling"
        );
        assert_eq!(candidates[0].period_type, "Q3");
    }

    /// (a') The sibling fallback survives for a genuinely dead canonical: a
    /// zero-byte (unreadable/empty) XHTML is still not-extractable, so the period
    /// falls back to its extractable PDF sibling. This is the residual T-A2 path
    /// after the T-B2 contract change — only dead files, not non-iXBRL renders.
    #[test]
    fn empty_xhtml_canonical_falls_back_to_extractable_pdf_sibling() {
        let s = state_with_dir();
        let c = company(&s);
        fetched_file_report(
            &s,
            &c,
            "Skonsolidowane sprawozdanie finansowe Q3 2024 SSF",
            "https://example.com/ssf_q3_2024.xhtml",
            "canonical.xhtml",
            b"",
            "application/xhtml+xml",
        );
        let pdf = fetched_file_report(
            &s,
            &c,
            "Jednostkowe sprawozdanie finansowe Q3 2024 JSF",
            "https://example.com/jsf_q3_2024.pdf",
            "sibling.pdf",
            b"%PDF-1.4 minimal placeholder",
            "application/pdf",
        );

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        assert_eq!(candidates.len(), 1, "one Q3 2024 period");
        assert_eq!(
            candidates[0].document_id, pdf,
            "a zero-byte XHTML canonical is dead → the sweep falls back to the extractable PDF sibling"
        );
    }

    /// (b) A period whose canonical is a valid iXBRL instance keeps the canonical.
    #[test]
    fn ixbrl_xhtml_canonical_is_kept() {
        let s = state_with_dir();
        let c = company(&s);
        let xhtml = fetched_file_report(
            &s,
            &c,
            "Skonsolidowane sprawozdanie finansowe 2024 SSF",
            "https://example.com/ssf_2024.xhtml",
            "canonical.xhtml",
            IXBRL_XHTML,
            "application/xhtml+xml",
        );

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].document_id, xhtml,
            "a valid iXBRL instance stays the sweep candidate"
        );
    }

    /// (c) A period with ONLY a non-iXBRL XHTML emits it — now extractable via the
    /// positional tier (T-B2), so the gap is enqueued and read deterministically
    /// rather than degrading `not_pdf`.
    #[test]
    fn non_ixbrl_xhtml_only_period_still_emits_the_xhtml() {
        let s = state_with_dir();
        let c = company(&s);
        let xhtml = fetched_file_report(
            &s,
            &c,
            "Skonsolidowane sprawozdanie finansowe Q3 2024 SSF",
            "https://example.com/ssf_q3_2024.xhtml",
            "canonical.xhtml",
            NON_IXBRL_XHTML,
            "application/xhtml+xml",
        );

        let candidates = history_sweep_candidates(&s, &c).expect("candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].document_id, xhtml,
            "with no extractable sibling the gap is still enqueued, never silently dropped"
        );
    }

    // ---- run_history_sweep_job ---------------------------------------------

    /// Count autopilot runs for a company, and how many carry a given trigger.
    fn run_counts(state: &AppState, company_id: &str, trigger: &str) -> (i64, i64) {
        let runs = state
            .autopilot()
            .list_runs(&ListAutopilotRunsInput {
                company_id: Some(company_id.to_owned()),
                limit: Some(500),
                ..Default::default()
            })
            .expect("list runs");
        let total = runs.len() as i64;
        let with_trigger = runs.iter().filter(|run| run.trigger == trigger).count() as i64;
        (total, with_trigger)
    }

    fn run_sweep(
        state: &AppState,
        company_id: &str,
        trigger: &str,
    ) -> crate::storage::HistorySweep {
        let sweep = state
            .history_sweeps()
            .create_history_sweep(company_id, trigger)
            .expect("create sweep");
        let payload = serde_json::to_string(&HistorySweepPayload {
            sweep_id: sweep.id.clone(),
        })
        .expect("payload");
        run_history_sweep_job(state, &payload).expect("run sweep");
        state
            .history_sweeps()
            .get_history_sweep(&sweep.id)
            .expect("reload sweep")
    }

    /// A company in mode `off` completes the sweep with `automation_off` and
    /// enqueues nothing (ADR 0077 §3 amendment (c) — never a silent skip).
    #[test]
    fn off_mode_completes_with_skipped_reason_and_no_runs() {
        let s = state();
        let c = company(&s);
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        // Mode defaults to `off` (no settings row).

        let sweep = run_sweep(&s, &c, "manual");
        assert_eq!(sweep.status, "completed");
        assert_eq!(sweep.skipped_reason.as_deref(), Some("automation_off"));
        assert_eq!(sweep.runs_enqueued, 0);
        assert!(sweep.enqueued_run_ids.is_empty());

        let (total, _) = run_counts(&s, &c, HISTORY_SWEEP_KIND);
        assert_eq!(total, 0, "off-mode must enqueue no runs");
    }

    /// Assist mode with two candidates enqueues two `history_sweep`-triggered
    /// runs and records the counters + run ids.
    #[test]
    fn assist_mode_enqueues_a_run_per_candidate() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2024 SSF",
            "x/ssf-2024.pdf",
            true,
        );
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );

        let sweep = run_sweep(&s, &c, "backfill");
        assert_eq!(sweep.status, "completed");
        assert_eq!(sweep.candidates_total, 2);
        assert_eq!(sweep.runs_enqueued, 2);
        assert_eq!(sweep.skipped_existing, 0);
        assert_eq!(sweep.runs_failed, 0);
        assert_eq!(sweep.enqueued_run_ids.len(), 2);
        assert!(sweep.skipped_reason.is_none());

        let (total, with_trigger) = run_counts(&s, &c, HISTORY_SWEEP_KIND);
        assert_eq!(total, 2);
        assert_eq!(
            with_trigger, 2,
            "each enqueued run carries trigger='history_sweep'"
        );
    }

    /// A second sweep over the same still-empty state re-arms the existing runs
    /// (honest: rearmed, not double-created) — no new autopilot_run rows.
    #[test]
    fn second_sweep_rearms_without_double_creating_runs() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );

        let first = run_sweep(&s, &c, "backfill");
        assert_eq!(first.runs_enqueued, 1);
        let (after_first, _) = run_counts(&s, &c, HISTORY_SWEEP_KIND);
        assert_eq!(after_first, 1);

        let second = run_sweep(&s, &c, "manual");
        // The pending run is still a gap (no facts yet), so it is re-armed.
        assert_eq!(second.runs_enqueued, 1);
        assert_eq!(second.skipped_existing, 0);
        let (after_second, _) = run_counts(&s, &c, HISTORY_SWEEP_KIND);
        assert_eq!(
            after_second, 1,
            "re-arming must not create a second run for the same (company, document)"
        );
    }

    /// A candidate whose (company, document) already has a terminal run is
    /// deduped, not re-created — counted as `skipped_existing`.
    #[test]
    fn candidate_with_terminal_run_is_skipped_existing() {
        let s = state();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        let doc = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );

        // Seed a terminal (succeeded) run that produced no facts, so the period is
        // still a candidate but the run dedups. The run id is the same
        // deterministic `autopilot_run:{company}:{document}` the sweep will build.
        let run_id = format!("autopilot_run:{c}:{doc}");
        s.autopilot()
            .create_run_if_absent(&run_id, &c, &doc, "detection", "assist", None)
            .expect("seed run");
        s.autopilot()
            .finalize_run(&run_id, "succeeded", "notify", None, None)
            .expect("finalize run");

        let sweep = run_sweep(&s, &c, "manual");
        assert_eq!(sweep.status, "completed");
        assert_eq!(sweep.candidates_total, 1);
        assert_eq!(sweep.runs_enqueued, 0);
        assert_eq!(sweep.skipped_existing, 1);
        assert!(sweep.enqueued_run_ids.is_empty());
    }

    // ---- terminal-run re-arm on capability upgrade (ADR 0077 §3, 2026-07-10) ----

    /// Seed a TERMINAL succeeded `history_sweep` run for `(company, document)` that
    /// recorded `extractionAvailable:false` with `reason` — exactly the shape an
    /// earlier pipeline version left for a document it concluded it could not
    /// extract. The run id is the deterministic `autopilot_run:{company}:{document}`
    /// the sweep rebuilds.
    fn seed_terminal_unavailable_run(
        state: &AppState,
        company_id: &str,
        document_id: &str,
        reason: &str,
    ) {
        let run_id = format!("autopilot_run:{company_id}:{document_id}");
        state
            .autopilot()
            .create_run_if_absent(
                &run_id,
                company_id,
                document_id,
                TRIGGER_HISTORY_SWEEP,
                "assist",
                None,
            )
            .expect("seed run")
            .expect("run created");
        state
            .autopilot()
            .set_kpi_delta_json(
                &run_id,
                &format!("{{\"extractionAvailable\":false,\"reason\":\"{reason}\"}}"),
            )
            .expect("set delta");
        state
            .autopilot()
            .finalize_run(&run_id, "succeeded", "notify", None, None)
            .expect("finalize run");
    }

    /// (a) A document with a TERMINAL succeeded run whose delta says
    /// `not_extractable`, that IS now extractable, is RE-ARMED and re-executed — the
    /// capability upgrade (the tier-3b positional tier) reaches a document a prior
    /// pipeline version concluded it could not read. Today this reddens: the run is
    /// counted `skipped_existing` and its stale `not_extractable` delta stands.
    #[test]
    fn terminal_not_extractable_run_is_rearmed_when_document_now_extractable() {
        let s = state_with_dir();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        // A valid iXBRL instance — extractable, and determinism emits from it.
        let doc = fetched_file_report(
            &s,
            &c,
            "Skonsolidowane sprawozdanie finansowe Q3 2024 SSF",
            "https://example.com/ssf_q3_2024.xhtml",
            "canonical.xhtml",
            IXBRL_XHTML,
            "application/xhtml+xml",
        );
        seed_terminal_unavailable_run(&s, &c, &doc, "not_extractable");

        // Drive the whole sweep end-to-end through the durable queue so the re-armed
        // run actually re-executes and replaces its delta.
        let sweep = enqueue_history_sweep(&s, &c, "manual").expect("enqueue sweep");
        crate::jobs::handlers::build_worker(s.clone())
            .run_until_idle()
            .expect("drain the queue");

        let sweep = s
            .history_sweeps()
            .get_history_sweep(&sweep.id)
            .expect("reload sweep");
        assert_eq!(
            sweep.runs_enqueued, 1,
            "a now-extractable not_extractable run is re-armed, not skipped"
        );
        assert_eq!(sweep.skipped_existing, 0);

        let run_id = format!("autopilot_run:{c}:{doc}");
        let after = s.autopilot().get_run(&run_id).expect("get run");
        let delta = after.kpi_delta_json.expect("kpi delta recorded");
        assert!(
            delta.contains("\"extractionAvailable\":true"),
            "the re-run emitted; the stale not_extractable delta must be replaced: {delta}"
        );
        assert!(
            !after.produced_fact_ids.is_empty(),
            "the re-run produced facts"
        );
    }

    /// (b) A genuinely dead file (zero-byte XHTML — still not extractable) with a
    /// terminal `not_extractable` run STAYS deduped: no re-arm loop that re-parses a
    /// dead document every sweep.
    #[test]
    fn terminal_not_extractable_run_with_dead_file_stays_deduped() {
        let s = state_with_dir();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        let doc = fetched_file_report(
            &s,
            &c,
            "Skonsolidowane sprawozdanie finansowe Q3 2024 SSF",
            "https://example.com/ssf_q3_2024.xhtml",
            "dead.xhtml",
            b"",
            "application/xhtml+xml",
        );
        seed_terminal_unavailable_run(&s, &c, &doc, "not_extractable");

        let sweep = run_sweep(&s, &c, "manual");
        assert_eq!(sweep.candidates_total, 1);
        assert_eq!(
            sweep.skipped_existing, 1,
            "a still-unextractable dead file stays deduped — no re-arm"
        );
        assert_eq!(sweep.runs_enqueued, 0);
    }

    /// (c) skipped_budget decision (ADR 0077 §3, 2026-07-10): a budget-skipped
    /// terminal run is RE-ARMED on a fresh sweep — a new sweep carries fresh budget,
    /// so a period the last sweep skipped for budget must be retried, never left
    /// permanently blind. Gated on extractability like every re-arm (a PDF is
    /// extractable by construction here).
    #[test]
    fn terminal_skipped_budget_run_is_rearmed_on_a_fresh_sweep() {
        let s = state_with_dir();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        let doc = fetched_file_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.pdf",
            "r2025.pdf",
            b"%PDF-1.4 minimal placeholder",
            "application/pdf",
        );
        seed_terminal_unavailable_run(&s, &c, &doc, "skipped_budget");

        let sweep = run_sweep(&s, &c, "manual");
        assert_eq!(sweep.candidates_total, 1);
        assert_eq!(
            sweep.runs_enqueued, 1,
            "a budget-skipped period is retried on a fresh sweep, not permanently skipped"
        );
        assert_eq!(sweep.skipped_existing, 0);
    }

    /// (d) A terminal succeeded run that EMITTED facts (`extractionAvailable:true`)
    /// is NEVER re-armed — the dedup that stops finished reports from being
    /// re-extracted must survive the re-arm rule.
    #[test]
    fn terminal_run_with_emitted_facts_is_never_rearmed() {
        let s = state_with_dir();
        let c = company(&s);
        s.autopilot().set_mode(&c, "assist").expect("set mode");
        let doc = fetched_file_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.pdf",
            "r2025.pdf",
            b"%PDF-1.4 minimal placeholder",
            "application/pdf",
        );
        let run_id = format!("autopilot_run:{c}:{doc}");
        s.autopilot()
            .create_run_if_absent(&run_id, &c, &doc, TRIGGER_HISTORY_SWEEP, "assist", None)
            .expect("seed run")
            .expect("run created");
        s.autopilot()
            .set_kpi_delta_json(
                &run_id,
                "{\"extractionAvailable\":true,\"factsProposed\":3,\"factsAutoConfirmed\":3}",
            )
            .expect("set delta");
        s.autopilot()
            .add_produced_facts(&run_id, &["fact_x".to_owned()])
            .expect("record facts");
        s.autopilot()
            .finalize_run(&run_id, "succeeded", "notify", None, None)
            .expect("finalize run");

        let sweep = run_sweep(&s, &c, "manual");
        assert_eq!(sweep.candidates_total, 1);
        assert_eq!(
            sweep.skipped_existing, 1,
            "a run that emitted facts is never re-armed"
        );
        assert_eq!(sweep.runs_enqueued, 0);
    }
}
