//! HTML financial-data aggregator adapter (BiznesRadar-primary, ADR 0086).
//!
//! Aggregators (BiznesRadar, Bankier „wyniki finansowe", StockWatch) publish a
//! company's headline figures as clean HTML tables: rows are metrics, columns
//! are reporting periods. This adapter parses that grid, maps the Polish row
//! labels to our `metric_key` catalog (reusing the tier-2 dictionary and number
//! rules), and reads every period column the page carries.
//!
//! ## Role: PRIMARY for core KPIs, witnessed in REVERSE (ADR 0086 dec. 2/4)
//! The original ADR 0061 posture ("witness, not source of truth") is **retired**.
//! BiznesRadar is the primary, daily source of the core KPI history
//! ([`crate::jobs::aggregator_fundamentals_pull`]): its facts are written under
//! `source_tier = html_aggregator`, which sits at the BOTTOM of the precedence
//! order (`manual > esef > espi_cover_note > positional > html_aggregator`), so
//! an issuer tier still owns any slot it reads.
//!
//! Witnessing therefore runs REVERSED: where an issuer tier (or the user's own
//! manual entry) holds the slot, the aggregator's figure never overwrites it and
//! instead records what it saw — a `witness_disagreement` extraction outcome when
//! the two diverge beyond tolerance, or a positive corroboration stamp on the
//! held fact's provenance when they agree (epic #229 T5). Pure over `&str`
//! HTML — no network.

use scraper::{Html, Selector};

use super::text_numbers::{
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

/// A reporting period one aggregator column carries, derived from its header
/// text ([`parse_period_header`]). The `period_type` is the canonical marker the
/// rest of the pipeline uses (`Q1` | `H1` | `Q3` | `FY`), so an aggregator column
/// lands in the SAME fact slot `(company, fiscal_year, period_type)` an issuer
/// tier would — which is what makes tier precedence (ADR 0086) comparable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregatorPeriod {
    /// The fiscal-year label the header carries (BR's FY number).
    pub fiscal_year: i64,
    /// Canonical period marker: `Q1` | `H1` | `Q3` | `FY`.
    pub period_type: &'static str,
    /// ISO `YYYY-MM-DD` period end. For an annual column the header MONTH gives
    /// the real (possibly shifted) fiscal-year end (card c465c54), not a forced
    /// calendar December.
    pub period_end: String,
}

/// Parses aggregator HTML into the tracked facts for the requested column.
/// Every fact carries [`SourceTier::HtmlAggregator`] and its row label as the
/// citation; monetary values are scaled to base units by the page's stated unit.
pub fn parse_html_financials(html: &str, column: &AggregatorColumn<'_>) -> Vec<ExtractedFact> {
    let document = Html::parse_document(html);
    let unit_factor =
        detect_unit_scale(&document.root_element().text().collect::<String>()).factor();

    let table_sel = Selector::parse("table").unwrap();
    let mut facts: Vec<ExtractedFact> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for table in document.select(&table_sel) {
        let rows = table_rows(table);
        if rows.len() < 2 {
            continue;
        }
        let header = &rows[0];
        let Some(col_idx) = select_column_index(header, column, &rows) else {
            continue;
        };
        for fact in facts_for_column(&rows, col_idx, unit_factor, column.period_end, &mut seen) {
            facts.push(fact);
        }
    }

    facts
}

/// Parses aggregator HTML into facts for **every** period column the page
/// carries — the BiznesRadar-primary backfill path (ADR 0086 decision 2). A
/// single fetched page holds many annual columns; this reads each, deriving its
/// period from the column header ([`parse_period_header`]) so one fetch yields
/// the whole reported history, not one fetch per period.
///
/// Returns one entry per recognized period column, `(period, facts)`, sorted by
/// fiscal year. A column whose header does not parse to a canonical period is
/// skipped (e.g. a "~branża" sector column or a malformed header) — never
/// guessed into a period.
pub fn parse_all_financials(html: &str) -> Vec<(AggregatorPeriod, Vec<ExtractedFact>)> {
    let document = Html::parse_document(html);
    let unit_factor =
        detect_unit_scale(&document.root_element().text().collect::<String>()).factor();
    let table_sel = Selector::parse("table").unwrap();

    // Group by (fiscal_year, period_type) so several tables (or a re-listed
    // column) accrue into one period; `seen` is per period so the same metric
    // in two tables does not double-write, while the SAME metric in two DIFFERENT
    // periods is kept.
    use std::collections::BTreeMap;
    let mut by_period: BTreeMap<(i64, &'static str), (AggregatorPeriod, Vec<ExtractedFact>)> =
        BTreeMap::new();
    let mut seen: BTreeMap<(i64, &'static str), std::collections::BTreeSet<String>> =
        BTreeMap::new();

    for table in document.select(&table_sel) {
        let rows = table_rows(table);
        if rows.len() < 2 {
            continue;
        }
        let header = &rows[0];
        for (col_idx, header_cell) in header.iter().enumerate().skip(1) {
            let Some(period) = parse_period_header(header_cell) else {
                continue;
            };
            let key = (period.fiscal_year, period.period_type);
            let column_seen = seen.entry(key).or_default();
            let column_facts =
                facts_for_column(&rows, col_idx, unit_factor, &period.period_end, column_seen);
            if column_facts.is_empty() {
                continue;
            }
            let entry = by_period
                .entry(key)
                .or_insert_with(|| (period.clone(), Vec::new()));
            entry.1.extend(column_facts);
        }
    }

    by_period.into_values().collect()
}

/// Derives a canonical reporting period from one aggregator column header.
///
/// BiznesRadar annual headers read `YEAR (mon YY)` (e.g. `2013 (gru 13)`); the
/// month in parentheses is the fiscal-year END month, so a non-December fiscal
/// year (`(cze 13)` → June) yields the real end `2013-06-30`, never a forced
/// calendar December (card c465c54). A glued/explicit quarter marker (`1Q2013`,
/// `Q1 2013`) maps to the canonical cumulative interim type. Returns `None` for
/// a header that carries no fiscal year — a sector/label column is not a period.
pub fn parse_period_header(header: &str) -> Option<AggregatorPeriod> {
    let lowered = header.to_lowercase();
    let fiscal_year = first_year(&lowered)?;

    // A quarter marker takes precedence over the annual reading: "1Q", "q1",
    // "q 1" all name an interim column.
    if let Some(quarter) = quarter_marker(&lowered) {
        let (period_type, month, day) = match quarter {
            1 => ("Q1", 3, 31),
            2 => ("H1", 6, 30),
            3 => ("Q3", 9, 30),
            _ => ("FY", 12, 31),
        };
        return Some(AggregatorPeriod {
            fiscal_year,
            period_type,
            period_end: iso_date(fiscal_year, month, day),
        });
    }

    // Annual column: the "(mon YY)" marker, if present, gives the fiscal-year end
    // month; default December when absent.
    let (month, day) = month_end(&lowered).unwrap_or((12, 31));
    // February fiscal-year ends land on the 29th in leap years (review 2026-07-22).
    let day = if month == 2 && is_leap_year(fiscal_year) {
        29
    } else {
        day
    };
    Some(AggregatorPeriod {
        fiscal_year,
        period_type: "FY",
        period_end: iso_date(fiscal_year, month, day),
    })
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// The first standalone 4-digit fiscal year (1990..=2099) in the header.
fn first_year(lowered: &str) -> Option<i64> {
    let bytes = lowered.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i].is_ascii_digit() && lowered.is_char_boundary(i + 4) {
            if let Ok(year) = lowered[i..i + 4].parse::<i64>() {
                if (1990..=2099).contains(&year) {
                    return Some(year);
                }
            }
        }
        i += 1;
    }
    None
}

/// A quarter number 1..=4 named by a `Q1`/`1Q`/`q 1` token, else `None`.
fn quarter_marker(lowered: &str) -> Option<u8> {
    let bytes = lowered.as_bytes();
    let digit_at = |i: usize| bytes.get(i).and_then(|b| (*b as char).to_digit(10));
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'q' {
            // A STANDALONE digit immediately before ("1q", "3q2024") — but never
            // the trailing digit of a longer number ("2024q3" must not read the
            // year's '4' as the quarter).
            if i > 0 && digit_at(i.wrapping_sub(2)).is_none() {
                if let Some(d) = digit_at(i - 1) {
                    if (1..=4).contains(&d) {
                        return Some(d as u8);
                    }
                }
            }
            // A STANDALONE digit immediately after ("q1", "q3 2024") — but never
            // the leading digit of a longer number ("q2024", and the year in a
            // glued "3q2024", whose '2' would otherwise win — review 2026-07-22).
            if let Some(d) = digit_at(i + 1) {
                if (1..=4).contains(&d) && digit_at(i + 2).is_none() {
                    return Some(d as u8);
                }
            }
        }
    }
    None
}

/// The `(month, last_day)` a Polish month abbreviation in the header denotes —
/// the fiscal-year end for an annual column. `None` when no month is present.
fn month_end(lowered: &str) -> Option<(u32, u32)> {
    // Ordered so a 3-char abbreviation matches its month; BiznesRadar uses
    // `sty lut mar kwi maj cze lip sie wrz paź lis gru`.
    const MONTHS: [(&str, (u32, u32)); 12] = [
        ("sty", (1, 31)),
        ("lut", (2, 28)),
        ("mar", (3, 31)),
        ("kwi", (4, 30)),
        ("maj", (5, 31)),
        ("cze", (6, 30)),
        ("lip", (7, 31)),
        ("sie", (8, 31)),
        ("wrz", (9, 30)),
        ("paź", (10, 31)),
        ("lis", (11, 30)),
        ("gru", (12, 31)),
    ];
    // `paz` without the diacritic, defensively.
    if lowered.contains("paz") {
        return Some((10, 31));
    }
    MONTHS
        .iter()
        .find(|(abbrev, _)| lowered.contains(abbrev))
        .map(|(_, end)| *end)
}

fn iso_date(year: i64, month: u32, day: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}")
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

/// The tracked facts one column of a parsed table yields, scaled to base units.
/// `seen` dedups by `metric_key` so the first matching row wins (dictionary is
/// longest-label-first, so the most specific label already won at match time).
fn facts_for_column(
    rows: &[Vec<String>],
    col_idx: usize,
    unit_factor: i64,
    period_end: &str,
    seen: &mut std::collections::BTreeSet<String>,
) -> Vec<ExtractedFact> {
    let mut facts = Vec::new();
    for row in &rows[1..] {
        if row.len() <= col_idx {
            continue;
        }
        let label = normalize_label(&row[0]);
        let Some(metric_key) = match_dictionary_label(&label) else {
            continue;
        };
        let Some(raw) = parse_amount(&row[col_idx]) else {
            continue;
        };
        if !seen.insert(metric_key.to_string()) {
            continue;
        }
        let value = if is_per_share(metric_key) {
            raw
        } else {
            raw * rust_decimal::Decimal::from(unit_factor)
        };
        facts.push(ExtractedFact {
            metric_key: metric_key.to_string(),
            value,
            period: FactPeriod::Instant(period_end.to_string()),
            basis: None,
            currency: None,
            tier: SourceTier::HtmlAggregator,
            citation: label,
        });
    }
    facts
}

/// One table's rows as text cells, VALUE-SPAN aware. BiznesRadar wraps the figure
/// in `<span class="value">` while a sibling `<div class="changeyy">` in the same
/// cell carries the r/r & sector change percentages; reading whole-cell text
/// ("7 203 r/r -80.94% …") would parse as garbage, so a value cell reads only its
/// value span. A label/header cell (no such span) reads its whole text.
fn table_rows(table: scraper::ElementRef<'_>) -> Vec<Vec<String>> {
    let tr_sel = Selector::parse("tr").unwrap();
    let cell_sel = Selector::parse("th, td").unwrap();
    table
        .select(&tr_sel)
        .map(|tr| tr.select(&cell_sel).map(cell_text).collect::<Vec<_>>())
        .filter(|cells| !cells.is_empty())
        .collect()
}

/// The value of one cell: the `<span class="value">` figure when present (real
/// BiznesRadar markup), otherwise the whole cell text (simplified samples /
/// label & header cells).
fn cell_text(element: scraper::ElementRef<'_>) -> String {
    let value_sel = Selector::parse("span.value").unwrap();
    if let Some(value_span) = element.select(&value_sel).next() {
        return joined_text(value_span);
    }
    joined_text(element)
}

fn joined_text(element: scraper::ElementRef<'_>) -> String {
    element
        .text()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;
