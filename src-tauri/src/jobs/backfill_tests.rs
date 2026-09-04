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

/// A tracked GPW company with pre-seeded Bankier identifiers under a caller-chosen
/// ticker (so several can coexist in one test without a `qualified_ticker` clash).
fn tracked_company_with_ticker(state: &AppState, ticker: &str) -> String {
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should create");
    state
        .upsert_bankier_company_identifiers(
            &company.id,
            &BankierCompanyIdentifiers {
                slug: ticker.to_owned(),
                tag_id: "999".to_owned(),
            },
        )
        .expect("identifiers should upsert");
    company.id
}

/// A GPW company with NO Bankier identifiers: it is a backfill target (its
/// exchange is history-capable) but the walk must resolve its Bankier page
/// first — the rung where a missing/renamed slug surfaces.
fn gpw_company_without_identifiers(state: &AppState, ticker: &str) -> String {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should create")
        .id
}

/// Machine-readable reason code carried by a failed backfill, i.e. the prefix
/// of `error` before the human detail (`"code: detail"`). `None` for a
/// non-failed or code-less progress.
fn failure_code(progress: &BackfillProgress) -> Option<String> {
    progress
        .error
        .as_deref()
        .map(|error| error.split(':').next().unwrap_or(error).trim().to_owned())
}

/// Company page returns HTML with no Bankier identifiers — the "no Bankier
/// page found for this company/slug" cause (parse `MissingTagId`).
struct FakeBankierNoIdentifiers;
impl BankierCompanyFetcher for FakeBankierNoIdentifiers {
    fn fetch_text(&self, _url: &str) -> Result<String, BankierCompanyError> {
        Ok("<html><body>brak identyfikatorow spolki</body></html>".to_owned())
    }
}

/// Listing endpoint returns malformed JSON — the "page fetched but
/// unparseable" cause (parse `Json`). Company is pre-identified so the walk
/// goes straight to the listing.
struct FakeBankierBadJson;
impl BankierCompanyFetcher for FakeBankierBadJson {
    fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
        if url.contains("/articles/listing/") {
            return Ok("{ not valid json ".to_owned());
        }
        Ok("<html></html>".to_owned())
    }
}

/// Listing endpoint returns an empty article set — the "page fetched but zero
/// komunikaty" cause. This is an honest empty result, not a failure.
struct FakeBankierEmpty;
impl BankierCompanyFetcher for FakeBankierEmpty {
    fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
        if url.contains("/articles/listing/") {
            return Ok(r#"{ "articles": [] }"#.to_owned());
        }
        Ok("<html></html>".to_owned())
    }
}

fn new_connect_company(state: &AppState, ticker: &str) -> String {
    state
        .create_company(NewCompany {
            exchange: "NC".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} NewConnect S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should create")
        .id
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

/// ADR 0077 §3: a completed backfill chains a history sweep — one queued
/// `history_sweep` job keyed by the sweep row it just created.
#[test]
fn completed_backfill_chains_a_history_sweep() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company_id = tracked_company(&state);

    let bankier = FakeBankier {
        page_requests: RefCell::new(0),
    };
    let progress = run_backfill(&state, &company_id, &bankier, &FakeDocs, Duration::ZERO);
    assert_eq!(progress.status, "completed", "error: {:?}", progress.error);

    let sweep = state
        .history_sweeps()
        .get_latest_history_sweep(&company_id)
        .expect("latest sweep")
        .expect("backfill must create a sweep row");
    assert_eq!(sweep.trigger, "backfill");
    assert_eq!(sweep.status, "queued");

    // The result threads the chained sweep's id so the coverage panel polls
    // THIS sweep specifically (never "the latest sweep"): the row is created
    // eagerly at enqueue time, so the id is known before the command returns.
    assert_eq!(
        progress.chained_sweep_id.as_deref(),
        Some(sweep.id.as_str()),
        "the backfill result must carry the chained sweep's id"
    );

    // The sweep's durable job is queued under the sweep id (still pending —
    // no worker runs in this test).
    let payload = state
        .jobs()
        .pending_payload(&sweep.id)
        .expect("pending payload query")
        .expect("a history_sweep job must be queued for the sweep");
    assert!(
        payload.contains(&sweep.id),
        "the queued job payload names the sweep, got: {payload}"
    );
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
fn cutoff_respects_configured_years() {
    // A shorter configured depth yields a more recent lower bound; a longer
    // depth reaches further back (ADR 0077 §3).
    let one = backfill_cutoff(1);
    let ten = backfill_cutoff(10);
    assert!(
        one > ten,
        "a 1-year depth cutoff must be more recent than a 10-year one: {one} vs {ten}"
    );

    let now_year = OffsetDateTime::now_utc().year();
    let year_one: i32 = one[..4].parse().expect("cutoff year parses");
    assert!(
        (now_year - 1 - year_one).abs() <= 1,
        "a 1-year cutoff lands about a year ago (now {now_year}, cutoff {one})"
    );
}

/// T-A4 (card bfc4c98): a company on a market with no history-capable source
/// adapter (here NewConnect, `NC`) fails with the machine-readable
/// `unsupported_market` prefix the frontend maps to a localized message —
/// not the misleading "not a tracked GPW company".
#[test]
fn backfill_for_unsupported_market_fails_with_machine_readable_error() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "NC".to_owned(),
            ticker: "XYZ".to_owned(),
            display_name: "NewConnect Co S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should create");

    let bankier = FakeBankier {
        page_requests: RefCell::new(0),
    };
    let progress = run_backfill(&state, &company.id, &bankier, &FakeDocs, Duration::ZERO);

    assert_eq!(progress.status, "failed");
    let error = progress.error.expect("a failed backfill carries an error");
    assert!(
        error.starts_with("unsupported_market"),
        "error must start with the machine-readable prefix, got: {error}"
    );
}

/// T-A4: every failed backfill leaves a log line (the `fail()` path was
/// previously silent). Uses the shared capture logger; assertions filter by
/// the unique company id since other tests append to the same buffer.
#[test]
fn failed_backfill_emits_a_log_line() {
    crate::test_support::install_capture_logger();
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "NC".to_owned(),
            ticker: "LOG".to_owned(),
            display_name: "LogTrail NC S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should create");

    let bankier = FakeBankier {
        page_requests: RefCell::new(0),
    };
    let progress = run_backfill(&state, &company.id, &bankier, &FakeDocs, Duration::ZERO);
    assert_eq!(progress.status, "failed");

    let logs = crate::test_support::captured_logs()
        .lock()
        .expect("capture buffer");
    assert!(
        logs.iter()
            .any(|line| line.contains(&company.id) && line.contains("status=failed")),
        "a failed backfill must emit a warn line naming the company; buffer: {logs:?}"
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

// --- automatic backfill catch-up (v0.57, ADR 0077 amendment) ---------------

fn automated_company(state: &AppState, ticker: &str) -> String {
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should create");
    state
        .autopilot()
        .set_mode(&company.id, crate::storage::MODE_AUTOPILOT)
        .expect("mode should set");
    company.id
}

/// Seed a fetched **periodic** (`periodic_ssf`) report document — the coverage
/// the catch-up predicate treats as "has report history".
fn seed_fetched_periodic_doc(state: &AppState, company_id: &str, url: &str) {
    let doc = state
        .report_documents()
        .create_or_find_pending_report_document(crate::storage::CaptureReportDocumentInput {
            company_id: company_id.to_owned(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Skonsolidowany raport kwartalny QSr 1/2025".to_owned()),
            attribution: None,
        })
        .expect("register document");
    state
        .report_documents()
        .mark_report_document_fetched(
            &doc.id,
            Some("reports/x.pdf"),
            Some("application/pdf"),
            None,
            Some(10),
        )
        .expect("mark fetched");
}

fn backfill_job_is_pending(state: &AppState, company_id: &str) -> bool {
    state
        .jobs()
        .pending_payload(&format!("{COMPANY_BACKFILL_KIND}:{company_id}"))
        .expect("pending payload query")
        .is_some()
}

#[test]
fn catch_up_enqueues_one_backfill_for_an_automated_company_without_coverage() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company_id = automated_company(&state, "CDR");

    let enqueued = enqueue_company_backfill_catch_up(&state, None);
    assert_eq!(
        enqueued, 1,
        "an automated company with no report history is backfilled"
    );
    assert!(
        backfill_job_is_pending(&state, &company_id),
        "a durable company_backfill job is queued under the stable id"
    );

    // Idempotent: a second pass enqueues nothing (the stable job id already
    // exists) — no re-fetch loop while the backfill is queued/running/done.
    let again = enqueue_company_backfill_catch_up(&state, None);
    assert_eq!(again, 0, "a second catch-up pass does not re-enqueue");
}

#[test]
fn catch_up_skips_a_company_in_mode_off() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    // A company with no autopilot row defaults to `off`; set it explicitly.
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "PKN".to_owned(),
            display_name: "PKN ORLEN S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    state
        .autopilot()
        .set_mode(&company.id, crate::storage::MODE_OFF)
        .expect("mode off");

    let enqueued = enqueue_company_backfill_catch_up(&state, None);
    assert_eq!(enqueued, 0, "an off-mode company is never auto-backfilled");
    assert!(!backfill_job_is_pending(&state, &company.id));
}

#[test]
fn catch_up_skips_a_company_that_already_has_periodic_coverage() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company_id = automated_company(&state, "LPP");
    seed_fetched_periodic_doc(&state, &company_id, "https://x/lpp-ssf-2025.pdf");

    let enqueued = enqueue_company_backfill_catch_up(&state, None);
    assert_eq!(
        enqueued, 0,
        "a company with a fetched periodic report is not re-backfilled"
    );
    assert!(!backfill_job_is_pending(&state, &company_id));
}

/// Card bfc4c98 (a): a NewConnect company's backfill surfaces the typed
/// `unsupported_market` cause AND leaves a durable, machine-readable trail in
/// the app log (`code=unsupported_market`) — never the silent generic failure
/// the card reported. The eligibility check runs before any fetch, so it must
/// NOT flag the shared Bankier-company adapter as unhealthy (that would falsely
/// red the source that serves every GPW company).
#[test]
fn newconnect_backfill_records_typed_cause_and_durable_trail() {
    crate::test_support::install_capture_logger();
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company_id = new_connect_company(&state, "EXC");

    let bankier = FakeBankier {
        page_requests: RefCell::new(0),
    };
    let progress = run_backfill(&state, &company_id, &bankier, &FakeDocs, Duration::ZERO);

    assert_eq!(progress.status, "failed");
    assert_eq!(
        failure_code(&progress).as_deref(),
        Some("unsupported_market"),
        "NewConnect must carry the machine-readable unsupported_market code, got: {:?}",
        progress.error
    );

    // Durable trail: the warn line names the company AND the typed code.
    let logs = crate::test_support::captured_logs()
        .lock()
        .expect("capture buffer");
    assert!(
        logs.iter()
            .any(|line| line.contains(&company_id) && line.contains("code=unsupported_market")),
        "a failed backfill must log its typed code; buffer: {logs:?}"
    );
    drop(logs);

    // Pre-fetch eligibility failure must not touch the shared adapter's health.
    let adapters = state
        .list_source_adapters_with_developer(true)
        .expect("adapters list");
    let bankier_company = adapters
        .iter()
        .find(|adapter| adapter.id == crate::source_adapters::bankier_company::ADAPTER_ID)
        .expect("bankier-company adapter exists");
    assert!(
        bankier_company
            .last_error
            .as_deref()
            .unwrap_or("")
            .is_empty(),
        "an ineligible-market backfill must not flag the shared adapter, got: {:?}",
        bankier_company.last_error
    );
}

/// Card bfc4c98 (a): a genuine adapter-interaction failure (the Bankier page
/// could not be resolved) records a durable **source outcome** on the adapter —
/// its `last_error` carries the typed code, so the failure is queryable, not
/// only in the transient progress row.
#[test]
fn adapter_fault_backfill_records_source_outcome() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company_id = gpw_company_without_identifiers(&state, "NOID");

    let progress = run_backfill(
        &state,
        &company_id,
        &FakeBankierNoIdentifiers,
        &FakeDocs,
        Duration::ZERO,
    );

    assert_eq!(progress.status, "failed");
    assert_eq!(
        failure_code(&progress).as_deref(),
        Some("no_bankier_page"),
        "a missing Bankier page must carry the no_bankier_page code, got: {:?}",
        progress.error
    );

    let adapters = state
        .list_source_adapters_with_developer(true)
        .expect("adapters list");
    let bankier_company = adapters
        .iter()
        .find(|adapter| adapter.id == crate::source_adapters::bankier_company::ADAPTER_ID)
        .expect("bankier-company adapter exists");
    assert!(
        bankier_company
            .last_error
            .as_deref()
            .is_some_and(|error| error.starts_with("no_bankier_page")),
        "an adapter-fault backfill must record a typed source outcome, got: {:?}",
        bankier_company.last_error
    );
    assert!(
        bankier_company.last_error_at.is_some(),
        "the recorded source outcome must be timestamped"
    );
}

/// Card bfc4c98 (b): the four distinct backfill failure causes stay
/// distinguishable — each carries its own machine-readable code, never
/// collapsed into one generic "failed".
#[test]
fn distinct_backfill_failure_causes_stay_distinguishable() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // (1) unsupported market — NewConnect, rejected before any fetch.
    let nc = new_connect_company(&state, "NCX");
    let unsupported = run_backfill(
        &state,
        &nc,
        &FakeBankier {
            page_requests: RefCell::new(0),
        },
        &FakeDocs,
        Duration::ZERO,
    );

    // (2) not tracked — no company row for the id.
    let not_tracked = run_backfill(
        &state,
        "company_missing",
        &FakeBankier {
            page_requests: RefCell::new(0),
        },
        &FakeDocs,
        Duration::ZERO,
    );

    // (3) no Bankier page — identifier-less company page.
    let no_id = gpw_company_without_identifiers(&state, "NOPG");
    let no_page = run_backfill(
        &state,
        &no_id,
        &FakeBankierNoIdentifiers,
        &FakeDocs,
        Duration::ZERO,
    );

    // (4) parse error — malformed listing JSON.
    let badjson = tracked_company_with_ticker(&state, "BADJ");
    let parse_err = run_backfill(
        &state,
        &badjson,
        &FakeBankierBadJson,
        &FakeDocs,
        Duration::ZERO,
    );

    for progress in [&unsupported, &not_tracked, &no_page, &parse_err] {
        assert_eq!(progress.status, "failed", "each cause is a failure");
    }

    let codes: Vec<Option<String>> = [&unsupported, &not_tracked, &no_page, &parse_err]
        .iter()
        .map(|progress| failure_code(progress))
        .collect();
    assert_eq!(
        codes,
        vec![
            Some("unsupported_market".to_owned()),
            Some("not_tracked".to_owned()),
            Some("no_bankier_page".to_owned()),
            Some("parse_error".to_owned()),
        ],
        "each distinct cause must carry its own machine-readable code"
    );

    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(
        unique.len(),
        4,
        "distinct causes must never collapse into one message: {codes:?}"
    );
}

/// Card bfc4c98 (b, cause 4): a page fetched with zero komunikaty is an honest
/// empty result — it completes with zero items, never masquerading as a
/// failure and never as a silent success with a hidden error.
#[test]
fn empty_listing_completes_without_collapsing_into_failure() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company_id = tracked_company_with_ticker(&state, "EMPT");

    let progress = run_backfill(
        &state,
        &company_id,
        &FakeBankierEmpty,
        &FakeDocs,
        Duration::ZERO,
    );

    assert_eq!(
        progress.status, "completed",
        "an empty listing completes honestly, error: {:?}",
        progress.error
    );
    assert_eq!(progress.items_ingested, 0);
    assert!(
        progress.error.is_none(),
        "a completed empty backfill carries no error"
    );
}

#[test]
fn run_company_backfill_job_rejects_a_malformed_payload() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let error = run_company_backfill_job(&state, "{}").expect_err("missing companyId is an error");
    assert!(
        error.contains("companyId"),
        "error names the missing field: {error}"
    );
}
