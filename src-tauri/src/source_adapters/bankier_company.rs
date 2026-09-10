use super::bankier_links::{
    is_report_attachment_url, resolve_attachment_href, resolve_listing_link,
};
use super::parsing::slug_part;
use super::USER_AGENT;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use time::{
    format_description::well_known::Rfc3339, macros::format_description, Duration, OffsetDateTime,
};
use url::Url;

pub const ADAPTER_ID: &str = "bankier-company-komunikaty";
pub const DISPLAY_NAME: &str = "Bankier Company Komunikaty";
pub const SOURCE_URL: &str = "https://www.bankier.pl/gielda/notowania/akcje/{TICKER}/komunikaty";
pub const API_SOURCE_URL: &str = "https://api.bankier.pl/articles/listing/";
pub const ATTRIBUTION: &str = "Bankier.pl";
pub const PAGE_LIMIT: usize = 25;
pub const RECENT_WINDOW_DAYS: i64 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankierCompanyTarget {
    pub company_id: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub bankier_slug: Option<String>,
    pub bankier_tag_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankierCompanyIdentifiers {
    pub slug: String,
    pub tag_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankierCompanyItem {
    pub company_id: String,
    pub qualified_ticker: String,
    pub title: String,
    pub link: String,
    pub summary: String,
    pub published_at: Option<String>,
    pub fetched_at: String,
    pub article_id: String,
    pub pub_id: i64,
    pub dedupe_key: String,
    pub duplicate_signature: String,
    pub body_text: Option<String>,
    pub attachments: Vec<BankierCompanyAttachment>,
    pub detail_fetch_attempted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankierCompanyAttachment {
    pub label: String,
    pub url: String,
    /// The source's own `href` was incomplete (no directory), joined against
    /// the page URL rather than trusted as-is (#460); never fetched.
    pub incomplete: bool,
}

#[derive(Debug, Error)]
pub enum BankierCompanyError {
    #[error("Bankier company HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Bankier company parse failed: {0}")]
    Parse(#[from] BankierCompanyParseError),
    #[error("fetched timestamp formatting failed: {0}")]
    TimestampFormat(#[from] time::error::Format),
    #[error("Bankier company URL build failed: {0}")]
    Url(#[from] url::ParseError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankierCompanyParseError {
    #[error("missing Bankier company tag id")]
    MissingTagId,
    #[error("missing Bankier company canonical slug")]
    MissingSlug,
    #[error("invalid Bankier company JSON: {0}")]
    Json(String),
}

pub trait BankierCompanyFetcher {
    fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError>;
}

pub struct HttpBankierCompanyFetcher;

impl BankierCompanyFetcher for HttpBankierCompanyFetcher {
    fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        Ok(client.get(url).send()?.error_for_status()?.text()?)
    }
}

pub fn fetch_company_items(
    fetcher: &impl BankierCompanyFetcher,
    target: &BankierCompanyTarget,
) -> Result<(Option<BankierCompanyIdentifiers>, Vec<BankierCompanyItem>), BankierCompanyError> {
    fetch_company_items_with_detail_filter(fetcher, target, |_| true)
}

pub fn fetch_company_items_with_detail_filter(
    fetcher: &impl BankierCompanyFetcher,
    target: &BankierCompanyTarget,
    should_fetch_detail: impl Fn(&BankierCompanyItem) -> bool,
) -> Result<(Option<BankierCompanyIdentifiers>, Vec<BankierCompanyItem>), BankierCompanyError> {
    let fetched_at = time::OffsetDateTime::now_utc().format(&Rfc3339)?;

    fetch_company_items_with_detail_filter_at(fetcher, target, &fetched_at, should_fetch_detail)
}

fn fetch_company_items_with_detail_filter_at(
    fetcher: &impl BankierCompanyFetcher,
    target: &BankierCompanyTarget,
    fetched_at: &str,
    should_fetch_detail: impl Fn(&BankierCompanyItem) -> bool,
) -> Result<(Option<BankierCompanyIdentifiers>, Vec<BankierCompanyItem>), BankierCompanyError> {
    let identifiers = match (&target.bankier_slug, &target.bankier_tag_id) {
        (Some(slug), Some(tag_id)) if !slug.trim().is_empty() && !tag_id.trim().is_empty() => None,
        _ => {
            let html = fetcher.fetch_text(&company_page_url(&target.ticker))?;
            Some(parse_company_identifiers(&html)?)
        }
    };

    let tag_id = identifiers
        .as_ref()
        .map(|identifiers| identifiers.tag_id.as_str())
        .or(target.bankier_tag_id.as_deref())
        .expect("tag id must be present after identifier resolution");
    let json = fetcher.fetch_text(&listing_api_url(tag_id, 1, PAGE_LIMIT)?)?;
    let mut items = parse_company_listing_json(target, &json, fetched_at)?;

    for item in &mut items {
        if !should_fetch_detail(item) {
            continue;
        }

        let html = fetcher.fetch_text(&item.link)?;
        let detail = parse_company_report_detail(&html, &item.title, &item.link);
        item.detail_fetch_attempted = true;
        item.body_text = detail.body_text;
        item.attachments = detail.attachments;
    }

    Ok((identifiers, items))
}

/// Per-page diagnostics surfaced by `fetch_company_backfill_items`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BackfillFetchStats {
    pub pages_fetched: usize,
    pub detail_errors: usize,
    /// True when the page cap (`max_pages`) ended the walk before the cutoff date
    /// was reached — older filings may exist but were not fetched (ADR 0077 §3).
    /// The caller surfaces this as an explicit truncation warning, never silent.
    pub truncated: bool,
}

/// Paginate the company komunikaty listing backward, keeping items down to `cutoff` (an
/// ISO `YYYY-MM-DDTHH:MM:SS` lower bound), up to `max_pages`. Report detail (body + attachments)
/// is fetched for kept items so classification, attachment registration, and event derivation
/// see the same data the live path does. Detail-fetch failures are counted, not fatal. The
/// listing is time-desc, so pagination stops once a page reaches items older than the cutoff.
/// Throttling between requests is the caller's `delay`. See ADR 0036.
#[allow(clippy::type_complexity)]
pub fn fetch_company_backfill_items(
    fetcher: &impl BankierCompanyFetcher,
    target: &BankierCompanyTarget,
    cutoff: &str,
    max_pages: usize,
    delay: std::time::Duration,
    mut on_progress: impl FnMut(usize, usize),
) -> Result<
    (
        Option<BankierCompanyIdentifiers>,
        Vec<BankierCompanyItem>,
        BackfillFetchStats,
    ),
    BankierCompanyError,
> {
    let fetched_at = time::OffsetDateTime::now_utc().format(&Rfc3339)?;

    let identifiers = match (&target.bankier_slug, &target.bankier_tag_id) {
        (Some(slug), Some(tag_id)) if !slug.trim().is_empty() && !tag_id.trim().is_empty() => None,
        _ => {
            let html = fetcher.fetch_text(&company_page_url(&target.ticker))?;
            Some(parse_company_identifiers(&html)?)
        }
    };
    let tag_id = identifiers
        .as_ref()
        .map(|identifiers| identifiers.tag_id.as_str())
        .or(target.bankier_tag_id.as_deref())
        .expect("tag id must be present after identifier resolution")
        .to_owned();

    let mut items: Vec<BankierCompanyItem> = Vec::new();
    let mut stats = BackfillFetchStats::default();
    // The walk ends "naturally" when it runs out of filings (empty page) or
    // reaches the cutoff date. If instead it exhausts `max_pages` while filings
    // newer than the cutoff remain, the page cap truncated the history.
    let mut reached_natural_end = false;

    for page in 1..=max_pages.max(1) {
        if page > 1 && !delay.is_zero() {
            std::thread::sleep(delay);
        }

        let json = fetcher.fetch_text(&listing_api_url(&tag_id, page, PAGE_LIMIT)?)?;
        let page_items = parse_company_listing_json_all(target, &json, &fetched_at)?;
        stats.pages_fetched += 1;
        on_progress(stats.pages_fetched, items.len());

        if page_items.is_empty() {
            reached_natural_end = true;
            break;
        }

        let mut reached_cutoff = false;
        for mut item in page_items {
            if item_is_older_than_cutoff(&item, cutoff) {
                reached_cutoff = true;
                continue;
            }

            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
            match fetcher.fetch_text(&item.link) {
                Ok(html) => {
                    let detail = parse_company_report_detail(&html, &item.title, &item.link);
                    item.detail_fetch_attempted = true;
                    item.body_text = detail.body_text;
                    item.attachments = detail.attachments;
                }
                Err(_) => {
                    item.detail_fetch_attempted = true;
                    stats.detail_errors += 1;
                }
            }
            items.push(item);
            on_progress(stats.pages_fetched, items.len());
        }

        if reached_cutoff {
            reached_natural_end = true;
            break;
        }
    }

    stats.truncated = !reached_natural_end;

    Ok((identifiers, items, stats))
}

fn item_is_older_than_cutoff(item: &BankierCompanyItem, cutoff: &str) -> bool {
    match item.published_at.as_deref() {
        Some(published_at) => published_at < cutoff,
        // Undated items are kept (treated as in-window) so they are not silently dropped.
        None => false,
    }
}

/// Whether a Bankier company filing is a periodic / financial report (quarterly, half-year,
/// or annual financial statements) as opposed to a routine current report (`raport bieżący`).
///
/// Periodic reports are the documents AI KPI extraction and report-over-report diff consume,
/// so their attachments are downloaded and stored in full; other ESPI/EBI attachments persist
/// as metadata + URL only (ADR 0036). Detection is a deterministic Polish-language heuristic
/// over the report title and body, not a per-company rule.
pub fn is_periodic_report_item(item: &BankierCompanyItem) -> bool {
    let haystack = match &item.body_text {
        Some(body) => format!("{} {}", item.title, body),
        None => item.title.clone(),
    };
    text_marks_periodic_report(&haystack)
}

fn text_marks_periodic_report(text: &str) -> bool {
    let normalized = text.to_lowercase();
    const MARKERS: &[&str] = &[
        "raport okresowy",
        "raport kwartalny",
        "raport półroczny",
        "raport polroczny",
        "raport roczny",
        "skonsolidowany raport",
        "jednostkowy raport",
        "rozszerzony skonsolidowany raport",
        "wyniki finansowe",
        "sprawozdanie finansowe",
    ];
    if MARKERS.iter().any(|marker| normalized.contains(marker)) {
        return true;
    }

    // GPW ESPI periodic-report form codes: SA-Q / SA-R / SA-P / QSr / PSr and consolidated variants.
    const FORM_CODES: &[&str] = &[
        "sa-q", "sa-r", "sa-p", "qsr", "psr", "skr-q", "skr-r", "scr-q", "scr-r",
    ];
    let tokens: Vec<&str> = normalized
        .split(|c: char| !(c.is_alphanumeric() || c == '-'))
        .filter(|t| !t.is_empty())
        .collect();
    FORM_CODES
        .iter()
        .any(|code| tokens.iter().any(|token| token == code))
}

pub fn company_page_url(ticker: &str) -> String {
    format!(
        "https://www.bankier.pl/gielda/notowania/akcje/{}/komunikaty",
        ticker.trim().to_uppercase()
    )
}

pub fn listing_api_url(tag_id: &str, page: usize, limit: usize) -> Result<String, url::ParseError> {
    let mut url = Url::parse(&format!("{API_SOURCE_URL}{page}/{limit}"))?;
    url.query_pairs_mut()
        .append_pair("tags_ids", tag_id)
        .append_pair("sort", "time_utc desc")
        .append_pair(
            "fields",
            "url,title,time,pub_id,messages_filters,tags,main_section_priority",
        )
        .append_pair("fq", r#"{"pub_id":"3,379"}"#);
    Ok(url.to_string())
}

pub fn parse_company_identifiers(
    html: &str,
) -> Result<BankierCompanyIdentifiers, BankierCompanyParseError> {
    let document = Html::parse_document(html);
    let tag_selector = Selector::parse("[data-tag-id]").expect("valid selector");
    let canonical_selector = Selector::parse("link[rel='canonical']").expect("valid selector");
    let og_url_selector = Selector::parse("meta[property='og:url']").expect("valid selector");

    let tag_id = document
        .select(&tag_selector)
        .find_map(|element| element.value().attr("data-tag-id"))
        .or_else(|| extract_between(html, "data-tag-id=\"", "\""))
        .ok_or(BankierCompanyParseError::MissingTagId)?;
    let canonical_url = document
        .select(&canonical_selector)
        .find_map(|element| element.value().attr("href"))
        .or_else(|| {
            document
                .select(&og_url_selector)
                .find_map(|element| element.value().attr("content"))
        })
        .or_else(|| extract_between(html, "rel=\"canonical\" href=\"", "\""))
        .or_else(|| extract_between(html, "property=\"og:url\" content=\"", "\""))
        .ok_or(BankierCompanyParseError::MissingSlug)?;
    let slug = canonical_url
        .split("/akcje/")
        .nth(1)
        .and_then(|value| value.split('/').next())
        .filter(|value| !value.trim().is_empty())
        .ok_or(BankierCompanyParseError::MissingSlug)?;

    Ok(BankierCompanyIdentifiers {
        slug: slug.to_owned(),
        tag_id: tag_id.to_owned(),
    })
}

pub fn parse_company_listing_json(
    target: &BankierCompanyTarget,
    json: &str,
    fetched_at: &str,
) -> Result<Vec<BankierCompanyItem>, BankierCompanyParseError> {
    let response: ListingResponse = serde_json::from_str(json)
        .map_err(|error| BankierCompanyParseError::Json(error.to_string()))?;
    let cutoff = recent_window_cutoff(fetched_at);

    Ok(response
        .articles
        .into_iter()
        .filter_map(|article| article.into_item(target, fetched_at))
        .filter(|item| item_is_in_recent_window(item, cutoff.as_deref()))
        .collect())
}

/// Parse a listing page without applying the recent-window filter. Used by history backfill,
/// which keeps items down to its own multi-year cutoff instead of the live 7-day window.
pub fn parse_company_listing_json_all(
    target: &BankierCompanyTarget,
    json: &str,
    fetched_at: &str,
) -> Result<Vec<BankierCompanyItem>, BankierCompanyParseError> {
    let response: ListingResponse = serde_json::from_str(json)
        .map_err(|error| BankierCompanyParseError::Json(error.to_string()))?;

    Ok(response
        .articles
        .into_iter()
        .filter_map(|article| article.into_item(target, fetched_at))
        .collect())
}

#[derive(Debug, Deserialize)]
struct ListingResponse {
    #[serde(default)]
    articles: Vec<ListingArticle>,
}

#[derive(Debug, Deserialize)]
struct ListingArticle {
    title: Option<String>,
    url: Option<String>,
    time: Option<String>,
    pub_id: Option<i64>,
    article_id: Option<i64>,
    #[serde(default)]
    messages_filters: Vec<String>,
}

impl ListingArticle {
    fn into_item(
        self,
        target: &BankierCompanyTarget,
        fetched_at: &str,
    ) -> Option<BankierCompanyItem> {
        let title = normalize_company_report_title(self.title?.trim());
        let raw_link = self.url?.trim().to_owned();
        let article_id = self.article_id?.to_string();
        if title.is_empty() || raw_link.is_empty() {
            return None;
        }

        let link = resolve_listing_link(&raw_link);
        let published_at = self.time.as_deref().and_then(normalize_article_time);
        let pub_id = self.pub_id.unwrap_or_default();
        let summary = summary_from_filters(&self.messages_filters);
        let dedupe_key = format!("{ADAPTER_ID}:article:{article_id}");
        let duplicate_signature = official_duplicate_signature(target, &title, &article_id);

        Some(BankierCompanyItem {
            company_id: target.company_id.clone(),
            qualified_ticker: target.qualified_ticker.clone(),
            title,
            link,
            summary,
            published_at,
            fetched_at: fetched_at.to_owned(),
            article_id,
            pub_id,
            dedupe_key,
            duplicate_signature,
            body_text: None,
            attachments: Vec::new(),
            detail_fetch_attempted: false,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BankierCompanyReportDetail {
    body_text: Option<String>,
    attachments: Vec<BankierCompanyAttachment>,
}

fn parse_company_report_detail(
    html: &str,
    title: &str,
    page_url: &str,
) -> BankierCompanyReportDetail {
    let document = Html::parse_document(html);
    let body_text = extract_structured_article_body(&document).unwrap_or_else(|| {
        let root_text = normalized_lines(document.root_element().text());
        extract_report_body(&root_text, title)
    });
    let attachments = extract_report_attachments(&document, page_url);

    BankierCompanyReportDetail {
        body_text: empty_string_to_none(body_text),
        attachments,
    }
}

fn extract_structured_article_body(document: &Html) -> Option<String> {
    let script_selector =
        Selector::parse("script[type='application/ld+json']").expect("valid selector");

    document
        .select(&script_selector)
        .filter_map(|script| serde_json::from_str::<Value>(&script.inner_html()).ok())
        .find_map(article_body_from_json_value)
        .map(normalize_report_body_text)
        .filter(|body| !body.trim().is_empty())
}

fn article_body_from_json_value(value: Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if json_type_matches(&object, "NewsArticle") {
                object
                    .get("articleBody")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            } else if let Some(graph) = object.get("@graph") {
                article_body_from_json_value(graph.clone())
            } else {
                None
            }
        }
        Value::Array(values) => values.into_iter().find_map(article_body_from_json_value),
        _ => None,
    }
}

fn json_type_matches(object: &serde_json::Map<String, Value>, expected_type: &str) -> bool {
    match object.get("@type") {
        Some(Value::String(value)) => value == expected_type,
        Some(Value::Array(values)) => values
            .iter()
            .any(|value| value.as_str() == Some(expected_type)),
        _ => false,
    }
}

fn normalize_report_body_text(value: String) -> String {
    decode_basic_html_entities(&value)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_basic_html_entities(value: &str) -> String {
    value
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn extract_report_body(lines: &[String], title: &str) -> String {
    let source_index = lines
        .iter()
        .position(|line| line.starts_with("Źródło:Komunikaty spółek"))
        .unwrap_or(lines.len());
    let start_index = report_body_start_index(&lines[..source_index], title).unwrap_or(0);

    lines[start_index..source_index]
        .iter()
        .map(String::as_str)
        .filter(|line| !is_report_detail_noise(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn report_body_start_index(lines: &[String], title: &str) -> Option<usize> {
    let title = normalize_text_for_match(title);
    lines
        .iter()
        .position(|line| {
            let normalized = normalize_text_for_match(line);
            !title.is_empty() && (normalized == title || normalized.ends_with(&title))
        })
        .map(|index| index + 1)
        .or_else(|| {
            lines.iter().position(|line| {
                matches!(
                    line.as_str(),
                    "Spis treści:"
                        | "RAPORT BIEŻĄCY"
                        | "RAPORT OKRESOWY"
                        | "STRONA TYTUŁOWA>>>"
                        | "STRONA TYTUŁOWA"
                )
            })
        })
}

fn is_report_detail_noise(line: &str) -> bool {
    matches!(
        line,
        "Podziel się"
            | "Skomentuj"
            | "publikacja"
            | "REKLAMA"
            | "BADANIE"
            | "Tematy"
            | "Komentarze (0)"
            | "dodaj komentarz"
    ) || line.starts_with("Spis załączników:")
}

fn extract_report_attachments(document: &Html, page_url: &str) -> Vec<BankierCompanyAttachment> {
    let anchor_selector = Selector::parse("a").expect("valid selector");

    document
        .select(&anchor_selector)
        .filter_map(|anchor| {
            let href = anchor.value().attr("href")?.trim();
            let label = normalized_lines(anchor.text()).join(" ");
            if !is_report_attachment_url(href) || is_source_page_chrome_link(&label) {
                return None;
            }

            let resolved = resolve_attachment_href(href, page_url);
            if !resolved.scheme_ok {
                return None;
            }
            Some(BankierCompanyAttachment {
                label: if label.is_empty() {
                    href.rsplit('/').next().unwrap_or(href).to_owned()
                } else {
                    label
                },
                url: resolved.url,
                incomplete: resolved.incomplete,
            })
        })
        .fold(Vec::new(), |mut attachments, attachment| {
            if !attachments
                .iter()
                .any(|existing: &BankierCompanyAttachment| existing.url == attachment.url)
            {
                attachments.push(attachment);
            }
            attachments
        })
}

fn is_source_page_chrome_link(label: &str) -> bool {
    matches!(
        label.trim().to_lowercase().as_str(),
        "regulamin" | "polityka prywatności" | "polityka prywatnosci" | "polityka cookies"
    )
}

fn normalized_lines<'a>(text: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    text.into_iter()
        .flat_map(str::lines)
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

fn normalize_text_for_match(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn empty_string_to_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn normalize_company_report_title(value: &str) -> String {
    value
        .split_once(':')
        .map(|(_, title)| title.trim())
        .filter(|title| !title.is_empty())
        .unwrap_or(value)
        .to_owned()
}

fn normalize_article_time(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.len() >= 19 {
        Some(format!("{}T{}", &trimmed[0..10], &trimmed[11..19]))
    } else {
        Some(trimmed.to_owned())
    }
}

fn recent_window_cutoff(fetched_at: &str) -> Option<String> {
    let cutoff = OffsetDateTime::parse(fetched_at, &Rfc3339)
        .ok()?
        .saturating_sub(Duration::days(RECENT_WINDOW_DAYS));
    cutoff
        .format(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second]"
        ))
        .ok()
}

fn item_is_in_recent_window(item: &BankierCompanyItem, cutoff: Option<&str>) -> bool {
    let Some(cutoff) = cutoff else {
        return true;
    };
    let Some(published_at) = item.published_at.as_deref() else {
        return true;
    };

    published_at >= cutoff
}

fn summary_from_filters(filters: &[String]) -> String {
    if filters.iter().any(|value| !value.trim().is_empty()) {
        "Komunikat ESPI/EBI".to_owned()
    } else {
        String::new()
    }
}

fn official_duplicate_signature(
    target: &BankierCompanyTarget,
    title: &str,
    article_id: &str,
) -> String {
    format!(
        "official-secondary:{}:{}:{}",
        target.qualified_ticker,
        slug_part(title),
        article_id
    )
}

fn extract_between<'a>(value: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    let start = value.find(prefix)? + prefix.len();
    let end = value[start..].find(suffix)? + start;
    Some(&value[start..end])
}

/// Bankier per-company adapter refresh (ADR 0069, plan v0.55 T2). Behavior-preserving
/// lift of the former `RefreshBehavior::Feed(refresh_bankier_company_for_trigger)`
/// arm — it is a *planner* (ADR 0059) that enqueues one per-company job, not an
/// ingest itself, so its outcome is an empty ingestion result.
pub struct BankierCompanyRefresh;

impl crate::jobs::source_refresh::Fetcher for BankierCompanyRefresh {
    fn refresh(
        &self,
        state: &crate::app_state::AppState,
        ctx: &crate::jobs::source_refresh::RefreshContext,
    ) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
        refresh_bankier_company_for_trigger(state, ctx.trigger)
            .map(crate::jobs::source_refresh::RefreshOutcome::Ingestion)
    }
}

/// Plan a bankier-company refresh: enqueue one idempotent `source_company_refresh`
/// job per tracked company instead of looping every company in a single monolith
/// job (ADR 0059). The former monolith (a ~100-company loop with a 1 s sleep each)
/// monopolized the worker for minutes and starved autopilot; the per-company jobs
/// are serialized by the per-source lock (politeness preserved), run alongside other
/// lanes, and resume across restarts. Returns quickly with a summary — the per-company
/// jobs do the actual fetch/ingest and each rides detection on its own completion.
pub fn refresh_bankier_company_for_trigger(
    state: &crate::app_state::AppState,
    trigger: &str,
) -> Result<crate::storage::SourceIngestionResult, String> {
    use crate::jobs::source_refresh::{empty_source_result, SOURCE_COMPANY_REFRESH_KIND};

    let adapter_id = ADAPTER_ID;
    let _ = state.record_source_adapter_attempt(adapter_id, trigger);
    let targets = state
        .list_bankier_company_targets()
        .map_err(|error| error.to_string())?;

    let mut planned = 0usize;
    for target in &targets {
        let job_id = format!(
            "{SOURCE_COMPANY_REFRESH_KIND}:{adapter_id}:{}",
            target.company_id
        );
        let payload =
            serde_json::json!({ "adapterId": adapter_id, "companyId": target.company_id })
                .to_string();
        // `reschedule` re-arms a stable per-company id: pending/terminal rows reset,
        // an in-flight row is left alone — so a re-plan never disturbs a running job
        // and never accumulates duplicate rows.
        match state
            .jobs()
            .reschedule(&job_id, SOURCE_COMPANY_REFRESH_KIND, &payload, 3)
        {
            Ok(_) => planned += 1,
            Err(error) => log::warn!(
                "module=sources stage=plan_failed adapterId={adapter_id} companyId={} error={error}",
                target.company_id
            ),
        }
    }
    log::info!(
        "module=sources stage=planned adapterId={adapter_id} trigger={trigger} companiesPlanned={planned}"
    );
    Ok(empty_source_result(adapter_id))
}

#[cfg(test)]
mod tests;
