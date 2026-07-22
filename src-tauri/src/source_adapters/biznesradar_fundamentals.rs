//! BiznesRadar fundamentals **witness** ([ADR 0085](../../../docs/adr/0085-biznesradar-fundamentals-witness.md),
//! realizing [ADR 0061](../../../docs/adr/0061-deterministic-fundamentals-data-gathering.md)
//! decision 4).
//!
//! With the in-app AI tier retired (ADR 0084), this is the fundamentals
//! pipeline's LAST layer — and it is deliberately the weakest-privileged one.
//! Its entire job is **corroboration**:
//!
//! - it may never create a fact on its own from this seam,
//! - it may never overwrite or "correct" a primary value,
//! - agreement raises confidence (`accepted_via_witness`); disagreement leaves
//!   the primary value in place and raises a reviewable diff.
//!
//! Scope (ADR 0085, resolved at sign-off): the witness runs where the primary
//! tier is `Pdf`/`EspiCoverNote` and is **skipped for `Esef`** — issuer-tagged
//! IFRS data is already near-certain, so spending the one-fetch-per-company-per-
//! day budget re-confirming it buys nothing. `run_structured_extraction` enforces
//! this by routing: the ESEF route and the PDF route are mutually exclusive
//! there, so "no PDF text" means "no witness fetch", decided BEFORE any request.
//!
//! Politeness (ADR 0085 decision 3): at most one page fetch per tracked company
//! per day, cached in `fundamentals_witness_pages` so a re-run inside the window
//! reuses the body. Fetch failures and no-coverage landings are cached too, so a
//! dead slug costs one request per day, not one per report document.
//!
//! Slug mapping (ADR 0085 decision 4): the GPW ticker is appended to the base
//! path and BiznesRadar 301-redirects it to the canonical slug
//! (`CDPROJEKT` -> `CD-PROJEKT`); `reqwest` follows redirects by default. This is
//! exactly the mechanism the two live BiznesRadar adapters use — there is no
//! separate mapping table to maintain, and an unresolvable slug serves the
//! generic landing, which is "no witness available", not an error.
//!
//! Parsing reuses the source-generic aggregator-table parser
//! ([`crate::fundamentals::extraction::html`]); this module adds no second
//! parser and no second HTTP path.

use super::USER_AGENT;
use crate::fundamentals::extraction::html::{parse_html_financials, AggregatorColumn};
use crate::fundamentals::extraction::ExtractedFact;
use crate::fundamentals::validation::{cross_check_prior, CrossCheck, FactSet, Outcome, Tolerance};
use crate::storage::{AggregatorPageKind, WitnessPageStatus};

/// Registry id for the fundamentals adapter (catalog row seeded by migration 0104).
pub const ADAPTER_ID: &str = "biznesradar-fundamenty";
/// Human catalog name.
pub const DISPLAY_NAME: &str = "BiznesRadar Wyniki Finansowe";
/// Income-statement page base (`raporty-finansowe-rachunek-zyskow-i-strat`); the
/// tracked company's GPW ticker is appended (BiznesRadar redirects it to the
/// canonical slug). Kept as `SOURCE_URL` for the registry descriptor.
pub const SOURCE_URL: &str =
    "https://www.biznesradar.pl/raporty-finansowe-rachunek-zyskow-i-strat/";
/// Balance-sheet page base (`raporty-finansowe-bilans`) — robots-allowed (ADR
/// 0085 robots quote; ADR 0086 decision 2).
pub const BALANCE_URL: &str = "https://www.biznesradar.pl/raporty-finansowe-bilans/";
/// Cash-flow page base (`raporty-finansowe-przeplywy-pieniezne`) — robots-allowed.
pub const CASHFLOW_URL: &str = "https://www.biznesradar.pl/raporty-finansowe-przeplywy-pieniezne/";
/// Attribution shown wherever an aggregator value is surfaced.
pub const ATTRIBUTION: &str = "BiznesRadar.pl — Wyniki finansowe";
/// The cadence window: one page fetch per tracked company **per page kind** per
/// day (ADR 0085 decision 3, widened to three pages by ADR 0086 decision 2). A
/// re-run inside the window reads the cache instead of the host.
pub const CADENCE_SECONDS: i64 = 86_400;

/// The public page base for one report kind. The GPW ticker is appended; the host
/// 301-redirects it to the canonical slug (ADR 0085 decision 4).
pub fn page_base(kind: AggregatorPageKind) -> &'static str {
    match kind {
        AggregatorPageKind::Income => SOURCE_URL,
        AggregatorPageKind::Balance => BALANCE_URL,
        AggregatorPageKind::Cashflow => CASHFLOW_URL,
    }
}

/// The canonical page URL for a (kind, ticker). Public so the cache row and the
/// log line name the same URL the fetch used.
pub fn page_url_for(kind: AggregatorPageKind, ticker: &str) -> String {
    format!("{}{ticker}", page_base(kind))
}

/// Why the witness produced no facts. Every variant means "witness unavailable"
/// — never "witness agreed" (ADR 0085 decision 5). These strings are typed
/// markers, not prose (ADR 0084 decision 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessUnavailable {
    /// No fetcher is installed (the app was not bootstrapped with one).
    NotConfigured,
    /// The company has no ticker to build a page URL from.
    NoSlug,
    /// The request failed (network, timeout, non-2xx).
    FetchFailed,
    /// The host served a page with no financial report table (unresolvable slug
    /// or a genuinely uncovered company).
    NoCoverage,
    /// The page parsed but carried no tracked metric for the requested period.
    NoFactsForPeriod,
}

impl WitnessUnavailable {
    pub fn as_str(self) -> &'static str {
        match self {
            WitnessUnavailable::NotConfigured => "witness_not_configured",
            WitnessUnavailable::NoSlug => "witness_no_slug",
            WitnessUnavailable::FetchFailed => "witness_fetch_failed",
            WitnessUnavailable::NoCoverage => "witness_no_coverage",
            WitnessUnavailable::NoFactsForPeriod => "witness_no_facts_for_period",
        }
    }
}

/// What the witness seam resolved for one extraction attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WitnessResolution {
    /// The witness is not applicable to this run at all — the primary tier is
    /// `Esef` (ADR 0085, resolved at sign-off). Structurally distinct from
    /// [`WitnessResolution::Unavailable`]: nothing was attempted, nothing failed,
    /// and **no fetch happened**.
    Skipped,
    /// The witness produced facts for the requested period. `page_url` is the
    /// aggregator page they came from — carried so a fallback-sourced fact can
    /// cite its third-party origin (ADR 0085 amendment conditions 2 and 4:
    /// attribution stays visible wherever the value surfaces).
    Facts {
        facts: Vec<ExtractedFact>,
        page_url: String,
    },
    /// The witness could not speak. Degradation, never a failure.
    Unavailable(WitnessUnavailable),
}

impl WitnessResolution {
    /// The witness facts, if any — the shape [`crate::fundamentals::extraction::pipeline::PipelineInput`]
    /// takes. `Skipped`/`Unavailable` both yield `None`, which is exactly the
    /// pre-ADR-0085 behavior: the pipeline decides without a witness.
    pub fn facts(&self) -> Option<&[ExtractedFact]> {
        match self {
            WitnessResolution::Facts { facts, .. } => Some(facts),
            _ => None,
        }
    }

    /// Stable marker for logs/outcome detail.
    pub fn as_str(&self) -> &'static str {
        match self {
            WitnessResolution::Skipped => "witness_skipped_esef",
            WitnessResolution::Facts { .. } => "witness_available",
            WitnessResolution::Unavailable(reason) => reason.as_str(),
        }
    }
}

/// Fetches a raw fundamentals report page for one ticker. A trait so the parse
/// and the cadence logic are tested against stored sample pages with no network.
pub trait FundamentalsWitnessFetcher: Send + Sync {
    /// The income-statement page — the ADR 0085 witness page. The one required
    /// method, so existing single-page fetchers keep working unchanged.
    fn fetch_fundamentals(&self, ticker: &str) -> Result<String, String>;

    /// Fetch one report `kind` for `ticker` (ADR 0086 decision 2). The default
    /// delegates to [`FundamentalsWitnessFetcher::fetch_fundamentals`] (ignoring
    /// the kind) — enough for a stub fetcher that replays one canned body; the
    /// production fetcher OVERRIDES this to build the per-kind URL.
    fn fetch_page(&self, _kind: AggregatorPageKind, ticker: &str) -> Result<String, String> {
        self.fetch_fundamentals(ticker)
    }
}

/// Production fetcher: a single GET (redirects followed) with UA + timeout, the
/// same shape as the two live BiznesRadar adapters. Installed only by the real
/// app bootstrap (`AppState::with_pool`).
pub struct HttpFundamentalsWitnessFetcher;

impl FundamentalsWitnessFetcher for HttpFundamentalsWitnessFetcher {
    fn fetch_fundamentals(&self, ticker: &str) -> Result<String, String> {
        self.fetch_page(AggregatorPageKind::Income, ticker)
    }

    fn fetch_page(&self, kind: AggregatorPageKind, ticker: &str) -> Result<String, String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|error| error.to_string())?;
        client
            .get(page_url_for(kind, ticker))
            .send()
            .and_then(|response| response.error_for_status())
            .map_err(|error| error.to_string())?
            .text()
            .map_err(|error| error.to_string())
    }
}

/// The canonical income-statement page URL for a ticker. Public so the cache row
/// and the log line name the same URL the fetch used.
pub fn page_url(ticker: &str) -> String {
    page_url_for(AggregatorPageKind::Income, ticker)
}

/// Whether a fetched page carries a financial report table at all.
///
/// This is the coverage test, and it is deliberately structural rather than
/// "did we get any facts?": a page that HAS a report table but yields no tracked
/// metric is markup **drift** (the labels or the layout moved) and must be
/// visible as such, whereas the generic landing is honest absence. Collapsing
/// the two would let a drifted parse masquerade as "no coverage" forever — the
/// exact silent-failure mode ADR 0085 §Consequences names.
pub fn has_report_table(html: &str) -> bool {
    use scraper::{Html, Selector};
    let document = Html::parse_document(html);
    let table = Selector::parse("table").expect("static selector");
    let row = Selector::parse("tr").expect("static selector");
    document
        .select(&table)
        .any(|table| table.select(&row).count() >= 2)
}

/// Parse a fetched page into witness facts for one period column.
pub fn parse_witness_page(
    html: &str,
    period_end: &str,
    fiscal_year: i64,
    period_hint: Option<&str>,
) -> Vec<ExtractedFact> {
    parse_html_financials(
        html,
        &AggregatorColumn {
            period_end,
            fiscal_year,
            period_hint,
        },
    )
}

/// The period-column hint for a `period_type`, matched as a normalized substring
/// of the aggregator's header cell (`"Q1 2026"`). Annual columns carry no
/// quarter marker, so they hint nothing and match on the year alone.
fn period_hint(period_type: &str) -> Option<&'static str> {
    match period_type {
        "Q1" | "q1" => Some("Q1"),
        "Q2" | "q2" => Some("Q2"),
        "Q3" | "q3" => Some("Q3"),
        "Q4" | "q4" => Some("Q4"),
        _ => None,
    }
}

/// Cache-ONLY witness resolution for the ingest path — never fetches (ADR 0085
/// decision 3 politeness: no synchronous fetch in the feed path). Reads the
/// shared witness-page cache via a raw `connection` (the ingest-time cover-note
/// tier holds one post-commit, with no pool handle) and, on a fresh `Ok` page,
/// parses the column for the requested period.
///
/// Returns:
/// - `Some(Facts { .. })` — a fresh cached page carried facts for the period;
/// - `Some(Unavailable(reason))` — a fresh cached page, but nothing usable for
///   this period (no coverage, a cached failed fetch, or no row for the period);
/// - `None` — nothing cached fresh: the caller must **defer** the comparison
///   (record "pending"), and must NOT fetch inline.
///
/// The cache read goes through `storage::get_fresh_witness_page` and the
/// parse/period-selection through `parse_witness_page`/`facts_or_unavailable`,
/// so a cached page is always read and interpreted the same way. (The pipeline's
/// fetching `resolve_witness` is retired with ADR 0086 — this ingest-time
/// comparison is the surviving ADR 0085 witness seam.)
pub fn resolve_witness_from_cache(
    connection: &rusqlite::Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
) -> Option<WitnessResolution> {
    let hint = period_hint(period_type);
    match crate::storage::get_fresh_witness_page(connection, company_id, CADENCE_SECONDS) {
        Ok(Some(cached)) => Some(match (cached.status, cached.html.as_deref()) {
            (WitnessPageStatus::Ok, Some(html)) => {
                facts_or_unavailable(html, &cached.page_url, period_end, fiscal_year, hint)
            }
            (WitnessPageStatus::NoCoverage, _) => {
                WitnessResolution::Unavailable(WitnessUnavailable::NoCoverage)
            }
            // `Ok` with no body cannot happen through `put`, but a hand-edited row
            // must degrade rather than be read as agreement.
            _ => WitnessResolution::Unavailable(WitnessUnavailable::FetchFailed),
        }),
        // Nothing cached inside the cadence window → defer. NEVER a fetch here.
        Ok(None) => None,
        Err(error) => {
            log::warn!("module=fundamentals_witness stage=cache_read_ingest error={error}");
            Some(WitnessResolution::Unavailable(
                WitnessUnavailable::FetchFailed,
            ))
        }
    }
}

/// What resolving ONE report page for the BiznesRadar-primary pull produced
/// (ADR 0086 decision 2). Distinct from [`WitnessResolution`]: the pull parses
/// EVERY period column itself ([`crate::fundamentals::extraction::html::parse_all_financials`]),
/// so this carries the raw page body, not facts for a single period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageResolution {
    /// A fresh (cached or just-fetched) page carrying a report table.
    Page { html: String, page_url: String },
    /// The page could not be resolved. Degradation, never a hard failure.
    Unavailable(WitnessUnavailable),
}

/// Resolve one report page for one tracked company, respecting the **per page
/// kind** daily cadence (ADR 0085 decision 3, widened by ADR 0086 decision 2).
///
/// Inside the cadence window the cached page (or cached failure/landing) is
/// reused — never refetched. Outside it, at most ONE fetch happens, serialized
/// by the caller's per-host politeness; the outcome is cached and recorded for
/// honest source freshness, and — matching the ADR 0085 background posture — the
/// pull records the OUTCOME only, never a `record_source_adapter_attempt`
/// refresh-in-flight marker (that would make Sources read as continuously
/// refreshing during a warm-up over N companies).
pub fn resolve_aggregator_page(
    state: &crate::app_state::AppState,
    company_id: &str,
    kind: AggregatorPageKind,
) -> PageResolution {
    let cache = state.fundamentals_witness_cache();

    // --- Inside the cadence window: reuse, never refetch --------------------
    match cache.get_fresh_kind(company_id, kind, CADENCE_SECONDS) {
        Ok(Some(cached)) => {
            return match (cached.status, cached.html) {
                (WitnessPageStatus::Ok, Some(html)) => PageResolution::Page {
                    html,
                    page_url: cached.page_url,
                },
                (WitnessPageStatus::NoCoverage, _) => {
                    PageResolution::Unavailable(WitnessUnavailable::NoCoverage)
                }
                _ => PageResolution::Unavailable(WitnessUnavailable::FetchFailed),
            };
        }
        Ok(None) => {}
        Err(error) => {
            log::warn!(
                "module=fundamentals_aggregator stage=cache_read kind={} error={error}",
                kind.as_str()
            );
            return PageResolution::Unavailable(WitnessUnavailable::FetchFailed);
        }
    }

    // --- Outside the window: at most one fetch ------------------------------
    let Some(fetcher) = state.fundamentals_witness_fetcher() else {
        return PageResolution::Unavailable(WitnessUnavailable::NotConfigured);
    };
    let Some(ticker) = company_ticker(state, company_id) else {
        return PageResolution::Unavailable(WitnessUnavailable::NoSlug);
    };
    let url = page_url_for(kind, &ticker);

    let html = match fetcher.fetch_page(kind, &ticker) {
        Ok(html) => html,
        Err(error) => {
            log::warn!(
                "module=fundamentals_aggregator stage=fetch kind={} ticker={ticker} error={error}",
                kind.as_str()
            );
            let _ = cache.put_kind(company_id, kind, &url, WitnessPageStatus::FetchFailed, None);
            let _ = state.record_source_adapter_error(ADAPTER_ID, &error);
            return PageResolution::Unavailable(WitnessUnavailable::FetchFailed);
        }
    };

    if !has_report_table(&html) {
        let _ = cache.put_kind(company_id, kind, &url, WitnessPageStatus::NoCoverage, None);
        record_success(state);
        return PageResolution::Unavailable(WitnessUnavailable::NoCoverage);
    }

    let _ = cache.put_kind(company_id, kind, &url, WitnessPageStatus::Ok, Some(&html));
    record_success(state);
    PageResolution::Page {
        html,
        page_url: url,
    }
}

/// Cross-check a primary fact set against a BiznesRadar aggregator column, with
/// the **aggregator-zero guard** applied ([ADR 0085](../../../docs/adr/0085-biznesradar-fundamentals-witness.md)
/// amendment, 2026-07-21).
///
/// Convention (ADR 0085 decision 2): `cross_check_prior(primary, aggregator)`
/// sets `expected = aggregator`, `actual = primary`. An aggregator cell of
/// **exactly `0`** against a non-zero primary parse is a scrape/cache artifact —
/// an empty / unreported / wrong-period cell BiznesRadar renders as `0`, never a
/// filed value (BFT H1 2025 `net_profit`: aggregator `0` vs the filed
/// 242 454 000). An empty/zero aggregator cell is never evidence, so such
/// `Fail`s are dropped (and logged); they must never block emission.
///
/// Genuine disagreements (both sides non-zero, beyond tolerance) and true
/// agreements (`0` vs `0` → `Pass`, residual 0) are untouched. The guard is keyed
/// on the aggregator (`expected`) side, so it protects any path where a
/// BiznesRadar cell is the value being compared — the witness role today and the
/// core-KPI primary-write path once BiznesRadar is promoted (ADR 0086).
pub fn witness_cross_check(
    primary: &FactSet,
    aggregator: &FactSet,
    tol: &Tolerance,
) -> Vec<CrossCheck> {
    let mut checks = cross_check_prior(primary, aggregator, tol);
    checks.retain(|check| match &check.outcome {
        Outcome::Fail {
            expected, actual, ..
        } if expected.is_zero() && !actual.is_zero() => {
            log::debug!(
                "module=fundamentals_witness stage=zero_witness_skip metric={} actual={actual}",
                check.metric_key
            );
            false
        }
        _ => true,
    });
    checks
}

/// Parse, and report honest absence when the page covers no tracked metric for
/// the period (an old company with no such column, or markup drift).
fn facts_or_unavailable(
    html: &str,
    page_url: &str,
    period_end: &str,
    fiscal_year: i64,
    hint: Option<&str>,
) -> WitnessResolution {
    let facts = parse_witness_page(html, period_end, fiscal_year, hint);
    if facts.is_empty() {
        WitnessResolution::Unavailable(WitnessUnavailable::NoFactsForPeriod)
    } else {
        WitnessResolution::Facts {
            facts,
            page_url: page_url.to_owned(),
        }
    }
}

/// Mark the witness adapter healthy on Sources (`last_success_at`) — DoD §C.
/// Best-effort: a bookkeeping failure must never affect the extraction verdict.
fn record_success(state: &crate::app_state::AppState) {
    let fetched_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned());
    let _ = state.record_source_outcome_for_adapter(ADAPTER_ID, &fetched_at, 1, 0, 0, 0);
}

/// The GPW ticker for a company, or `None` when it has none — the "unresolvable
/// slug" state ADR 0085 decision 4 treats as normal.
fn company_ticker(state: &crate::app_state::AppState, company_id: &str) -> Option<String> {
    // Tracked companies are a small set (tens) and this runs at most once per
    // company per day, on the miss side of the cadence cache — the list read is
    // cheaper than adding a one-caller storage accessor.
    let company = state
        .list_companies()
        .ok()?
        .into_iter()
        .find(|company| company.id == company_id)?;
    let ticker = company.ticker.trim().to_owned();
    (!ticker.is_empty()).then_some(ticker)
}

#[cfg(test)]
mod tests;
