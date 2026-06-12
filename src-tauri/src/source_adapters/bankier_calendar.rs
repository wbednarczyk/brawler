use scraper::{Html, Selector};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, Date, Month, OffsetDateTime, Time, UtcOffset};
use url::Url;

use super::parsing::{normalize_polish, polish_slug_part as slug_part};
use super::USER_AGENT;

pub const ADAPTER_ID: &str = "bankier-kalendarium-html";
pub const DISPLAY_NAME: &str = "Bankier Kalendarium";
pub const SOURCE_URL: &str = "https://www.bankier.pl/gielda/kalendarium";
pub const ATTRIBUTION: &str = "Bankier.pl";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankierCalendarEventItem {
    pub ticker: String,
    pub event_type: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub link: String,
    pub event_date: String,
    pub fetched_at: String,
    pub source_event_key: String,
}

#[derive(Debug, Error)]
pub enum BankierCalendarError {
    #[error("Bankier Kalendarium HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Bankier Kalendarium parse failed: {0}")]
    Parse(#[from] BankierCalendarParseError),
    #[error("fetched timestamp formatting failed: {0}")]
    TimestampFormat(#[from] time::error::Format),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BankierCalendarParseError {
    #[error("missing selector: {0}")]
    Selector(String),
}

pub trait BankierCalendarFetcher {
    fn fetch_html(&self, url: &str) -> Result<String, BankierCalendarError>;
}

pub struct HttpBankierCalendarFetcher;

impl BankierCalendarFetcher for HttpBankierCalendarFetcher {
    fn fetch_html(&self, url: &str) -> Result<String, BankierCalendarError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        Ok(client.get(url).send()?.error_for_status()?.text()?)
    }
}

pub fn fetch_calendar_events(
    fetcher: &impl BankierCalendarFetcher,
) -> Result<Vec<BankierCalendarEventItem>, BankierCalendarError> {
    fetch_calendar_events_for_date(fetcher, None)
}

pub fn fetch_calendar_events_for_date(
    fetcher: &impl BankierCalendarFetcher,
    date: Option<&str>,
) -> Result<Vec<BankierCalendarEventItem>, BankierCalendarError> {
    let url = calendar_url(date);
    let html = fetcher.fetch_html(&url)?;
    let fetched_at = time::OffsetDateTime::now_utc().format(&Rfc3339)?;

    Ok(parse_calendar_events(&html, &fetched_at)?)
}

pub fn calendar_url(date: Option<&str>) -> String {
    match date
        .map(str::trim)
        .filter(|date| is_iso_date(date))
        .and_then(bankier_navigation_start)
    {
        Some(timestamp) => {
            format!("{SOURCE_URL}?navigation_type=week&navigation_start={timestamp}")
        }
        None => SOURCE_URL.to_owned(),
    }
}

pub fn parse_calendar_events(
    html: &str,
    fetched_at: &str,
) -> Result<Vec<BankierCalendarEventItem>, BankierCalendarParseError> {
    let document = Html::parse_document(html);
    let day_selector = selector(".m-quotes-calendar-list__item")?;
    let date_selector = selector(".m-quotes-calendar-list__date")?;
    let event_selector = selector(".m-quotes-calendar-list__event")?;
    let symbol_selector = selector(".m-quotes-calendar-list__symbol")?;
    let info_selector = selector(".m-quotes-calendar-list__info")?;
    let category_selector = selector(".a-quotes-badge .value")?;
    let base_url = Url::parse("https://www.bankier.pl").expect("Bankier base URL should be valid");
    let mut items = Vec::new();

    for day in document.select(&day_selector) {
        let date_text = day
            .select(&date_selector)
            .next()
            .map(element_text)
            .unwrap_or_default();
        let Some(event_date) = parse_polish_calendar_date(&date_text) else {
            continue;
        };

        for event in day.select(&event_selector) {
            let Some(symbol) = event.select(&symbol_selector).next() else {
                continue;
            };
            let ticker = element_text(symbol).trim().to_uppercase();
            if ticker.is_empty() {
                continue;
            }

            let description = event
                .select(&info_selector)
                .next()
                .map(element_text)
                .unwrap_or_default();
            if description.is_empty() {
                continue;
            }

            let category = event
                .select(&category_selector)
                .next()
                .map(element_text)
                .unwrap_or_else(|| "Inne".to_owned());
            let link = symbol
                .value()
                .attr("href")
                .and_then(|href| base_url.join(href).ok())
                .map(|url| url.to_string())
                .unwrap_or_else(|| SOURCE_URL.to_owned());
            let event_type = classify_event_type(&category);
            let title = format!("{ticker}: {description}");
            let source_event_key = format!(
                "{ADAPTER_ID}:{}:{}:{}",
                slug_part(&ticker),
                slug_part(&category),
                slug_part(&description)
            );

            items.push(BankierCalendarEventItem {
                ticker,
                event_type,
                title,
                description,
                category,
                link,
                event_date: event_date.clone(),
                fetched_at: fetched_at.to_owned(),
                source_event_key,
            });
        }
    }

    Ok(items)
}

fn selector(value: &str) -> Result<Selector, BankierCalendarParseError> {
    Selector::parse(value).map_err(|_| BankierCalendarParseError::Selector(value.to_owned()))
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();

    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn bankier_navigation_start(value: &str) -> Option<i64> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }

    let month = Month::try_from(month).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let offset_hours = if warsaw_dst_active_at_local_midnight(year, month, day) {
        2
    } else {
        1
    };
    let offset = UtcOffset::from_hms(offset_hours, 0, 0).ok()?;

    Some(OffsetDateTime::new_in_offset(date, Time::MIDNIGHT, offset).unix_timestamp())
}

fn warsaw_dst_active_at_local_midnight(year: i32, month: Month, day: u8) -> bool {
    match month {
        Month::April
        | Month::May
        | Month::June
        | Month::July
        | Month::August
        | Month::September => true,
        Month::November | Month::December | Month::January | Month::February => false,
        Month::March => day > last_sunday_of_month(year, Month::March),
        Month::October => day <= last_sunday_of_month(year, Month::October),
    }
}

fn last_sunday_of_month(year: i32, month: Month) -> u8 {
    (22..=31)
        .rev()
        .find(|day| {
            Date::from_calendar_date(year, month, *day)
                .map(|date| date.weekday() == time::Weekday::Sunday)
                .unwrap_or(false)
        })
        .expect("every month should have a Sunday in the last ten days")
}

fn element_text(element: scraper::ElementRef<'_>) -> String {
    element
        .text()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_polish_calendar_date(value: &str) -> Option<String> {
    let mut parts = value.split_whitespace();
    let day = parts
        .next()?
        .trim_end_matches('.')
        .parse::<u8>()
        .ok()
        .filter(|day| (1..=31).contains(day))?;
    let month = polish_month_number(parts.next()?)?;
    let year = parts
        .next()?
        .parse::<u16>()
        .ok()
        .filter(|year| *year >= 2000)?;

    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn polish_month_number(value: &str) -> Option<u8> {
    match normalize_polish(value).as_str() {
        "stycznia" => Some(1),
        "lutego" => Some(2),
        "marca" => Some(3),
        "kwietnia" => Some(4),
        "maja" => Some(5),
        "czerwca" => Some(6),
        "lipca" => Some(7),
        "sierpnia" => Some(8),
        "wrzesnia" => Some(9),
        "pazdziernika" => Some(10),
        "listopada" => Some(11),
        "grudnia" => Some(12),
        _ => None,
    }
}

fn classify_event_type(category: &str) -> String {
    match normalize_polish(category).as_str() {
        "wyniki spolek" => "periodic_report",
        "dywidendy" => "dividend",
        "wza" => "shareholder_meeting",
        "wezwania" => "corporate_action",
        "rynek pierwotny" => "listing_change",
        _ => "other_market_event",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bankier_calendar_events() {
        let html = r#"
            <section class="m-quotes-calendar-list__item" data-timestamp="1780264800">
                <h3 class="m-quotes-calendar-list__date">1 Czerwca 2026 ● Poniedziałek</h3>
                <div class="m-quotes-calendar-list__event">
                    <a class="m-quotes-calendar-list__symbol" href="/gielda/notowania/akcje/DIAG/kalendarium">DIAG</a>
                    <div class="m-quotes-calendar-list__info">Dzień ustalenia prawa do dywidendy 4,40 zł na akcję.</div>
                    <span class="a-quotes-badge"><span class="value">Dywidendy</span></span>
                </div>
                <div class="m-quotes-calendar-list__event">
                    <a class="m-quotes-calendar-list__symbol" href="/gielda/notowania/akcje/HILANDER/kalendarium">HILANDER</a>
                    <div class="m-quotes-calendar-list__info">Zwyczajne walne zgromadzenie akcjonariuszy.</div>
                    <span class="a-quotes-badge"><span class="value">WZA</span></span>
                </div>
            </section>
        "#;

        let events = parse_calendar_events(html, "2026-06-01T10:00:00Z")
            .expect("calendar events should parse");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].ticker, "DIAG");
        assert_eq!(events[0].event_type, "dividend");
        assert_eq!(events[0].event_date, "2026-06-01");
        assert_eq!(
            events[0].link,
            "https://www.bankier.pl/gielda/notowania/akcje/DIAG/kalendarium"
        );
        assert_eq!(
            events[0].source_event_key,
            "bankier-kalendarium-html:diag:dywidendy:dzien-ustalenia-prawa-do-dywidendy-4-40-zl-na-akcje"
        );
        assert_eq!(events[1].ticker, "HILANDER");
        assert_eq!(events[1].event_type, "shareholder_meeting");
    }

    #[test]
    fn builds_default_and_dated_calendar_urls() {
        assert_eq!(
            calendar_url(None),
            "https://www.bankier.pl/gielda/kalendarium"
        );
        assert_eq!(
            calendar_url(Some("2026-06-08")),
            "https://www.bankier.pl/gielda/kalendarium?navigation_type=week&navigation_start=1780869600"
        );
        assert_eq!(
            calendar_url(Some("../bad")),
            "https://www.bankier.pl/gielda/kalendarium"
        );
    }
}
