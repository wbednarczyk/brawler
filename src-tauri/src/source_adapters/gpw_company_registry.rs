use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::{
    company_directory::{
        parse_company_directory_html, CompanyDirectoryEntry, CompanyDirectoryParseError,
    },
    USER_AGENT,
};

pub const ADAPTER_ID: &str = "gpw-company-registry";
pub const DISPLAY_NAME: &str = "GPW Company Registry";
pub const SOURCE_URL: &str = "https://www.gpw.pl/spolki?offset=0&limit=500";
const BASE_URL: &str = "https://www.gpw.pl";

pub type GpwCompanyRegistryEntry = CompanyDirectoryEntry;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GpwCompanyRegistryParseError {
    #[error("invalid selector: {0}")]
    InvalidSelector(String),
    #[error("no GPW company registry rows found")]
    NoRows,
}

impl From<CompanyDirectoryParseError> for GpwCompanyRegistryParseError {
    fn from(error: CompanyDirectoryParseError) -> Self {
        match error {
            CompanyDirectoryParseError::InvalidSelector(value) => Self::InvalidSelector(value),
            CompanyDirectoryParseError::NoRows => Self::NoRows,
        }
    }
}

#[derive(Debug, Error)]
pub enum GpwCompanyRegistryFetchError {
    #[error("GPW company registry HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("GPW company registry parse failed: {0}")]
    Parse(#[from] GpwCompanyRegistryParseError),
    #[error("fetched timestamp formatting failed: {0}")]
    TimestampFormat(#[from] time::error::Format),
}

pub trait GpwCompanyRegistryFetcher {
    fn fetch_company_registry_page(&self) -> Result<String, GpwCompanyRegistryFetchError>;
}

pub struct HttpGpwCompanyRegistryFetcher;

impl GpwCompanyRegistryFetcher for HttpGpwCompanyRegistryFetcher {
    fn fetch_company_registry_page(&self) -> Result<String, GpwCompanyRegistryFetchError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()?;

        Ok(client.get(SOURCE_URL).send()?.error_for_status()?.text()?)
    }
}

#[cfg(test)]
pub struct EmbeddedGpwCompanyRegistryFetcher;

#[cfg(test)]
impl GpwCompanyRegistryFetcher for EmbeddedGpwCompanyRegistryFetcher {
    fn fetch_company_registry_page(&self) -> Result<String, GpwCompanyRegistryFetchError> {
        Ok(include_str!("../../samples/gpw_company_registry.html").to_owned())
    }
}

pub fn fetch_company_registry_entries(
    fetcher: &impl GpwCompanyRegistryFetcher,
) -> Result<(Vec<GpwCompanyRegistryEntry>, String), GpwCompanyRegistryFetchError> {
    let html = fetcher.fetch_company_registry_page()?;
    let fetched_at = OffsetDateTime::now_utc().format(&Rfc3339)?;

    Ok((parse_company_registry_html(&html)?, fetched_at))
}

pub fn parse_company_registry_html(
    html: &str,
) -> Result<Vec<GpwCompanyRegistryEntry>, GpwCompanyRegistryParseError> {
    parse_company_directory_html(html, "GPW", BASE_URL).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_registry_entries_from_gpw_html() {
        let entries =
            parse_company_registry_html(include_str!("../../samples/gpw_company_registry.html"))
                .expect("fixture should parse");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].qualified_ticker, "GPW:11B");
        assert_eq!(entries[0].display_name, "11 BIT STUDIOS SPÓŁKA AKCYJNA");
        assert_eq!(entries[0].isin, "PL11BTS00015");
        assert_eq!(
            entries[0].source_url,
            "https://www.gpw.pl/spolka?isin=PL11BTS00015"
        );
        assert_eq!(entries[1].qualified_ticker, "GPW:CDR");
    }
}
