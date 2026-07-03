//! On-track company history backfill (ADR 0036, milestone v0.41.0).
//!
//! An explicit, user-triggered action that paginates the active Bankier company-komunikaty
//! listing backward ~3 years and ingests periodic reports + ESPI/EBI filings through the
//! normal ingestion path (preserving original publication dates, dedup, classification, and
//! attachment registration). Backfill is idempotent, throttled, app-open-only, and reports
//! live progress/diagnostics. Historical calendar entries are not backfilled.

use std::time::Duration;

use time::{format_description::well_known::Rfc3339, macros::format_description, OffsetDateTime};

use crate::document_fetcher::{DocumentFetcher, HttpDocumentFetcher};
use crate::source_adapters::bankier_company::{
    self, BankierCompanyFetcher, BankierCompanyTarget, HttpBankierCompanyFetcher,
};
use crate::storage::{AppState, BackfillProgress};

/// Years of history the backfill action covers.
pub const BACKFILL_YEARS: i64 = 3;
/// Page cap so a single backfill cannot run unbounded (25 items/page ≫ 3 years of filings).
pub const MAX_BACKFILL_PAGES: usize = 80;
/// Throttle between Bankier requests, matching the company-komunikaty rate policy.
const REQUEST_DELAY: Duration = Duration::from_secs(1);

/// Run a 3-year history backfill for one tracked company using the live HTTP fetchers.
/// Returns the final progress snapshot. Errors are recorded on the progress, not propagated,
/// so the command always resolves with a status the UI can render.
pub fn backfill_company_history(state: &AppState, company_id: &str) -> BackfillProgress {
    let bankier_fetcher = HttpBankierCompanyFetcher;
    let document_fetcher = HttpDocumentFetcher::new();
    run_backfill(
        state,
        company_id,
        &bankier_fetcher,
        &document_fetcher,
        REQUEST_DELAY,
    )
}

/// Core backfill routine, generic over the fetchers so tests can inject deterministic ones.
pub fn run_backfill(
    state: &AppState,
    company_id: &str,
    bankier_fetcher: &impl BankierCompanyFetcher,
    document_fetcher: &dyn DocumentFetcher,
    delay: Duration,
) -> BackfillProgress {
    let started_at = now_rfc3339();
    let mut progress = BackfillProgress {
        company_id: company_id.to_owned(),
        status: "running".to_owned(),
        pages_fetched: 0,
        items_ingested: 0,
        documents_stored: 0,
        detail_errors: 0,
        error: None,
        started_at: started_at.clone(),
        updated_at: started_at.clone(),
    };
    state.set_backfill_progress(progress.clone());

    let target = match find_target(state, company_id) {
        Ok(Some(target)) => target,
        Ok(None) => return fail(state, &mut progress, "company is not a tracked GPW company"),
        Err(error) => return fail(state, &mut progress, &error),
    };

    let cutoff = backfill_cutoff();
    // Report progress live as pages and items stream in, so the long fetch phase is visible
    // instead of sitting at zero until it finishes.
    let progress_company = company_id.to_owned();
    let progress_started = started_at.clone();
    let (_, items, stats) = match bankier_company::fetch_company_backfill_items(
        bankier_fetcher,
        &target,
        &cutoff,
        MAX_BACKFILL_PAGES,
        delay,
        |pages, items_collected| {
            state.set_backfill_progress(BackfillProgress {
                company_id: progress_company.clone(),
                status: "running".to_owned(),
                pages_fetched: pages,
                items_ingested: items_collected,
                documents_stored: 0,
                detail_errors: 0,
                error: None,
                started_at: progress_started.clone(),
                updated_at: now_rfc3339(),
            });
        },
    ) {
        Ok(result) => result,
        Err(error) => return fail(state, &mut progress, &error.to_string()),
    };

    progress.pages_fetched = stats.pages_fetched;
    progress.detail_errors = stats.detail_errors;
    progress.updated_at = now_rfc3339();
    state.set_backfill_progress(progress.clone());

    // Ingest through the normal path: dedup, classification, and attachment registration all
    // apply, so re-running the backfill produces no duplicates (ADR 0036).
    match state.ingest_bankier_company_items(&items) {
        Ok(result) => {
            progress.items_ingested = result.items_created;
        }
        Err(error) => return fail(state, &mut progress, &error.to_string()),
    }

    // Fetch files for periodic-report attachments registered during ingestion.
    match crate::report_documents_capture::fetch_pending_attachments(state, document_fetcher) {
        Ok(summary) => {
            progress.documents_stored = summary.stored;
            progress.detail_errors += summary.failed;
        }
        Err(error) => return fail(state, &mut progress, &error.to_string()),
    }

    progress.status = "completed".to_owned();
    progress.updated_at = now_rfc3339();
    state.set_backfill_progress(progress.clone());
    progress
}

fn find_target(state: &AppState, company_id: &str) -> Result<Option<BankierCompanyTarget>, String> {
    let targets = state
        .list_bankier_company_targets()
        .map_err(|error| error.to_string())?;
    Ok(targets
        .into_iter()
        .find(|target| target.company_id == company_id))
}

fn fail(state: &AppState, progress: &mut BackfillProgress, message: &str) -> BackfillProgress {
    progress.status = "failed".to_owned();
    progress.error = Some(message.to_owned());
    progress.updated_at = now_rfc3339();
    state.set_backfill_progress(progress.clone());
    progress.clone()
}

/// Lower bound for backfill, as `YYYY-MM-DDTHH:MM:SS` to compare against Bankier item dates.
fn backfill_cutoff() -> String {
    let cutoff =
        OffsetDateTime::now_utc().saturating_sub(time::Duration::days(BACKFILL_YEARS * 365));
    cutoff
        .format(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second]"
        ))
        .unwrap_or_else(|_| "1970-01-01T00:00:00".to_owned())
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_fetcher::{DocumentFetcherError, FetchedDocument};
    use crate::source_adapters::bankier_company::{BankierCompanyError, BankierCompanyIdentifiers};
    use crate::storage::{open_in_memory_database, AppState, NewCompany};
    use std::cell::RefCell;

    /// Deterministic Bankier fetcher: page 1 returns two recent periodic-report rows, page 2
    /// is empty, and each article link returns a body with one PDF attachment.
    struct FakeBankier {
        page_requests: RefCell<usize>,
    }

    impl BankierCompanyFetcher for FakeBankier {
        fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
            if url.contains("/articles/listing/") {
                let page1 = url.contains("/listing/1/");
                *self.page_requests.borrow_mut() += 1;
                if page1 {
                    return Ok(r#"{
                      "articles": [
                        {"title": "CD PROJEKT SA: Skonsolidowany raport kwartalny QSr 1/2025",
                         "url": "/wiadomosc/report-a-1.html", "time": "2025-05-20 10:00:00",
                         "pub_id": 3, "article_id": 1, "messages_filters": ["ESPI"]},
                        {"title": "CD PROJEKT SA: Raport roczny za 2024",
                         "url": "/wiadomosc/report-b-2.html", "time": "2025-03-10 10:00:00",
                         "pub_id": 3, "article_id": 2, "messages_filters": ["ESPI"]}
                      ]
                    }"#
                    .to_owned());
                }
                return Ok(r#"{ "articles": [] }"#.to_owned());
            }

            // Article detail page with one attachment, distinct per article.
            let attachment = if url.contains("report-a") {
                "https://bonnier.pl/report-a.pdf"
            } else {
                "https://bonnier.pl/report-b.pdf"
            };
            Ok(format!(
                r#"
                <html><head>
                  <script type="application/ld+json">
                    {{"@type":"NewsArticle","articleBody":"Skonsolidowany raport kwartalny QSr."}}
                  </script>
                </head><body>
                  <a href="{attachment}">Raport PDF</a>
                </body></html>
            "#
            ))
        }
    }

    struct FakeDocs;
    impl DocumentFetcher for FakeDocs {
        fn fetch(&self, _url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
            Ok(FetchedDocument {
                bytes: b"%PDF-1.7 fake report".to_vec(),
                content_type: Some("application/pdf".to_owned()),
            })
        }
    }

    fn tracked_company(state: &AppState) -> String {
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should create");
        // Pre-seed Bankier identifiers so the backfill skips the company-page lookup.
        state
            .upsert_bankier_company_identifiers(
                &company.id,
                &BankierCompanyIdentifiers {
                    slug: "CDPROJEKT".to_owned(),
                    tag_id: "722".to_owned(),
                },
            )
            .expect("identifiers should upsert");
        company.id
    }

    #[test]
    fn backfill_populates_history_and_stores_periodic_documents() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company_id = tracked_company(&state);

        let bankier = FakeBankier {
            page_requests: RefCell::new(0),
        };
        let progress = run_backfill(&state, &company_id, &bankier, &FakeDocs, Duration::ZERO);

        assert_eq!(progress.status, "completed", "error: {:?}", progress.error);
        assert_eq!(progress.items_ingested, 2);
        assert_eq!(progress.documents_stored, 2);

        let docs = state
            .list_report_documents_by_company(&company_id)
            .expect("documents should list");
        assert_eq!(docs.len(), 2);
        assert!(docs.iter().all(|d| d.fetch_status == "fetched"));
        assert!(docs.iter().all(|d| d.local_path.is_some()));

        let stored = state.get_backfill_progress(&company_id).expect("progress");
        assert_eq!(stored.status, "completed");
    }

    #[test]
    fn rerunning_backfill_does_not_duplicate() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company_id = tracked_company(&state);

        for _ in 0..2 {
            let bankier = FakeBankier {
                page_requests: RefCell::new(0),
            };
            run_backfill(&state, &company_id, &bankier, &FakeDocs, Duration::ZERO);
        }

        let docs = state
            .list_report_documents_by_company(&company_id)
            .expect("documents should list");
        assert_eq!(
            docs.len(),
            2,
            "re-running backfill must not duplicate documents"
        );
    }

    /// Deterministic Bankier fetcher: one page with a single non-periodic (ESPI
    /// insider-transaction notice) item whose attachment is a structured `.xhtml`
    /// statement, no PDF sibling.
    struct FakeBankierNonPeriodicXhtml;

    impl BankierCompanyFetcher for FakeBankierNonPeriodicXhtml {
        fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
            if url.contains("/articles/listing/") {
                if url.contains("/listing/1/") {
                    return Ok(r#"{
                      "articles": [
                        {"title": "CD PROJEKT SA: Powiadomienie o transakcjach na akcjach - art. 19 ust. 1 MAR",
                         "url": "/wiadomosc/insider-1.html", "time": "2025-05-20 10:00:00",
                         "pub_id": 3, "article_id": 1, "messages_filters": ["ESPI"]}
                      ]
                    }"#
                    .to_owned());
                }
                return Ok(r#"{ "articles": [] }"#.to_owned());
            }

            Ok(r#"
                <html><head>
                  <script type="application/ld+json">
                    {"@type":"NewsArticle","articleBody":"Powiadomienie o transakcji osoby pełniącej obowiązki zarządcze."}
                  </script>
                </head><body>
                  <a href="https://bonnier.pl/static/att/emitent/2025-05/insider.xhtml">Raport XHTML</a>
                </body></html>
            "#
            .to_owned())
        }
    }

    struct FakeDocsXhtml;
    impl DocumentFetcher for FakeDocsXhtml {
        fn fetch(&self, _url: &str) -> Result<FetchedDocument, DocumentFetcherError> {
            Ok(FetchedDocument {
                bytes: b"<html>fake xhtml statement</html>".to_vec(),
                content_type: Some("application/xhtml+xml".to_owned()),
            })
        }
    }

    /// ADR 0061 decision 1b: a structured `.xhtml` attachment on a non-periodic
    /// (ESPI insider-notice) item is still fetched — it is not gated behind the
    /// periodic-report text classifier the way a PDF sibling would be.
    #[test]
    fn non_periodic_xhtml_attachment_is_fetched_during_backfill() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company_id = tracked_company(&state);

        let progress = run_backfill(
            &state,
            &company_id,
            &FakeBankierNonPeriodicXhtml,
            &FakeDocsXhtml,
            Duration::ZERO,
        );

        assert_eq!(progress.status, "completed", "error: {:?}", progress.error);
        assert_eq!(progress.documents_stored, 1);

        let docs = state
            .list_report_documents_by_company(&company_id)
            .expect("documents should list");
        assert_eq!(docs.len(), 1);
        let doc = &docs[0];
        assert!(doc.url.ends_with(".xhtml"));
        assert_eq!(doc.fetch_status, "fetched");
        assert!(
            doc.local_path
                .as_deref()
                .is_some_and(|path| path.ends_with(".xhtml")),
            "local_path: {:?}",
            doc.local_path
        );
    }

    #[test]
    fn untracked_company_fails_cleanly() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let bankier = FakeBankier {
            page_requests: RefCell::new(0),
        };
        let progress = run_backfill(
            &state,
            "company_missing",
            &bankier,
            &FakeDocs,
            Duration::ZERO,
        );

        assert_eq!(progress.status, "failed");
        assert!(progress.error.is_some());
    }
}
