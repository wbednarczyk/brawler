//! Insider attachment-PDF tier (v0.57 **T4b**, ADR 0083 Decision 6 + the
//! 2026-07-17 ground-truth amendment; plan v0.57 T4b).
//!
//! The MAR art. 19 transaction figures (volume / price / currency / tx_date /
//! instrument / direction) live in the attached "Powiadomienie…" notification
//! document, not the Bankier cover note (which the T4 parser already turned into
//! `insider_transactions` rows with those columns NULL). This tier **fetches** the
//! notification document (an official disclosure, reusing the report-document fetch
//! infra — the document is already registered at ingest as a `metadata_only`
//! `report_documents` row), **deterministically parses** its standard form (the
//! shared ADR 0061 extraction tier + [`crate::fundamentals::insider::attachment`]),
//! and **merges** the parsed figures into the substrate, filling only NULLs
//! (conflicts recorded, never overwritten; unmatched rows appended).
//!
//! **Lane (ADR 0059).** Network IO → this runs on the source/refresh lane, off the
//! ingestion write path (alongside `fetch_pending_attachments`), NOT the CPU lane.
//!
//! **Triggers (DoD §C).**
//! - *after-refresh seam* — [`fetch_and_parse_insider_attachments`] from
//!   `refresh_one_bankier_company`, right after the periodic-attachment fetch, so a
//!   newly-classified+cover-note-parsed insider filing gets its figures in the same
//!   refresh.
//! - *backfill* — [`backfill_company_insider_attachments`] force-re-attempts every
//!   classified insider filing of one company.
//!
//! **Attempt-once (idempotence).** A filing whose attachment tier reaches a terminal
//! outcome (parsed / no fetchable document / scanned no-text-layer / no recognizable
//! form) is recorded once in `insider_attachment_attempts` (with the merge
//! diagnostics), so the sweep processes each filing exactly once and re-runs issue
//! zero fetches. A transient fetch failure is NOT recorded (the report-document row
//! stays retryable), so it retries on the next sweep. The merge itself is idempotent
//! regardless (fill-NULLs only).

use crate::app_state::AppState;
use crate::document_fetcher::DocumentFetcher;
use crate::fundamentals::insider::attachment::{parse_notification_text, AttachmentParse};
use crate::report_diff::extraction::{extract_report, ExtractionState, SourceFormat};
use crate::storage::AttachmentMergeOutcome;

/// Aggregate result of one attachment-tier sweep (surfaced in logs + the closure
/// report; the per-filing diagnostics persist on `insider_attachment_attempts`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InsiderAttachmentSummary {
    pub filings_attempted: usize,
    pub parsed: usize,
    pub no_attachment: usize,
    pub no_text_layer: usize,
    pub not_found: usize,
    /// Filings left un-terminated by a transient fetch failure (retried next sweep).
    pub fetch_retry: usize,
    pub filled: usize,
    pub appended: usize,
    pub conflicts: usize,
}

/// Sweep every classified insider filing still needing the attachment tier
/// (after-refresh seam). Best-effort per filing; a storage abort surfaces as `Err`.
pub fn fetch_and_parse_insider_attachments(
    state: &AppState,
    fetcher: &dyn DocumentFetcher,
) -> Result<InsiderAttachmentSummary, String> {
    run(state, fetcher, None)
}

/// Backfill: force-re-attempt the attachment tier for every insider filing of one
/// company (idempotent — the fill-NULLs merge never duplicates).
pub fn backfill_company_insider_attachments(
    state: &AppState,
    company_id: &str,
    fetcher: &dyn DocumentFetcher,
) -> Result<InsiderAttachmentSummary, String> {
    state
        .insider()
        .clear_company_attachment_attempts(company_id)
        .map_err(|error| error.to_string())?;
    run(state, fetcher, Some(company_id))
}

fn run(
    state: &AppState,
    fetcher: &dyn DocumentFetcher,
    company_id: Option<&str>,
) -> Result<InsiderAttachmentSummary, String> {
    let pending = state
        .insider()
        .filings_needing_attachment(company_id)
        .map_err(|error| error.to_string())?;

    let mut summary = InsiderAttachmentSummary::default();
    for filing in &pending {
        let result = process_filing(state, fetcher, &filing.company_id, &filing.feed_item_id)?;
        summary.filings_attempted += 1;
        match result {
            FilingResult::Parsed(diag) => {
                summary.parsed += 1;
                summary.filled += diag.filled;
                summary.appended += diag.appended;
                summary.conflicts += diag.conflicts.len();
                for conflict in &diag.conflicts {
                    log::info!(
                        "module=insider_attachment stage=conflict feed_item={} unit={} field={} \
                         existing={:?} incoming={:?} (kept existing)",
                        conflict.feed_item_id,
                        conflict.unit_index,
                        conflict.field,
                        conflict.existing,
                        conflict.incoming
                    );
                }
                mark(state, filing, "parsed", &diag)?;
            }
            FilingResult::NoAttachment => {
                summary.no_attachment += 1;
                mark(
                    state,
                    filing,
                    "no_attachment",
                    &AttachmentMergeOutcome::default(),
                )?;
            }
            FilingResult::NoTextLayer => {
                summary.no_text_layer += 1;
                mark(
                    state,
                    filing,
                    "no_text_layer",
                    &AttachmentMergeOutcome::default(),
                )?;
            }
            FilingResult::NotFound => {
                summary.not_found += 1;
                mark(
                    state,
                    filing,
                    "not_found",
                    &AttachmentMergeOutcome::default(),
                )?;
            }
            // Transient: no marker written — the filing retries next sweep.
            FilingResult::FetchRetry => summary.fetch_retry += 1,
        }
    }
    Ok(summary)
}

fn mark(
    state: &AppState,
    filing: &crate::storage::AttachmentPendingFiling,
    outcome: &str,
    diagnostics: &AttachmentMergeOutcome,
) -> Result<(), String> {
    state
        .insider()
        .record_attachment_attempt(
            &filing.feed_item_id,
            &filing.company_id,
            outcome,
            diagnostics,
        )
        .map_err(|error| error.to_string())
}

enum FilingResult {
    Parsed(AttachmentMergeOutcome),
    NoAttachment,
    NoTextLayer,
    NotFound,
    FetchRetry,
}

/// Fetch + parse + merge one filing's notification document(s). Discovers the
/// filing's registered attachment documents (`origin_ref`), skips signature files,
/// fetches each on demand (idempotent — an already-fetched file is not re-fetched),
/// extracts its text (PDF or xhtml), and parses the standard notification form.
fn process_filing(
    state: &AppState,
    fetcher: &dyn DocumentFetcher,
    company_id: &str,
    feed_item_id: &str,
) -> Result<FilingResult, String> {
    let docs = state
        .list_report_documents_by_origin(feed_item_id)
        .map_err(|error| error.to_string())?;

    // Candidate notification documents: registered ESPI/EBI attachments that are not
    // digital-signature files. The parser rejects a non-notification document, so we
    // do not need a title heuristic here.
    let candidates: Vec<_> = docs
        .into_iter()
        .filter(|doc| {
            doc.source_type == "espi_attachment"
                && !crate::source_adapters::bankier_company::is_signature_attachment_url(&doc.url)
        })
        .collect();

    if candidates.is_empty() {
        return Ok(FilingResult::NoAttachment);
    }

    let mut units = Vec::new();
    let mut any_fetch_error = false;
    let mut any_extracted = false;
    let mut any_no_text = false;
    let mut fetched_this_filing = 0usize;

    for doc in &candidates {
        // Fetch on demand (once). `fetch_report_document` early-returns a doc that
        // already has a stored file; a metadata_only/failed doc is fetched now.
        let needs_fetch = doc
            .local_path
            .as_deref()
            .map(|p| p.trim().is_empty())
            .unwrap_or(true);
        let fetched_doc = if needs_fetch {
            // Politeness: space out real network fetches (a fake fetcher is instant).
            if fetched_this_filing > 0 {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            fetched_this_filing += 1;
            match crate::report_documents_capture::fetch_report_document(state, fetcher, &doc.id) {
                Ok(fetched) => fetched,
                Err(error) => {
                    log::warn!(
                        "module=insider_attachment stage=fetch feed_item={feed_item_id} \
                         doc={} error={error}",
                        doc.id
                    );
                    any_fetch_error = true;
                    continue;
                }
            }
        } else {
            doc.clone()
        };

        let Some(local_path) = fetched_doc.local_path.as_deref() else {
            any_fetch_error = true;
            continue;
        };
        let bytes = match std::fs::read(state.data_dir().join(local_path)) {
            Ok(bytes) => bytes,
            Err(error) => {
                log::warn!(
                    "module=insider_attachment stage=read feed_item={feed_item_id} \
                     path={local_path} error={error}"
                );
                any_fetch_error = true;
                continue;
            }
        };

        let format = SourceFormat::resolve(fetched_doc.content_type.as_deref(), local_path);
        let extracted = extract_report(&bytes, format);
        match extracted.state {
            ExtractionState::Extracted => {
                any_extracted = true;
                let text = flatten_sections(&extracted.sections);
                if let AttachmentParse::Units(mut parsed) = parse_notification_text(&text) {
                    units.append(&mut parsed);
                }
            }
            // Scanned / image / unreadable: parked for the vision path — never guessed.
            ExtractionState::NoTextLayer | ExtractionState::ExtractionFailed => any_no_text = true,
        }
    }

    if !units.is_empty() {
        let outcome = state
            .insider()
            .merge_attachment_units(company_id, feed_item_id, &units)
            .map_err(|error| error.to_string())?;
        return Ok(FilingResult::Parsed(outcome));
    }
    // No units. A transient fetch failure with nothing extracted is retried; a
    // clean no-text-layer / no-form document is terminal.
    if any_fetch_error && !any_extracted && !any_no_text {
        return Ok(FilingResult::FetchRetry);
    }
    if any_no_text && !any_extracted {
        return Ok(FilingResult::NoTextLayer);
    }
    if any_extracted {
        return Ok(FilingResult::NotFound);
    }
    Ok(FilingResult::FetchRetry)
}

/// Flatten extracted sections into one text blob for the form parser (headings +
/// bodies in document order).
fn flatten_sections(sections: &[crate::report_diff::extraction::Section]) -> String {
    let mut out = String::new();
    for section in sections {
        if section.heading != "<preamble>" {
            out.push_str(&section.heading);
            out.push('\n');
        }
        out.push_str(&section.body);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_fetcher::{DocumentFetcher, DocumentFetcherError, FetchedDocument};
    use crate::source_adapters::bankier_company::BankierCompanyItem;
    use crate::storage::{open_in_memory_database, AppState, NewCompany};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn unique_dir() -> std::path::PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("brawler-insatt-{}-{n}", std::process::id()))
    }

    fn state_with_dir() -> AppState {
        let dir = unique_dir();
        std::fs::create_dir_all(&dir).expect("temp dir");
        AppState::with_data_dir(open_in_memory_database().expect("db"), dir)
    }

    /// A fetcher returning fixed bytes, counting calls (attempt-once assertion).
    struct CountingFetcher {
        response: Result<FetchedDocument, ()>,
        calls: AtomicUsize,
    }
    impl CountingFetcher {
        fn ok(bytes: &str, content_type: &str) -> Self {
            Self {
                response: Ok(FetchedDocument {
                    bytes: bytes.as_bytes().to_vec(),
                    content_type: Some(content_type.to_owned()),
                }),
                calls: AtomicUsize::new(0),
            }
        }
        fn err() -> Self {
            Self {
                response: Err(()),
                calls: AtomicUsize::new(0),
            }
        }
    }
    impl DocumentFetcher for CountingFetcher {
        fn fetch(&self, _url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            match &self.response {
                Ok(doc) => Ok(FetchedDocument {
                    bytes: doc.bytes.clone(),
                    content_type: doc.content_type.clone(),
                }),
                Err(()) => Err(DocumentFetcherError::InvalidContentType("boom".into())),
            }
        }
    }

    fn company(state: &AppState, ticker: &str) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: ticker.to_owned(),
                display_name: format!("{ticker} S.A."),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    /// Ingest a classified insider filing (cover note → one NULL-figure row) plus a
    /// registered notification attachment (metadata_only, awaiting T4b fetch).
    fn ingest_insider_filing(
        state: &AppState,
        company_id: &str,
        ticker: &str,
        article_id: &str,
        attachment_url: &str,
    ) {
        let body = "Treść raportu:Zarząd Przykład S.A. informuje o otrzymaniu w dniu \
            dzisiejszym od Jana Testowego, Wiceprezesa Zarządu Spółki, powiadomienia w trybie \
            art. 19 MAR o transakcji nabycia akcji Emitenta.ZałącznikiPlikOpisPowiadomienie.pdf\
            MESSAGE (ENGLISH VERSION)the board informs";
        let item = BankierCompanyItem {
            company_id: company_id.to_owned(),
            qualified_ticker: format!("GPW:{ticker}"),
            title: "Informacja o transakcjach uzyskana w trybie art. 19 MAR".to_owned(),
            link: format!("https://www.bankier.pl/wiadomosc/X-{article_id}.html"),
            summary: "Komunikat ESPI".to_owned(),
            published_at: Some("2026-07-01T09:00:00".to_owned()),
            fetched_at: "2026-07-01T10:00:00Z".to_owned(),
            article_id: article_id.to_owned(),
            pub_id: 3,
            dedupe_key: format!("bankier-company-komunikaty:article:{article_id}"),
            duplicate_signature: format!("official-secondary:GPW:{ticker}:{article_id}"),
            body_text: Some(body.to_owned()),
            attachments: vec![
                crate::source_adapters::bankier_company::BankierCompanyAttachment {
                    label: "Powiadomienie".to_owned(),
                    url: attachment_url.to_owned(),
                },
            ],
            detail_fetch_attempted: true,
        };
        state.ingest_bankier_company_items(&[item]).expect("ingest");
    }

    /// A machine-readable notification document (served as xhtml — the DVL/ESAP
    /// class; the extraction tier's format resolution handles it). Structurally
    /// equivalent to the KNF form, never copied from `private/`.
    fn notification_xhtml() -> String {
        let filler = "Powiadomienie o transakcji w trybie art. 19 MAR rozporzadzenia. ".repeat(80);
        format!(
            "<html><body><p>{filler}</p>\
             <p>Imię i nazwisko: Jan Testowy</p>\
             <p>Stanowisko / status: Wiceprezes Zarządu</p>\
             <p>Szczegóły transakcji</p>\
             <p>Opis instrumentu finansowego: Akcje zwykłe na okaziciela</p>\
             <p>Rodzaj transakcji: Nabycie</p>\
             <p>Cena: 12,50 PLN</p>\
             <p>Wolumen: 1 000</p>\
             <p>Data transakcji: 2026-07-03</p></body></html>"
        )
    }

    fn feed_id(article_id: &str) -> String {
        format!("feed_bankier_company_komunikatyarticle{article_id}")
    }

    #[test]
    fn fills_null_figures_from_notification_document() {
        let s = state_with_dir();
        let c = company(&s, "PRZ");
        ingest_insider_filing(
            &s,
            &c,
            "PRZ",
            "9400001",
            "https://static.att/powiadomienie.xhtml",
        );

        // Cover-note row exists with NULL figures.
        let before = &s.insider().list_by_company(&c).expect("list")[0];
        assert_eq!(before.person_normalized, "JAN TESTOWY");
        assert!(before.volume.is_none() && before.tx_date.is_none());

        let fetcher = CountingFetcher::ok(&notification_xhtml(), "application/xhtml+xml");
        let summary = fetch_and_parse_insider_attachments(&s, &fetcher).expect("sweep");
        assert_eq!(summary.parsed, 1);
        assert!(summary.filled >= 4, "volume/price/currency/tx_date filled");

        let after = &s.insider().list_by_company(&c).expect("list")[0];
        assert_eq!(after.volume.as_deref(), Some("1000"));
        assert_eq!(after.price.as_deref(), Some("12.50"));
        assert_eq!(after.currency.as_deref(), Some("PLN"));
        assert_eq!(after.tx_date.as_deref(), Some("2026-07-03"));
        // Person untouched; a filled row keeps its cover-note identity.
        assert_eq!(after.id, before.id);

        // Attempt-once: a second sweep re-selects nothing and issues no fetch.
        let fetcher2 = CountingFetcher::ok(&notification_xhtml(), "application/xhtml+xml");
        let summary2 = fetch_and_parse_insider_attachments(&s, &fetcher2).expect("sweep2");
        assert_eq!(summary2.filings_attempted, 0, "filing terminally attempted");
        assert_eq!(fetcher2.calls.load(Ordering::Relaxed), 0, "no re-fetch");
    }

    #[test]
    fn conflict_is_recorded_never_overwritten() {
        let s = state_with_dir();
        let c = company(&s, "CFL");
        ingest_insider_filing(
            &s,
            &c,
            "CFL",
            "9400002",
            "https://static.att/powiadomienie.xhtml",
        );

        // Seed a NON-NULL, disagreeing volume on the cover-note row.
        let row_id = s.insider().list_by_company(&c).expect("list")[0].id.clone();
        s.checkout_for_tests()
            .expect("conn")
            .execute(
                "UPDATE insider_transactions SET volume = '999' WHERE id = ?1",
                [&row_id],
            )
            .expect("seed volume");

        let fetcher = CountingFetcher::ok(&notification_xhtml(), "application/xhtml+xml");
        let summary = fetch_and_parse_insider_attachments(&s, &fetcher).expect("sweep");
        assert_eq!(
            summary.conflicts, 1,
            "the PDF volume disagreed → 1 conflict"
        );

        let after = &s.insider().list_by_company(&c).expect("list")[0];
        assert_eq!(after.volume.as_deref(), Some("999"), "existing value kept");
        // Other NULLs still filled around the conflict.
        assert_eq!(after.price.as_deref(), Some("12.50"));
    }

    #[test]
    fn transient_fetch_failure_retries_no_marker() {
        let s = state_with_dir();
        let c = company(&s, "RTY");
        ingest_insider_filing(
            &s,
            &c,
            "RTY",
            "9400003",
            "https://static.att/powiadomienie.xhtml",
        );

        let fetcher = CountingFetcher::err();
        let summary = fetch_and_parse_insider_attachments(&s, &fetcher).expect("sweep");
        assert_eq!(summary.fetch_retry, 1);
        assert!(
            !s.insider()
                .is_attachment_attempted(&feed_id("9400003"))
                .expect("attempted"),
            "a transient fetch failure is NOT marked terminal (retryable)"
        );

        // Next sweep with a working fetcher fills the figures.
        let ok = CountingFetcher::ok(&notification_xhtml(), "application/xhtml+xml");
        let summary2 = fetch_and_parse_insider_attachments(&s, &ok).expect("sweep2");
        assert_eq!(summary2.parsed, 1);
    }

    #[test]
    fn no_text_layer_document_parks_terminally() {
        let s = state_with_dir();
        let c = company(&s, "SCN");
        ingest_insider_filing(&s, &c, "SCN", "9400004", "https://static.att/scan.pdf");

        // A scanned PDF: bytes with no extractable text layer → ExtractionFailed.
        let fetcher = CountingFetcher::ok("%PDF-1.4 not-a-real-text-layer", "application/pdf");
        let summary = fetch_and_parse_insider_attachments(&s, &fetcher).expect("sweep");
        assert_eq!(summary.no_text_layer + summary.not_found, 1);
        assert!(
            s.insider()
                .is_attachment_attempted(&feed_id("9400004"))
                .expect("attempted"),
            "an unreadable document is terminal (parked for the vision path)"
        );
        // No figures guessed.
        assert!(s.insider().list_by_company(&c).expect("list")[0]
            .volume
            .is_none());
    }

    #[test]
    fn backfill_reattempts_and_is_idempotent() {
        let s = state_with_dir();
        let c = company(&s, "BKF");
        ingest_insider_filing(
            &s,
            &c,
            "BKF",
            "9400005",
            "https://static.att/powiadomienie.xhtml",
        );

        let fetcher = CountingFetcher::ok(&notification_xhtml(), "application/xhtml+xml");
        fetch_and_parse_insider_attachments(&s, &fetcher).expect("sweep");
        let filled_once = s.insider().list_by_company(&c).expect("list")[0]
            .volume
            .clone();
        assert_eq!(filled_once.as_deref(), Some("1000"));

        // Backfill clears the marker and re-runs; the fill is idempotent.
        let fetcher2 = CountingFetcher::ok(&notification_xhtml(), "application/xhtml+xml");
        let summary = backfill_company_insider_attachments(&s, &c, &fetcher2).expect("backfill");
        assert_eq!(summary.parsed, 1);
        let rows = s.insider().list_by_company(&c).expect("list");
        assert_eq!(rows.len(), 1, "no duplicate row after re-attempt");
        assert_eq!(rows[0].volume.as_deref(), Some("1000"));
    }
}
