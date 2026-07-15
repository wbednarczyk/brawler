use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::{
    company_directory::{
        parse_company_directory_html, CompanyDirectoryEntry, CompanyDirectoryParseError,
    },
    USER_AGENT,
};

pub const ADAPTER_ID: &str = "newconnect-company-directory";
pub const DISPLAY_NAME: &str = "NewConnect Company Directory";
pub const SOURCE_URL: &str = "https://newconnect.pl/spolki?offset=0&limit=500";
const BASE_URL: &str = "https://newconnect.pl";
const EXCHANGE_CODE: &str = "NC";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NewConnectCompanyDirectoryParseError {
    #[error("invalid selector: {0}")]
    InvalidSelector(String),
    #[error("no NewConnect company directory rows found")]
    NoRows,
}

impl From<CompanyDirectoryParseError> for NewConnectCompanyDirectoryParseError {
    fn from(error: CompanyDirectoryParseError) -> Self {
        match error {
            CompanyDirectoryParseError::InvalidSelector(value) => Self::InvalidSelector(value),
            CompanyDirectoryParseError::NoRows => Self::NoRows,
        }
    }
}

#[derive(Debug, Error)]
pub enum NewConnectCompanyDirectoryFetchError {
    #[error("NewConnect company directory HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("NewConnect company directory parse failed: {0}")]
    Parse(#[from] NewConnectCompanyDirectoryParseError),
    #[error("fetched timestamp formatting failed: {0}")]
    TimestampFormat(#[from] time::error::Format),
}

pub trait NewConnectCompanyDirectoryFetcher {
    fn fetch_company_directory_page(&self) -> Result<String, NewConnectCompanyDirectoryFetchError>;
}

pub struct HttpNewConnectCompanyDirectoryFetcher;

impl NewConnectCompanyDirectoryFetcher for HttpNewConnectCompanyDirectoryFetcher {
    fn fetch_company_directory_page(&self) -> Result<String, NewConnectCompanyDirectoryFetchError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()?;

        Ok(client.get(SOURCE_URL).send()?.error_for_status()?.text()?)
    }
}

#[cfg(test)]
pub struct EmbeddedNewConnectCompanyDirectoryFetcher;

#[cfg(test)]
impl NewConnectCompanyDirectoryFetcher for EmbeddedNewConnectCompanyDirectoryFetcher {
    fn fetch_company_directory_page(&self) -> Result<String, NewConnectCompanyDirectoryFetchError> {
        Ok(include_str!("../../samples/newconnect_company_directory.html").to_owned())
    }
}

pub fn fetch_company_directory_entries(
    fetcher: &impl NewConnectCompanyDirectoryFetcher,
) -> Result<(Vec<CompanyDirectoryEntry>, String), NewConnectCompanyDirectoryFetchError> {
    let html = fetcher.fetch_company_directory_page()?;
    let fetched_at = OffsetDateTime::now_utc().format(&Rfc3339)?;

    Ok((parse_company_directory_page_html(&html)?, fetched_at))
}

pub fn parse_company_directory_page_html(
    html: &str,
) -> Result<Vec<CompanyDirectoryEntry>, NewConnectCompanyDirectoryParseError> {
    parse_company_directory_html(html, EXCHANGE_CODE, BASE_URL).map_err(Into::into)
}

/// NewConnect company-directory adapter refresh (ADR 0069, plan v0.55 T2).
/// Behavior-preserving lift of the former `RefreshBehavior::Directory` arm — a
/// company-directory source, so it produces a `RefreshOutcome::Directory` and stays
/// OUT of the full-refresh sweep (`joins_full_refresh` = false).
pub struct NewConnectCompanyDirectoryRefresh;

impl crate::jobs::source_refresh::Fetcher for NewConnectCompanyDirectoryRefresh {
    fn refresh(
        &self,
        state: &crate::app_state::AppState,
        ctx: &crate::jobs::source_refresh::RefreshContext,
    ) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
        refresh_newconnect_company_directory_for_trigger(state, ctx.trigger)
            .map(crate::jobs::source_refresh::RefreshOutcome::Directory)
    }

    fn joins_full_refresh(&self) -> bool {
        false
    }
}

pub fn refresh_newconnect_company_directory_for_trigger(
    state: &crate::app_state::AppState,
    trigger: &str,
) -> Result<crate::storage::CompanyRegistryRefreshResult, String> {
    let _ = state.record_source_adapter_attempt(ADAPTER_ID, trigger);

    let fetcher = HttpNewConnectCompanyDirectoryFetcher;
    let (entries, fetched_at) = match fetch_company_directory_entries(&fetcher) {
        Ok(result) => result,
        Err(error) => {
            let message = error.to_string();
            let _ = state.record_source_adapter_error(ADAPTER_ID, &message);

            return Err(message);
        }
    };

    state
        .refresh_newconnect_company_directory(&entries, &fetched_at)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_directory_entries_from_newconnect_html() {
        let entries = parse_company_directory_page_html(include_str!(
            "../../samples/newconnect_company_directory.html"
        ))
        .expect("fixture should parse");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].qualified_ticker, "NC:4MB");
        assert_eq!(entries[0].display_name, "4MOBILITY SPÓŁKA AKCYJNA");
        assert_eq!(entries[0].isin, "PLESLTN00010");
        assert_eq!(
            entries[0].source_url,
            "https://newconnect.pl/spolka?isin=PLESLTN00010"
        );
        assert_eq!(entries[1].qualified_ticker, "NC:7FT");
    }

    #[test]
    fn golden_parsed_directory_entries() {
        // Golden ingestion pin (ADR 0069 / plan v0.55 T2): the sample directory HTML
        // must parse into a byte-stable set of entries across the Fetcher migration.
        let entries = parse_company_directory_page_html(include_str!(
            "../../samples/newconnect_company_directory.html"
        ))
        .expect("fixture should parse");
        insta::assert_debug_snapshot!("golden_newconnect_company_directory_entries", entries);
    }
}
