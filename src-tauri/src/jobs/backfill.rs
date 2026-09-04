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

/// Direct-activity wrapper for a history backfill (ADR 0109 dec. 3): the
/// automatic queue handler (`CompanyBackfillHandler` → `run_company_backfill_job`)
/// calls [`backfill_company_history`] directly and writes its own occurrence via
/// the queue's dispatch seam, so this wrapper is for the awaited command paths
/// only (the Tauri command and its MCP twin) — nothing double-counts.
pub fn backfill_company_history_direct(state: &AppState, company_id: &str) -> BackfillProgress {
    let identity = state.checkout().ok().and_then(|connection| {
        crate::jobs::activity_identity::identity_for_job(
            COMPANY_BACKFILL_KIND,
            &format!("direct:{COMPANY_BACKFILL_KIND}:{company_id}"),
            &serde_json::json!({ "companyId": company_id }).to_string(),
            &connection,
        )
    });
    let guard = identity.map(|identity| crate::storage::activity_registry::start(state, identity));
    let progress = backfill_company_history(state, company_id);
    if let Some(guard) = guard {
        guard.settle(if progress.status == "failed" {
            Err(progress.error.as_deref().unwrap_or("backfill failed"))
        } else {
            Ok(())
        });
    }
    progress
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
    let progress = backfill_company_history(state, company_id);
    // sol diff R1 #7: `BackfillProgress` used to be discarded, so the queue
    // ALWAYS settled the occurrence `succeeded` even when the backfill
    // itself ended `failed` — an `Err` here enters the ordinary settle/retry
    // path so the ledger records the truthful outcome.
    if progress.status == "failed" {
        return Err(progress
            .error
            .unwrap_or_else(|| "company backfill failed".to_owned()));
    }
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
#[path = "backfill_tests.rs"]
mod tests;
