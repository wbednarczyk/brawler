use scraper::{Html, Selector};
use thiserror::Error;
use time::{
    format_description::well_known::Rfc3339, macros::format_description, OffsetDateTime,
    PrimitiveDateTime, UtcOffset,
};

pub const ADAPTER_ID: &str = "gpw-espi-ebi";
pub const DISPLAY_NAME: &str = "GPW ESPI/EBI";
pub const SOURCE_URL: &str = "https://www.gpw.pl/komunikaty";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwReportListing {
    pub report_type: String,
    pub system: String,
    pub report_number: String,
    pub company_name: String,
    pub isin: String,
    pub title: String,
    pub detail_url: String,
    pub published_at: String,
    pub fetched_at: String,
    pub dedupe_key: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GpwParseError {
    #[error("invalid selector: {0}")]
    InvalidSelector(String),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),
}

#[derive(Debug, Error)]
pub enum GpwFetchError {
    #[error("GPW HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("GPW listing parse failed: {0}")]
    Parse(#[from] GpwParseError),
    #[error("fetched timestamp formatting failed: {0}")]
    TimestampFormat(#[from] time::error::Format),
}

pub trait GpwPageFetcher {
    fn fetch_report_page(&self) -> Result<String, GpwFetchError>;
}

pub struct HttpGpwPageFetcher;

impl GpwPageFetcher for HttpGpwPageFetcher {
    fn fetch_report_page(&self) -> Result<String, GpwFetchError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Brawler/0.4 local-first investor research app")
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        Ok(client.get(SOURCE_URL).send()?.error_for_status()?.text()?)
    }
}

pub fn fetch_report_listings(
    fetcher: &impl GpwPageFetcher,
) -> Result<Vec<GpwReportListing>, GpwFetchError> {
    let html = fetcher.fetch_report_page()?;
    let fetched_at = OffsetDateTime::now_utc().format(&Rfc3339)?;

    Ok(parse_report_listings(&html, &fetched_at)?)
}

pub fn parse_report_listings(
    html: &str,
    fetched_at: &str,
) -> Result<Vec<GpwReportListing>, GpwParseError> {
    let document = Html::parse_document(html);
    let listing_selector = selector("[data-gpw-report-listing], .gpw-report-listing, li")?;
    let title_selector = selector("[data-field='title'], .report-title")?;
    let anchor_selector = selector("a")?;

    document
        .select(&listing_selector)
        .filter_map(|element| {
            let text = normalized_lines(element.text()).join("\n");
            if !(text.contains("ESPI") || text.contains("EBI"))
                || find_gpw_timestamp(&text).is_none()
            {
                return None;
            }

            Some(parse_listing_element(
                &text,
                element
                    .select(&title_selector)
                    .next()
                    .map(|title| normalized_lines(title.text()).join(" ")),
                element.select(&anchor_selector).filter_map(|anchor| {
                    let label = normalized_lines(anchor.text()).join(" ");
                    let href = anchor.value().attr("href")?;
                    Some((label, href.to_owned()))
                }),
                fetched_at,
            ))
        })
        .collect()
}

fn parse_listing_element(
    listing_text: &str,
    structured_title: Option<String>,
    anchors: impl Iterator<Item = (String, String)>,
    fetched_at: &str,
) -> Result<GpwReportListing, GpwParseError> {
    let lines = normalized_lines([listing_text]);
    let header = lines
        .iter()
        .find(|line| find_gpw_timestamp(line).is_some() && line.contains('|'))
        .map_or(listing_text, String::as_str);
    let header_parts = header
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    let raw_timestamp = find_gpw_timestamp(header).unwrap_or_default();
    let published_at = parse_source_timestamp(raw_timestamp)?;
    let report_type = header_parts.get(1).copied().unwrap_or("Unknown").to_owned();
    let system = header_parts.get(2).copied().unwrap_or("GPW").to_owned();
    let report_number = header_parts
        .get(3)
        .and_then(|value| value.split_whitespace().next())
        .unwrap_or("")
        .to_owned();

    let anchors = anchors.collect::<Vec<_>>();
    let company_label = anchors
        .iter()
        .map(|(label, _)| label.as_str())
        .find(|label| label.contains('(') && label.contains(')'))
        .unwrap_or("");
    let (company_name, isin) = parse_company_label(company_label);
    let detail_url = anchors
        .iter()
        .find(|(label, _)| label.to_lowercase().contains("więcej"))
        .or_else(|| anchors.first())
        .map(|(_, href)| absolute_gpw_url(href))
        .unwrap_or_else(|| SOURCE_URL.to_owned());
    let title = structured_title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| infer_title(&lines));
    let dedupe_key = build_dedupe_key(
        &system,
        &isin,
        &report_number,
        &published_at,
        &detail_url,
        &title,
    );

    Ok(GpwReportListing {
        report_type,
        system,
        report_number,
        company_name,
        isin,
        title,
        detail_url,
        published_at,
        fetched_at: fetched_at.to_owned(),
        dedupe_key,
    })
}

fn selector(raw: &str) -> Result<Selector, GpwParseError> {
    Selector::parse(raw).map_err(|_| GpwParseError::InvalidSelector(raw.to_owned()))
}

fn normalized_lines<'a>(text: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    text.into_iter()
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

fn find_gpw_timestamp(text: &str) -> Option<&str> {
    text.as_bytes()
        .windows(19)
        .position(is_gpw_timestamp_window)
        .map(|index| &text[index..index + 19])
}

fn is_gpw_timestamp_window(window: &[u8]) -> bool {
    matches!(
        window,
        [
            d1,
            d2,
            b'-',
            m1,
            m2,
            b'-',
            y1,
            y2,
            y3,
            y4,
            b' ',
            h1,
            h2,
            b':',
            min1,
            min2,
            b':',
            s1,
            s2
        ] if [
            d1, d2, m1, m2, y1, y2, y3, y4, h1, h2, min1, min2, s1, s2
        ].iter().all(|byte| byte.is_ascii_digit())
    )
}

fn parse_source_timestamp(raw: &str) -> Result<String, GpwParseError> {
    let format = format_description!("[day]-[month]-[year] [hour]:[minute]:[second]");
    let parsed = PrimitiveDateTime::parse(raw, &format)
        .map_err(|_| GpwParseError::InvalidTimestamp(raw.to_owned()))?;
    let warsaw_market_offset = UtcOffset::from_hms(2, 0, 0)
        .map_err(|_| GpwParseError::InvalidTimestamp(raw.to_owned()))?;
    parsed
        .assume_offset(warsaw_market_offset)
        .format(&Rfc3339)
        .map_err(|_| GpwParseError::InvalidTimestamp(raw.to_owned()))
}

fn parse_company_label(label: &str) -> (String, String) {
    if let (Some(open), Some(close)) = (label.rfind('('), label.rfind(')')) {
        if close > open {
            return (
                label[..open].trim().to_owned(),
                label[open + 1..close].trim().to_owned(),
            );
        }
    }

    (label.trim().to_owned(), String::new())
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

fn infer_title(lines: &[String]) -> String {
    lines
        .iter()
        .find(|line| {
            !line.contains('|')
                && !line.to_lowercase().contains("więcej")
                && !line.contains("Zmiana")
                && !line.contains("Kurs")
        })
        .cloned()
        .unwrap_or_default()
}

fn build_dedupe_key(
    system: &str,
    isin: &str,
    report_number: &str,
    published_at: &str,
    detail_url: &str,
    title: &str,
) -> String {
    if !system.is_empty()
        && !isin.is_empty()
        && !report_number.is_empty()
        && !published_at.is_empty()
    {
        format!(
            "{ADAPTER_ID}:{}:{}:{}:{}",
            slug_part(system),
            isin,
            report_number,
            published_at
        )
    } else {
        format!(
            "{ADAPTER_ID}:fallback:{}:{}",
            slug_part(detail_url),
            slug_part(title)
        )
    }
}

fn slug_part(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../fixtures/gpw_espi_ebi_listing.html");

    struct FixtureFetcher;

    impl GpwPageFetcher for FixtureFetcher {
        fn fetch_report_page(&self) -> Result<String, GpwFetchError> {
            Ok(FIXTURE.to_owned())
        }
    }

    #[test]
    fn parses_gpw_report_listing_fixture() {
        let listings =
            parse_report_listings(FIXTURE, "2026-05-30T17:30:00Z").expect("fixture should parse");

        assert_eq!(listings.len(), 2);
        assert_eq!(listings[0].report_type, "Bieżący");
        assert_eq!(listings[0].system, "ESPI");
        assert_eq!(listings[0].report_number, "7/2026");
        assert_eq!(listings[0].company_name, "NEW TECH CAPITAL SPÓŁKA AKCYJNA");
        assert_eq!(listings[0].isin, "PLECMNG00019");
        assert_eq!(
            listings[0].title,
            "Oświadczenie w sprawie formy przekazywania raportów kwartalnych."
        );
        assert_eq!(
            listings[0].detail_url,
            "https://www.gpw.pl/komunikaty?ph_main_01_start=show&ph_main_01_cmn_id=123456"
        );
        assert_eq!(listings[0].published_at, "2026-05-30T17:13:31+02:00");
        assert_eq!(listings[0].fetched_at, "2026-05-30T17:30:00Z");
        assert_eq!(
            listings[0].dedupe_key,
            "gpw-espi-ebi:espi:PLECMNG00019:7/2026:2026-05-30T17:13:31+02:00"
        );
    }

    #[test]
    fn parses_periodic_report_without_report_number_prefix() {
        let listings =
            parse_report_listings(FIXTURE, "2026-05-30T17:30:00Z").expect("fixture should parse");

        assert_eq!(listings[1].report_type, "Kwartalny");
        assert_eq!(listings[1].system, "ESPI");
        assert_eq!(listings[1].report_number, "/2026");
        assert_eq!(listings[1].isin, "PLWRTPL00027");
        assert_eq!(listings[1].published_at, "2026-05-30T09:51:45+02:00");
        assert!(listings[1].dedupe_key.contains("PLWRTPL00027:/2026"));
    }

    #[test]
    fn fetches_and_parses_with_injected_fetcher() {
        let listings = fetch_report_listings(&FixtureFetcher).expect("fixture fetch should parse");

        assert_eq!(listings.len(), 2);
        assert!(listings[0].detail_url.starts_with(SOURCE_URL));
        assert!(listings[0].fetched_at.ends_with('Z'));
    }
}
