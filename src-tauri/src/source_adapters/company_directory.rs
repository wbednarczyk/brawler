use scraper::{Html, Selector};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyDirectoryEntry {
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub isin: String,
    pub source_url: String,
    /// Sector/industry classification read from the registry's market-segment
    /// line (`Główny Rynek | <indices> | <sector>`), if the page exposes one
    /// for this row. `None` when the row has no such line.
    pub sector: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompanyDirectoryParseError {
    #[error("invalid selector: {0}")]
    InvalidSelector(String),
    #[error("no company directory rows found")]
    NoRows,
}

pub fn parse_company_directory_html(
    html: &str,
    exchange: &str,
    base_url: &str,
) -> Result<Vec<CompanyDirectoryEntry>, CompanyDirectoryParseError> {
    let document = Html::parse_document(html);
    let row_selector = selector("#search-result tr")?;
    let anchor_selector = selector("a[href*='spolka?isin=']")?;
    let name_selector = selector("strong.name")?;
    let segment_selector = selector("small.grey")?;
    let exchange = exchange.trim().to_uppercase();

    let entries = document
        .select(&row_selector)
        .filter_map(|row| {
            let anchor = row.select(&anchor_selector).next()?;
            let href = anchor.value().attr("href")?;
            let isin = extract_isin_from_company_href(href)?;
            let source_url = absolute_url(base_url, href);
            let name_text = row
                .select(&name_selector)
                .next()
                .map(|name| normalized_text(name.text()))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| normalized_text(anchor.text()));
            let (display_name, ticker) = split_company_name_and_ticker(&name_text)?;
            let sector = row
                .select(&segment_selector)
                .next()
                .map(|segment| normalized_text(segment.text()))
                .and_then(|value| extract_sector_from_segment_line(&value));

            Some(CompanyDirectoryEntry {
                qualified_ticker: format!("{exchange}:{ticker}"),
                exchange: exchange.clone(),
                ticker,
                display_name,
                isin,
                source_url,
                sector,
            })
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        Err(CompanyDirectoryParseError::NoRows)
    } else {
        Ok(entries)
    }
}

fn selector(value: &str) -> Result<Selector, CompanyDirectoryParseError> {
    Selector::parse(value)
        .map_err(|_| CompanyDirectoryParseError::InvalidSelector(value.to_owned()))
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

/// Extracts the sector from the market-segment line, e.g.
/// `"Główny Rynek | WIG140, WIG-gry | Gry"` -> `Some("Gry")`. The sector is
/// the last `|`-separated segment; a line with no `|` (no sector reported)
/// yields `None`.
fn extract_sector_from_segment_line(value: &str) -> Option<String> {
    if !value.contains('|') {
        return None;
    }
    value
        .rsplit('|')
        .next()
        .map(str::trim)
        .filter(|sector| !sector.is_empty())
        .map(str::to_owned)
}

fn normalized_text<'a>(text: impl Iterator<Item = &'a str>) -> String {
    text.collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn absolute_url(base_url: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_owned()
    } else if href.starts_with('/') {
        format!("{}{}", base_url.trim_end_matches('/'), href)
    } else {
        format!("{}/{}", base_url.trim_end_matches('/'), href)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_directory_rows_from_standard_search_result_html() {
        let html = r#"
            <table id="search-result">
                <tr>
                    <td>
                        <a href="/spolka?isin=PLESLTN00010">
                            4MOBILITY SPÓŁKA AKCYJNA (4MB)
                        </a>
                    </td>
                </tr>
            </table>
        "#;

        let entries = parse_company_directory_html(html, "NC", "https://newconnect.pl")
            .expect("should parse");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].qualified_ticker, "NC:4MB");
        assert_eq!(entries[0].display_name, "4MOBILITY SPÓŁKA AKCYJNA");
        assert_eq!(entries[0].isin, "PLESLTN00010");
        assert_eq!(
            entries[0].source_url,
            "https://newconnect.pl/spolka?isin=PLESLTN00010"
        );
        assert_eq!(entries[0].sector, None);
    }

    #[test]
    fn extracts_sector_from_trailing_segment_of_market_segment_line() {
        let html = r#"
            <table id="search-result">
                <tr>
                    <td>
                        <a href="/spolka?isin=PL11BTS00015">
                            <strong class="name">11 BIT STUDIOS SPÓŁKA AKCYJNA (11B)</strong>
                        </a>
                        <small class="grey">Główny Rynek | WIG140, WIG-gry | Gry</small>
                    </td>
                </tr>
                <tr>
                    <td>
                        <a href="/spolka?isin=PLPEKAO00016">
                            <strong class="name">ALIOR BANK SPÓŁKA AKCYJNA (ALR)</strong>
                        </a>
                        <small class="grey">Główny Rynek | WIG30, WIG20 | banki komercyjne</small>
                    </td>
                </tr>
            </table>
        "#;

        let entries =
            parse_company_directory_html(html, "GPW", "https://www.gpw.pl").expect("should parse");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sector.as_deref(), Some("Gry"));
        assert_eq!(entries[1].sector.as_deref(), Some("banki komercyjne"));
    }
}
