use scraper::{Html, Selector};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::USER_AGENT;

pub const ADAPTER_ID: &str = "gpw-company-registry";
pub const DISPLAY_NAME: &str = "GPW Company Registry";
pub const SOURCE_URL: &str = "https://www.gpw.pl/spolki?offset=0&limit=500";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwCompanyRegistryEntry {
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub isin: String,
    pub source_url: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GpwCompanyRegistryParseError {
    #[error("invalid selector: {0}")]
    InvalidSelector(String),
    #[error("no GPW company registry rows found")]
    NoRows,
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
        Ok(include_str!("../../fixtures/gpw_company_registry.html").to_owned())
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
    let document = Html::parse_document(html);
    let row_selector = selector("#search-result tr")?;
    let anchor_selector = selector("a[href*='spolka?isin=']")?;
    let name_selector = selector("strong.name")?;

    let entries = document
        .select(&row_selector)
        .filter_map(|row| {
            let anchor = row.select(&anchor_selector).next()?;
            let href = anchor.value().attr("href")?;
            let isin = extract_isin_from_company_href(href)?;
            let source_url = absolute_gpw_url(href);
            let name_text = row
                .select(&name_selector)
                .next()
                .map(|name| normalized_text(name.text()))
                .unwrap_or_else(|| normalized_text(anchor.text()));
            let (display_name, ticker) = split_company_name_and_ticker(&name_text)?;
            let exchange = "GPW".to_owned();

            Some(GpwCompanyRegistryEntry {
                qualified_ticker: format!("{exchange}:{ticker}"),
                exchange,
                ticker,
                display_name,
                isin,
                source_url,
            })
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        Err(GpwCompanyRegistryParseError::NoRows)
    } else {
        Ok(entries)
    }
}

fn selector(value: &str) -> Result<Selector, GpwCompanyRegistryParseError> {
    Selector::parse(value)
        .map_err(|_| GpwCompanyRegistryParseError::InvalidSelector(value.to_owned()))
}

fn extract_isin_from_company_href(href: &str) -> Option<String> {
    href.split('?')
        .nth(1)?
        .split('&')
        .find_map(|part| part.strip_prefix("isin="))
        .map(|isin| isin.trim().to_uppercase())
        .filter(|isin| !isin.is_empty())
}

fn split_company_name_and_ticker(value: &str) -> Option<(String, String)> {
    let open = value.rfind('(')?;
    let close = value.rfind(')')?;
    if close <= open {
        return None;
    }

    let display_name = value[..open].trim().to_owned();
    let ticker = value[open + 1..close].trim().to_uppercase();

    if display_name.is_empty() || ticker.is_empty() {
        None
    } else {
        Some((display_name, ticker))
    }
}

fn normalized_text<'a>(text: impl Iterator<Item = &'a str>) -> String {
    text.collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn absolute_gpw_url(href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_owned()
    } else if href.starts_with('/') {
        format!("https://www.gpw.pl{href}")
    } else {
        format!("https://www.gpw.pl/{href}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixture_registry_entries_from_gpw_html() {
        let entries =
            parse_company_registry_html(include_str!("../../fixtures/gpw_company_registry.html"))
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
