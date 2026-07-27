//! NBP Table-A mid-rate adapter (ADR 0089 decision 2). The FX substrate's source
//! layer: a pure parser (no I/O — mirrors `yahoo_eod`) plus an HTTP fetch seam
//! (mirrors `market_data_fetch`) and a pure date-chunking helper for the
//! full-history backfill.
//!
//! NBP's official public API is keyless and PLN-based (the app's comparison
//! currency). Table A carries the daily average ("mid") rates. Endpoints:
//! - latest table:   `.../exchangerates/tables/A/?format=json`
//! - single date:    `.../exchangerates/tables/A/{date}/?format=json`  (404 on a
//!   non-publication day — weekend/holiday)
//! - date range:     `.../exchangerates/tables/A/{start}/{end}/?format=json`
//!   with a **hard 93-day window** (HTTP 400 "Limit of 93 days has been
//!   exceeded" beyond it — verified by live probe 2026-07-27), so the backfill
//!   chunks in <=90-day windows.
//!
//! Mids are decimal-exact: parsed from the raw JSON number token (serde_json
//! `RawValue`) straight into `rust_decimal::Decimal`, never via `f64`.

use std::collections::BTreeSet;

use rust_decimal::prelude::FromStr;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::value::RawValue;
use thiserror::Error;
use time::macros::format_description;
use time::Date;

use crate::fx::FxRate;

/// Source-adapter id for the NBP FX substrate (matches the registry descriptor
/// and the migration 0115 seed row).
pub const ADAPTER_ID: &str = "nbp-fx";
pub const DISPLAY_NAME: &str = "NBP — kursy średnie (Tabela A)";
pub const SOURCE_URL: &str = "https://api.nbp.pl/api/exchangerates/tables/A/";

const NBP_BASE_URL: &str = "https://api.nbp.pl/api/exchangerates/tables/A";

/// NBP's documented range limit is 93 days; we chunk in 90-day windows for a
/// safety margin (verified: a 152-day range returns HTTP 400).
pub const NBP_MAX_RANGE_DAYS: i64 = 90;

/// The earliest date NBP Table-A history is available (2002-01-02). Full-history
/// backfill starts here; chunks before real data simply return no rows.
pub const NBP_HISTORY_START: &str = "2002-01-02";

/// The currencies the app converts by default (ADR 0089: at minimum EUR, USD,
/// GBP, CHF). Table-driven — extend by adding codes here or via the pull
/// payload / the currencies already present in `fx_rates`; never a hardcoded
/// enum in the conversion path.
pub const DEFAULT_CURRENCIES: &[&str] = &["CHF", "EUR", "GBP", "USD"];

#[derive(Debug, Error)]
pub enum NbpFetchError {
    #[error("NBP HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Non-2xx status. `404` = no publication for that date/window (weekend,
    /// holiday, or a window entirely before history) — the caller treats it as
    /// "no data", not an error.
    #[error("NBP API returned HTTP {0}")]
    Status(u16),
}

#[derive(Debug, Error, PartialEq)]
pub enum NbpParseError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("invalid mid rate {value:?} for {code}: {reason}")]
    InvalidMid {
        code: String,
        value: String,
        reason: String,
    },
}

/// One parsed NBP Table-A day: its effective date and every `(code, mid)` on it.
#[derive(Debug, Clone, PartialEq)]
pub struct NbpDay {
    pub effective_date: String,
    pub rates: Vec<(String, Decimal)>,
}

#[derive(Debug, Deserialize)]
struct RawNbpTable {
    #[serde(rename = "effectiveDate")]
    effective_date: String,
    rates: Vec<RawNbpRate>,
}

#[derive(Debug, Deserialize)]
struct RawNbpRate {
    code: String,
    /// Captured as the raw JSON number token so the exact decimal digits survive
    /// (no `f64` round-trip).
    mid: Box<RawValue>,
}

/// Parse an NBP Table-A JSON body (single date OR a range — both are a JSON
/// array of table objects) into per-day mid tables. Mids are decimal-exact.
pub fn parse_nbp_table_a(json: &str) -> Result<Vec<NbpDay>, NbpParseError> {
    let tables: Vec<RawNbpTable> =
        serde_json::from_str(json).map_err(|error| NbpParseError::Json(error.to_string()))?;

    let mut days = Vec::with_capacity(tables.len());
    for table in tables {
        let mut rates = Vec::with_capacity(table.rates.len());
        for raw in table.rates {
            let token = raw.mid.get().trim();
            let mid = Decimal::from_str(token).map_err(|error| NbpParseError::InvalidMid {
                code: raw.code.clone(),
                value: token.to_owned(),
                reason: error.to_string(),
            })?;
            rates.push((raw.code, mid));
        }
        days.push(NbpDay {
            effective_date: table.effective_date,
            rates,
        });
    }
    Ok(days)
}

/// Flatten parsed days into `FxRate` rows, keeping only the `needed` currency
/// codes (table-driven filtering). Output is in the days' order.
pub fn extract_fx_rates(days: &[NbpDay], needed: &BTreeSet<String>) -> Vec<FxRate> {
    let mut out = Vec::new();
    for day in days {
        for (code, mid) in &day.rates {
            if needed.contains(code) {
                out.push(FxRate {
                    currency: code.clone(),
                    date: day.effective_date.clone(),
                    mid: *mid,
                });
            }
        }
    }
    out
}

/// Split the inclusive date span `[start, end]` into contiguous, non-overlapping
/// windows each at most [`NBP_MAX_RANGE_DAYS`] wide (inclusive day count), so no
/// range request exceeds NBP's 93-day limit. Returns `[]` when `start > end` or
/// a date fails to parse. Pure and deterministic.
pub fn date_chunks(start: &str, end: &str, max_days: i64) -> Vec<(String, String)> {
    let fmt = format_description!("[year]-[month]-[day]");
    let (Ok(start_date), Ok(end_date)) = (Date::parse(start, &fmt), Date::parse(end, &fmt)) else {
        return Vec::new();
    };
    if start_date > end_date || max_days < 1 {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut cursor = start_date;
    let step = time::Duration::days(max_days - 1); // inclusive window width
    loop {
        let chunk_end = (cursor + step).min(end_date);
        chunks.push((
            cursor.format(&fmt).unwrap_or_default(),
            chunk_end.format(&fmt).unwrap_or_default(),
        ));
        if chunk_end >= end_date {
            break;
        }
        cursor = chunk_end + time::Duration::days(1);
    }
    chunks
}

/// The set of currencies a pull maintains: the defaults ∪ any extras (e.g.
/// currencies already present in `fx_rates`, or a payload-requested code),
/// upper-cased. Table-driven.
pub fn currency_set<I, S>(extras: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut set: BTreeSet<String> = DEFAULT_CURRENCIES.iter().map(|c| (*c).to_owned()).collect();
    for extra in extras {
        let code = extra.as_ref().trim().to_uppercase();
        if !code.is_empty() {
            set.insert(code);
        }
    }
    set
}

/// HTTP fetch seam so pull logic stays network-free and deterministic in tests
/// (mirrors `MarketDataFetcher`). Implementors never parse. Every pull — backfill
/// and daily recent alike — is a bounded date-range request, so one method
/// covers both (the caller keeps each window <=93d via [`date_chunks`]).
pub trait NbpFxFetcher {
    /// Table A over an inclusive `[start, end]` window (caller keeps it <=93d).
    /// `Err(NbpFetchError::Status(404))` means the window has no publication days.
    fn fetch_range(&self, start: &str, end: &str) -> Result<String, NbpFetchError>;
}

pub struct HttpNbpFxFetcher;

impl HttpNbpFxFetcher {
    fn client() -> Result<reqwest::blocking::Client, NbpFetchError> {
        Ok(reqwest::blocking::Client::builder()
            .user_agent(super::USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()?)
    }

    fn get(url: &str) -> Result<String, NbpFetchError> {
        let client = Self::client()?;
        let response = client.get(url).send()?;
        let status = response.status();
        if !status.is_success() {
            return Err(NbpFetchError::Status(status.as_u16()));
        }
        Ok(response.text()?)
    }
}

impl NbpFxFetcher for HttpNbpFxFetcher {
    fn fetch_range(&self, start: &str, end: &str) -> Result<String, NbpFetchError> {
        Self::get(&format!("{NBP_BASE_URL}/{start}/{end}/?format=json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real, trimmed NBP Table-A single-date response (probe 2026-07-27,
    /// effectiveDate 2024-01-15) with the four default currencies plus one
    /// non-needed code (THB) to prove table-driven filtering, and HUF to prove a
    /// small sub-cent mid parses decimal-exact.
    const SAMPLE_SINGLE: &str = r#"[
        {
            "table": "A",
            "no": "010/A/NBP/2024",
            "effectiveDate": "2024-01-15",
            "rates": [
                {"currency": "bat (Tajlandia)", "code": "THB", "mid": 0.1143},
                {"currency": "dolar amerykański", "code": "USD", "mid": 3.9963},
                {"currency": "euro", "code": "EUR", "mid": 4.3748},
                {"currency": "forint (Węgry)", "code": "HUF", "mid": 0.011517},
                {"currency": "frank szwajcarski", "code": "CHF", "mid": 4.6797},
                {"currency": "funt szterling", "code": "GBP", "mid": 5.0861}
            ]
        }
    ]"#;

    fn needed() -> BTreeSet<String> {
        currency_set(std::iter::empty::<&str>())
    }

    #[test]
    fn parses_sample_into_decimal_exact_mids() {
        let days = parse_nbp_table_a(SAMPLE_SINGLE).expect("sample should parse");
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].effective_date, "2024-01-15");
        let eur = days[0]
            .rates
            .iter()
            .find(|(c, _)| c == "EUR")
            .map(|(_, m)| *m)
            .expect("EUR present");
        assert_eq!(eur, Decimal::from_str("4.3748").unwrap());
        // Sub-cent HUF is exact, not a float approximation.
        let huf = days[0]
            .rates
            .iter()
            .find(|(c, _)| c == "HUF")
            .map(|(_, m)| *m)
            .expect("HUF present");
        assert_eq!(huf, Decimal::from_str("0.011517").unwrap());
    }

    #[test]
    fn extract_keeps_only_needed_currencies() {
        let days = parse_nbp_table_a(SAMPLE_SINGLE).expect("parse");
        let rates = extract_fx_rates(&days, &needed());
        // Four defaults kept; THB and HUF dropped.
        let mut codes: Vec<&str> = rates.iter().map(|r| r.currency.as_str()).collect();
        codes.sort_unstable();
        assert_eq!(codes, vec!["CHF", "EUR", "GBP", "USD"]);
        assert!(rates.iter().all(|r| r.date == "2024-01-15"));
    }

    #[test]
    fn golden_extracted_fx_rates() {
        let days = parse_nbp_table_a(SAMPLE_SINGLE).expect("parse");
        let rates = extract_fx_rates(&days, &needed());
        insta::assert_debug_snapshot!("golden_nbp_table_a_fx_rates", rates);
    }

    #[test]
    fn currency_set_is_defaults_union_extras_uppercased() {
        let set = currency_set(["nok", "  ", "usd"]);
        assert!(set.contains("NOK"));
        assert!(set.contains("USD"));
        assert!(set.contains("EUR"));
        // Blank extra is ignored.
        assert!(!set.contains(""));
    }

    #[test]
    fn malformed_json_is_a_typed_error_not_a_panic() {
        assert!(matches!(
            parse_nbp_table_a("not json"),
            Err(NbpParseError::Json(_))
        ));
    }

    #[test]
    fn date_chunks_cover_the_span_without_gaps_or_overlaps() {
        // 152 days (Jan 1 .. Jun 1) exceeds the 93-day limit -> multiple chunks.
        let chunks = date_chunks("2024-01-01", "2024-06-01", NBP_MAX_RANGE_DAYS);
        assert!(
            chunks.len() >= 2,
            "a >90-day span must split, got {chunks:?}"
        );
        // First chunk starts at the span start; last ends at the span end.
        assert_eq!(chunks.first().unwrap().0, "2024-01-01");
        assert_eq!(chunks.last().unwrap().1, "2024-06-01");
        // Contiguous: each chunk starts the day after the previous chunk's end.
        let fmt = format_description!("[year]-[month]-[day]");
        for pair in chunks.windows(2) {
            let prev_end = Date::parse(&pair[0].1, &fmt).unwrap();
            let next_start = Date::parse(&pair[1].0, &fmt).unwrap();
            assert_eq!(next_start, prev_end + time::Duration::days(1));
        }
    }

    #[test]
    fn date_chunks_each_window_is_within_the_limit() {
        let fmt = format_description!("[year]-[month]-[day]");
        for (s, e) in date_chunks("2002-01-02", "2026-07-27", NBP_MAX_RANGE_DAYS) {
            let sd = Date::parse(&s, &fmt).unwrap();
            let ed = Date::parse(&e, &fmt).unwrap();
            let inclusive_days = (ed - sd).whole_days() + 1;
            assert!(
                inclusive_days <= 93,
                "chunk {s}..{e} spans {inclusive_days} days (> 93 NBP limit)"
            );
        }
    }

    #[test]
    fn date_chunks_single_window_when_span_fits() {
        let chunks = date_chunks("2024-01-01", "2024-01-10", NBP_MAX_RANGE_DAYS);
        assert_eq!(
            chunks,
            vec![("2024-01-01".to_owned(), "2024-01-10".to_owned())]
        );
    }

    #[test]
    fn date_chunks_empty_on_inverted_or_bad_dates() {
        assert!(date_chunks("2024-06-01", "2024-01-01", NBP_MAX_RANGE_DAYS).is_empty());
        assert!(date_chunks("garbage", "2024-01-01", NBP_MAX_RANGE_DAYS).is_empty());
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn parse_never_panics_on_arbitrary_input(input in ".*") {
            let _ = parse_nbp_table_a(&input);
        }

        #[test]
        fn date_chunks_never_panics_and_windows_stay_bounded(
            y1 in 2002i32..2027, m1 in 1u32..13, d1 in 1u32..29,
            y2 in 2002i32..2027, m2 in 1u32..13, d2 in 1u32..29,
        ) {
            let a = format!("{y1:04}-{m1:02}-{d1:02}");
            let b = format!("{y2:04}-{m2:02}-{d2:02}");
            let chunks = date_chunks(&a, &b, NBP_MAX_RANGE_DAYS);
            let fmt = format_description!("[year]-[month]-[day]");
            for (s, e) in &chunks {
                let sd = Date::parse(s, &fmt).unwrap();
                let ed = Date::parse(e, &fmt).unwrap();
                let inclusive_days = (ed - sd).whole_days() + 1;
                prop_assert!(inclusive_days <= 93);
                prop_assert!(sd <= ed);
            }
        }
    }
}
