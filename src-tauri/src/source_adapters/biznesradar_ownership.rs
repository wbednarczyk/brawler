//! BiznesRadar "Akcjonariat" ownership **breadth source** (ADR 0072 §2c as
//! amended 2026-07-16, plan v0.56 T4 stream 3).
//!
//! The automatic breadth source for ownership: for each tracked GPW company it
//! fetches BiznesRadar's public `/akcjonariat/<ticker>` page (server-rendered
//! `qTableFull` table; BiznesRadar resolves the GPW ticker to its canonical slug
//! via redirect — probe 2026-07-16), parses the holder/%capital/%votes rows, and
//! **writes the whole table as a full-picture `aggregator` snapshot** (all rows
//! share one `as_of` = the newest per-row "Data aktualizacji", fallback: the fetch
//! date). A same-basis re-ingest reconciles that basis's row set. Disclosed reports
//! and ESPI filings keep running as depth/freshness; the witness comparison stays
//! but reversed — the disclosed (non-`aggregator`) state now witnesses the
//! aggregator, so the write outcome never depends on the comparison.
//!
//! BiznesRadar chosen over Bankier "Akcjonariat" (probe 2026-07-16): its robots.txt
//! is fully permissive (`Allow: /`, no akcjonariat/gielda restriction) whereas
//! Bankier's is AI-bot-hostile and Bankier is already our primary official-report
//! source (reusing it weakens source independence). Both are server-rendered with
//! separate capital/votes columns; BiznesRadar's independent host is the stronger
//! breadth source with a disclosed witness over it.

use rust_decimal::Decimal;
use scraper::{ElementRef, Html, Selector};
use std::str::FromStr;
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::USER_AGENT;
use crate::storage::WitnessHolder;

/// Registry id for the BiznesRadar ownership adapter.
pub const ADAPTER_ID: &str = "biznesradar-akcjonariat";
/// Human catalog name.
pub const DISPLAY_NAME: &str = "BiznesRadar Akcjonariat";
/// Public page base; the tracked company's GPW ticker is appended (BiznesRadar
/// redirects the ticker to its canonical slug).
pub const SOURCE_URL: &str = "https://www.biznesradar.pl/akcjonariat/";
/// Attribution shown for witness observability.
pub const ATTRIBUTION: &str = "BiznesRadar.pl — Akcjonariat";

/// Politeness delay between per-company page fetches.
const REQUEST_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

/// Disclosed-capital sum (percent) above which a parsed basis is implausible —
/// write nothing (the margin over 100% absorbs rounding / preferred-share quirks).
const IMPLAUSIBLE_CAPITAL_SUM_PCT: i64 = 102;

#[derive(Debug, Error)]
pub enum BiznesRadarOwnershipError {
    #[error("BiznesRadar akcjonariat HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("BiznesRadar akcjonariat parse failed: {0}")]
    Parse(String),
}

/// Fetches the raw akcjonariat HTML for one ticker. Trait so parsing/witnessing is
/// tested against sample bytes without network access.
pub trait BiznesRadarOwnershipFetcher {
    fn fetch_akcjonariat(&self, ticker: &str) -> Result<String, BiznesRadarOwnershipError>;
}

/// Production fetcher: a single GET (redirects followed) with UA + timeout.
pub struct HttpBiznesRadarOwnershipFetcher;

impl BiznesRadarOwnershipFetcher for HttpBiznesRadarOwnershipFetcher {
    fn fetch_akcjonariat(&self, ticker: &str) -> Result<String, BiznesRadarOwnershipError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        let url = format!("{SOURCE_URL}{ticker}");
        Ok(client.get(url).send()?.error_for_status()?.text()?)
    }
}

/// A parsed akcjonariat page: the holder rows plus the page-level basis date.
#[derive(Debug, Clone, PartialEq)]
pub struct AkcjonariatPage {
    pub holders: Vec<WitnessHolder>,
    /// The newest per-row "Data aktualizacji" on the page, normalized to ISO
    /// `YYYY-MM-DD`. `None` when no row carries a parsable date.
    pub basis_as_of: Option<String>,
}

/// Parse the akcjonariat `qTableFull` tables into holder rows only (the whole-page
/// basis date is discarded — see [`parse_akcjonariat_page`] for the breadth-source
/// write path). Kept for the compare path and existing callers.
pub fn parse_akcjonariat(html: &str) -> Result<Vec<WitnessHolder>, BiznesRadarOwnershipError> {
    Ok(parse_akcjonariat_page(html)?.holders)
}

/// Parse ONLY the "Główni akcjonariusze" (>=5% disclosure) table into holder rows
/// and the page-level basis date. Columns: `Akcjonariusz | Udział | Liczba akcji |
/// Wartość rynkowa | Udział na WZA | Liczba głosów | Data aktualizacji` — capital %
/// is column 1, votes % (WZA) is column 4, and the last column is the per-row
/// update date (`dd.mm.yyyy`). The page basis is the NEWEST parsable per-row date.
///
/// Table scope (real-page harvest 2026-07-16): the live page carries TWO
/// identically-headed `qTableFull` tables — "Główni akcjonariusze" and "Pozostali
/// akcjonariusze" (sub-5% fund-statement stakes). Only the główni table is
/// ingested (see [`select_glowni_table`]). Row guards reject the junk that the old
/// td-only parse ingested: a row with any `<th>` (header / "razem" summary) is
/// skipped, and a data row is dropped unless the holder name is non-empty and not
/// itself a percentage, and every parsed percentage is <= 100 (a value > 100 is a
/// share COUNT from column drift). ADR 0072 §2c amended 2026-07-16.
pub fn parse_akcjonariat_page(html: &str) -> Result<AkcjonariatPage, BiznesRadarOwnershipError> {
    let document = Html::parse_document(html);
    let table = select_glowni_table(&document)?;

    let row_selector = Selector::parse("tr")
        .map_err(|error| BiznesRadarOwnershipError::Parse(error.to_string()))?;
    let cell_selector = Selector::parse("td")
        .map_err(|error| BiznesRadarOwnershipError::Parse(error.to_string()))?;
    let th_selector = Selector::parse("th")
        .map_err(|error| BiznesRadarOwnershipError::Parse(error.to_string()))?;
    let hundred = Decimal::from(100);

    let mut holders = Vec::new();
    let mut basis_as_of: Option<String> = None;
    for row in table.select(&row_selector) {
        // A row containing any `<th>` is a header OR the "razem" summary row.
        if row.select(&th_selector).next().is_some() {
            continue;
        }
        let cells: Vec<String> = row
            .select(&cell_selector)
            .map(|cell| collapse_ws(&cell.text().collect::<String>()))
            .collect();
        if cells.len() < 5 {
            continue; // malformed / too few columns
        }
        let holder_name = cells[0].clone();
        // Reject an empty holder or a name that is itself a percentage (column
        // drift put a "%" value where the holder should be).
        if holder_name.is_empty() || looks_like_percentage(&holder_name) {
            continue;
        }
        let capital_pct = parse_pct(&cells[1]);
        let votes_pct = parse_pct(&cells[4]);
        // A parsed percentage > 100 is a share count (column drift) — reject the row.
        if capital_pct.is_some_and(|value| value > hundred)
            || votes_pct.is_some_and(|value| value > hundred)
        {
            continue;
        }
        holders.push(WitnessHolder {
            holder_name,
            capital_pct,
            votes_pct,
        });
        // The last column carries the per-row "Data aktualizacji"; the page basis
        // is the newest of them (ISO strings sort chronologically).
        if let Some(iso) = cells.get(6).and_then(|cell| br_date_to_iso(cell)) {
            if basis_as_of
                .as_deref()
                .is_none_or(|current| iso.as_str() > current)
            {
                basis_as_of = Some(iso);
            }
        }
    }
    Ok(AkcjonariatPage {
        holders,
        basis_as_of,
    })
}

/// Select the "Główni akcjonariusze" `qTableFull` — the ONLY table we ingest.
/// Anchor: the `table.qTableFull` that follows the `h2` whose trimmed text equals
/// "Główni akcjonariusze" (document order). Fallback (older/synthetic pages with
/// no heading): the first `table.qTableFull` whose header row has a `th` reading
/// "Akcjonariusz". No match → a parse error (never ingest an unknown table).
fn select_glowni_table(document: &Html) -> Result<ElementRef<'_>, BiznesRadarOwnershipError> {
    let anchor_selector = Selector::parse("h2, table.qTableFull")
        .map_err(|error| BiznesRadarOwnershipError::Parse(error.to_string()))?;
    let mut seen_glowni_heading = false;
    for element in document.select(&anchor_selector) {
        match element.value().name() {
            "h2" => {
                seen_glowni_heading =
                    collapse_ws(&element.text().collect::<String>()) == "Główni akcjonariusze";
            }
            "table" if seen_glowni_heading => return Ok(element),
            _ => {}
        }
    }

    // Header fallback: the first qTableFull whose header row names "Akcjonariusz".
    let table_selector = Selector::parse("table.qTableFull")
        .map_err(|error| BiznesRadarOwnershipError::Parse(error.to_string()))?;
    let th_selector = Selector::parse("th")
        .map_err(|error| BiznesRadarOwnershipError::Parse(error.to_string()))?;
    for table in document.select(&table_selector) {
        if table
            .select(&th_selector)
            .any(|th| collapse_ws(&th.text().collect::<String>()) == "Akcjonariusz")
        {
            return Ok(table);
        }
    }
    Err(BiznesRadarOwnershipError::Parse(
        "no \"Główni akcjonariusze\" ownership table found".to_owned(),
    ))
}

fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether a cell's text is itself a percentage/number (`"93.22 %"`, `"5,97"`) —
/// a holder-name column that parses as a number is column drift, not a holder.
fn looks_like_percentage(text: &str) -> bool {
    let trimmed = text.trim().trim_end_matches('%').trim();
    !trimmed.is_empty() && Decimal::from_str(&trimmed.replace(',', ".")).is_ok()
}

/// Normalize a BiznesRadar `dd.mm.yyyy` date to ISO `YYYY-MM-DD`; `None` when the
/// cell is missing, a placeholder (`-`), or otherwise unparsable.
fn br_date_to_iso(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.trim().split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let day: u32 = parts[0].trim().parse().ok()?;
    let month: u32 = parts[1].trim().parse().ok()?;
    let year = parts[2].trim();
    if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) {
        return None;
    }
    Some(format!("{year}-{month:02}-{day:02}"))
}

/// Parse a BiznesRadar percentage cell (`"12.66 %"`, `"30,00 %"`) into a decimal.
fn parse_pct(raw: &str) -> Option<Decimal> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '%')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    Decimal::from_str(&cleaned.replace(',', ".")).ok()
}

/// Refresh-level [`Fetcher`](crate::jobs::source_refresh::Fetcher) impl for the
/// BiznesRadar ownership breadth source. It writes each parsed table as an
/// `aggregator` snapshot and then witnesses it against the disclosed state.
pub struct BiznesRadarOwnershipAdapter;

impl crate::jobs::source_refresh::Fetcher for BiznesRadarOwnershipAdapter {
    fn refresh(
        &self,
        state: &crate::app_state::AppState,
        ctx: &crate::jobs::source_refresh::RefreshContext,
    ) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
        refresh_with(&HttpBiznesRadarOwnershipFetcher, state, ctx, true)
    }
}

/// The refresh body with an injectable page fetcher (testable seam). `polite`
/// toggles the inter-request delay off for tests. Per company: parse the page,
/// write the whole table as an `aggregator` basis (reconciling a same-basis
/// re-ingest), classify the new holders, then witness the aggregator against the
/// DISCLOSED-only reference — the write never depends on the comparison.
pub(crate) fn refresh_with(
    fetcher: &impl BiznesRadarOwnershipFetcher,
    state: &crate::app_state::AppState,
    ctx: &crate::jobs::source_refresh::RefreshContext,
    polite: bool,
) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
    let _ = state.record_source_adapter_attempt(ADAPTER_ID, ctx.trigger);

    let targets = state
        .list_bankier_company_targets()
        .map_err(|error| error.to_string())?;
    if targets.is_empty() {
        // Nothing to refresh yet — a clean no-op, not a failure.
        return Ok(crate::jobs::source_refresh::RefreshOutcome::Ingestion(
            crate::jobs::source_refresh::empty_source_result(ADAPTER_ID),
        ));
    }

    // One fetch date for the whole run (the fallback basis for a page whose rows
    // carry no parsable "Data aktualizacji").
    let fetch_date = {
        let format = time::format_description::parse("[year]-[month]-[day]")
            .map_err(|error| error.to_string())?;
        OffsetDateTime::now_utc()
            .date()
            .format(&format)
            .map_err(|error| error.to_string())?
    };

    let ownership = state.ownership();
    let mut comparisons = Vec::new();
    let mut fetch_failures = 0usize;
    for (index, target) in targets.iter().enumerate() {
        if polite && index > 0 {
            std::thread::sleep(REQUEST_DELAY);
        }
        let html = match fetcher.fetch_akcjonariat(&target.ticker) {
            Ok(html) => html,
            Err(error) => {
                fetch_failures += 1;
                log::warn!(
                    "module=ownership stage=aggregator_fetch ticker={} error={error}",
                    target.ticker
                );
                continue;
            }
        };
        let page = match parse_akcjonariat_page(&html) {
            Ok(page) => page,
            Err(error) => {
                fetch_failures += 1;
                log::warn!(
                    "module=ownership stage=aggregator_parse ticker={} error={error}",
                    target.ticker
                );
                continue;
            }
        };

        // Basis sanity gate: a disclosed-capital sum > 102% is implausible (column
        // drift / a summary row that slipped a guard). Write NOTHING, record a
        // diagnostic, count it as a failed page (so an all-garbage run never marks
        // the adapter healthy), and skip the comparison. ADR 0072 §2c amended.
        let capital_sum: Decimal = page.holders.iter().filter_map(|h| h.capital_pct).sum();
        if capital_sum > Decimal::from(IMPLAUSIBLE_CAPITAL_SUM_PCT) {
            fetch_failures += 1;
            log::warn!(
                "module=ownership stage=aggregator_implausible ticker={} capital_sum={}",
                target.ticker,
                capital_sum.normalize()
            );
            let _ = ownership.record_aggregator_implausible(
                &target.company_id,
                &target.ticker,
                &capital_sum.normalize().to_string(),
            );
            continue;
        }

        // 1. Write the full-picture aggregator basis (reconciles same-basis
        //    re-ingest), THEN classify the new holders. Write before compare.
        let as_of = page
            .basis_as_of
            .clone()
            .unwrap_or_else(|| fetch_date.clone());
        ownership
            .replace_aggregator_basis(&target.company_id, &as_of, &page.holders)
            .map_err(|error| error.to_string())?;
        ownership
            .classify_unclassified_for_company(&target.company_id)
            .map_err(|error| error.to_string())?;

        // 2. Witness the aggregator against the DISCLOSED-only reference (reports/
        //    ESPI), never against `current_state` (which now includes aggregator).
        let disclosed = ownership
            .disclosed_reference_state(&target.company_id)
            .map_err(|error| error.to_string())?;
        comparisons.push(crate::storage::compare_witness(
            &target.company_id,
            &page.holders,
            &disclosed,
        ));
    }

    // Every company failed to fetch/parse: a transient upstream/structure fault —
    // do not record success (mirrors the KNF/GPW empty-source guard).
    if comparisons.is_empty() && fetch_failures > 0 {
        let message = "BiznesRadar akcjonariat fetched no usable pages".to_owned();
        let _ = state.record_source_adapter_error(ADAPTER_ID, &message);
        return Err(message);
    }

    let checked_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| error.to_string())?;
    ownership
        .record_witness_comparisons(ADAPTER_ID, &comparisons, &checked_at)
        .map(crate::jobs::source_refresh::RefreshOutcome::Ingestion)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../../samples/biznesradar_akcjonariat_cdr.html");
    const DBC_SAMPLE: &str = include_str!("../../samples/biznesradar_akcjonariat_dbc.html");

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn parses_holders_with_separate_capital_and_votes() {
        let holders = parse_akcjonariat(SAMPLE).expect("sample should parse");
        // Only the "Główni akcjonariusze" table is ingested now (ADR 0072 amended
        // 2026-07-16): 3 główni holders; the pozostali table is excluded.
        assert_eq!(
            holders.len(),
            3,
            "one row per główni holder, pozostali excluded"
        );

        let iwinski = &holders[0];
        assert_eq!(iwinski.holder_name, "Marcin Iwiński");
        assert_eq!(iwinski.capital_pct, Some(dec("12.66")));
        assert_eq!(iwinski.votes_pct, Some(dec("12.66")));
    }

    #[test]
    fn preserves_preferred_vote_gap() {
        let holders = parse_akcjonariat(SAMPLE).expect("sample should parse");
        let skarb = holders
            .iter()
            .find(|h| h.holder_name == "Skarb Państwa")
            .expect("Skarb Państwa row present");
        assert_eq!(skarb.capital_pct, Some(dec("30")), "capital column");
        assert_eq!(skarb.votes_pct, Some(dec("45")), "WZA (votes) column");
        assert_ne!(
            skarb.capital_pct, skarb.votes_pct,
            "capital and votes must not be conflated — the gap is the signal"
        );
    }

    #[test]
    fn excludes_the_pozostali_table() {
        // The pozostali (sub-5%) table is never ingested. The old empty-residual
        // skip test is repurposed: the CDR pozostali holder "OFE Allianz Polska"
        // (and the empty residual row) live in that excluded table.
        let holders = parse_akcjonariat(SAMPLE).expect("sample should parse");
        assert!(
            holders.iter().all(|h| !h.holder_name.is_empty()),
            "no empty holder rows"
        );
        assert!(
            !holders
                .iter()
                .any(|h| h.holder_name == "OFE Allianz Polska"),
            "a pozostali-table holder must not be ingested"
        );
    }

    #[test]
    fn parses_basis_as_of_from_data_aktualizacji() {
        // The page-level basis date is the NEWEST per-row "Data aktualizacji" of the
        // ingested główni table (BR prints `dd.mm.yyyy`), normalized to ISO. CDR
        // główni rows carry 23.06.2026 / 23.06.2026 / 30.12.2025 → newest 2026-06-23.
        let page = parse_akcjonariat_page(SAMPLE).expect("sample should parse");
        assert_eq!(page.basis_as_of.as_deref(), Some("2026-06-23"));
        assert_eq!(
            page.holders.len(),
            3,
            "page holders match parse_akcjonariat"
        );
    }

    #[test]
    fn dbc_sample_parses_only_the_main_table() {
        // Real DBC page: TWO identically-headed tables. Only "Główni akcjonariusze"
        // is ingested → exactly its two holders, and the newest główni date wins
        // (19.09.2025 > 20.05.2025). No pozostali holder appears.
        let page = parse_akcjonariat_page(DBC_SAMPLE).expect("DBC sample should parse");
        assert_eq!(
            page.holders,
            vec![
                WitnessHolder {
                    holder_name: "Goodyear Holdings S.ar.l.".to_owned(),
                    capital_pct: Some(dec("87.25")),
                    votes_pct: Some(dec("87.25")),
                },
                WitnessHolder {
                    holder_name: "Porozumienie NN PTE, PKO PTE".to_owned(),
                    capital_pct: Some(dec("5.97")),
                    votes_pct: Some(dec("5.97")),
                },
            ]
        );
        assert_eq!(page.basis_as_of.as_deref(), Some("2025-09-19"));
        assert!(
            !page
                .holders
                .iter()
                .any(|h| h.holder_name == "OFE PKO BP Bankowy"),
            "no pozostali-table holder is ingested"
        );
    }

    #[test]
    fn summary_razem_rows_are_skipped() {
        // The `TH razem` summary row (first cell "93.22 %", second "12 867 479")
        // must never become a holder: no name is a percentage and no pct > 100.
        let page = parse_akcjonariat_page(DBC_SAMPLE).expect("DBC sample should parse");
        let hundred = dec("100");
        for holder in &page.holders {
            assert!(
                !looks_like_percentage(&holder.holder_name),
                "holder name must not be a percentage: {}",
                holder.holder_name
            );
            assert!(
                holder.capital_pct.is_none_or(|v| v <= hundred),
                "capital <= 100"
            );
            assert!(
                holder.votes_pct.is_none_or(|v| v <= hundred),
                "votes <= 100"
            );
        }
    }

    #[test]
    fn basis_as_of_is_none_when_date_column_absent() {
        // Defensive: a page with no "Data aktualizacji" column yields None.
        let html = r#"<table class="qTableFull">
            <tr><th>Akcjonariusz</th><th>Udział</th><th>a</th><th>b</th><th>Udział na WZA</th></tr>
            <tr><td>Holder X</td><td>10.00 %</td><td>1</td><td>2</td><td>10.00 %</td></tr>
        </table>"#;
        let page = parse_akcjonariat_page(html).expect("parses");
        assert_eq!(page.holders.len(), 1);
        assert_eq!(page.basis_as_of, None);
    }
}
