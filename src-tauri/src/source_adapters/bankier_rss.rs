use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesCData, BytesRef, BytesText, Event};
use quick_xml::name::QName;
use quick_xml::Reader;
use thiserror::Error;
use time::{format_description::well_known::Rfc2822, format_description::well_known::Rfc3339};
use url::Url;

use super::USER_AGENT;

pub const ADAPTER_ID: &str = "bankier-market-rss";
pub const DISPLAY_NAME: &str = "Bankier Giełda RSS";
pub const SOURCE_URL: &str = "https://www.bankier.pl/rss/gielda.xml";
pub const HUMAN_SOURCE_URL: &str = "https://www.bankier.pl/rss";
pub const ATTRIBUTION: &str = "Bankier.pl";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankierRssChannel {
    pub adapter_id: &'static str,
    pub display_name: &'static str,
    pub source_url: &'static str,
    pub human_source_url: &'static str,
    pub attribution: &'static str,
}

pub const MARKET_CHANNEL: BankierRssChannel = BankierRssChannel {
    adapter_id: ADAPTER_ID,
    display_name: DISPLAY_NAME,
    source_url: SOURCE_URL,
    human_source_url: HUMAN_SOURCE_URL,
    attribution: ATTRIBUTION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankierRssItem {
    pub title: String,
    pub link: String,
    pub summary: String,
    pub published_at: Option<String>,
    pub fetched_at: String,
    pub dedupe_key: String,
}

#[derive(Debug, Error)]
pub enum BankierRssError {
    #[error("Bankier RSS HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Bankier RSS parse failed: {0}")]
    Parse(#[from] BankierRssParseError),
    #[error("fetched timestamp formatting failed: {0}")]
    TimestampFormat(#[from] time::error::Format),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankierRssParseError {
    #[error("invalid RSS XML: {0}")]
    Xml(String),
}

pub trait BankierRssFetcher {
    fn fetch_rss(&self, source_url: &str) -> Result<String, BankierRssError>;
}

pub struct HttpBankierRssFetcher;

impl BankierRssFetcher for HttpBankierRssFetcher {
    fn fetch_rss(&self, source_url: &str) -> Result<String, BankierRssError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        Ok(client.get(source_url).send()?.error_for_status()?.text()?)
    }
}

pub fn fetch_rss_items(
    fetcher: &impl BankierRssFetcher,
) -> Result<Vec<BankierRssItem>, BankierRssError> {
    fetch_channel_rss_items(MARKET_CHANNEL, fetcher)
}

pub fn fetch_channel_rss_items(
    channel: BankierRssChannel,
    fetcher: &impl BankierRssFetcher,
) -> Result<Vec<BankierRssItem>, BankierRssError> {
    let xml = fetcher.fetch_rss(channel.source_url)?;
    let fetched_at = time::OffsetDateTime::now_utc().format(&Rfc3339)?;

    Ok(parse_channel_rss_items(channel, &xml, &fetched_at)?)
}

pub fn parse_rss_items(
    xml: &str,
    fetched_at: &str,
) -> Result<Vec<BankierRssItem>, BankierRssParseError> {
    parse_channel_rss_items(MARKET_CHANNEL, xml, fetched_at)
}

pub fn parse_channel_rss_items(
    channel: BankierRssChannel,
    xml: &str,
    fetched_at: &str,
) -> Result<Vec<BankierRssItem>, BankierRssParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut items = Vec::new();
    let mut current_item: Option<RawRssItem> = None;
    let mut current_field: Option<Field> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match element.name() {
                QName(b"item") => current_item = Some(RawRssItem::default()),
                QName(b"title") => current_field = Some(Field::Title),
                QName(b"link") => current_field = Some(Field::Link),
                QName(b"description") => current_field = Some(Field::Description),
                QName(b"pubDate") => current_field = Some(Field::PubDate),
                QName(b"guid") => current_field = Some(Field::Guid),
                _ => {}
            },
            Ok(Event::End(element)) => match element.name() {
                QName(b"item") => {
                    if let Some(item) = current_item
                        .take()
                        .and_then(|item| item.into_item(channel, fetched_at))
                    {
                        items.push(item);
                    }
                    current_field = None;
                }
                QName(b"title")
                | QName(b"link")
                | QName(b"description")
                | QName(b"pubDate")
                | QName(b"guid") => current_field = None,
                _ => {}
            },
            Ok(Event::Text(text)) => {
                if let (Some(item), Some(field)) = (&mut current_item, current_field) {
                    let value = decode_text_value(text)?;
                    item.push(field, &value);
                }
            }
            Ok(Event::CData(text)) => {
                if let (Some(item), Some(field)) = (&mut current_item, current_field) {
                    let value = decode_cdata_value(text)?;
                    item.push(field, &value);
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let (Some(item), Some(field)) = (&mut current_item, current_field) {
                    let value = decode_reference_value(reference)?;
                    item.push(field, &value);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(BankierRssParseError::Xml(error.to_string())),
            _ => {}
        }
    }

    Ok(items)
}

fn decode_text_value(text: BytesText<'_>) -> Result<String, BankierRssParseError> {
    let decoded = text
        .decode()
        .map_err(|error| BankierRssParseError::Xml(error.to_string()))?;
    unescape_text_value(&decoded)
}

fn decode_cdata_value(text: BytesCData<'_>) -> Result<String, BankierRssParseError> {
    let decoded = text
        .decode()
        .map_err(|error| BankierRssParseError::Xml(error.to_string()))?;
    unescape_text_value(&decoded)
}

fn decode_reference_value(reference: BytesRef<'_>) -> Result<String, BankierRssParseError> {
    let decoded = reference
        .decode()
        .map_err(|error| BankierRssParseError::Xml(error.to_string()))?;

    Ok(resolve_xml_entity(&decoded)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("&{decoded};")))
}

fn unescape_text_value(value: &str) -> Result<String, BankierRssParseError> {
    unescape(value)
        .map(|value| value.into_owned())
        .map_err(|error| BankierRssParseError::Xml(error.to_string()))
}

#[derive(Debug, Default)]
struct RawRssItem {
    title: String,
    link: String,
    description: String,
    pub_date: String,
    guid: String,
}

#[derive(Debug, Clone, Copy)]
enum Field {
    Title,
    Link,
    Description,
    PubDate,
    Guid,
}

impl RawRssItem {
    fn push(&mut self, field: Field, value: &str) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }

        let target = match field {
            Field::Title => &mut self.title,
            Field::Link => &mut self.link,
            Field::Description => &mut self.description,
            Field::PubDate => &mut self.pub_date,
            Field::Guid => &mut self.guid,
        };

        if !matches!(field, Field::Link) && should_insert_text_separator(target, value) {
            target.push(' ');
        }
        target.push_str(value);
    }

    fn into_item(self, channel: BankierRssChannel, fetched_at: &str) -> Option<BankierRssItem> {
        let title = self.title.trim();
        let raw_link = self.link.trim();
        if title.is_empty() || raw_link.is_empty() {
            return None;
        }

        let link = normalize_rss_link(raw_link);
        let published_at = parse_pub_date(self.pub_date.trim());
        let dedupe_seed = first_non_empty([self.guid.trim(), link.as_str(), title]);
        let dedupe_key = format!("{}:{}", channel.adapter_id, slug_part(dedupe_seed));

        Some(BankierRssItem {
            title: title.to_owned(),
            link,
            summary: strip_html(self.description.trim()),
            published_at,
            fetched_at: fetched_at.to_owned(),
            dedupe_key,
        })
    }
}

fn normalize_rss_link(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return value.to_owned();
    };

    url.set_fragment(None);

    let retained_query_pairs = url
        .query_pairs()
        .filter(|(key, _)| !key.to_ascii_lowercase().starts_with("utm_"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();

    url.set_query(None);

    if !retained_query_pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(
            retained_query_pairs
                .iter()
                .map(|(key, value)| (&**key, &**value)),
        );
    }

    url.to_string()
}

fn should_insert_text_separator(current: &str, next: &str) -> bool {
    if current.is_empty()
        || current.ends_with(['\'', '(', '['])
        || matches!(
            next,
            "\"" | "'" | "." | "," | ":" | ";" | "!" | "?" | ")" | "]"
        )
    {
        return false;
    }

    !(current.ends_with('"') && current.matches('"').count() % 2 == 1)
}

fn parse_pub_date(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    time::OffsetDateTime::parse(value, &Rfc2822)
        .ok()
        .and_then(|timestamp| timestamp.format(&Rfc3339).ok())
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = &'a str>) -> &'a str {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or("missing")
}

fn strip_html(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;

    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }

    let stripped = output.split_whitespace().collect::<Vec<_>>().join(" ");
    unescape(&stripped)
        .map(|value| value.into_owned())
        .unwrap_or(stripped)
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

    const FIXTURE: &str = include_str!("../../fixtures/bankier_gielda_rss.xml");

    struct FixtureFetcher;

    impl BankierRssFetcher for FixtureFetcher {
        fn fetch_rss(&self, source_url: &str) -> Result<String, BankierRssError> {
            assert_eq!(source_url, SOURCE_URL);
            Ok(FIXTURE.to_owned())
        }
    }

    #[test]
    fn parses_bankier_rss_items() {
        let items = parse_rss_items(FIXTURE, "2026-05-31T10:00:00Z").expect("fixture should parse");

        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0].title,
            "\"Maluchy\" z nowym rekordem. CD Projekt rośnie po komentarzu zarządu"
        );
        assert_eq!(
            items[0].link,
            "https://www.bankier.pl/wiadomosc/cd-projekt-komentarz-900001.html"
        );
        assert_eq!(
            items[0].summary,
            "\"Kurs\" CD Projektu zyskuje po publikacji komentarza zarządu."
        );
        assert_eq!(
            items[0].published_at,
            Some("2026-05-31T09:15:00+02:00".to_owned())
        );
        assert_eq!(items[0].fetched_at, "2026-05-31T10:00:00Z");
        assert_eq!(items[0].dedupe_key, "bankier-market-rss:bankier-900001");
    }

    #[test]
    fn fetches_and_parses_with_injected_fetcher() {
        let items = fetch_rss_items(&FixtureFetcher).expect("fixture fetch should parse");

        assert_eq!(items.len(), 2);
        assert!(items[0].fetched_at.ends_with('Z'));
    }

    #[test]
    fn parses_channel_specific_dedupe_keys() {
        let test_channel = BankierRssChannel {
            adapter_id: "bankier-test-rss",
            display_name: "Bankier Test RSS",
            source_url: "https://www.bankier.pl/rss/test.xml",
            human_source_url: HUMAN_SOURCE_URL,
            attribution: ATTRIBUTION,
        };

        let items = parse_channel_rss_items(test_channel, FIXTURE, "2026-05-31T10:00:00Z")
            .expect("fixture should parse");

        assert_eq!(items[0].dedupe_key, "bankier-test-rss:bankier-900001");
    }

    #[test]
    fn normalizes_tracking_parameters_for_link_dedupe() {
        let xml = r#"
        <rss version="2.0">
          <channel>
            <item>
              <title>CD Projekt publikuje komentarz</title>
              <link>https://www.bankier.pl/wiadomosc/cd-projekt-900003.html?utm_source=RSS&amp;utm_medium=RSS&amp;id=42#comments</link>
              <description>Opis</description>
              <pubDate>Sun, 31 May 2026 09:15:00 +0200</pubDate>
            </item>
          </channel>
        </rss>
        "#;

        let items = parse_rss_items(xml, "2026-05-31T10:00:00Z").expect("RSS should parse");

        assert_eq!(
            items[0].link,
            "https://www.bankier.pl/wiadomosc/cd-projekt-900003.html?id=42"
        );
        assert_eq!(
            items[0].dedupe_key,
            "bankier-market-rss:https-www-bankier-pl-wiadomosc-cd-projekt-900003-html-id-42"
        );
    }
}
