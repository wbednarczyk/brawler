//! Pure parser for the Yahoo Finance v8 chart JSON (ADR 0082 primary EOD
//! source). No I/O here — fetching lives in the T2 job/adapter layer; this
//! module only turns provider JSON into the common [`QuoteSeries`] shape.
//! `// migrates to Fetcher (ADR 0069)`.

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

/// One EOD bar. `date` is an ISO `YYYY-MM-DD` calendar date (UTC), never a
/// timestamp — daily quotes are keyed by session day, not instant.
#[derive(Debug, Clone, PartialEq)]
pub struct DailyQuote {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

/// Common output of every market-data parser: a symbol's bars in ascending
/// date order, plus the currency the prices are denominated in.
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteSeries {
    pub symbol: String,
    pub currency: String,
    pub quotes: Vec<DailyQuote>,
}

/// Shared parse-error shape across all market-data providers (ADR 0082):
/// malformed JSON, an explicit provider-side error payload, an empty/missing
/// result, or a value that cannot be reconciled into a calendar date.
#[derive(Debug, Error, PartialEq)]
pub enum QuoteParseError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("provider returned an error: {0}")]
    ProviderError(String),
    #[error("no quote data present")]
    NoData,
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),
}

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: Chart,
}

#[derive(Debug, Deserialize)]
struct Chart {
    #[serde(default)]
    result: Option<Vec<ChartResult>>,
    #[serde(default)]
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: ChartMeta,
    #[serde(default)]
    timestamp: Option<Vec<i64>>,
    indicators: ChartIndicators,
}

#[derive(Debug, Deserialize)]
struct ChartMeta {
    symbol: String,
    #[serde(default)]
    currency: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChartIndicators {
    #[serde(default)]
    quote: Vec<ChartQuoteBlock>,
}

#[derive(Debug, Deserialize)]
struct ChartQuoteBlock {
    #[serde(default)]
    open: Vec<Option<f64>>,
    #[serde(default)]
    high: Vec<Option<f64>>,
    #[serde(default)]
    low: Vec<Option<f64>>,
    #[serde(default)]
    close: Vec<Option<f64>>,
    #[serde(default)]
    volume: Vec<Option<i64>>,
}

/// Parse a Yahoo v8 chart JSON payload (`query1.finance.yahoo.com/v8/finance/chart/<ticker>.WA`)
/// into a [`QuoteSeries`]. Rows with any null OHLC field (holidays/halts) are
/// skipped; a non-null `chart.error` or an empty/missing result is an `Err`.
pub fn parse_yahoo_chart(json: &str) -> Result<QuoteSeries, QuoteParseError> {
    let response: ChartResponse =
        serde_json::from_str(json).map_err(|error| QuoteParseError::Json(error.to_string()))?;

    if let Some(error) = response.chart.error {
        return Err(QuoteParseError::ProviderError(error.to_string()));
    }

    let result = response
        .chart
        .result
        .and_then(|results| results.into_iter().next())
        .ok_or(QuoteParseError::NoData)?;

    let timestamps = result.timestamp.unwrap_or_default();
    let block = result.indicators.quote.into_iter().next();

    let quotes = match block {
        Some(block) => {
            let mut quotes = Vec::with_capacity(timestamps.len());
            for (index, unix_seconds) in timestamps.into_iter().enumerate() {
                let (Some(Some(open)), Some(Some(high)), Some(Some(low)), Some(Some(close))) = (
                    block.open.get(index).copied(),
                    block.high.get(index).copied(),
                    block.low.get(index).copied(),
                    block.close.get(index).copied(),
                ) else {
                    continue;
                };
                let volume = block
                    .volume
                    .get(index)
                    .copied()
                    .flatten()
                    .unwrap_or_default();
                let date = unix_seconds_to_date(unix_seconds)?;

                quotes.push(DailyQuote {
                    date,
                    open,
                    high,
                    low,
                    close,
                    volume,
                });
            }
            quotes
        }
        None => Vec::new(),
    };

    Ok(QuoteSeries {
        symbol: result.meta.symbol,
        currency: result.meta.currency.unwrap_or_default(),
        quotes,
    })
}

fn unix_seconds_to_date(unix_seconds: i64) -> Result<String, QuoteParseError> {
    let timestamp = OffsetDateTime::from_unix_timestamp(unix_seconds)
        .map_err(|_| QuoteParseError::InvalidTimestamp(unix_seconds))?;

    // Rfc3339 always renders a full `YYYY-MM-DDT...` timestamp; the calendar
    // date is the first 10 bytes, which are ASCII and safe to slice.
    let rendered = timestamp
        .format(&Rfc3339)
        .map_err(|_| QuoteParseError::InvalidTimestamp(unix_seconds))?;

    Ok(rendered[..10].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "chart": {
            "result": [{
                "meta": {"symbol": "CDR.WA", "currency": "PLN"},
                "timestamp": [1752364800, 1752451200, 1752537600],
                "indicators": {
                    "quote": [{
                        "open": [231.5, 233.0, null],
                        "high": [234.0, 235.2, null],
                        "low": [230.8, 232.1, null],
                        "close": [233.0, 231.4, null],
                        "volume": [1706615, 1420332, null]
                    }]
                }
            }],
            "error": null
        }
    }"#;

    #[test]
    fn parses_sample_chart_json_into_quote_series() {
        let series = parse_yahoo_chart(SAMPLE).expect("sample should parse");

        assert_eq!(series.symbol, "CDR.WA");
        assert_eq!(series.currency, "PLN");
        // The third bar is null (holiday/halt) and must be skipped.
        assert_eq!(series.quotes.len(), 2);
        assert_eq!(series.quotes[0].date, "2025-07-13");
        assert_eq!(series.quotes[0].close, 233.0);
        assert_eq!(series.quotes[1].date, "2025-07-14");
    }

    #[test]
    fn golden_parsed_chart_quote_series() {
        let series = parse_yahoo_chart(SAMPLE).expect("sample should parse");
        insta::assert_debug_snapshot!("golden_yahoo_chart_quote_series", series);
    }

    #[test]
    fn provider_error_payload_is_rejected() {
        let json = r#"{"chart": {"result": null, "error": {"code": "Not Found", "description": "No data found"}}}"#;

        let error = parse_yahoo_chart(json).expect_err("provider error should surface");
        assert!(matches!(error, QuoteParseError::ProviderError(_)));
    }

    #[test]
    fn missing_result_is_rejected() {
        let json = r#"{"chart": {"result": [], "error": null}}"#;

        let error = parse_yahoo_chart(json).expect_err("empty result should be rejected");
        assert_eq!(error, QuoteParseError::NoData);
    }
}

#[cfg(test)]
mod proptests {
    //! Parser fuzzing (ADR 0049): arbitrary/garbage input must never panic —
    //! only a well-typed `Err` is an acceptable outcome for malformed shapes.
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn parse_yahoo_chart_never_panics_on_arbitrary_input(input in ".*") {
            let _ = parse_yahoo_chart(&input);
        }

        #[test]
        fn parse_yahoo_chart_never_panics_on_arbitrary_json_like_input(
            input in r#"\{"chart":\s*\{[^{}]{0,80}\}\}"#
        ) {
            let _ = parse_yahoo_chart(&input);
        }
    }
}
