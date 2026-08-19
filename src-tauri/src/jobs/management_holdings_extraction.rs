//! Management-holdings extraction job — the insider-substrate sibling of the
//! ownership extraction stream (v0.57 T5, ADR 0083 Decision 6, card `9730f5f`).
//!
//! Turns the mandatory "Zestawienie stanu posiadania akcji … przez osoby
//! zarządzające i nadzorujące" section of an already-stored periodic report into
//! `management_holdings` rows, then stamps `founder_insider` on matching
//! `ownership_stakes` (person-name or `indirect_via` vehicle bridge). Deterministic
//! and final by design (mirrors ownership extraction): the parser writes rows
//! directly; only what it cannot read (glyph-mangled text, image tables, a missing
//! section) parks in `management_holdings_residual` for the tier-4 OCR path.
//! Nothing is ever fabricated — a residual writes zero rows.
//!
//! **Lane.** A deterministic CPU parse chained from document ingestion — the same
//! autopilot lane as ownership extraction and the history sweep (ADR 0059).
//!
//! **Triggers (DoD §C).**
//! - *on-new-report* — [`enqueue_management_extraction_catch_up`] from
//!   `after_successful_refresh`, enqueuing every fetched periodic document that
//!   still lacks coverage.
//! - *startup catch-up* — the same, from app startup.
//! - *backfill* — [`backfill_company_management_extraction`] force-enqueues every
//!   fetched periodic document of one company.
//!
//! **VRC / glyph class.** A glyph-encoded xhtml (a pdf2htmlEX container whose
//! digits are custom-font PUA) first attempts its **PDF sibling** — a fetched
//! periodic PDF of the same company and period — under the Rust `pdf-extract`
//! tier before parking a glyph residual (plan v0.57 T5, VRC resolution note).
//!
//! **Idempotence.** [`ManagementHoldingsStore::upsert_holding`] upserts by
//! `(report_document_id, person_normalized)`; re-running produces an identical set.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::fundamentals::management_holdings::{
    parse_management_holdings, MgmtHoldingsOutcome, MgmtHoldingsState, MgmtRole,
};
use crate::jobs::structured_extraction::derive_report_period;
use crate::report_diff::extraction::{extract_report, Section, SourceFormat};
use crate::storage::{
    ListFinancialPeriodsInput, ManagementHoldingsResidual, NewManagementHolding, ReportDocument,
};

/// Durable-queue job kind for one document's management-holdings extraction.
pub const MANAGEMENT_EXTRACTION_KIND: &str = "management_holdings_extraction";

/// Deterministic parse — a single attempt (a storage abort surfaces as `Err`).
const MANAGEMENT_EXTRACTION_MAX_ATTEMPTS: i64 = 1;

/// Payload for one job: which document to parse.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementExtractionPayload {
    pub company_id: String,
    pub report_document_id: String,
}

fn job_id(document_id: &str) -> String {
    format!("{MANAGEMENT_EXTRACTION_KIND}:{document_id}")
}

/// Enqueue (or re-arm) extraction for one document.
pub fn enqueue_management_extraction(state: &AppState, company_id: &str, document_id: &str) {
    let payload = match serde_json::to_string(&ManagementExtractionPayload {
        company_id: company_id.to_owned(),
        report_document_id: document_id.to_owned(),
    }) {
        Ok(payload) => payload,
        Err(error) => {
            log::warn!(
                "management extraction: serialize payload failed for {document_id}: {error}"
            );
            return;
        }
    };
    if let Err(error) = state.jobs().reschedule(
        &job_id(document_id),
        MANAGEMENT_EXTRACTION_KIND,
        &payload,
        MANAGEMENT_EXTRACTION_MAX_ATTEMPTS,
    ) {
        log::warn!("management extraction: enqueue failed for {document_id}: {error}");
    }
}

/// Backfill: force-enqueue extraction for every fetched periodic document of one
/// company (idempotent re-parse). Returns how many documents were enqueued.
pub fn backfill_company_management_extraction(state: &AppState, company_id: &str) -> usize {
    let documents = match state.list_report_documents_by_company(company_id) {
        Ok(documents) => documents,
        Err(error) => {
            log::warn!("management backfill: list documents failed for {company_id}: {error}");
            return 0;
        }
    };
    let mut enqueued = 0usize;
    for document in documents {
        if is_extractable_periodic(&document) {
            enqueue_management_extraction(state, company_id, &document.id);
            enqueued += 1;
        }
    }
    enqueued
}

/// Catch-up (startup + on-new-report): enqueue every fetched periodic document
/// still lacking management-holdings coverage. Returns how many were enqueued.
pub fn enqueue_management_extraction_catch_up(state: &AppState, company_id: Option<&str>) -> usize {
    let pending = match state
        .management_holdings()
        .documents_needing_management_extraction(company_id)
    {
        Ok(pending) => pending,
        Err(error) => {
            log::warn!("management catch-up: selection failed: {error}");
            return 0;
        }
    };
    for document in &pending {
        enqueue_management_extraction(state, &document.company_id, &document.report_document_id);
    }
    pending.len()
}

fn is_extractable_periodic(document: &ReportDocument) -> bool {
    if document.fetch_status != "fetched" || document.local_path.is_none() {
        return false;
    }
    // Periodic financial statements (the combined QSr bundles carry the holdings
    // table inline) OR the management activity report (SzD), whose holdings table
    // is a `doc_kind='other'` document (F-A3): KRU discloses management holdings
    // ONLY in its SzD, so a periodic-only selection produced zero rows.
    matches!(
        document.doc_kind.as_deref(),
        Some("periodic_ssf") | Some("periodic_jsf")
    ) || crate::fundamentals::extraction::classify::is_management_report(
        document.title.as_deref().unwrap_or(""),
        &document.url,
    )
}

/// Run one extraction job (the handler entry point).
pub fn run_management_extraction_job(state: &AppState, payload: &str) -> Result<(), String> {
    let payload: ManagementExtractionPayload =
        serde_json::from_str(payload).map_err(|error| error.to_string())?;

    let document = match state.get_report_document(&payload.report_document_id) {
        Ok(document) => document,
        Err(error) => {
            log::warn!(
                "management extraction: document {} not found: {error}",
                payload.report_document_id
            );
            return Ok(());
        }
    };

    let Some(bytes) = read_document(state, &document) else {
        return Ok(());
    };

    // Container truth (epic #229 T2) — see the ownership tier: markup under a
    // `.pdf` name is parsed as markup; a ZIP package / unrecognised container has
    // no text layer, so it is skipped honestly rather than parsed as a PDF.
    let Some(format) = crate::report_documents_container::resolved_source_format(&document) else {
        log::warn!(
            "management extraction: document {} is a {} container, not readable text — skipping",
            document.id,
            crate::report_documents_container::resolved_container(&document).as_str()
        );
        return Ok(());
    };
    let extracted = extract_report(&bytes, format);
    let outcome = parse_management_holdings(&extracted.sections, format);

    persist_outcome(state, &document, &extracted.sections, &outcome)
}

/// Read a fetched document's bytes; `None` (a logged no-op) for a pruned/unfetched
/// document or an unreadable file.
fn read_document(state: &AppState, document: &ReportDocument) -> Option<Vec<u8>> {
    let local_path = document.local_path.as_deref()?;
    if document.fetch_status != "fetched" {
        return None;
    }
    let path = state.data_dir().join(local_path);
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            log::warn!(
                "management extraction: read {} failed: {error}",
                path.display()
            );
            None
        }
    }
}

fn persist_outcome(
    state: &AppState,
    document: &ReportDocument,
    sections: &[Section],
    outcome: &MgmtHoldingsOutcome,
) -> Result<(), String> {
    let detected_as_of = resolve_as_of(state, document, sections, outcome);

    match outcome.state {
        MgmtHoldingsState::Parsed => {
            write_rows(state, document, outcome, detected_as_of.as_deref())?;
        }
        MgmtHoldingsState::ZeroHoldingAggregate => {
            let as_of = detected_as_of.clone().unwrap_or_default();
            let entries: Vec<(Option<MgmtRole>, String)> = outcome
                .zero_organs
                .iter()
                .map(|zero| (zero.role, as_of.clone()))
                .collect();
            state
                .management_holdings()
                .upsert_zero_aggregates(&document.company_id, &document.id, &entries)
                .map_err(|error| error.to_string())?;
            finalize(state, document)?;
        }
        MgmtHoldingsState::GlyphEncoded => {
            // VRC class: try the PDF sibling before parking a glyph residual.
            if try_pdf_sibling(state, document, detected_as_of.as_deref())? {
                return Ok(());
            }
            record_residual(state, document, "glyph_encoded", detected_as_of, outcome)?;
        }
        MgmtHoldingsState::SectionMissing => {
            let parse_state = if outcome.matched_heading.is_some() {
                "table_unparsable"
            } else {
                "section_missing"
            };
            record_residual(state, document, parse_state, detected_as_of, outcome)?;
        }
    }
    Ok(())
}

/// Write parsed by-person rows for `document`, then stamp founders + self-heal.
fn write_rows(
    state: &AppState,
    document: &ReportDocument,
    outcome: &MgmtHoldingsOutcome,
    as_of: Option<&str>,
) -> Result<(), String> {
    let as_of = as_of.unwrap_or("").to_owned();
    // Batched (#404 H2): one checkout + one IMMEDIATE tx for the whole document
    // instead of one autocommit fsync per row (see
    // `ManagementHoldingsStore::upsert_holdings`).
    let holdings: Vec<NewManagementHolding> = outcome
        .rows
        .iter()
        .map(|row| NewManagementHolding {
            company_id: document.company_id.clone(),
            report_document_id: document.id.clone(),
            person_name_raw: row.person_raw.clone(),
            role: row.role.map(|r| r.as_str().to_owned()),
            shares: row.shares.clone(),
            indirect_via_raw: row.indirect_via_raw.clone(),
            prior_shares: row.prior_shares.clone(),
            prior_as_of: row.prior_as_of.clone(),
            as_of: as_of.clone(),
        })
        .collect();
    state
        .management_holdings()
        .upsert_holdings(&holdings)
        .map_err(|error| error.to_string())?;
    finalize(state, document)
}

/// After a successful parse: stamp founder/insiders and clear any stale residual.
fn finalize(state: &AppState, document: &ReportDocument) -> Result<(), String> {
    state
        .management_holdings()
        .stamp_founder_insiders(&document.company_id)
        .map_err(|error| error.to_string())?;
    state
        .management_holdings()
        .clear_residual(&document.id)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn record_residual(
    state: &AppState,
    document: &ReportDocument,
    parse_state: &str,
    detected_as_of: Option<String>,
    outcome: &MgmtHoldingsOutcome,
) -> Result<(), String> {
    state
        .management_holdings()
        .record_residual(ManagementHoldingsResidual {
            report_document_id: document.id.clone(),
            company_id: document.company_id.clone(),
            parse_state: parse_state.to_owned(),
            detected_as_of,
            matched_heading: outcome.matched_heading.clone(),
            created_at: String::new(),
            updated_at: String::new(),
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Attempt the PDF sibling of a glyph-encoded xhtml: a fetched periodic PDF of the
/// same company and derived period whose text layer the deterministic parser can
/// read. On success, the rows are written with provenance to the *original*
/// document (so its coverage gap closes) and `true` is returned; otherwise `false`
/// (the caller parks the glyph residual).
fn try_pdf_sibling(
    state: &AppState,
    document: &ReportDocument,
    as_of: Option<&str>,
) -> Result<bool, String> {
    // Shared sibling rule (T8 refactor): a fetched periodic PDF of the same
    // company + derived period. `None` when the document is not xhtml or no such
    // PDF exists — the caller then parks the glyph residual.
    let Some(sibling) = crate::jobs::structured_extraction::find_pdf_sibling(state, document)
    else {
        return Ok(false);
    };
    let Some(bytes) = read_document(state, &sibling) else {
        return Ok(false);
    };
    let extracted = extract_report(&bytes, SourceFormat::Pdf);
    let outcome = parse_management_holdings(&extracted.sections, SourceFormat::Pdf);
    if outcome.state == MgmtHoldingsState::Parsed && !outcome.rows.is_empty() {
        log::info!(
            "management extraction: glyph xhtml {} resolved via PDF sibling {}",
            document.id,
            sibling.id
        );
        write_rows(state, document, &outcome, as_of)?;
        return Ok(true);
    }
    Ok(false)
}

/// Resolve the disclosure `as_of`: an explicit "na dzień <date>" on/near the
/// matched heading (numeric or Polish month) FIRST, else the document's linked
/// period-end, else the derived report period end (data-model § Company Health).
fn resolve_as_of(
    state: &AppState,
    document: &ReportDocument,
    sections: &[Section],
    outcome: &MgmtHoldingsOutcome,
) -> Option<String> {
    if let Some(heading) = outcome.matched_heading.as_deref() {
        if let Some(date) = date_near_heading(sections, heading) {
            return Some(date);
        }
    }
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
    derive_report_period(state, document).map(|(_, _, end)| end)
}

/// Scan from the matched heading forward for the first "na dzień"-style date
/// (numeric `DD.MM.YYYY` / `YYYY-MM-DD`, or a Polish textual "1 stycznia 2025").
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
    let end = (start + 40).min(lines.len());
    for line in &lines[start..end] {
        if let Some(date) = first_date(line) {
            return Some(date);
        }
    }
    None
}

fn first_date(text: &str) -> Option<String> {
    if let Some(caps) = iso_date_regex().captures(text) {
        return Some(format!("{}-{:0>2}-{:0>2}", &caps[1], &caps[2], &caps[3]));
    }
    if let Some(caps) = dmy_date_regex().captures(text) {
        return Some(format!("{}-{:0>2}-{:0>2}", &caps[3], &caps[2], &caps[1]));
    }
    if let Some(caps) = polish_date_regex().captures(text) {
        let month = polish_month(&caps[2])?;
        return Some(format!("{}-{month:0>2}-{:0>2}", &caps[3], &caps[1]));
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

fn polish_date_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(\d{1,2})\s+([a-ząćęłńóśźż]+)\s+(20\d{2})").expect("valid regex")
    })
}

/// Map a Polish month name (nominative or genitive) to its number.
fn polish_month(word: &str) -> Option<&'static str> {
    let w = word.to_lowercase();
    let month = match w.as_str() {
        m if m.starts_with("stycz") => "1",
        m if m.starts_with("lut") => "2",
        m if m.starts_with("mar") => "3",
        m if m.starts_with("kwie") => "4",
        m if m.starts_with("maj") => "5",
        m if m.starts_with("czerw") => "6",
        m if m.starts_with("lip") => "7",
        m if m.starts_with("sierp") => "8",
        m if m.starts_with("wrze") => "9",
        m if m.starts_with("paźdz") || m.starts_with("pazdz") => "10",
        m if m.starts_with("listop") => "11",
        m if m.starts_with("grud") => "12",
        _ => return None,
    };
    Some(month)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fundamentals::management_holdings::MgmtHoldingRow;
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, NewCompany,
        NewOwnershipStake,
    };

    fn unique_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("brawler-mgmtext-{}-{n}", std::process::id()))
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

    fn holdings_xhtml() -> String {
        let filler = "Niniejszy raport okresowy zawiera dane porownawcze oraz komentarz zarzadu. "
            .repeat(70);
        format!(
            "<html><body>\
             <p>{filler}</p>\
             <h2>Akcje w posiadaniu osób zarządzających i nadzorujących</h2>\
             <table>\
             <tr><th>Imię i nazwisko</th><th>Stanowisko</th><th>Liczba akcji</th></tr>\
             <tr><td>Tadeusz Wróblewski</td><td>Prezes Zarządu</td><td>3 366 250</td></tr>\
             <tr><td>Bogdan Sitko</td><td>Przewodniczący Rady Nadzorczej</td><td>2 366 280</td></tr>\
             </table></body></html>"
        )
    }

    fn glyph_xhtml() -> String {
        let filler = "Sprawozdanie zarzadu z dzialalnosci spolki za rok obrotowy. ".repeat(50);
        let pua: String = "\u{E000}".repeat(700);
        format!(
            "<html><body><p>{filler}</p>\
             <h2>Akcje Spółki w posiadaniu członków Zarządu i Rady Nadzorczej</h2>\
             <p>{pua}</p></body></html>"
        )
    }

    fn fetched_periodic_report(
        state: &AppState,
        company_id: &str,
        title: &str,
        url: &str,
        file_name: &str,
        content: &str,
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
        assert!(
            matches!(
                doc.doc_kind.as_deref(),
                Some("periodic_ssf") | Some("periodic_jsf")
            ),
            "sample title must classify as periodic, got {:?}",
            doc.doc_kind
        );
        std::fs::write(state.data_dir().join(file_name), content).expect("write file");
        state
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some("application/xhtml+xml"),
                Some("hash"),
                Some(content.len() as i64),
            )
            .expect("mark fetched");
        doc.id
    }

    fn stake(state: &AppState, company_id: &str, holder: &str) {
        state
            .ownership()
            .append_snapshot(NewOwnershipStake {
                company_id: company_id.to_owned(),
                holder_name_raw: holder.to_owned(),
                holder_type: None,
                capital_pct: Some("15.0".to_owned()),
                votes_pct: Some("15.0".to_owned()),
                as_of: "2025-12-31".to_owned(),
                source: "report_document".to_owned(),
                report_document_id: None,
                feed_item_id: None,
            })
            .expect("stake");
    }

    fn run_job(state: &AppState, company_id: &str, document_id: &str) {
        let payload = serde_json::to_string(&ManagementExtractionPayload {
            company_id: company_id.to_owned(),
            report_document_id: document_id.to_owned(),
        })
        .expect("payload");
        run_management_extraction_job(state, &payload).expect("run job");
    }

    /// #404 H2: a per-row `upsert_holding` loop issues one pool checkout per
    /// row, each an autocommit fsync under `synchronous=FULL` — a large filing's
    /// insider table amplifies into thousands of fsyncs. Drives a synthetic
    /// 500-person outcome through the real persist path and asserts the
    /// pool-checkout delta stays small regardless of row count (batched: one
    /// checkout for the holdings, plus the fixed founder-stamp/residual-clear
    /// calls `finalize` already makes).
    #[test]
    fn management_extraction_persists_a_document_under_a_bounded_checkout_count() {
        let s = state_with_dir();
        let c = company(&s);
        let doc_id = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025-bulk.pdf",
            "mgmt-bulk.pdf",
            &holdings_xhtml(),
        );
        let document = s.get_report_document(&doc_id).expect("document");
        let sections = vec![Section {
            ordinal: 0,
            heading: "Akcje w posiadaniu osób zarządzających i nadzorujących".to_owned(),
            body: "na dzień 2026-01-01".to_owned(),
        }];
        let rows: Vec<MgmtHoldingRow> = (0..500)
            .map(|i| MgmtHoldingRow {
                person_raw: format!("Person {i}"),
                role: None,
                shares: Some("100".to_owned()),
                indirect_via_raw: None,
                prior_shares: None,
                prior_as_of: None,
            })
            .collect();
        let outcome = MgmtHoldingsOutcome {
            state: MgmtHoldingsState::Parsed,
            matched_heading: Some(
                "Akcje w posiadaniu osób zarządzających i nadzorujących".to_owned(),
            ),
            rows,
            zero_organs: Vec::new(),
        };

        let before = s.checkout_count();
        persist_outcome(&s, &document, &sections, &outcome).expect("persist");
        let delta = s.checkout_count() - before;

        assert!(
            delta <= 8,
            "persisting 500 management-holdings rows for one document took {delta} pool checkouts (budget: 8)"
        );
    }

    #[test]
    fn job_parses_holdings_and_stamps_founder() {
        let s = state_with_dir();
        let c = company(&s);
        // The Prezes is also a >5% disclosed holder — the founder-badge case.
        stake(&s, &c, "Tadeusz Wróblewski");
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "mgmt-ssf-2025.xhtml",
            &holdings_xhtml(),
        );

        run_job(&s, &c, &doc);

        let rows = s.management_holdings().list_by_company(&c).expect("rows");
        assert_eq!(rows.len(), 2, "two by-person holdings written");
        let prezes = rows
            .iter()
            .find(|r| r.person_name_raw.contains("Wróblewski"))
            .expect("prezes row");
        assert_eq!(prezes.role.as_deref(), Some("management"));
        assert_eq!(prezes.shares.as_deref(), Some("3366250"));

        // The founder's own >5% stake is stamped founder_insider.
        let stamped = s
            .ownership()
            .current_state(&c)
            .expect("state")
            .into_iter()
            .find(|st| st.holder_name_normalized == "TADEUSZ WRÓBLEWSKI")
            .and_then(|st| st.holder_type);
        assert_eq!(stamped.as_deref(), Some("founder_insider"));

        // Idempotent: a re-run writes no new rows.
        run_job(&s, &c, &doc);
        assert_eq!(
            s.management_holdings()
                .list_by_company(&c)
                .expect("rows")
                .len(),
            2
        );
    }

    #[test]
    fn catch_up_enqueues_uncovered_then_skips_covered() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://example.com/ssf-2025.xhtml",
            "mgmt-ssf-cu.xhtml",
            &holdings_xhtml(),
        );
        assert_eq!(enqueue_management_extraction_catch_up(&s, None), 1);
        run_job(&s, &c, &doc);
        // Now covered → catch-up enqueues nothing.
        assert_eq!(enqueue_management_extraction_catch_up(&s, None), 0);
    }

    /// A fetched document of ANY doc_kind with a stored file (no periodic assert).
    fn fetched_document(
        state: &AppState,
        company_id: &str,
        title: &str,
        url: &str,
        file_name: &str,
        content: &str,
    ) -> ReportDocument {
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
        std::fs::write(state.data_dir().join(file_name), content).expect("write file");
        state
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some("application/xhtml+xml"),
                Some("hash"),
                Some(content.len() as i64),
            )
            .expect("mark fetched");
        state.get_report_document(&doc.id).expect("fetched doc")
    }

    /// F-A3 (owner dogfooding 2026-07-17): KRU discloses management holdings ONLY
    /// in its SzD ("Sprawozdanie z działalności Zarządu"), a `doc_kind='other'`
    /// document. The periodic-only selection skipped it, so KRU had ZERO
    /// management-holdings rows on the live DB. The SzD carries the exact holdings
    /// table the periodic reports carry — it must be enqueued, parsed, and stamped;
    /// a plain `other` announcement must still be skipped.
    #[test]
    fn management_report_szd_is_selected_parsed_and_stamped() {
        let s = state_with_dir();
        let c = company(&s);
        let szd = fetched_document(
            &s,
            &c,
            "Sprawozdanie z działalności Zarządu 2025",
            "https://bonnier.pl/.../SZD_Grupa_2025rok.xhtml",
            "mgmt-szd.xhtml",
            &holdings_xhtml(),
        );
        // The SzD really is a `doc_kind='other'` document (the classifier keeps it
        // out of the financial taxonomy) — the exact live shape.
        assert_eq!(szd.doc_kind.as_deref(), Some("other"));
        // A plain announcement (also `other`) is NOT a holdings source.
        let _noise = fetched_document(
            &s,
            &c,
            "Powiadomienie o transakcjach MAR",
            "https://bonnier.pl/.../powiadomienie.xhtml",
            "mgmt-noise.xhtml",
            "<html><body><p>Powiadomienie</p></body></html>",
        );

        // Selection: exactly the SzD is enqueued (the announcement is skipped).
        assert_eq!(
            enqueue_management_extraction_catch_up(&s, None),
            1,
            "the SzD management report must be selected for extraction"
        );
        run_job(&s, &c, &szd.id);
        let rows = s.management_holdings().list_by_company(&c).expect("rows");
        assert!(
            !rows.is_empty(),
            "the SzD holdings table must produce management-holdings rows"
        );
        // Covered now → catch-up enqueues nothing (idempotent, announcement stays out).
        assert_eq!(enqueue_management_extraction_catch_up(&s, None), 0);
    }

    /// A fetched periodic report stored under a `.pdf` name **and** served
    /// `application/pdf` — the corpus's mislabeled shape, where nothing but the
    /// bytes reveals the real container.
    fn fetched_periodic_report_as_pdf(
        state: &AppState,
        company_id: &str,
        title: &str,
        url: &str,
        file_name: &str,
        content: &str,
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
        assert!(
            matches!(
                doc.doc_kind.as_deref(),
                Some("periodic_ssf") | Some("periodic_jsf")
            ),
            "sample title must classify as periodic, got {:?}",
            doc.doc_kind
        );
        std::fs::write(state.data_dir().join(file_name), content).expect("write file");
        state
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some("application/pdf"),
                Some("hash"),
                Some(content.len() as i64),
            )
            .expect("mark fetched");
        doc.id
    }

    /// Epic #229 T2: the holdings table arrives as markup stored under a `.pdf`
    /// name (24 such files in the maintainer's corpus); the sniffed container
    /// routes it to the markup reader and it parses.
    #[test]
    fn markup_stored_under_a_pdf_name_parses_management_holdings() {
        let s = state_with_dir();
        let c = company(&s);
        // The server called it a PDF and so does the filename — both lie, so the
        // document is seeded here rather than through the xhtml-typed helper.
        let doc = fetched_periodic_report_as_pdf(
            &s,
            &c,
            "Raport półroczny 2025 JSF",
            "https://example.com/jsf-2025.pdf",
            "mgmt-liar.pdf",
            &holdings_xhtml(),
        );
        s.set_report_document_detected_container(&doc, "html")
            .expect("stamp container");

        run_job(&s, &c, &doc);
        let rows = s.management_holdings().list_by_company(&c).expect("rows");
        assert!(
            !rows.is_empty(),
            "container truth must route the markup to the markup reader, \
             not the PDF reader the .pdf name implies"
        );
    }

    /// A ZIP report package stored under a `.pdf` name has no text layer this tier
    /// can read. It must be skipped honestly — never fed to the PDF reader, and
    /// never parked as a "glyph-encoded" residual it is not.
    #[test]
    fn a_zip_package_under_a_pdf_name_is_skipped_not_parsed() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report_as_pdf(
            &s,
            &c,
            "Raport półroczny 2025 JSF",
            "https://example.com/jsf-2025-pkg.pdf",
            "mgmt-pkg.pdf",
            "PK\u{3}\u{4}\u{14}\u{0}not really readable",
        );
        s.set_report_document_detected_container(&doc, "zip")
            .expect("stamp container");

        run_job(&s, &c, &doc);
        assert!(s
            .management_holdings()
            .list_by_company(&c)
            .expect("rows")
            .is_empty());
        assert!(
            s.management_holdings()
                .list_residuals(&c)
                .expect("residuals")
                .is_empty(),
            "a package is not a parse residual — it is simply not this tier's input"
        );
    }

    #[test]
    fn glyph_document_parks_residual_without_pdf_sibling() {
        let s = state_with_dir();
        let c = company(&s);
        let doc = fetched_periodic_report(
            &s,
            &c,
            "Raport półroczny 2025 JSF",
            "https://example.com/jsf-2025.xhtml",
            "mgmt-glyph.xhtml",
            &glyph_xhtml(),
        );

        run_job(&s, &c, &doc);
        assert!(s
            .management_holdings()
            .list_by_company(&c)
            .expect("rows")
            .is_empty());
        let residuals = s
            .management_holdings()
            .list_residuals(&c)
            .expect("residuals");
        assert_eq!(residuals.len(), 1);
        assert_eq!(residuals[0].parse_state, "glyph_encoded");
        // Parked → catch-up does not re-enqueue.
        assert_eq!(enqueue_management_extraction_catch_up(&s, None), 0);
    }
}
