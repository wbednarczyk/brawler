//! HTML financial-data aggregator adapter — tier 3 (ADR 0061).
//!
//! Aggregators (BiznesRadar, Bankier „wyniki finansowe", StockWatch) publish a
//! company's headline figures as clean HTML tables: rows are metrics, columns
//! are reporting periods. This adapter parses that grid, maps the Polish row
//! labels to our `metric_key` catalog (reusing the tier-2 dictionary and number
//! rules), and reads the requested period's column.
//!
//! Its role is **witness, not source of truth**: where an aggregator covers a
//! company it is cross-checked against the primary filing on every period
//! (agreement ⇒ ~100% confidence), and it is the rescue path when the PDF
//! parser drifts. The primary filing always wins on disagreement; the
//! aggregator only ever confirms or flags. Pure over `&str` HTML — no network.

use scraper::{Html, Selector};

use super::pdf::{
    detect_unit_scale, is_per_share, match_dictionary_label, normalize_label, parse_amount,
};
use super::{ExtractedFact, FactPeriod, SourceTier};

/// The period column to read from an aggregator table.
#[derive(Debug, Clone)]
pub struct AggregatorColumn<'a> {
    /// ISO `YYYY-MM-DD` end date stamped on the emitted facts (aligns them to
    /// the pipeline's period).
    pub period_end: &'a str,
    /// The fiscal year whose column to read (matched against header text).
    pub fiscal_year: i64,
    /// Optional disambiguator when a year has several columns (e.g. `"Q1"`,
    /// `"1Q"`, `"I"`), matched as a normalized substring of the header.
    pub period_hint: Option<&'a str>,
}

/// Parses aggregator HTML into the tracked facts for the requested column.
/// Every fact carries [`SourceTier::HtmlAggregator`] and its row label as the
/// citation; monetary values are scaled to base units by the page's stated unit.
pub fn parse_html_financials(html: &str, column: &AggregatorColumn<'_>) -> Vec<ExtractedFact> {
    let document = Html::parse_document(html);
    let unit_factor =
        detect_unit_scale(&document.root_element().text().collect::<String>()).factor();

    let table_sel = Selector::parse("table").unwrap();
    let tr_sel = Selector::parse("tr").unwrap();
    let cell_sel = Selector::parse("th, td").unwrap();

    let mut facts: Vec<ExtractedFact> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for table in document.select(&table_sel) {
        let rows: Vec<Vec<String>> = table
            .select(&tr_sel)
            .map(|tr| tr.select(&cell_sel).map(element_text).collect::<Vec<_>>())
            .filter(|cells| !cells.is_empty())
            .collect();
        if rows.len() < 2 {
            continue;
        }
        let header = &rows[0];
        let Some(col_idx) = select_column_index(header, column, &rows) else {
            continue;
        };

        for row in &rows[1..] {
            if row.len() <= col_idx {
                continue;
            }
            let label = normalize_label(&row[0]);
            let Some(metric_key) = match_dictionary_label(&label) else {
                continue;
            };
            if !seen.insert(metric_key.to_string()) {
                continue;
            }
            let Some(raw) = parse_amount(&row[col_idx]) else {
                continue;
            };
            let value = if is_per_share(metric_key) {
                raw
            } else {
                raw * rust_decimal::Decimal::from(unit_factor)
            };
            facts.push(ExtractedFact {
                metric_key: metric_key.to_string(),
                value,
                period: FactPeriod::Instant(column.period_end.to_string()),
                basis: None,
                currency: None,
                tier: SourceTier::HtmlAggregator,
                citation: label,
            });
        }
    }

    facts
}

/// Picks the value-column index for the requested period. Prefers a header cell
/// matching the year (and hint); falls back to the sole value column when the
/// table has exactly two columns (label + one period).
fn select_column_index(
    header: &[String],
    column: &AggregatorColumn<'_>,
    rows: &[Vec<String>],
) -> Option<usize> {
    let year = column.fiscal_year.to_string();
    let hint = column.period_hint.map(normalize_label);
    for (i, cell) in header.iter().enumerate().skip(1) {
        let norm = normalize_label(cell);
        let year_ok = norm.contains(&year);
        let hint_ok = hint.as_ref().map(|h| norm.contains(h)).unwrap_or(true);
        if year_ok && hint_ok {
            return Some(i);
        }
    }
    // Single-period table: label + exactly one value column.
    let widths_two = rows.iter().all(|r| r.len() == 2);
    if widths_two && header.len() == 2 {
        return Some(1);
    }
    None
}

fn element_text(element: scraper::ElementRef<'_>) -> String {
    element
        .text()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;
