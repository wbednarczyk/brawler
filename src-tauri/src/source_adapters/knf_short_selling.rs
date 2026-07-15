//! KNF short-selling registry adapter (ADR 0069 decision 3, plan v0.55 T4).
//!
//! The KNF (Polish FSA) publishes the national register of net short positions
//! (holder, size %, date) over a stable public JSON endpoint — no HTML scraping
//! (probe 2026-07-15, card `6204cd0`). This module fetches the *current* register
//! (`method: "Default"`, positions >= 0.5%), parses it into typed entries, and
//! decodes the HTML entities the register embeds in holder names. Diffing against
//! stored state, history persistence and the `short_position_change` signal live
//! in `storage::short_positions`; wiring into the refresh dispatch is done by the
//! registry (see the T4 report snippet).

use quick_xml::escape::unescape;
use serde::Deserialize;
use thiserror::Error;

use super::USER_AGENT;

/// Registry id for the KNF short-selling adapter.
pub const ADAPTER_ID: &str = "knf-short-selling";
/// Human catalog name.
pub const DISPLAY_NAME: &str = "KNF Short Selling Register";
/// Public JSON endpoint (probe 2026-07-15).
pub const SOURCE_URL: &str = "https://rss.knf.gov.pl/rss_pub/JSON";
/// Attribution shown on derived feed items.
pub const ATTRIBUTION: &str = "KNF — Rejestr krótkiej sprzedaży";

/// How many rows of the current register to request per poll. The current
/// register (`method: "Default"`) holds ~25 rows; a generous cap tolerates growth
/// without paging.
const REQUEST_LIMIT: u32 = 200;

#[derive(Debug, Error)]
pub enum KnfShortError {
    #[error("KNF short-selling HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("KNF short-selling JSON parse failed: {0}")]
    Parse(String),
}

/// One net-short-position entry from the current register, normalized: HTML
/// entities decoded in the holder name, percent parsed to `f64`, dates truncated
/// to `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq)]
pub struct KnfShortEntry {
    pub holder_name: String,
    pub issuer_name: String,
    pub isin: String,
    pub net_position_pct: f64,
    pub position_date: String,
    pub modify_date: Option<String>,
}

/// Raw JSON response envelope.
#[derive(Debug, Deserialize)]
struct KnfResponse {
    #[serde(default)]
    records: Vec<KnfRecord>,
}

/// Raw JSON record. KNF keys are UPPER_SNAKE; fields we do not consume are ignored.
#[derive(Debug, Deserialize)]
struct KnfRecord {
    #[serde(rename = "HOLDER_FULL_NAME", default)]
    holder_full_name: String,
    #[serde(rename = "ISSUER_NAME", default)]
    issuer_name: String,
    #[serde(rename = "ISIN", default)]
    isin: String,
    #[serde(rename = "NET_SHORT_POSITION_O", default)]
    net_short_position_o: String,
    #[serde(rename = "POSITION_DATE", default)]
    position_date: String,
    #[serde(rename = "MODIFY_DATE", default)]
    modify_date: Option<String>,
}

/// Fetches the raw KNF register JSON. Trait so ingestion can be tested against
/// sample bytes without network access.
pub trait KnfShortSellingFetcher {
    fn fetch_register(&self, source_url: &str) -> Result<String, KnfShortError>;
}

/// Production fetcher: POSTs the register query to the public endpoint.
pub struct HttpKnfShortSellingFetcher;

impl KnfShortSellingFetcher for HttpKnfShortSellingFetcher {
    fn fetch_register(&self, source_url: &str) -> Result<String, KnfShortError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()?;

        // The endpoint takes a single form field `request` carrying the query as a
        // JSON string. `method: "Default"` selects the current register.
        let request_body = current_register_request(REQUEST_LIMIT);
        let form = [("request", request_body.as_str())];

        Ok(client
            .post(source_url)
            .form(&form)
            .send()?
            .error_for_status()?
            .text()?)
    }
}

/// The JSON query string for the current register (`method: "Default"`),
/// newest-first, `limit` rows.
fn current_register_request(limit: u32) -> String {
    format!(
        "{{\"cmd\":\"get\",\"language\":\"pl\",\"search\":[],\"limit\":{limit},\"offset\":0,\"method\":\"Default\",\"sort\":[{{\"field\":\"POSITION_DATE\",\"direction\":\"desc\"}}],\"searchLogic\":\"AND\",\"searchValue\":\"\"}}"
    )
}

/// Fetch and parse the current KNF short-selling register into typed entries.
pub fn fetch_short_entries(
    fetcher: &impl KnfShortSellingFetcher,
) -> Result<Vec<KnfShortEntry>, KnfShortError> {
    let json = fetcher.fetch_register(SOURCE_URL)?;
    parse_short_entries(&json)
}

/// Parse the raw register JSON into normalized entries. Records missing an ISIN,
/// an unparseable percent, or a holder are dropped (they cannot be matched or
/// diffed).
pub fn parse_short_entries(json: &str) -> Result<Vec<KnfShortEntry>, KnfShortError> {
    let response: KnfResponse =
        serde_json::from_str(json).map_err(|error| KnfShortError::Parse(error.to_string()))?;

    let mut entries = Vec::new();
    for record in response.records {
        let holder_name = decode_entities(record.holder_full_name.trim());
        let isin = record.isin.trim().to_owned();
        let Some(net_position_pct) = parse_percent(&record.net_short_position_o) else {
            continue;
        };
        if holder_name.is_empty() || isin.is_empty() {
            continue;
        }
        entries.push(KnfShortEntry {
            holder_name,
            issuer_name: decode_entities(record.issuer_name.trim()),
            isin,
            net_position_pct,
            position_date: truncate_date(&record.position_date),
            modify_date: record
                .modify_date
                .as_deref()
                .map(truncate_date)
                .filter(|value| !value.is_empty()),
        });
    }
    Ok(entries)
}

/// Decode the HTML/XML entities KNF embeds in names (`&amp;`, …). Falls back to a
/// bare `&amp;` replacement if the value is not well-formed for the XML unescaper.
fn decode_entities(value: &str) -> String {
    unescape(value)
        .map(|decoded| decoded.into_owned())
        .unwrap_or_else(|_| value.replace("&amp;", "&"))
}

/// Parse a KNF percent string (`"0,53"`, `"1.20"`, possibly with a `%` suffix)
/// into `f64`. Tolerates the Polish decimal comma and stray whitespace.
fn parse_percent(raw: &str) -> Option<f64> {
    let cleaned: String = raw
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '%')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.replace(',', ".").parse::<f64>().ok()
}

/// Truncate a KNF date/timestamp to its `YYYY-MM-DD` prefix.
fn truncate_date(value: &str) -> String {
    value.trim().chars().take(10).collect()
}

/// Refresh-level [`Fetcher`](crate::jobs::source_refresh::Fetcher) impl for the KNF
/// short-selling register adapter (ADR 0069 decision 3, plan v0.55 T4).
pub struct KnfShortSellingRefresh;

impl crate::jobs::source_refresh::Fetcher for KnfShortSellingRefresh {
    fn refresh(
        &self,
        state: &crate::app_state::AppState,
        ctx: &crate::jobs::source_refresh::RefreshContext,
    ) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
        refresh_with(&HttpKnfShortSellingFetcher, state, ctx)
    }
}

/// The refresh body with an injectable register fetcher (testable seam).
pub(crate) fn refresh_with(
    fetcher: &impl KnfShortSellingFetcher,
    state: &crate::app_state::AppState,
    ctx: &crate::jobs::source_refresh::RefreshContext,
) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
    let _ = state.record_source_adapter_attempt(ADAPTER_ID, ctx.trigger);
    let entries = match fetch_short_entries(fetcher) {
        Ok(entries) => entries,
        Err(error) => {
            let message = error.to_string();
            let _ = state.record_source_adapter_error(ADAPTER_ID, &message);
            return Err(message);
        }
    };

    // Guard: the national register is never legitimately empty (it spans every
    // GPW issuer, ~25 rows). An empty response is a transient upstream fault —
    // diffing against it would mark every stored holder `exited` and spray
    // spurious signals/alerts, then mirror-image `entered` spam on recovery.
    if entries.is_empty() {
        let message =
            "KNF register returned zero rows; refusing to infer exits from an empty snapshot"
                .to_owned();
        let _ = state.record_source_adapter_error(ADAPTER_ID, &message);
        return Err(message);
    }

    state
        .short_positions()
        .ingest_knf_short_positions(&entries)
        .map(crate::jobs::source_refresh::RefreshOutcome::Ingestion)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Probe-shaped sample: two holders on one issuer (one holder name carries an
    /// HTML entity), plus a second issuer, plus a Polish-comma percent.
    const SAMPLE_JSON: &str = r#"{
        "total": 3,
        "records": [
            {
                "HOLDER_FULL_NAME": "AKO Capital LLP",
                "ISSUER_NAME": "CD PROJEKT SA",
                "ISIN": "PLOPTTC00011",
                "NET_SHORT_POSITION_O": "0,53",
                "POSITION_DATE": "2026-07-14T00:00:00",
                "MODIFY_DATE": "2026-07-15T09:00:00",
                "recid": 1
            },
            {
                "HOLDER_FULL_NAME": "Marshall Wace &amp; Co",
                "ISSUER_NAME": "CD PROJEKT SA",
                "ISIN": "PLOPTTC00011",
                "NET_SHORT_POSITION_O": "1.20",
                "POSITION_DATE": "2026-07-13",
                "MODIFY_DATE": null,
                "recid": 2
            },
            {
                "HOLDER_FULL_NAME": "BlackRock Investment Management",
                "ISSUER_NAME": "ALLEGRO.EU SA",
                "ISIN": "LU2237380790",
                "NET_SHORT_POSITION_O": "0,75",
                "POSITION_DATE": "2026-07-12",
                "MODIFY_DATE": "2026-07-12",
                "recid": 3
            }
        ],
        "status": "success"
    }"#;

    #[test]
    fn parses_sample_register_into_typed_entries() {
        let entries = parse_short_entries(SAMPLE_JSON).expect("sample should parse");
        insta::assert_debug_snapshot!(entries);
    }

    #[test]
    fn decodes_html_entities_in_holder_names() {
        let entries = parse_short_entries(SAMPLE_JSON).expect("sample should parse");
        assert_eq!(entries[1].holder_name, "Marshall Wace & Co");
    }

    #[test]
    fn parses_polish_comma_percent() {
        let entries = parse_short_entries(SAMPLE_JSON).expect("sample should parse");
        assert!((entries[0].net_position_pct - 0.53).abs() < f64::EPSILON);
        assert!((entries[1].net_position_pct - 1.20).abs() < f64::EPSILON);
    }

    #[test]
    fn drops_records_without_isin_or_percent() {
        let json = r#"{"records":[
            {"HOLDER_FULL_NAME":"No ISIN","ISSUER_NAME":"X","ISIN":"","NET_SHORT_POSITION_O":"0,60","POSITION_DATE":"2026-07-01"},
            {"HOLDER_FULL_NAME":"Bad pct","ISSUER_NAME":"Y","ISIN":"PLX0000001","NET_SHORT_POSITION_O":"n/a","POSITION_DATE":"2026-07-01"}
        ]}"#;
        let entries = parse_short_entries(json).expect("parse");
        assert!(entries.is_empty());
    }

    #[test]
    fn current_register_request_selects_default_method() {
        let request = current_register_request(50);
        assert!(request.contains("\"method\":\"Default\""));
        assert!(request.contains("\"limit\":50"));
        assert!(request.contains("POSITION_DATE"));
    }
}
