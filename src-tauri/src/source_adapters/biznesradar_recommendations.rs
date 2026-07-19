//! BiznesRadar "Rekomendacje" analyst-recommendation source (ADR 0073, plan v0.58
//! A2).
//!
//! For each tracked GPW company it fetches BiznesRadar's public
//! `/rekomendacje-spolki/<ticker>` page (the GPW ticker 301-redirects to the
//! canonical slug — reqwest follows redirects by default, same mechanism the
//! ownership adapter relies on), parses the `recommendations` table, and feeds the
//! parsed rows into the append-only recommendation store
//! ([`crate::storage::AnalystRecommendationStore`], slice A1) which derives the
//! direction, dedupes on the natural key, and emits `recommendation_change`
//! signals. Recommendations are third-party opinions, displayed strictly with
//! attribution — never advice (ADR 0073).
//!
//! Source facts (verified 2026-07-19, do not re-derive): the free page carries the
//! ~3–5 most recent recommendations (full history is BR Plus/premium, out of
//! policy scope); our own history accumulates append-only from ingestion start. An
//! unknown path serves a generic landing (`table.recommendations` absent) — that
//! is a clean EMPTY outcome, not an error. robots.txt is fully permissive for
//! `/rekomendacje-spolki/`.
//!
//! Table columns: `Rodzaj | Cena docelowa* | Kurs aktualny | CD/K** | Kurs z dnia
//! wydania* | Data upublicznienia | Autor | Plik`. We store the rating (verbatim),
//! target price, price-at-issue, publication date, author (split into analyst +
//! firm) and the broker PDF; the derived "Kurs aktualny" and "CD/K" columns are
//! skipped.

use scraper::{Html, Selector};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::USER_AGENT;
use crate::storage::AnalystRecommendationEntry;

/// Registry id for the BiznesRadar analyst-recommendation adapter (catalog row
/// seeded by migration 0100).
pub const ADAPTER_ID: &str = "biznesradar-rekomendacje";
/// Human catalog name.
pub const DISPLAY_NAME: &str = "BiznesRadar Rekomendacje";
/// Public page base; the tracked company's GPW ticker is appended (BiznesRadar
/// redirects the ticker to its canonical slug).
pub const SOURCE_URL: &str = "https://www.biznesradar.pl/rekomendacje-spolki/";
/// Attribution shown on emitted feed items.
pub const ATTRIBUTION: &str = "BiznesRadar.pl — Rekomendacje";
/// Host prefix for absolutizing relative broker-PDF hrefs (`/storage/...`).
const HOST: &str = "https://www.biznesradar.pl";
/// Default target currency for GPW recommendations.
const CURRENCY_PLN: &str = "PLN";
/// Politeness delay between per-company page fetches.
const REQUEST_DELAY: std::time::Duration = std::time::Duration::from_secs(2);
/// The recommendations table has exactly these columns; a data row with fewer is
/// structural drift (fail loud rather than mis-index).
const EXPECTED_COLUMNS: usize = 8;

#[derive(Debug, Error)]
pub enum BiznesRadarRecommendationsError {
    #[error("BiznesRadar rekomendacje HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("BiznesRadar rekomendacje parse failed: {0}")]
    Parse(String),
}

/// Fetches the raw rekomendacje HTML for one ticker. Trait so parsing is tested
/// against sample bytes without network access.
pub trait BiznesRadarRecommendationsFetcher {
    fn fetch_recommendations(
        &self,
        ticker: &str,
    ) -> Result<String, BiznesRadarRecommendationsError>;
}

/// Production fetcher: a single GET (redirects followed) with UA + timeout.
pub struct HttpBiznesRadarRecommendationsFetcher;

impl BiznesRadarRecommendationsFetcher for HttpBiznesRadarRecommendationsFetcher {
    fn fetch_recommendations(
        &self,
        ticker: &str,
    ) -> Result<String, BiznesRadarRecommendationsError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let url = format!("{SOURCE_URL}{ticker}");
        Ok(client.get(url).send()?.error_for_status()?.text()?)
    }
}

/// Parse the `recommendations` table into recommendation entries for one page.
///
/// `page_url` is the canonical page URL, stored on each entry as `source_url`.
/// When the page has NO `table.recommendations` (the generic landing served for an
/// unknown slug) this returns an empty vec — a clean no-op, not an error. A
/// structurally broken row (fewer than the expected columns, an empty rating, or
/// an unparsable publication date) is a `Parse` error: the source structure
/// drifted and must be surfaced rather than silently mis-ingested.
pub fn parse_recommendations(
    html: &str,
    page_url: &str,
) -> Result<Vec<AnalystRecommendationEntry>, BiznesRadarRecommendationsError> {
    let document = Html::parse_document(html);
    let table_selector = selector("table.recommendations")?;
    let Some(table) = document.select(&table_selector).next() else {
        // Landing / no recommendations table → clean empty outcome.
        return Ok(Vec::new());
    };

    let row_selector = selector("tr")?;
    let cell_selector = selector("td")?;
    let th_selector = selector("th")?;
    let span_selector = selector("span")?;
    let anchor_selector = selector("a")?;

    let mut entries = Vec::new();
    for row in table.select(&row_selector) {
        // Header rows carry `<th>` — skip them.
        if row.select(&th_selector).next().is_some() {
            continue;
        }
        let cells: Vec<scraper::ElementRef> = row.select(&cell_selector).collect();
        if cells.is_empty() {
            continue; // spacer / non-data row
        }
        if cells.len() < EXPECTED_COLUMNS {
            return Err(BiznesRadarRecommendationsError::Parse(format!(
                "recommendation row has {} columns, expected {EXPECTED_COLUMNS}",
                cells.len()
            )));
        }

        // 0: Rodzaj — the rating span text, verbatim.
        let rating = cells[0]
            .select(&span_selector)
            .next()
            .map(|span| collapse_ws(&span.text().collect::<String>()))
            .unwrap_or_else(|| collapse_ws(&cells[0].text().collect::<String>()));
        if rating.is_empty() {
            return Err(BiznesRadarRecommendationsError::Parse(
                "recommendation row has an empty rating".to_owned(),
            ));
        }

        // 1: Cena docelowa — target price (optional). 4: Kurs z dnia wydania —
        // price at issue (optional). Columns 2 (Kurs aktualny) and 3 (CD/K) are
        // derived and deliberately skipped.
        let target_price = cell_text_opt(&cells[1]);
        let price_at_issue = cell_text_opt(&cells[4]);

        // 5: Data upublicznienia — Polish "18 cze 2026 08:40".
        let raw_date = collapse_ws(&cells[5].text().collect::<String>());
        let published_at = parse_polish_datetime(&raw_date).ok_or_else(|| {
            BiznesRadarRecommendationsError::Parse(format!(
                "unparsable publication date: {raw_date:?}"
            ))
        })?;

        // 6: Autor — "Name (Firm)" → analyst + firm (firm required).
        let (analyst, firm) = split_author(&collapse_ws(&cells[6].text().collect::<String>()));

        // 7: Plik — broker PDF href, absolutized. None when no anchor.
        let report_url = cells[7]
            .select(&anchor_selector)
            .next()
            .and_then(|a| a.value().attr("href"))
            .map(absolutize_href);

        entries.push(AnalystRecommendationEntry {
            firm,
            analyst,
            rating,
            target_price,
            target_currency: Some(CURRENCY_PLN.to_owned()),
            price_at_issue,
            published_at,
            source_url: page_url.to_owned(),
            report_url,
        });
    }

    Ok(entries)
}

fn selector(spec: &str) -> Result<Selector, BiznesRadarRecommendationsError> {
    Selector::parse(spec).map_err(|error| BiznesRadarRecommendationsError::Parse(error.to_string()))
}

fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Trimmed cell text, or `None` when empty (an omitted optional value).
fn cell_text_opt(cell: &scraper::ElementRef) -> Option<String> {
    let text = collapse_ws(&cell.text().collect::<String>());
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Absolutize a broker-file href: `/storage/...` gets the host prefix; an
/// already-absolute URL is kept as-is.
fn absolutize_href(href: &str) -> String {
    let href = href.trim();
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_owned()
    } else {
        format!("{HOST}{href}")
    }
}

/// Split a BiznesRadar author cell into `(analyst, firm)`. The page prints
/// "Analyst Name (Firm)"; the firm is required, so a cell with no parentheses is
/// treated as the firm with no named analyst.
fn split_author(raw: &str) -> (Option<String>, String) {
    if let (Some(open), Some(close)) = (raw.rfind('('), raw.rfind(')')) {
        if open < close {
            let firm = raw[open + 1..close].trim();
            let analyst = raw[..open].trim();
            if !firm.is_empty() {
                let analyst = (!analyst.is_empty()).then(|| analyst.to_owned());
                return (analyst, firm.to_owned());
            }
        }
    }
    (None, raw.trim().to_owned())
}

/// Parse a BiznesRadar publication datetime ("18 cze 2026 08:40", Polish month
/// abbreviation) into a local ISO-8601 datetime string "YYYY-MM-DDTHH:MM:00".
/// A time is always present on the page; the value is a local wall-clock time
/// (Warsaw) and is intentionally NOT tagged `Z` — it is stored as an unqualified
/// local datetime, which orders lexically and feeds the natural key.
fn parse_polish_datetime(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() != 4 {
        return None;
    }
    let day: u32 = parts[0].parse().ok()?;
    let month = polish_month_number(parts[1])?;
    let year: i32 = parts[2].parse().ok()?;
    if parts[2].len() != 4 || !(1..=31).contains(&day) {
        return None;
    }
    let (hour, minute) = parts[3].split_once(':')?;
    let hour: u32 = hour.parse().ok()?;
    let minute: u32 = minute.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:00"
    ))
}

/// Polish month abbreviation → number, tolerant of the diacritic-free `paz`.
fn polish_month_number(value: &str) -> Option<u32> {
    match value.trim().to_lowercase().as_str() {
        "sty" => Some(1),
        "lut" => Some(2),
        "mar" => Some(3),
        "kwi" => Some(4),
        "maj" => Some(5),
        "cze" => Some(6),
        "lip" => Some(7),
        "sie" => Some(8),
        "wrz" => Some(9),
        "paź" | "paz" => Some(10),
        "lis" => Some(11),
        "gru" => Some(12),
        _ => None,
    }
}

/// Refresh-level [`Fetcher`](crate::jobs::source_refresh::Fetcher) impl: for each
/// tracked company, fetch + parse the rekomendacje page and feed the parsed rows
/// into the append-only recommendation store.
pub struct BiznesRadarRecommendationsAdapter;

impl crate::jobs::source_refresh::Fetcher for BiznesRadarRecommendationsAdapter {
    fn refresh(
        &self,
        state: &crate::app_state::AppState,
        ctx: &crate::jobs::source_refresh::RefreshContext,
    ) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
        refresh_with(&HttpBiznesRadarRecommendationsFetcher, state, ctx, true)
    }
}

/// The refresh body with an injectable page fetcher (testable seam). `polite`
/// toggles the inter-request delay off for tests. A landing page (no table) is a
/// clean per-company no-op; a fetch/parse failure is logged and skipped. If EVERY
/// company failed to fetch/parse (none succeeded), the run errors so a transient
/// upstream/structure fault never marks the adapter healthy (KNF/ownership guard).
pub(crate) fn refresh_with(
    fetcher: &impl BiznesRadarRecommendationsFetcher,
    state: &crate::app_state::AppState,
    ctx: &crate::jobs::source_refresh::RefreshContext,
    polite: bool,
) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
    let _ = state.record_source_adapter_attempt(ADAPTER_ID, ctx.trigger);

    let targets = state
        .list_bankier_company_targets()
        .map_err(|error| error.to_string())?;
    if targets.is_empty() {
        return Ok(crate::jobs::source_refresh::RefreshOutcome::Ingestion(
            crate::jobs::source_refresh::empty_source_result(ADAPTER_ID),
        ));
    }

    let store = state.analyst_recommendations();
    let mut total = crate::jobs::source_refresh::empty_source_result(ADAPTER_ID);
    let mut fetch_failures = 0usize;
    let mut any_success = false;

    for (index, target) in targets.iter().enumerate() {
        if polite && index > 0 {
            std::thread::sleep(REQUEST_DELAY);
        }
        let page_url = format!("{SOURCE_URL}{}", target.ticker);
        let html = match fetcher.fetch_recommendations(&target.ticker) {
            Ok(html) => html,
            Err(error) => {
                fetch_failures += 1;
                log::warn!(
                    "module=analyst_recommendations stage=fetch ticker={} error={error}",
                    target.ticker
                );
                continue;
            }
        };
        let entries = match parse_recommendations(&html, &page_url) {
            Ok(entries) => entries,
            Err(error) => {
                fetch_failures += 1;
                log::warn!(
                    "module=analyst_recommendations stage=parse ticker={} error={error}",
                    target.ticker
                );
                continue;
            }
        };
        any_success = true; // fetch + parse succeeded (empty = clean landing)
        if entries.is_empty() {
            continue;
        }
        let result = store
            .ingest_analyst_recommendations(&target.company_id, &entries)
            .map_err(|error| error.to_string())?;
        total.items_fetched += result.items_fetched;
        total.items_created += result.items_created;
        total.items_matched += result.items_matched;
    }

    if !any_success && fetch_failures > 0 {
        let message = "BiznesRadar rekomendacje fetched no usable pages".to_owned();
        let _ = state.record_source_adapter_error(ADAPTER_ID, &message);
        return Err(message);
    }

    total.fetched_at = Some(
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned()),
    );
    Ok(crate::jobs::source_refresh::RefreshOutcome::Ingestion(
        total,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CDR_SAMPLE: &str = include_str!("../../samples/biznesradar_rekomendacje_cdr.html");
    const VARIANTS_SAMPLE: &str =
        include_str!("../../samples/biznesradar_rekomendacje_variants.html");
    const LANDING_SAMPLE: &str =
        include_str!("../../samples/biznesradar_rekomendacje_landing.html");
    const MALFORMED_SAMPLE: &str =
        include_str!("../../samples/biznesradar_rekomendacje_malformed.html");

    const PAGE_URL: &str = "https://www.biznesradar.pl/rekomendacje-spolki/CDPROJEKT";

    #[test]
    fn parses_the_recommendations_table_golden() {
        let entries = parse_recommendations(CDR_SAMPLE, PAGE_URL).expect("sample should parse");
        assert_eq!(entries.len(), 3);
        insta::assert_debug_snapshot!("golden_biznesradar_recommendations", entries);
    }

    #[test]
    fn preserves_verbatim_rating_and_fields() {
        let entries = parse_recommendations(CDR_SAMPLE, PAGE_URL).expect("parse");
        let first = &entries[0];
        assert_eq!(first.rating, "akumuluj", "rating preserved verbatim");
        assert_eq!(first.target_price.as_deref(), Some("250.00"));
        assert_eq!(first.target_currency.as_deref(), Some("PLN"));
        assert_eq!(first.price_at_issue.as_deref(), Some("224.70"));
        assert_eq!(first.published_at, "2026-06-18T08:40:00");
        assert_eq!(first.analyst.as_deref(), Some("Mateusz Chrzanowski"));
        assert_eq!(first.firm, "Noble Securities");
        assert_eq!(first.source_url, PAGE_URL);
    }

    #[test]
    fn absolutizes_relative_report_url() {
        let entries = parse_recommendations(CDR_SAMPLE, PAGE_URL).expect("parse");
        assert_eq!(
            entries[0].report_url.as_deref(),
            Some("https://www.biznesradar.pl/storage/a/cd/r-/cdr-noble-2026-06-23-0e33ac3.pdf"),
        );
    }

    #[test]
    fn parses_polish_month_abbreviations_including_diacritics() {
        // The CDR sample exercises cze / lut / lis; the variants sample adds
        // paź / lis / sty.
        let cdr = parse_recommendations(CDR_SAMPLE, PAGE_URL).expect("parse");
        assert_eq!(cdr[1].published_at, "2026-02-27T18:05:00"); // 27 lut 2026
        assert_eq!(cdr[2].published_at, "2025-11-26T00:00:00"); // 26 lis 2025

        let variants = parse_recommendations(VARIANTS_SAMPLE, PAGE_URL).expect("parse");
        assert_eq!(variants[0].published_at, "2025-10-07T09:15:00"); // 07 paź 2025
        assert_eq!(variants[2].published_at, "2026-01-03T14:30:00"); // 03 sty 2026
    }

    #[test]
    fn handles_missing_target_missing_pdf_and_parenless_author() {
        let entries = parse_recommendations(VARIANTS_SAMPLE, PAGE_URL).expect("parse");
        assert_eq!(entries.len(), 3);

        // Missing target price → None; rating still verbatim.
        assert_eq!(entries[0].rating, "redukuj");
        assert!(entries[0].target_price.is_none(), "empty target → None");
        assert_eq!(entries[0].firm, "Ipopema Securities");

        // Author with no parentheses → whole is the firm, analyst None.
        assert!(entries[1].analyst.is_none());
        assert_eq!(entries[1].firm, "Erste Group");
        // No anchor in the Plik cell → report_url None.
        assert!(entries[1].report_url.is_none(), "no PDF anchor → None");

        // Already-absolute broker href is kept as-is.
        assert_eq!(
            entries[2].report_url.as_deref(),
            Some("https://broker.example.com/report.pdf")
        );
    }

    #[test]
    fn landing_page_without_table_is_empty_not_an_error() {
        let entries = parse_recommendations(LANDING_SAMPLE, PAGE_URL)
            .expect("a landing page must parse cleanly, not error");
        assert!(entries.is_empty(), "no recommendations table → empty vec");
    }

    #[test]
    fn malformed_row_is_a_parse_error_not_a_panic() {
        let error = parse_recommendations(MALFORMED_SAMPLE, PAGE_URL)
            .expect_err("a structurally broken row must be a Parse error");
        assert!(matches!(error, BiznesRadarRecommendationsError::Parse(_)));
    }

    #[test]
    fn adapter_joins_the_full_refresh_sweep() {
        use crate::jobs::source_refresh::Fetcher;
        assert!(
            BiznesRadarRecommendationsAdapter.joins_full_refresh(),
            "the recommendation source joins the refresh-all sweep"
        );
    }

    // --- Offline refresh integration (stub fetcher; no network) ---

    use crate::app_state::AppState;
    use crate::jobs::source_refresh::{RefreshContext, RefreshOutcome};
    use crate::storage::{open_in_memory_database, NewCompany};
    use std::collections::HashMap;

    struct StubFetcher {
        pages: HashMap<String, Result<String, ()>>,
    }

    impl BiznesRadarRecommendationsFetcher for StubFetcher {
        fn fetch_recommendations(
            &self,
            ticker: &str,
        ) -> Result<String, BiznesRadarRecommendationsError> {
            match self.pages.get(ticker) {
                Some(Ok(html)) => Ok(html.clone()),
                Some(Err(())) | None => Err(BiznesRadarRecommendationsError::Parse(
                    "stub: no page".to_owned(),
                )),
            }
        }
    }

    fn company(state: &AppState, ticker: &str) -> crate::storage::Company {
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
    }

    fn ctx() -> RefreshContext<'static> {
        RefreshContext {
            trigger: "manual",
            date: None,
        }
    }

    #[test]
    fn refresh_ingests_populated_pages_and_treats_landing_as_no_op() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let cdr = company(&state, "CDR");
        company(&state, "LND");
        let mut pages = HashMap::new();
        pages.insert("CDR".to_owned(), Ok(CDR_SAMPLE.to_owned()));
        pages.insert("LND".to_owned(), Ok(LANDING_SAMPLE.to_owned()));
        let fetcher = StubFetcher { pages };

        let outcome =
            refresh_with(&fetcher, &state, &ctx(), false).expect("refresh should succeed");
        let RefreshOutcome::Ingestion(result) = outcome else {
            panic!("expected an ingestion outcome");
        };
        // CDR's 3 rows ingested; the landing page contributed nothing but did not
        // fail the run.
        assert_eq!(result.items_created, 3);

        let stored = state
            .analyst_recommendations()
            .list_analyst_recommendations(&cdr.id)
            .expect("list");
        assert_eq!(stored.len(), 3, "CDR's three recommendations are stored");
        assert_eq!(stored[0].rating, "akumuluj");
    }

    #[test]
    fn refresh_errors_when_every_company_fails() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        company(&state, "CDR");
        let mut pages = HashMap::new();
        pages.insert("CDR".to_owned(), Err(()));
        let fetcher = StubFetcher { pages };

        let result = refresh_with(&fetcher, &state, &ctx(), false);
        assert!(
            result.is_err(),
            "an all-failed run must not mark the adapter healthy"
        );
    }

    /// First real-network validation of the adapter against a throwaway copy of
    /// the maintainer's database (testing.md real-data rule): fetches the live
    /// BiznesRadar recommendation page for every tracked company (politely, 2s
    /// apart) and prints what was ingested. Run it manually:
    ///
    /// ```text
    /// BRAWLER_REAL_DB=private/realdata/worktest_recommendations.sqlite3 \
    ///   cargo test -p brawler --lib biznesradar_recommendations_live_real_data -- --nocapture --ignored
    /// ```
    #[test]
    #[ignore = "live network + real data; needs BRAWLER_REAL_DB (a throwaway copy)"]
    fn biznesradar_recommendations_live_real_data() {
        let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
            eprintln!("SKIP: set BRAWLER_REAL_DB to a throwaway copy");
            return;
        };
        let connection = crate::storage::open_database(&db_path).expect("open real db");
        let state = AppState::new(connection);

        let outcome = refresh_with(&HttpBiznesRadarRecommendationsFetcher, &state, &ctx(), true)
            .expect("live refresh should succeed");
        let RefreshOutcome::Ingestion(result) = outcome else {
            panic!("expected an ingestion outcome");
        };
        eprintln!(
            "== live refresh: fetched={} created={}",
            result.items_fetched, result.items_created
        );

        let targets = state.list_bankier_company_targets().expect("targets");
        let mut companies_with_entries = 0usize;
        let mut total_entries = 0usize;
        for target in &targets {
            let rows = state
                .analyst_recommendations()
                .list_analyst_recommendations(&target.company_id)
                .expect("list");
            if !rows.is_empty() {
                companies_with_entries += 1;
                total_entries += rows.len();
                let newest = &rows[0];
                eprintln!(
                    "{:>5}: {} entries | newest: {} {} target={:?} {} ({})",
                    target.ticker,
                    rows.len(),
                    newest.firm,
                    newest.rating,
                    newest.target_price,
                    newest.published_at,
                    newest.direction,
                );
            }
        }
        eprintln!(
            "== companies with entries: {companies_with_entries}/{} | total entries: {total_entries}",
            targets.len()
        );
        assert!(
            companies_with_entries > 0,
            "a live run over the real watchlist should ingest at least one recommendation"
        );
    }
}
