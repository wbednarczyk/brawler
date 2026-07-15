use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use thiserror::Error;
use time::format_description::well_known::Rfc3339;

use super::parsing::{
    decode_xml_cdata_value, decode_xml_reference_value, decode_xml_text_value,
    parse_rfc2822_timestamp, slug_part,
};
use super::USER_AGENT;

pub const ADAPTER_ID: &str = "gpw-market-events-rss";
pub const DISPLAY_NAME: &str = "GPW Market Events RSS";
pub const SOURCE_URL: &str = "https://www.gpw.pl/rss-calendar-of-market-events";
pub const ATTRIBUTION: &str = "GPW";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpwMarketEventItem {
    pub market: String,
    pub event_label: String,
    pub instrument_type: String,
    pub ticker: String,
    pub event_type: String,
    pub title: String,
    pub link: String,
    pub event_date: String,
    pub fetched_at: String,
    pub source_event_key: String,
}

#[derive(Debug, Error)]
pub enum GpwMarketEventsError {
    #[error("GPW market events RSS HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("GPW market events RSS parse failed: {0}")]
    Parse(#[from] GpwMarketEventsParseError),
    #[error("fetched timestamp formatting failed: {0}")]
    TimestampFormat(#[from] time::error::Format),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GpwMarketEventsParseError {
    #[error("invalid RSS XML: {0}")]
    Xml(String),
}

pub trait GpwMarketEventsFetcher {
    fn fetch_rss(&self) -> Result<String, GpwMarketEventsError>;
}

pub struct HttpGpwMarketEventsFetcher;

impl GpwMarketEventsFetcher for HttpGpwMarketEventsFetcher {
    fn fetch_rss(&self) -> Result<String, GpwMarketEventsError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        Ok(client.get(SOURCE_URL).send()?.error_for_status()?.text()?)
    }
}

pub fn fetch_market_events(
    fetcher: &impl GpwMarketEventsFetcher,
) -> Result<Vec<GpwMarketEventItem>, GpwMarketEventsError> {
    let xml = fetcher.fetch_rss()?;
    let fetched_at = time::OffsetDateTime::now_utc().format(&Rfc3339)?;

    Ok(parse_market_events(&xml, &fetched_at)?)
}

/// Refresh-level [`Fetcher`](crate::jobs::source_refresh::Fetcher) impl for the GPW
/// market-events RSS adapter (ADR 0069, plan v0.55 T1). Behavior-preserving lift of
/// the former `refresh_gpw_market_events_for_trigger` fn-pointer arm — the internal
/// [`GpwMarketEventsFetcher`] HTTP trait is unchanged.
pub struct GpwMarketEventsRefresh;

impl crate::jobs::source_refresh::Fetcher for GpwMarketEventsRefresh {
    fn refresh(
        &self,
        state: &crate::app_state::AppState,
        ctx: &crate::jobs::source_refresh::RefreshContext,
    ) -> Result<crate::jobs::source_refresh::RefreshOutcome, String> {
        let _ = state.record_source_adapter_attempt(ADAPTER_ID, ctx.trigger);
        let event_items = match fetch_market_events(&HttpGpwMarketEventsFetcher) {
            Ok(items) => items,
            Err(error) => {
                let message = error.to_string();
                let _ = state.record_source_adapter_error(ADAPTER_ID, &message);
                return Err(message);
            }
        };

        state
            .ingest_gpw_market_event_items(&event_items)
            .map(crate::jobs::source_refresh::RefreshOutcome::Ingestion)
            .map_err(|error| error.to_string())
    }
}

pub fn parse_market_events(
    xml: &str,
    fetched_at: &str,
) -> Result<Vec<GpwMarketEventItem>, GpwMarketEventsParseError> {
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
                _ => {}
            },
            Ok(Event::End(element)) => match element.name() {
                QName(b"item") => {
                    if let Some(item) = current_item
                        .take()
                        .and_then(|item| item.into_item(fetched_at))
                    {
                        items.push(item);
                    }
                    current_field = None;
                }
                QName(b"title") | QName(b"link") | QName(b"description") | QName(b"pubDate") => {
                    current_field = None;
                }
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
            Err(error) => return Err(GpwMarketEventsParseError::Xml(error.to_string())),
            _ => {}
        }
    }

    Ok(items)
}

fn decode_text_value(
    text: quick_xml::events::BytesText<'_>,
) -> Result<String, GpwMarketEventsParseError> {
    decode_xml_text_value(text).map_err(GpwMarketEventsParseError::Xml)
}

fn decode_cdata_value(
    text: quick_xml::events::BytesCData<'_>,
) -> Result<String, GpwMarketEventsParseError> {
    decode_xml_cdata_value(text).map_err(GpwMarketEventsParseError::Xml)
}

fn decode_reference_value(
    reference: quick_xml::events::BytesRef<'_>,
) -> Result<String, GpwMarketEventsParseError> {
    decode_xml_reference_value(reference).map_err(GpwMarketEventsParseError::Xml)
}

#[derive(Debug, Default)]
struct RawRssItem {
    title: String,
    link: String,
    description: String,
    pub_date: String,
}

#[derive(Debug, Clone, Copy)]
enum Field {
    Title,
    Link,
    Description,
    PubDate,
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
        };

        if !matches!(field, Field::Link) && should_insert_text_separator(target, value) {
            target.push(' ');
        }
        target.push_str(value);
    }

    fn into_item(self, fetched_at: &str) -> Option<GpwMarketEventItem> {
        let title = self.title.trim();
        let link = self.link.trim();
        if title.is_empty() || link.is_empty() {
            return None;
        }

        let title_parts = title.split(" - ").collect::<Vec<_>>();
        if title_parts.len() < 4 {
            return None;
        }

        let market = title_parts[0].trim();
        let ticker = title_parts
            .last()
            .map(|value| value.trim().to_uppercase())
            .filter(|value| !value.is_empty())?;
        let instrument_type = title_parts[title_parts.len() - 2].trim();
        let event_label = title_parts[1..title_parts.len() - 2].join(" - ");
        let event_date = parse_event_date(self.pub_date.trim())?;
        let event_type = classify_event_type(&event_label);
        let source_event_key = format!(
            "{}:{}:{}:{}:{}",
            ADAPTER_ID,
            event_date,
            slug_part(&event_label),
            slug_part(instrument_type),
            slug_part(&ticker)
        );

        Some(GpwMarketEventItem {
            market: market.to_owned(),
            event_label,
            instrument_type: instrument_type.to_owned(),
            ticker,
            event_type,
            title: title.to_owned(),
            link: link.to_owned(),
            event_date,
            fetched_at: fetched_at.to_owned(),
            source_event_key,
        })
    }
}

fn parse_event_date(value: &str) -> Option<String> {
    parse_rfc2822_timestamp(value).map(|timestamp| timestamp.chars().take(10).collect())
}

fn classify_event_type(event_label: &str) -> String {
    let normalized = event_label.to_ascii_lowercase();

    if normalized.contains("corporate actions") || normalized.contains("change of volume") {
        "corporate_action"
    } else if normalized.contains("market making") || normalized.contains("issuer") {
        "market_making"
    } else if normalized.contains("exclusion") || normalized.contains("admission") {
        "listing_change"
    } else {
        "other_market_event"
    }
    .to_owned()
}

fn should_insert_text_separator(existing: &str, incoming: &str) -> bool {
    !existing.is_empty()
        && !existing.ends_with(char::is_whitespace)
        && !incoming.starts_with(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <item>
      <title>Main Market - Corporate actions - Equity - DIAG</title>
      <link>https://www.gpw.pl/market-events-calendar?market_section=RGL&amp;market_category=64&amp;date=2026-06-01</link>
      <description>Main Market - Corporate actions - Equity - DIAG</description>
      <pubDate>Mon, 01 Jun 2026 00:00:00 +0200</pubDate>
    </item>
    <item>
      <title>Main Market - Exclusion - Structured certificate - RCFS5W20</title>
      <link>https://www.gpw.pl/market-events-calendar?market_section=RGL&amp;market_category=512&amp;date=2026-06-01</link>
      <description>Main Market - Exclusion - Structured certificate - RCFS5W20</description>
      <pubDate>Mon, 01 Jun 2026 00:00:00 +0200</pubDate>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn parses_gpw_market_events_rss_items() {
        let items =
            parse_market_events(SAMPLE, "2026-06-01T08:00:00Z").expect("sample should parse");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].market, "Main Market");
        assert_eq!(items[0].event_label, "Corporate actions");
        assert_eq!(items[0].instrument_type, "Equity");
        assert_eq!(items[0].ticker, "DIAG");
        assert_eq!(items[0].event_type, "corporate_action");
        assert_eq!(items[0].event_date, "2026-06-01");
        assert_eq!(
            items[0].source_event_key,
            "gpw-market-events-rss:2026-06-01:corporate-actions:equity:diag"
        );
        assert_eq!(items[1].event_type, "listing_change");
    }
}
