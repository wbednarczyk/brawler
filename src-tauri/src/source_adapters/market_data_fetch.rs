//! HTTP fetch seam for market-data providers (ADR 0082, v0.53 T2).
//!
//! Mirrors the existing per-adapter fetcher-trait pattern (e.g.
//! `gpw_company_registry::GpwCompanyRegistryFetcher`): a trait so job logic is
//! network-free and deterministic in tests, plus a real `reqwest` impl. The
//! `yahoo_eod` module stays a pure parser (no I/O); this is the
//! "T2 job/adapter layer" their doc comments point to.
//! `// migrates to Fetcher (ADR 0069)`.

use thiserror::Error;

use super::USER_AGENT;

const YAHOO_BASE_URL: &str = "https://query1.finance.yahoo.com/v8/finance/chart";

#[derive(Debug, Error)]
pub enum MarketDataFetchError {
    #[error("market data HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Non-2xx HTTP status. Callers distinguish 429/999 (Yahoo rate-limit /
    /// undocumented block, ADR 0082) to trigger the Twelve Data fallback.
    #[error("market data provider returned HTTP {0}")]
    Status(u16),
}

impl MarketDataFetchError {
    /// Whether this error should trigger the Yahoo -> Twelve Data fallback
    /// (ADR 0082: 429 rate-limit or 999 Yahoo's undocumented block code).
    pub fn is_fallback_trigger(&self) -> bool {
        matches!(self, Self::Status(429) | Self::Status(999))
    }
}

/// Fetches raw provider JSON bodies. Implementors never parse — parsing stays
/// in the pure `yahoo_eod` module.
pub trait MarketDataFetcher {
    /// Full history: Yahoo v8 chart JSON for `<ticker>.WA`, `period1=0` to now
    /// (ADR 0082 backfill mode).
    fn fetch_yahoo_full_history(&self, ticker_wa: &str) -> Result<String, MarketDataFetchError>;

    /// Post-session pull: Yahoo v8 chart JSON for `<ticker>.WA`, `range=5d`
    /// (ADR 0082 daily mode).
    fn fetch_yahoo_recent(&self, ticker_wa: &str) -> Result<String, MarketDataFetchError>;
}

pub struct HttpMarketDataFetcher;

impl HttpMarketDataFetcher {
    fn client() -> Result<reqwest::blocking::Client, MarketDataFetchError> {
        Ok(reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()?)
    }

    fn get(url: &str) -> Result<String, MarketDataFetchError> {
        let client = Self::client()?;
        let response = client.get(url).send()?;
        let status = response.status();
        if !status.is_success() {
            return Err(MarketDataFetchError::Status(status.as_u16()));
        }
        Ok(response.text()?)
    }
}

impl MarketDataFetcher for HttpMarketDataFetcher {
    fn fetch_yahoo_full_history(&self, ticker_wa: &str) -> Result<String, MarketDataFetchError> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let url = format!("{YAHOO_BASE_URL}/{ticker_wa}?period1=0&period2={now}&interval=1d");
        Self::get(&url)
    }

    fn fetch_yahoo_recent(&self, ticker_wa: &str) -> Result<String, MarketDataFetchError> {
        let url = format!("{YAHOO_BASE_URL}/{ticker_wa}?range=5d&interval=1d");
        Self::get(&url)
    }
}
