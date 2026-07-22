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
    self, BankierCompanyError, BankierCompanyFetcher, BankierCompanyParseError,
    BankierCompanyTarget, HttpBankierCompanyFetcher,
};
use crate::storage::{AppState, BackfillProgress};

/// Machine-readable backfill failure reason codes (card bfc4c98). Every failed
/// backfill records `BackfillProgress.error` as `"<code>: <human detail>"`, so
/// the four distinct causes stay distinguishable and the UI maps each prefix to a
/// cause-specific localized message — never one collapsed "backfill failed".
pub mod reason {
    /// The company's market has no history-capable source adapter (e.g.
    /// NewConnect today — the Bankier company path serves GPW only). Surfaced
    /// before any fetch; not an adapter fault.
    pub const UNSUPPORTED_MARKET: &str = "unsupported_market";
    /// No Bankier-company backfill target for the id (unknown / untracked
    /// company). Not an adapter fault.
    pub const NOT_TRACKED: &str = "not_tracked";
    /// The company's Bankier page could not be resolved — a missing or renamed
    /// slug / tag id (no page found for this company).
    pub const NO_BANKIER_PAGE: &str = "no_bankier_page";
    /// The Bankier request itself failed (HTTP status / transport / timeout).
    pub const HTTP_ERROR: &str = "http_error";
    /// A Bankier page was fetched but could not be parsed (malformed listing).
    pub const PARSE_ERROR: &str = "parse_error";
    /// An internal/storage error unrelated to the Bankier source.
    pub const INTERNAL: &str = "internal";
}

/// Map a Bankier fetch error to its typed backfill reason code and human detail.
/// A transport failure, a missing company page, and a malformed page stay three
/// distinct causes (card bfc4c98) — never one opaque string.
fn classify_fetch_error(error: &BankierCompanyError) -> (&'static str, String) {
    let code = match error {
        BankierCompanyError::Request(_) => reason::HTTP_ERROR,
        BankierCompanyError::Parse(
            BankierCompanyParseError::MissingTagId | BankierCompanyParseError::MissingSlug,
        ) => reason::NO_BANKIER_PAGE,
        BankierCompanyError::Parse(BankierCompanyParseError::Json(_)) => reason::PARSE_ERROR,
        BankierCompanyError::TimestampFormat(_) | BankierCompanyError::Url(_) => reason::INTERNAL,
    };
    (code, error.to_string())
}

/// Fallback backfill depth in years, used only when the persisted
/// `backfill_years` setting cannot be read (ADR 0077 §3). The live depth is the
/// user-configurable setting (default 3, clamped 1–10); this const is the
/// last-resort default so a settings read error never blocks the fetch.
pub const BACKFILL_YEARS: i64 = 3;
/// Durable-queue job kind for an automatic per-company report-history backfill
/// (v0.57 catch-up, ADR 0077 amendment). Payload `{companyId}`. Drains on the
/// **sources** lane and serializes on the Bankier-company source lock (ADR 0059),
/// so an automatic backfill never races the scheduled per-company refresh.
pub const COMPANY_BACKFILL_KIND: &str = "company_backfill";
/// Retry budget for one automatic backfill (transient Bankier/network failures
/// ride the queue's capped backoff; a terminal outcome is recorded on the
/// progress row + chained sweep).
const COMPANY_BACKFILL_MAX_ATTEMPTS: i64 = 3;
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
        truncated: false,
        chained_sweep_id: None,
        error: None,
        started_at: started_at.clone(),
        updated_at: started_at.clone(),
    };
    state.set_backfill_progress(progress.clone());

    // Market eligibility (T-A4, card bfc4c98): derive the history-capable markets
    // from `source_adapter_markets`, not a hardcoded exchange. A company on a
    // market with no history-capable source adapter fails with the
    // machine-readable `unsupported_market` prefix the UI maps to a localized
    // message, instead of the misleading "not a tracked GPW company".
    match state.backfill_market_status(company_id) {
        Ok(crate::storage::BackfillMarketStatus::Ineligible { exchange }) => {
            return fail(
                state,
                &mut progress,
                reason::UNSUPPORTED_MARKET,
                &format!("{exchange} has no history-capable source adapter"),
                false,
            );
        }
        Ok(_) => {}
        Err(error) => {
            return fail(
                state,
                &mut progress,
                reason::INTERNAL,
                &error.to_string(),
                false,
            )
        }
    }

    let target = match find_target(state, company_id) {
        Ok(Some(target)) => target,
        Ok(None) => {
            return fail(
                state,
                &mut progress,
                reason::NOT_TRACKED,
                "company is not a tracked company",
                false,
            )
        }
        Err(error) => return fail(state, &mut progress, reason::INTERNAL, &error, false),
    };

    // Backfill depth is user-configurable (ADR 0077 §3); fall back to the const
    // only if the settings read fails so the fetch is never blocked on it.
    let years = state
        .get_settings()
        .map(|settings| settings.backfill_years)
        .unwrap_or(BACKFILL_YEARS);
    let cutoff = backfill_cutoff(years);
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
                truncated: false,
                chained_sweep_id: None,
                error: None,
                started_at: progress_started.clone(),
                updated_at: now_rfc3339(),
            });
        },
    ) {
        Ok(result) => result,
        Err(error) => {
            // A genuine adapter-interaction fault (transport / missing page /
            // unparseable listing): record a durable, typed source outcome so the
            // failure is queryable, not only in the transient progress row.
            let (code, detail) = classify_fetch_error(&error);
            return fail(state, &mut progress, code, &detail, true);
        }
    };

    progress.pages_fetched = stats.pages_fetched;
    progress.detail_errors = stats.detail_errors;
    progress.truncated = stats.truncated;
    progress.updated_at = now_rfc3339();
    state.set_backfill_progress(progress.clone());

    // Ingest through the normal path: dedup, classification, and attachment registration all
    // apply, so re-running the backfill produces no duplicates (ADR 0036).
    match state.ingest_bankier_company_items(&items) {
        Ok(result) => {
            progress.items_ingested = result.items_created;
        }
        Err(error) => {
            return fail(
                state,
                &mut progress,
                reason::INTERNAL,
                &error.to_string(),
                false,
            )
        }
    }

    // Fetch files for periodic-report attachments registered during ingestion.
    match crate::report_documents_capture::fetch_pending_attachments(state, document_fetcher) {
        Ok(summary) => {
            progress.documents_stored = summary.stored;
            progress.detail_errors += summary.failed;
        }
        Err(error) => {
            return fail(
                state,
                &mut progress,
                reason::INTERNAL,
                &error.to_string(),
                false,
            )
        }
    }

    progress.status = "completed".to_owned();
    progress.updated_at = now_rfc3339();

    // Chain a history sweep so a completed backfill automatically extracts the
    // periods it just fetched (ADR 0077 §3, closes gap 1). Best-effort: a
    // chaining failure is logged, never surfaced as a backfill error — the
    // fetched documents are already stored and the sweep can be re-run manually.
    // The sweep row is created eagerly here, so its id is known before this
    // command returns; thread it onto the result so the coverage panel polls
    // THIS sweep specifically instead of guessing from "the latest sweep".
    match crate::jobs::history_sweep::enqueue_history_sweep(state, company_id, "backfill") {
        Ok(sweep) => progress.chained_sweep_id = Some(sweep.id),
        Err(error) => {
            log::warn!("backfill {company_id}: failed to chain history sweep: {error}");
        }
    }

    state.set_backfill_progress(progress.clone());
    progress
}

/// Enqueue an automatic report-history backfill for every automated company that
/// has NO fetched periodic report — the v0.57 catch-up that makes backfill happen
/// without the user clicking (ADR 0077 amendment: backfill is automatic for
/// automated companies). Runs at app startup and after every successful source
/// refresh, mirroring the ownership / management-holdings catch-up parity.
///
/// Idempotent by two independent guards:
/// 1. **Coverage predicate** — a company that already has a fetched periodic
///    report is not selected (`companies_lacking_periodic_coverage`).
/// 2. **Stable per-company job id** enqueued with INSERT-OR-IGNORE semantics
///    ([`crate::storage::JobsStore::enqueue`]) — a company with a queued, running,
///    or completed backfill is never re-enqueued, so a genuinely empty issuer is
///    attempted **once**, not on every refresh (no re-fetch loop).
///
/// A company in mode `off` (or with no autopilot row) is skipped with an explicit
/// logged reason (`automation_off`), never silently dropped (ADR 0077 §3
/// amendment (c) idiom). `company_id = None` scans all companies; `Some` narrows
/// to one. Returns the number of backfills enqueued. Pacing is the ADR 0059 queue
/// serialization (one Bankier backfill at a time via the source lock) — no extra
/// per-pass cap is needed.
pub fn enqueue_company_backfill_catch_up(state: &AppState, company_id: Option<&str>) -> usize {
    let pending = match state.companies_lacking_periodic_coverage(company_id) {
        Ok(pending) => pending,
        Err(error) => {
            log::warn!("module=backfill stage=catch_up selection failed: {error}");
            return 0;
        }
    };
    let mut enqueued = 0usize;
    let mut skipped_off = 0usize;
    for (company, mode) in &pending {
        if mode == crate::storage::MODE_OFF {
            skipped_off += 1;
            continue;
        }
        let job_id = format!("{COMPANY_BACKFILL_KIND}:{company}");
        let payload = serde_json::json!({ "companyId": company }).to_string();
        match state.jobs().enqueue(
            &job_id,
            COMPANY_BACKFILL_KIND,
            &payload,
            COMPANY_BACKFILL_MAX_ATTEMPTS,
        ) {
            Ok(true) => enqueued += 1,
            // Already queued / running / completed under the stable id — the
            // once-ever guard that prevents a re-fetch loop.
            Ok(false) => {}
            Err(error) => {
                log::warn!("module=backfill stage=catch_up enqueue failed for {company}: {error}")
            }
        }
    }
    if skipped_off > 0 {
        log::info!(
            "module=backfill stage=catch_up skipped={skipped_off} reason=automation_off (lack report history but mode=off)"
        );
    }
    enqueued
}

/// Queue entry point for the `company_backfill` job: resolve `companyId` and run
/// the report-history backfill with the live HTTP fetchers. Best-effort —
/// [`backfill_company_history`] records its own errors on the progress row and
/// chains the history sweep, so this returns `Ok` whatever the domain outcome; a
/// malformed payload is the only `Err`.
pub fn run_company_backfill_job(state: &AppState, payload: &str) -> Result<(), String> {
    let parsed: serde_json::Value =
        serde_json::from_str(payload).map_err(|error| error.to_string())?;
    let company_id = parsed
        .get("companyId")
        .and_then(|value| value.as_str())
        .ok_or("company backfill missing companyId")?;
    backfill_company_history(state, company_id);
    Ok(())
}

fn find_target(state: &AppState, company_id: &str) -> Result<Option<BankierCompanyTarget>, String> {
    let targets = state
        .list_bankier_company_targets()
        .map_err(|error| error.to_string())?;
    Ok(targets
        .into_iter()
        .find(|target| target.company_id == company_id))
}

/// Terminate a backfill with a **typed** diagnosis (card bfc4c98). Every failure
/// leaves a production-visible trail — a warn line carrying the machine-readable
/// `code` and the progress row's `error` (`"code: detail"`) the UI maps to a
/// cause-specific message. `record_source_outcome` marks a genuine
/// adapter-interaction fault on the shared Bankier-company adapter so the failure
/// is queryable beyond the transient progress row; it stays `false` for a
/// pre-fetch eligibility failure (unsupported market / untracked company), which
/// is not an adapter fault and must not falsely flag the adapter that serves every
/// GPW company.
fn fail(
    state: &AppState,
    progress: &mut BackfillProgress,
    code: &str,
    detail: &str,
    record_source_outcome: bool,
) -> BackfillProgress {
    let message = format!("{code}: {detail}");
    log::warn!(
        "module=backfill company={} status=failed code={} error={}",
        progress.company_id,
        code,
        detail
    );
    if record_source_outcome {
        if let Err(error) = state.record_source_adapter_error(bankier_company::ADAPTER_ID, &message)
        {
            log::warn!(
                "module=backfill company={} failed to record source outcome: {error}",
                progress.company_id
            );
        }
    }
    progress.status = "failed".to_owned();
    progress.error = Some(message);
    progress.updated_at = now_rfc3339();
    state.set_backfill_progress(progress.clone());
    progress.clone()
}

/// Lower bound for backfill, as `YYYY-MM-DDTHH:MM:SS` to compare against Bankier
/// item dates. `years` is the configured backfill depth (ADR 0077 §3).
fn backfill_cutoff(years: i64) -> String {
    let cutoff = OffsetDateTime::now_utc().saturating_sub(time::Duration::days(years * 365));
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
        let error =
            run_company_backfill_job(&state, "{}").expect_err("missing companyId is an error");
        assert!(
            error.contains("companyId"),
            "error names the missing field: {error}"
        );
    }
}
