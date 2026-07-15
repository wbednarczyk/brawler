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

/// GPW company-registry adapter refresh (ADR 0069, plan v0.55 T2). Behavior-preserving
/// lift of the former `RefreshBehavior::Directory` arm — a company-directory source,
/// so it produces a `RefreshOutcome::Directory` and stays OUT of the full-refresh
/// sweep (`joins_full_refresh` = false), refreshing on its own bootstrap cadence.
pub struct GpwCompanyRegistryRefresh;

impl crate::jobs::source_refresh::Fetcher for GpwCompanyRegistryRefresh {
    fn refresh(
        &self,
        state: &crate::app_state::AppState,
        ctx: &crate::jobs::source_refresh::RefreshContext,
    ) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
        refresh_gpw_company_registry_for_trigger(state, ctx.trigger)
            .map(crate::jobs::source_refresh::RefreshOutcome::Directory)
    }

    fn joins_full_refresh(&self) -> bool {
        false
    }
}

pub fn refresh_gpw_company_registry_for_trigger(
    state: &crate::app_state::AppState,
    trigger: &str,
) -> Result<crate::storage::CompanyRegistryRefreshResult, String> {
    let _ = state.record_source_adapter_attempt(ADAPTER_ID, trigger);

    let fetcher = HttpGpwCompanyRegistryFetcher;
    let (entries, fetched_at) = match fetch_company_registry_entries(&fetcher) {
        Ok(result) => result,
        Err(error) => {
            let message = error.to_string();
            let _ = state.record_source_adapter_error(ADAPTER_ID, &message);

            return Err(message);
        }
    };

    state
        .refresh_gpw_company_registry(&entries, &fetched_at)
        .map_err(|error| error.to_string())
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
        assert_eq!(entries[0].sector.as_deref(), Some("Gry"));
        assert_eq!(entries[1].qualified_ticker, "GPW:CDR");
        assert_eq!(entries[1].sector.as_deref(), Some("Gry"));
    }

    #[test]
    fn golden_parsed_registry_entries() {
        // Golden ingestion pin (ADR 0069 / plan v0.55 T2): the sample registry HTML
        // must parse into a byte-stable set of entries across the Fetcher migration.
        let entries =
            parse_company_registry_html(include_str!("../../samples/gpw_company_registry.html"))
                .expect("fixture should parse");
        insta::assert_debug_snapshot!("golden_gpw_company_registry_entries", entries);
    }
}
