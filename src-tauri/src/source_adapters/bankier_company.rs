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
        let detail = parse_company_report_detail(&html, &item.title);
        item.detail_fetch_attempted = true;
        item.body_text = detail.body_text;
        item.attachments = detail.attachments;
    }

    Ok((identifiers, items))
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

        let link = normalize_article_link(&raw_link);
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

fn parse_company_report_detail(html: &str, title: &str) -> BankierCompanyReportDetail {
    let document = Html::parse_document(html);
    let body_text = extract_structured_article_body(&document).unwrap_or_else(|| {
        let root_text = normalized_lines(document.root_element().text());
        extract_report_body(&root_text, title)
    });
    let attachments = extract_report_attachments(&document);

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

fn extract_report_attachments(document: &Html) -> Vec<BankierCompanyAttachment> {
    let anchor_selector = Selector::parse("a").expect("valid selector");

    document
        .select(&anchor_selector)
        .filter_map(|anchor| {
            let href = anchor.value().attr("href")?.trim();
            let label = normalized_lines(anchor.text()).join(" ");
            if !is_report_attachment_url(href) || is_source_page_chrome_link(&label) {
                return None;
            }

            Some(BankierCompanyAttachment {
                label: if label.is_empty() {
                    href.rsplit('/').next().unwrap_or(href).to_owned()
                } else {
                    label
                },
                url: normalize_article_link(href),
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

fn is_report_attachment_url(value: &str) -> bool {
    let lower = value.to_lowercase();
    lower.contains("bonnier.pl")
        || lower.ends_with(".pdf")
        || lower.contains(".pdf?")
        || lower.ends_with(".xhtml")
        || lower.contains(".xhtml?")
        || lower.ends_with(".xades")
        || lower.contains(".xades?")
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

fn normalize_article_link(value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_owned()
    } else {
        format!("https://www.bankier.pl{value}")
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
    use std::cell::RefCell;

    const HTML: &str = include_str!("../../fixtures/bankier_company_cdr.html");
    const JSON: &str = include_str!("../../fixtures/bankier_company_cdr_listing.json");

    fn target() -> BankierCompanyTarget {
        BankierCompanyTarget {
            company_id: "company_gpw_cdr".to_owned(),
            ticker: "CDR".to_owned(),
            qualified_ticker: "GPW:CDR".to_owned(),
            bankier_slug: None,
            bankier_tag_id: None,
        }
    }

    struct DetailFilterFetcher {
        fetched_urls: RefCell<Vec<String>>,
    }

    impl BankierCompanyFetcher for DetailFilterFetcher {
        fn fetch_text(&self, url: &str) -> Result<String, BankierCompanyError> {
            self.fetched_urls.borrow_mut().push(url.to_owned());

            if url.starts_with(API_SOURCE_URL) {
                Ok(JSON.to_owned())
            } else if url.contains("9141553") {
                Ok(r#"
                    <html>
                      <head>
                        <script type="application/ld+json">
                          {
                            "@type": "NewsArticle",
                            "articleBody": "First report body"
                          }
                        </script>
                      </head>
                      <body></body>
                    </html>
                "#
                .to_owned())
            } else {
                Ok(r#"
                    <html>
                      <head>
                        <script type="application/ld+json">
                          {
                            "@type": "NewsArticle",
                            "articleBody": "Second report body"
                          }
                        </script>
                      </head>
                      <body></body>
                    </html>
                "#
                .to_owned())
            }
        }
    }

    #[test]
    fn parses_company_page_identifiers() {
        let identifiers = parse_company_identifiers(HTML).expect("HTML should parse");

        assert_eq!(identifiers.slug, "CDPROJEKT");
        assert_eq!(identifiers.tag_id, "722");
    }

    #[test]
    fn builds_listing_api_url() {
        let url = listing_api_url("722", 1, 25).expect("URL should build");

        assert!(url.starts_with("https://api.bankier.pl/articles/listing/1/25?"));
        assert!(url.contains("tags_ids=722"));
        assert!(url.contains("pub_id"));
    }

    #[test]
    fn parses_company_listing_json() {
        let items =
            parse_company_listing_json(&target(), JSON, "2026-05-31T10:00:00Z").expect("JSON");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].company_id, "company_gpw_cdr");
        assert_eq!(items[0].qualified_ticker, "GPW:CDR");
        assert_eq!(items[0].pub_id, 3);
        assert_eq!(items[0].article_id, "9141553");
        assert_eq!(items[0].title, "Wyniki finansowe QSr 1/2026");
        assert_eq!(
            items[0].link,
            "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
        );
        assert_eq!(
            items[0].published_at,
            Some("2026-05-28T17:33:09".to_owned())
        );
        assert_eq!(
            items[0].dedupe_key,
            "bankier-company-komunikaty:article:9141553"
        );
        assert_eq!(items[0].summary, "Komunikat ESPI/EBI");
        assert_eq!(items[0].body_text, None);
        assert!(!items[0].detail_fetch_attempted);
    }

    #[test]
    fn skips_detail_fetches_when_filter_rejects_item() {
        let fetcher = DetailFilterFetcher {
            fetched_urls: RefCell::new(Vec::new()),
        };
        let target = BankierCompanyTarget {
            bankier_slug: Some("CDPROJEKT".to_owned()),
            bankier_tag_id: Some("722".to_owned()),
            ..target()
        };

        let (_, items) = fetch_company_items_with_detail_filter_at(
            &fetcher,
            &target,
            "2026-05-31T10:00:00Z",
            |item| item.article_id != "9141553",
        )
        .expect("items should fetch");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].article_id, "9141553");
        assert_eq!(items[0].body_text, None);
        assert!(!items[0].detail_fetch_attempted);
        assert_eq!(items[1].body_text, Some("Second report body".to_owned()));
        assert!(items[1].detail_fetch_attempted);
        assert_eq!(
            fetcher
                .fetched_urls
                .borrow()
                .iter()
                .filter(|url| url.contains("/wiadomosc/"))
                .count(),
            1
        );
    }

    #[test]
    fn parses_company_report_detail_body_and_attachments() {
        let html = r#"
            <html>
              <head>
                <script type="application/ld+json">
                  {
                    "@context": "https://schema.org",
                    "@type": "NewsArticle",
                    "headline": "CD PROJEKT SA: Wyniki finansowe QSr 1/2026",
                    "articleBody": "Spis treści: 1. STRONA TYTUŁOWA STRONA TYTUŁOWA>>> KOMISJA NADZORU FINANSOWEGO Skonsolidowany raport kwartalny QSr"
                  }
                </script>
              </head>
              <body>
                <nav>Giełda Wiadomości</nav>
                <article>
                  <h1>CD PROJEKT SA: Wyniki finansowe QSr 1/2026</h1>
                  <span>2026-05-28 17:33</span>
                  <span>publikacja</span>
                  <p>Spis treści:</p>
                  <ol><li>STRONA TYTUŁOWA</li></ol>
                  <p>Spis załączników:</p>
                  <a href="https://bonnier.pl/report.xhtml">Raport XHTML</a>
                  <a href="https://www.bankier.pl/regulamin.pdf">Regulamin</a>
                  <a href="https://www.bankier.pl/prywatnosc.pdf">Polityka prywatności</a>
                  <a href="https://www.bankier.pl/cookies.pdf">Polityka Cookies</a>
                  <h4>STRONA TYTUŁOWA&gt;&gt;&gt;</h4>
                  <p>KOMISJA NADZORU FINANSOWEGO</p>
                  <p>Skonsolidowany raport kwartalny QSr</p>
                  <p>Źródło:Komunikaty spółek (ESPI)</p>
                  <p>Podziel się</p>
                </article>
              </body>
            </html>
        "#;

        let detail = parse_company_report_detail(html, "Wyniki finansowe QSr 1/2026");

        assert_eq!(
            detail.body_text,
            Some(
                "Spis treści: 1. STRONA TYTUŁOWA STRONA TYTUŁOWA>>> KOMISJA NADZORU FINANSOWEGO Skonsolidowany raport kwartalny QSr"
                    .to_owned()
            )
        );
        assert_eq!(
            detail.attachments,
            vec![BankierCompanyAttachment {
                label: "Raport XHTML".to_owned(),
                url: "https://bonnier.pl/report.xhtml".to_owned(),
            }]
        );
    }

    #[test]
    fn filters_company_listing_items_older_than_recent_window() {
        let json = r#"{
          "articles": [
            {
              "title": "CD PROJEKT SA: Recent report",
              "url": "/wiadomosc/recent-1.html",
              "time": "2026-05-30 10:00:00",
              "pub_id": 3,
              "article_id": 1,
              "messages_filters": []
            },
            {
              "title": "CD PROJEKT SA: Old report",
              "url": "/wiadomosc/old-2.html",
              "time": "2026-05-20 10:00:00",
              "pub_id": 3,
              "article_id": 2,
              "messages_filters": []
            }
          ]
        }"#;

        let items =
            parse_company_listing_json(&target(), json, "2026-05-31T10:00:00Z").expect("JSON");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Recent report");
    }
}
