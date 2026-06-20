use scraper::{Html, Selector};
use thiserror::Error;
use time::{
    format_description::well_known::Rfc3339, macros::format_description, OffsetDateTime,
    PrimitiveDateTime, UtcOffset,
};

use super::parsing::slug_part;
use super::USER_AGENT;

pub const ADAPTER_ID: &str = "gpw-espi-ebi";
pub const DISPLAY_NAME: &str = "GPW ESPI/EBI";
pub const SOURCE_URL: &str = "https://www.gpw.pl/komunikaty";
const LISTING_AJAX_URL: &str = "https://www.gpw.pl/ajaxindex.php";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwReportListing {
    pub report_type: String,
    pub system: String,
    pub report_number: String,
    pub company_ticker: String,
    pub company_name: String,
    pub isin: String,
    pub title: String,
    pub detail_url: String,
    pub published_at: String,
    pub fetched_at: String,
    pub dedupe_key: String,
    pub body_text: Option<String>,
    pub attachments: Vec<GpwReportAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwReportAttachment {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwReportDetail {
    pub title: String,
    pub body_text: String,
    pub attachments: Vec<GpwReportAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwDetailEvaluation {
    pub usable_for_ingestion: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwDetailSpikeReport {
    pub samples_checked: usize,
    pub usable_samples: usize,
    pub rejected_samples: usize,
    pub warnings: Vec<String>,
    pub promote_to_ingestion: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwDetailFetchPolicy {
    pub enabled_by_default: bool,
    pub max_details_per_refresh: usize,
    pub min_delay_between_requests_seconds: u64,
    pub matched_items_only: bool,
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

pub trait GpwDetailPageFetcher {
    fn fetch_report_detail_page(&self, detail_url: &str) -> Result<String, GpwFetchError>;
}

pub struct HttpGpwPageFetcher;

impl GpwPageFetcher for HttpGpwPageFetcher {
    fn fetch_report_page(&self) -> Result<String, GpwFetchError> {
        let client = gpw_http_client()?;
        let form = gpw_listing_ajax_form();

        Ok(client
            .post(LISTING_AJAX_URL)
            .form(&form)
            .send()?
            .error_for_status()?
            .text()?)
    }
}

impl GpwDetailPageFetcher for HttpGpwPageFetcher {
    fn fetch_report_detail_page(&self, detail_url: &str) -> Result<String, GpwFetchError> {
        let client = gpw_http_client()?;

        Ok(client.get(detail_url).send()?.error_for_status()?.text()?)
    }
}

pub fn fetch_report_listings(
    fetcher: &impl GpwPageFetcher,
) -> Result<Vec<GpwReportListing>, GpwFetchError> {
    let html = fetcher.fetch_report_page()?;
    let fetched_at = OffsetDateTime::now_utc().format(&Rfc3339)?;

    Ok(parse_report_listings(&html, &fetched_at)?)
}

pub fn fetch_report_detail(
    fetcher: &impl GpwDetailPageFetcher,
    detail_url: &str,
) -> Result<GpwReportDetail, GpwFetchError> {
    let html = fetcher.fetch_report_detail_page(detail_url)?;

    Ok(parse_report_detail(&html)?)
}

pub fn detail_fetch_policy() -> GpwDetailFetchPolicy {
    GpwDetailFetchPolicy {
        enabled_by_default: true,
        max_details_per_refresh: 5,
        min_delay_between_requests_seconds: 2,
        matched_items_only: true,
    }
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

fn gpw_http_client() -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .build()
}

fn gpw_listing_ajax_form() -> Vec<(&'static str, &'static str)> {
    vec![
        ("action", "GPWEspiReportUnion"),
        ("start", "ajaxSearch"),
        ("page", "komunikaty"),
        ("format", "html"),
        ("lang", "PL"),
        ("letter", ""),
        ("offset", "0"),
        ("limit", "15"),
        ("categoryRaports[]", "EBI"),
        ("categoryRaports[]", "ESPI"),
        ("typeRaports[]", "RB"),
        ("typeRaports[]", "P"),
        ("typeRaports[]", "Q"),
        ("typeRaports[]", "R"),
        ("typeRaports[]", "S"),
        ("typeRaports[]", "Y"),
        ("searchText", ""),
        ("date", ""),
    ]
}

pub fn parse_report_detail(html: &str) -> Result<GpwReportDetail, GpwParseError> {
    let document = Html::parse_document(html);
    let title_selector = selector("h1")?;
    let body_selector = selector(
        "[data-field='body'], .report-content, .report-body, .komunikat-tresc, .report-data, article",
    )?;
    let attachment_scope_selector =
        selector("[data-field='attachments'], .report-attachments, .attachments, .zalaczniki")?;
    let anchor_selector = selector("a")?;

    let title = document
        .select(&title_selector)
        .map(|element| normalized_lines(element.text()).join(" "))
        .filter(|candidate| !candidate.eq_ignore_ascii_case("Raporty Spółek ESPI/EBI"))
        .max_by_key(|candidate| candidate.len())
        .unwrap_or_default();

    let body_text = document
        .select(&body_selector)
        .find_map(|element| {
            let text = normalized_lines(element.text()).join("\n");
            if text.is_empty() || text == title {
                None
            } else {
                let body_text = extract_report_body_text(&text);
                if body_text.is_empty() {
                    None
                } else {
                    Some(body_text)
                }
            }
        })
        .unwrap_or_default();

    let attachments = document
        .select(&attachment_scope_selector)
        .flat_map(|scope| {
            scope
                .select(&anchor_selector)
                .filter_map(parse_attachment_anchor)
        })
        .collect();

    Ok(GpwReportDetail {
        title,
        body_text,
        attachments,
    })
}

pub fn evaluate_report_detail(detail: &GpwReportDetail) -> GpwDetailEvaluation {
    let mut warnings = Vec::new();

    if detail.title.trim().is_empty() {
        warnings.push("missing title".to_owned());
    }

    if detail.body_text.trim().len() < 80 {
        warnings.push("missing or very short report body".to_owned());
    }

    let usable_for_ingestion = warnings.is_empty();

    GpwDetailEvaluation {
        usable_for_ingestion,
        warnings,
    }
}

pub fn evaluate_report_detail_samples(details: &[GpwReportDetail]) -> GpwDetailSpikeReport {
    let evaluations = details
        .iter()
        .map(evaluate_report_detail)
        .collect::<Vec<_>>();
    let usable_samples = evaluations
        .iter()
        .filter(|evaluation| evaluation.usable_for_ingestion)
        .count();
    let warnings = evaluations
        .iter()
        .enumerate()
        .flat_map(|(index, evaluation)| {
            evaluation
                .warnings
                .iter()
                .map(move |warning| format!("sample {}: {warning}", index + 1))
        })
        .collect::<Vec<_>>();
    let samples_checked = details.len();
    let rejected_samples = samples_checked.saturating_sub(usable_samples);
    let promote_to_ingestion = samples_checked > 0 && rejected_samples == 0;

    GpwDetailSpikeReport {
        samples_checked,
        usable_samples,
        rejected_samples,
        warnings,
        promote_to_ingestion,
    }
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
        company_ticker: String::new(),
        company_name,
        isin,
        title,
        detail_url,
        published_at,
        fetched_at: fetched_at.to_owned(),
        dedupe_key,
        body_text: None,
        attachments: Vec::new(),
    })
}

fn extract_report_body_text(text: &str) -> String {
    let mut in_body = false;
    let body_lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            if is_report_body_marker(line) {
                in_body = true;
                return false;
            }

            if in_body && is_report_section_boundary(line) {
                in_body = false;
            }

            in_body
        })
        .collect::<Vec<_>>();

    if body_lines.is_empty() {
        strip_detail_boilerplate(text)
    } else {
        body_lines.join("\n")
    }
}

fn strip_detail_boilerplate(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_report_body_marker(line))
        .filter(|line| !is_report_section_boundary(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_report_body_marker(line: &str) -> bool {
    line.trim()
        .trim_start_matches(|character: char| character.is_ascii_digit() || character == '.')
        .trim()
        .eq_ignore_ascii_case("treść raportu:")
}

fn is_report_section_boundary(line: &str) -> bool {
    let normalized = line.to_lowercase();
    normalized.starts_with("nazwa arkusza:")
        || normalized == "załączniki"
        || normalized == "zalaczniki"
        || normalized == "message _english version_"
}

fn parse_attachment_anchor(anchor: scraper::ElementRef<'_>) -> Option<GpwReportAttachment> {
    let label = normalized_lines(anchor.text()).join(" ");
    let href = anchor.value().attr("href")?;
    let href_lower = href.to_lowercase();
    let looks_like_attachment = [".pdf", ".doc", ".docx", ".xls", ".xlsx", ".zip"]
        .iter()
        .any(|extension| href_lower.contains(extension))
        || href_lower.contains("/pub/");

    if label.is_empty() || !looks_like_attachment {
        return None;
    }

    Some(GpwReportAttachment {
        label,
        url: absolute_gpw_url(href),
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

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../samples/gpw_espi_ebi_listing.html");
    const DETAIL_FIXTURE: &str = include_str!("../../samples/gpw_espi_ebi_detail.html");
    const DETAIL_NO_ATTACHMENTS_FIXTURE: &str =
        include_str!("../../samples/gpw_espi_ebi_detail_no_attachments.html");

    struct FixtureFetcher;

    impl GpwPageFetcher for FixtureFetcher {
        fn fetch_report_page(&self) -> Result<String, GpwFetchError> {
            Ok(FIXTURE.to_owned())
        }
    }

    struct FixtureDetailFetcher;

    impl GpwDetailPageFetcher for FixtureDetailFetcher {
        fn fetch_report_detail_page(&self, detail_url: &str) -> Result<String, GpwFetchError> {
            assert_eq!(detail_url, "https://www.gpw.pl/komunikaty?fixture=detail");

            Ok(DETAIL_FIXTURE.to_owned())
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

    #[test]
    fn gpw_listing_ajax_form_requests_espi_and_ebi_reports() {
        let form = gpw_listing_ajax_form();

        assert!(form.contains(&("action", "GPWEspiReportUnion")));
        assert!(form.contains(&("start", "ajaxSearch")));
        assert!(form.contains(&("page", "komunikaty")));
        assert!(form.contains(&("format", "html")));
        assert!(form.contains(&("categoryRaports[]", "ESPI")));
        assert!(form.contains(&("categoryRaports[]", "EBI")));
        assert!(form.contains(&("limit", "15")));
    }

    #[test]
    fn parses_gpw_report_detail_fixture() {
        let detail = parse_report_detail(DETAIL_FIXTURE).expect("fixture should parse");

        assert_eq!(
            detail.title,
            "Oświadczenie w sprawie formy przekazywania raportów kwartalnych. NEW TECH CAPITAL SPÓŁKA AKCYJNA (PLECMNG00019)"
        );
        assert!(detail
            .body_text
            .contains("Zarząd NEW TECH CAPITAL S.A. informuje, że raporty kwartalne"));
        assert!(detail.body_text.contains("Podstawa prawna"));
        assert_eq!(detail.attachments.len(), 1);
        assert_eq!(detail.attachments[0].label, "7_2026_oswiadczenie.pdf");
        assert_eq!(
            detail.attachments[0].url,
            "https://www.gpw.pl/pub/GPW/ESPI/2026/7_2026_oswiadczenie.pdf"
        );
    }

    #[test]
    fn parses_gpw_report_detail_without_attachments() {
        let detail =
            parse_report_detail(DETAIL_NO_ATTACHMENTS_FIXTURE).expect("fixture should parse");

        assert!(detail.title.contains("LUBAWA SPÓŁKA AKCYJNA"));
        assert!(detail.body_text.contains("wysokim prawdopodobieństwie"));
        assert!(detail.body_text.contains("odrębnym komunikacie bieżącym"));
        assert!(!detail.body_text.contains("MESSAGE _ENGLISH VERSION_"));
        assert!(!detail.body_text.contains("PODPISY OSÓB"));
        assert!(detail.attachments.is_empty());
    }

    #[test]
    fn fetches_and_parses_detail_with_injected_fetcher() {
        let detail = fetch_report_detail(
            &FixtureDetailFetcher,
            "https://www.gpw.pl/komunikaty?fixture=detail",
        )
        .expect("fixture detail fetch should parse");

        assert!(detail.title.contains("NEW TECH CAPITAL"));
        assert!(detail.body_text.contains("raporty kwartalne"));
        assert_eq!(detail.attachments.len(), 1);
    }

    #[test]
    fn evaluates_detail_as_usable_when_title_and_body_exist() {
        let detail = parse_report_detail(DETAIL_FIXTURE).expect("fixture should parse");
        let evaluation = evaluate_report_detail(&detail);

        assert!(evaluation.usable_for_ingestion);
        assert!(evaluation.warnings.is_empty());
    }

    #[test]
    fn evaluates_detail_as_unusable_when_body_is_missing() {
        let detail = GpwReportDetail {
            title: "Short report".to_owned(),
            body_text: String::new(),
            attachments: Vec::new(),
        };
        let evaluation = evaluate_report_detail(&detail);

        assert!(!evaluation.usable_for_ingestion);
        assert_eq!(
            evaluation.warnings,
            vec!["missing or very short report body".to_owned()]
        );
    }

    #[test]
    fn evaluates_detail_as_unusable_when_title_is_missing() {
        let detail = GpwReportDetail {
            title: String::new(),
            body_text: "Long enough report body that should otherwise be usable for ingestion."
                .repeat(2),
            attachments: Vec::new(),
        };
        let evaluation = evaluate_report_detail(&detail);

        assert!(!evaluation.usable_for_ingestion);
        assert_eq!(evaluation.warnings, vec!["missing title".to_owned()]);
    }

    #[test]
    fn evaluates_detail_sample_set_as_promotable_when_all_samples_are_usable() {
        let details = vec![
            parse_report_detail(DETAIL_FIXTURE).expect("fixture should parse"),
            parse_report_detail(DETAIL_NO_ATTACHMENTS_FIXTURE).expect("fixture should parse"),
        ];
        let report = evaluate_report_detail_samples(&details);

        assert_eq!(report.samples_checked, 2);
        assert_eq!(report.usable_samples, 2);
        assert_eq!(report.rejected_samples, 0);
        assert!(report.warnings.is_empty());
        assert!(report.promote_to_ingestion);
    }

    #[test]
    fn evaluates_detail_sample_set_as_not_promotable_when_any_sample_is_rejected() {
        let details = vec![
            parse_report_detail(DETAIL_FIXTURE).expect("fixture should parse"),
            GpwReportDetail {
                title: "Broken detail".to_owned(),
                body_text: String::new(),
                attachments: Vec::new(),
            },
        ];
        let report = evaluate_report_detail_samples(&details);

        assert_eq!(report.samples_checked, 2);
        assert_eq!(report.usable_samples, 1);
        assert_eq!(report.rejected_samples, 1);
        assert_eq!(
            report.warnings,
            vec!["sample 2: missing or very short report body".to_owned()]
        );
        assert!(!report.promote_to_ingestion);
    }

    #[test]
    fn exposes_conservative_detail_fetch_policy() {
        let policy = detail_fetch_policy();

        assert!(policy.enabled_by_default);
        assert_eq!(policy.max_details_per_refresh, 5);
        assert_eq!(policy.min_delay_between_requests_seconds, 2);
        assert!(policy.matched_items_only);
    }
}

#[cfg(test)]
mod dedupe_key_proptests {
    //! Invariant coverage of the dedup-key builder (ADR 0049): the key that
    //! decides whether two ingested reports are the same item must be
    //! deterministic, total (never panic on arbitrary source text), and always
    //! namespaced to this adapter so keys cannot collide across adapters.
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn build_dedupe_key_is_deterministic_total_and_namespaced(
            system in ".*", isin in ".*", report in ".*",
            published in ".*", url in ".*", title in ".*"
        ) {
            let a = build_dedupe_key(&system, &isin, &report, &published, &url, &title);
            let b = build_dedupe_key(&system, &isin, &report, &published, &url, &title);
            prop_assert_eq!(&a, &b, "dedupe key is not deterministic");
            prop_assert!(a.starts_with(ADAPTER_ID), "dedupe key not namespaced: {a:?}");
        }
    }
}
