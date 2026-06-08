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
        Ok(include_str!("../../fixtures/newconnect_company_directory.html").to_owned())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_directory_entries_from_newconnect_html() {
        let entries = parse_company_directory_page_html(include_str!(
            "../../fixtures/newconnect_company_directory.html"
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
}
