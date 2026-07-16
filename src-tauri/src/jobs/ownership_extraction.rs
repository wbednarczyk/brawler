//! Ownership extraction job — ingestion stream 1 (v0.56 T3, ADR 0072).
//!
//! Turns the mandatory "shareholders holding ≥5%" table of an already-stored
//! periodic report into `ownership_stakes` snapshots. **Deterministic and final
//! by design** (owner decision 2026-07-16): the T1 deterministic parser
//! ([`parse_shareholders`]) writes stakes directly with `source='report_document'`
//! and provenance to the exact document — no confirmation queue. Only the
//! *residual* it cannot parse (glyph-mangled text layers, image tables, a missing
//! section) is parked in `ownership_extraction_residual` for the later AI/OCR path,
//! whose results ALWAYS require confirmation. Nothing is ever fabricated: a
//! residual writes zero stakes.
//!
//! **Lane.** A deterministic CPU parse chained from document ingestion — the same
//! family as the history sweep, so it drains on the **autopilot** lane (ADR 0059),
//! never a new lane.
//!
//! **Triggers (DoD §C).**
//! - *on-new-report* — [`enqueue_ownership_extraction_catch_up`] runs from
//!   `after_successful_refresh` (the same post-ingest hook the detection sweep
//!   uses), enqueuing every fetched periodic document that still lacks coverage.
//! - *backfill* — [`backfill_company_ownership_extraction`] force-enqueues every
//!   fetched periodic document of one company (UI/T6 + epic backfill).
//! - *startup catch-up* — [`enqueue_ownership_extraction_catch_up`] again, from
//!   app startup, so a cold/persisted-but-stale DB re-arms its gaps.
//!
//! **as_of resolution order** (documented; deterministic and stable across
//! re-runs, so the append-only stake id never churns):
//!   1. the document's linked `financial_periods.period_end_date` (`period_id`);
//!   2. else the document-period derivation ([`derive_report_period`] — the ESEF
//!      iXBRL context end, else the title/URL end-of-period);
//!   3. else the first date at/after the matched shareholders heading in the
//!      extracted section text ("na dzień DD.MM.YYYY").
//!
//! A parse that yields rows but no resolvable date writes nothing (never
//! fabricates a date) — impossible in practice for a periodic report, whose
//! period always derives at step 2.
//!
//! **Idempotence.** [`OwnershipStore::append_snapshot`] upserts by
//! `(company, source, as_of, holder)` and never rewrites `created_at` or the
//! domain key, so re-running extraction for the same document produces an
//! identical, byte-stable history.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::fundamentals::ownership::{
    parse_shareholders, OwnershipParseOutcome, OwnershipParseState,
};
use crate::jobs::structured_extraction::derive_report_period;
use crate::report_diff::extraction::{extract_report, Section, SourceFormat};
use crate::storage::{
    ListFinancialPeriodsInput, NewOwnershipStake, OwnershipExtractionResidual, ReportDocument,
};

/// Durable-queue job kind for one document's ownership extraction.
pub const OWNERSHIP_EXTRACTION_KIND: &str = "ownership_extraction";

/// Deterministic parse — no transient provider failure to ride out, so a single
/// attempt (a storage abort surfaces as `Err` and the queue records it).
const OWNERSHIP_EXTRACTION_MAX_ATTEMPTS: i64 = 1;

/// The stake source tag for report-extracted stakes (ADR 0072 source CHECK set).
const SOURCE_REPORT_DOCUMENT: &str = "report_document";

/// Payload for one `ownership_extraction` job: which document to parse.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipExtractionPayload {
    pub company_id: String,
    pub report_document_id: String,
}

/// Stable per-document job id, so a re-enqueue collapses onto the same row.
fn job_id(document_id: &str) -> String {
    format!("{OWNERSHIP_EXTRACTION_KIND}:{document_id}")
}

/// Enqueue (or re-arm) extraction for one document. `reschedule` keeps a single
/// row per document and re-runs a terminal one — used by the backfill force-pass;
/// the catch-up triggers only pass documents that still lack coverage.
pub fn enqueue_ownership_extraction(state: &AppState, company_id: &str, document_id: &str) {
    let payload = match serde_json::to_string(&OwnershipExtractionPayload {
        company_id: company_id.to_owned(),
        report_document_id: document_id.to_owned(),
    }) {
        Ok(payload) => payload,
        Err(error) => {
            log::warn!("ownership extraction: serialize payload failed for {document_id}: {error}");
            return;
        }
    };
    if let Err(error) = state.jobs().reschedule(
        &job_id(document_id),
        OWNERSHIP_EXTRACTION_KIND,
        &payload,
        OWNERSHIP_EXTRACTION_MAX_ATTEMPTS,
    ) {
        log::warn!("ownership extraction: enqueue failed for {document_id}: {error}");
    }
}

/// Backfill pass: force-enqueue extraction for **every** fetched periodic document
/// of one company (ignores existing coverage — a re-parse is idempotent). Called
/// from the UI/T6 and the epic backfill. Returns how many documents were enqueued.
pub fn backfill_company_ownership_extraction(state: &AppState, company_id: &str) -> usize {
    let documents = match state.list_report_documents_by_company(company_id) {
        Ok(documents) => documents,
        Err(error) => {
            log::warn!("ownership backfill: list documents failed for {company_id}: {error}");
            return 0;
        }
    };
    let mut enqueued = 0usize;
    for document in documents {
        if is_extractable_periodic(&document) {
            enqueue_ownership_extraction(state, company_id, &document.id);
            enqueued += 1;
        }
    }
    enqueued
}

/// Catch-up pass (startup + on-new-report): enqueue extraction for every fetched
/// periodic document that still lacks ownership coverage — no `report_document`
/// stake AND no residual. `company_id = None` scans every company (startup /
/// post-refresh); `Some` narrows to one. Idempotent: a document already parsed or
/// already residual is skipped. Returns how many documents were enqueued.
pub fn enqueue_ownership_extraction_catch_up(state: &AppState, company_id: Option<&str>) -> usize {
    let pending = match state
        .ownership()
        .documents_needing_ownership_extraction(company_id)
    {
        Ok(pending) => pending,
        Err(error) => {
            log::warn!("ownership catch-up: selection failed: {error}");
            return 0;
        }
    };
    for document in &pending {
        enqueue_ownership_extraction(state, &document.company_id, &document.report_document_id);
    }
    pending.len()
}

/// A document eligible for report-extraction: a fetched periodic (ssf/jsf)
/// financial statement with a stored file.
fn is_extractable_periodic(document: &ReportDocument) -> bool {
    document.fetch_status == "fetched"
        && document.local_path.is_some()
        && matches!(
            document.doc_kind.as_deref(),
            Some("periodic_ssf") | Some("periodic_jsf")
        )
}

/// Run one `ownership_extraction` job (the handler entry point). Loads the
/// document, extracts its text, runs the deterministic shareholders parser, and
/// either writes stakes directly (a `Found` parse) or records a residual (a
/// non-Found parse). A storage-level failure returns `Err` so the queue records
/// it; a missing/unfetched document is a logged no-op (`Ok`).
pub fn run_ownership_extraction_job(state: &AppState, payload: &str) -> Result<(), String> {
    let payload: OwnershipExtractionPayload =
        serde_json::from_str(payload).map_err(|error| error.to_string())?;

    let document = match state.get_report_document(&payload.report_document_id) {
        Ok(document) => document,
        Err(error) => {
            // A pruned document is not a queue failure — nothing to extract.
            log::warn!(
                "ownership extraction: document {} not found: {error}",
                payload.report_document_id
            );
            return Ok(());
        }
    };

    let Some(local_path) = document.local_path.as_deref() else {
        log::info!(
            "ownership extraction: document {} has no stored file — skipping",
            document.id
        );
        return Ok(());
    };
    if document.fetch_status != "fetched" {
        log::info!(
            "ownership extraction: document {} is not fetched — skipping",
            document.id
        );
        return Ok(());
    }

    let path = state.data_dir().join(local_path);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            log::warn!(
                "ownership extraction: read {} failed: {error} — skipping",
                path.display()
            );
            return Ok(());
        }
    };

    let format = SourceFormat::resolve(document.content_type.as_deref(), local_path);
    let extracted = extract_report(&bytes, format);
    let outcome = parse_shareholders(&extracted.sections, format);

    persist_outcome(state, &document, &extracted.sections, &outcome)
}

/// Persist a parse outcome: a `Found` parse writes a stake per holder and clears
/// any prior residual; a non-Found parse records a residual and writes no stakes.
fn persist_outcome(
    state: &AppState,
    document: &ReportDocument,
    sections: &[Section],
    outcome: &OwnershipParseOutcome,
) -> Result<(), String> {
    let detected_as_of = resolve_as_of(state, document, sections, outcome);

    // A non-Found parse (or a Found section carrying only aggregate rows and no
    // individual holders) writes NO stakes — it is parked as a residual for the
    // AI/OCR path, never fabricated.
    let residual_state = residual_parse_state(outcome.state).or({
        if outcome.rows.is_empty() {
            Some("table_unparsable")
        } else {
            None
        }
    });
    if let Some(parse_state) = residual_state {
        state
            .ownership()
            .record_extraction_residual(OwnershipExtractionResidual {
                report_document_id: document.id.clone(),
                company_id: document.company_id.clone(),
                parse_state: parse_state.to_owned(),
                detected_as_of,
                matched_heading: outcome.matched_heading.clone(),
                created_at: String::new(),
                updated_at: String::new(),
            })
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    // Found with holder rows: date them and write directly (final, no confirmation).
    let Some(as_of) = detected_as_of else {
        // Rows parsed but no resolvable disclosure date — never invent one. Park as
        // a residual so the gap is visible rather than silently dropped.
        log::warn!(
            "ownership extraction: document {} parsed {} holder row(s) but no as_of resolved — recording residual",
            document.id,
            outcome.rows.len()
        );
        state
            .ownership()
            .record_extraction_residual(OwnershipExtractionResidual {
                report_document_id: document.id.clone(),
                company_id: document.company_id.clone(),
                parse_state: "table_unparsable".to_owned(),
                detected_as_of: None,
                matched_heading: outcome.matched_heading.clone(),
                created_at: String::new(),
                updated_at: String::new(),
            })
            .map_err(|error| error.to_string())?;
        return Ok(());
    };

    for row in &outcome.rows {
        state
            .ownership()
            .append_snapshot(NewOwnershipStake {
                company_id: document.company_id.clone(),
                holder_name_raw: row.holder_raw.clone(),
                // Classification is T5 (dictionary/AI-with-confirm); leave NULL.
                holder_type: None,
                capital_pct: row.capital_pct.clone(),
                votes_pct: row.votes_pct.clone(),
                as_of: as_of.clone(),
                source: SOURCE_REPORT_DOCUMENT.to_owned(),
                report_document_id: Some(document.id.clone()),
                feed_item_id: None,
            })
            .map_err(|error| error.to_string())?;
    }

    // Deterministic classification pass (T5): stamp holder types from the
    // dictionary/heuristics on the rows just written — only NULL types are
    // touched, so manual re-types and confirmed AI proposals stay authoritative.
    state
        .ownership()
        .classify_unclassified_for_company(&document.company_id)
        .map_err(|error| error.to_string())?;

    // Self-heal: a document that now parses must not keep a stale residual from an
    // earlier (weaker) parser version.
    state
        .ownership()
        .clear_extraction_residual(&document.id)
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Resolve the disclosure `as_of` for a report-sourced stake. Documented order:
/// linked period end date → document-period derivation → date near the heading.
fn resolve_as_of(
    state: &AppState,
    document: &ReportDocument,
    sections: &[Section],
    outcome: &OwnershipParseOutcome,
) -> Option<String> {
    // 1. The document's linked period end date (human/period-confirmed).
    if let Some(period_id) = document.period_id.as_deref() {
        if let Ok(periods) = state
            .financials()
            .list_financial_periods(ListFinancialPeriodsInput {
                company_id: document.company_id.clone(),
                fiscal_year: None,
            })
        {
            if let Some(end) = periods
                .iter()
                .find(|period| period.id == period_id)
                .and_then(|period| period.period_end_date.clone())
                .filter(|end| !end.trim().is_empty())
            {
                return Some(end);
            }
        }
    }
    // 2. The document-period derivation (ESEF iXBRL context end, else title/URL).
    if let Some((_, _, period_end)) = derive_report_period(state, document) {
        return Some(period_end);
    }
    // 3. A date at/after the matched shareholders heading in the section text.
    if let Some(heading) = outcome.matched_heading.as_deref() {
        if let Some(date) = date_near_heading(sections, heading) {
            return Some(date);
        }
    }
    None
}

/// Map a non-`Found` parse state to its residual `parse_state` string; `Found`
/// has no residual.
fn residual_parse_state(state: OwnershipParseState) -> Option<&'static str> {
    match state {
        OwnershipParseState::Found => None,
        OwnershipParseState::SectionMissing => Some("section_missing"),
        OwnershipParseState::TableUnparsable => Some("table_unparsable"),
        OwnershipParseState::GlyphEncoded => Some("glyph_encoded"),
    }
}

/// Scan the flattened section text from the matched heading forward for the first
/// ISO-normalizable date (`DD.MM.YYYY`, `DD-MM-YYYY`, or `YYYY-MM-DD`).
fn date_near_heading(sections: &[Section], heading: &str) -> Option<String> {
    let heading_key: String = heading.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut lines: Vec<String> = Vec::new();
    for section in sections {
        if section.heading != "<preamble>" {
            lines.push(section.heading.clone());
        }
        for line in section.body.lines() {
            lines.push(line.to_owned());
        }
    }
    let start = lines
        .iter()
        .position(|line| line.split_whitespace().collect::<Vec<_>>().join(" ") == heading_key)
        .unwrap_or(0);
    // A shareholders as-of date sits on the heading line or a header row a few
    // lines below it; a bounded window keeps a later unrelated date out.
    let end = (start + 40).min(lines.len());
    for line in &lines[start..end] {
        if let Some(date) = first_iso_date(line) {
            return Some(date);
        }
    }
    None
}

/// The first date in `text`, normalized to `YYYY-MM-DD`, or `None`.
fn first_iso_date(text: &str) -> Option<String> {
    if let Some(caps) = iso_date_regex().captures(text) {
        let year = &caps[1];
        let month = &caps[2];
        let day = &caps[3];
        return Some(format!("{year}-{month:0>2}-{day:0>2}"));
    }
    if let Some(caps) = dmy_date_regex().captures(text) {
        let day = &caps[1];
        let month = &caps[2];
        let year = &caps[3];
        return Some(format!("{year}-{month:0>2}-{day:0>2}"));
    }
    None
}

fn iso_date_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(20\d{2})-(\d{1,2})-(\d{1,2})").expect("valid regex"))
}

fn dmy_date_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\d{1,2})[.\-](\d{1,2})[.\-](20\d{2})").expect("valid regex"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::handlers::build_worker;
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, NewCompany,
    };

    fn unique_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("brawler-ownext-{}-{n}", std::process::id()))
    }

    fn state_with_dir() -> AppState {
        let dir = unique_dir();
        std::fs::create_dir_all(&dir).expect("temp dir");
        AppState::with_data_dir(open_in_memory_database().expect("in-memory db"), dir)
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

    /// A realistic 5-column ESEF-style shareholders table (name, share count,
    /// % capital, vote count, % votes) with capital ≠ votes, padded past the
    /// xhtml extractor's minimum so it reaches `Extracted`.
    fn sample_shareholders_xhtml() -> String {
        let filler =
            "Niniejszy raport okresowy zawiera dane porownawcze oraz komentarz. ".repeat(70);
        format!(
            "<html><body>\
             <p>{filler}</p>\
             <h2>Akcjonariusze posiadajacy co najmniej 5% ogolnej liczby glosow na WZ</h2>\
             <table>\
             <tr><th>Akcjonariusz</th><th>Liczba akcji</th><th>% kapitalu</th>\
             <th>Liczba glosow</th><th>% glosow</th></tr>\
             <tr><td>Jan Kowalski</td><td>1 234 567</td><td>12,34</td>\
             <td>2 000 000</td><td>15,00</td></tr>\
             <tr><td>Aviva OFE</td><td>987 654</td><td>9,88</td>\
             <td>987 654</td><td>7,41</td></tr>\
             <tr><td>Pozostali (free float)</td><td>7 777 779</td><td>77,78</td>\
             <td>10 000 000</td><td>77,59</td></tr>\
             </table></body></html>"
        )
    }

    /// An xhtml document with no shareholders section at all (deterministically
    /// `SectionMissing`), still large enough to reach `Extracted`.
    fn no_shareholders_xhtml() -> String {
        let filler = "Sprawozdanie finansowe oraz noty objasniajace do bilansu spolki. ".repeat(70);
        format!("<html><body><h2>Bilans</h2><p>{filler}</p></body></html>")
    }

    /// Create a fetched periodic report backed by a real stored file. Returns the
    /// document id. `file_name` must be unique within the state's data dir.
    fn fetched_periodic_report(
        state: &AppState,
        company_id: &str,
        title: &str,
        url: &str,
        file_name: &str,
        content: &str,
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
        // Guard the sample titles actually classify as periodic (the triggers gate
        // on doc_kind); a broken title would make the selector tests vacuous.
        assert!(
            matches!(
                doc.doc_kind.as_deref(),
                Some("periodic_ssf") | Some("periodic_jsf")
            ),
            "sample title must classify as a periodic report, got {:?}",
            doc.doc_kind
        );
        std::fs::write(state.data_dir().join(file_name), content).expect("write file");
        state
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some(content_type),
                Some("hash"),
                Some(content.len() as i64),
            )
            .expect("mark fetched");
        doc.id
    }

    fn run_job(state: &AppState, company_id: &str, document_id: &str) {
        let payload = serde_json::to_string(&OwnershipExtractionPayload {
            company_id: company_id.to_owned(),
            report_document_id: document_id.to_owned(),
        })
        .expect("payload");
        run_ownership_extraction_job(state, &payload).expect("run job");
    }

    // ---- (1) deterministic pipeline: parsed rows land as stakes directly ----

    #[test]
    fn deterministic_parse_writes_stakes_directly_with_provenance_and_as_of() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "ssf-2025.xhtml",
            &sample_shareholders_xhtml(),
            "application/xhtml+xml",
        );

        run_job(&s, &c, &doc);

        let state = s
            .ownership()
            .current_state_with_free_float(&c)
            .expect("current state");
        assert_eq!(state.stakes.len(), 2, "two holder rows written as stakes");

        let jan = state
            .stakes
            .iter()
            .find(|stake| stake.holder_name_raw == "Jan Kowalski")
            .expect("Jan Kowalski stake");
        assert_eq!(jan.capital_pct.as_deref(), Some("12.34"));
        assert_eq!(
            jan.votes_pct.as_deref(),
            Some("15.00"),
            "votes kept separate"
        );
        assert_eq!(jan.source, SOURCE_REPORT_DOCUMENT);
        assert_eq!(jan.report_document_id.as_deref(), Some(doc.as_str()));
        // Direct-write posture: dated from the document period (FY 2025 → year end),
        // written to the confirmed table with no confirmation queue.
        assert_eq!(jan.as_of, "2025-12-31");
        assert!(
            jan.holder_type.is_none(),
            "unknown holder stays NULL for the AI-with-confirm path"
        );

        // Integration with T5: the deterministic classification pass runs right
        // after extraction, so a dictionary/heuristic-known holder is stamped.
        let aviva = state
            .stakes
            .iter()
            .find(|stake| stake.holder_name_raw == "Aviva OFE")
            .expect("Aviva OFE stake");
        assert_eq!(
            aviva.holder_type.as_deref(),
            Some("ofe_pension"),
            "dictionary/heuristic type stamped on ingest"
        );

        let aviva = state
            .stakes
            .iter()
            .find(|stake| stake.holder_name_raw == "Aviva OFE")
            .expect("Aviva OFE stake");
        assert_eq!(aviva.capital_pct.as_deref(), Some("9.88"));
        assert_eq!(aviva.votes_pct.as_deref(), Some("7.41"));

        // A parsed document is NOT queued for confirmation — no residual.
        assert!(
            s.ownership()
                .get_extraction_residual(&doc)
                .expect("residual")
                .is_none(),
            "a deterministic parse writes stakes directly, never a residual"
        );
    }

    // ---- (2) re-run is idempotent: no history rewrite ----

    #[test]
    fn rerunning_extraction_does_not_rewrite_history() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "ssf-2025.xhtml",
            &sample_shareholders_xhtml(),
            "application/xhtml+xml",
        );

        run_job(&s, &c, &doc);
        let first = s.ownership().history(&c, None).expect("history");
        assert_eq!(first.len(), 2);

        run_job(&s, &c, &doc);
        let second = s.ownership().history(&c, None).expect("history");
        assert_eq!(second.len(), 2, "re-run must not duplicate history");

        // The first rows are byte-identical (append-only upsert never rewrites the
        // domain key or created_at).
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.created_at, b.created_at, "created_at is never rewritten");
            assert_eq!(a.as_of, b.as_of);
            assert_eq!(a.capital_pct, b.capital_pct);
            assert_eq!(a.votes_pct, b.votes_pct);
        }
    }

    // ---- (3) residual path: unparsable → zero stakes + one residual ----

    #[test]
    fn unparsable_document_records_one_residual_and_no_stakes() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/no-holders.xhtml",
            "no-holders.xhtml",
            &no_shareholders_xhtml(),
            "application/xhtml+xml",
        );

        run_job(&s, &c, &doc);

        assert!(
            s.ownership().current_state(&c).expect("state").is_empty(),
            "an unparsable document writes zero stakes (never fabricates)"
        );
        let residual = s
            .ownership()
            .get_extraction_residual(&doc)
            .expect("residual")
            .expect("a residual must be recorded");
        assert_eq!(residual.parse_state, "section_missing");
        assert_eq!(residual.company_id, c);

        // Re-run: still exactly one residual (idempotent upsert).
        run_job(&s, &c, &doc);
        let residuals = s
            .ownership()
            .list_extraction_residuals(&c)
            .expect("residuals");
        assert_eq!(residuals.len(), 1, "residual upserts, never duplicates");
    }

    // ---- (4) dispatch: an enqueued job is claimed and processed ----

    #[test]
    fn enqueued_job_is_dispatched_and_writes_stakes_through_the_queue() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "ssf-2025.xhtml",
            &sample_shareholders_xhtml(),
            "application/xhtml+xml",
        );

        enqueue_ownership_extraction(&s, &c, &doc);
        let processed = build_worker(s.clone())
            .run_until_idle()
            .expect("drain the queue");
        assert!(processed >= 1, "the ownership_extraction job was claimed");

        let counts = s.jobs().counts().expect("counts");
        assert_eq!(
            counts.failed, 0,
            "the handler found and ran (no unknown-kind)"
        );
        assert_eq!(
            s.ownership().current_state(&c).expect("state").len(),
            2,
            "the queued run wrote the stakes"
        );
    }

    // ---- (5) startup catch-up selection ----

    #[test]
    fn startup_catch_up_enqueues_uncovered_documents_only() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "ssf-2025.xhtml",
            &sample_shareholders_xhtml(),
            "application/xhtml+xml",
        );

        // No stakes, no residual → exactly one document enqueued.
        let enqueued = enqueue_ownership_extraction_catch_up(&s, None);
        assert_eq!(enqueued, 1, "one uncovered document is enqueued");
        assert_eq!(s.jobs().counts().expect("counts").pending, 1);

        // Once a residual is recorded, the same document is no longer a gap.
        s.ownership()
            .record_extraction_residual(OwnershipExtractionResidual {
                report_document_id: doc.clone(),
                company_id: c.clone(),
                parse_state: "glyph_encoded".to_owned(),
                detected_as_of: None,
                matched_heading: None,
                created_at: String::new(),
                updated_at: String::new(),
            })
            .expect("record residual");
        assert_eq!(
            enqueue_ownership_extraction_catch_up(&s, None),
            0,
            "a document with a residual is not re-enqueued"
        );
    }

    #[test]
    fn catch_up_skips_a_document_that_already_has_stakes() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "ssf-2025.xhtml",
            &sample_shareholders_xhtml(),
            "application/xhtml+xml",
        );
        run_job(&s, &c, &doc);
        assert_eq!(
            enqueue_ownership_extraction_catch_up(&s, None),
            0,
            "a document already yielding stakes is covered"
        );
    }

    // ---- (7) on-new-report chaining: a landed report is enqueued ----

    #[test]
    fn on_new_report_enqueues_extraction_for_the_landed_document() {
        // The on-new-report hook body is `enqueue_ownership_extraction_catch_up`.
        // A newly fetched periodic report (a report "landing") is enqueued under
        // its stable per-document job id; a non-periodic document is never enqueued.
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "ssf-2025.xhtml",
            &sample_shareholders_xhtml(),
            "application/xhtml+xml",
        );

        // A non-periodic sibling that must be ignored by the periodic gate.
        let non_periodic = s
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: c.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/current-report.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Raport biezacy 12/2025".to_owned()),
                attribution: None,
            })
            .expect("non-periodic document");
        std::fs::write(s.data_dir().join("cr.pdf"), b"%PDF-1.4 x").expect("write");
        s.mark_report_document_fetched(
            &non_periodic.id,
            Some("cr.pdf"),
            Some("application/pdf"),
            Some("h"),
            Some(9),
        )
        .expect("fetch");

        let enqueued = enqueue_ownership_extraction_catch_up(&s, Some(&c));
        assert_eq!(enqueued, 1, "only the periodic report is enqueued");

        // The job targets exactly the landed periodic document.
        let payload = s
            .jobs()
            .pending_payload(&job_id(&doc))
            .expect("pending payload")
            .expect("the landed document has a pending extraction job");
        assert!(
            payload.contains(&doc),
            "payload references the landed document"
        );
    }

    // ---- backfill: force-enqueue every fetched periodic document ----

    #[test]
    fn backfill_force_enqueues_periodic_documents_even_when_already_covered() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "ssf-2025.xhtml",
            &sample_shareholders_xhtml(),
            "application/xhtml+xml",
        );
        run_job(&s, &c, &doc);
        // Already covered → catch-up would skip; backfill re-arms regardless.
        assert_eq!(enqueue_ownership_extraction_catch_up(&s, Some(&c)), 0);
        assert_eq!(
            backfill_company_ownership_extraction(&s, &c),
            1,
            "backfill force-enqueues the periodic document"
        );
        assert_eq!(s.jobs().counts().expect("counts").pending, 1);
    }
}
