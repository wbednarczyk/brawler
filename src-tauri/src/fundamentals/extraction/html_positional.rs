//! Tier-3b — positional parser for **pdf2htmlEX visual-render XHTML** (F6b
//! spike, ADR 0077).
//!
//! Some GPW issuers (CD PROJEKT among them) publish their interim financial
//! statements only as a `pdf2htmlEX` visual render: an XHTML with **no
//! `<table>` and no `ix:` tags**, where every glyph run is an absolutely
//! positioned `<div class="t …">` whose horizontal/vertical position is carried
//! by CSS classes (`.x35{left:1.3px;}` `.y0{bottom:0.65px;}`) declared in a
//! `<style>` block, and whose numbers are **shredded across adjacent `<span>`s**
//! (`227 555` is `<span>227</span> <span>555</span>`). No existing tier can read
//! these: ESEF needs iXBRL tags, tier-2 needs a `<table>`, the tier-4 vision
//! path is PDF-only. This parser turns the geometry back into a grid and feeds
//! the **existing** fact-mapping seam (`normalize_label` → dictionary →
//! `parse_amount` → scale), exactly like [`super::html`] does for aggregators.
//!
//! ## What the sample data actually looks like (F6b measurement, 3 CDR interims)
//!
//! - **A statement line is one self-contained `<div>`.** pdf2htmlEX emits each
//!   typeset row as a single positioned div whose text content — once the inner
//!   `<span>`s are stripped — reconstructs the whole line, e.g. `Sales revenue
//!   ␣␣ 227 555 ␣␣ 442 682 ␣␣ 652 375 ␣␣ 767 692`. The shredding is **within the
//!   div at the span level**, and the generator preserves inter-part spacing as
//!   real space text-nodes: a **single** space between the thousands groups of
//!   one number, **two or more** spaces between columns. So the cell-break
//!   threshold is derived from the data: split a row on runs of **2+ whitespace**
//!   → cells; a single interior space is a thousands separator (the shredded
//!   number rejoins for free). This is the [`grid_rows`] heuristic.
//! - **Geometric row-clustering is counter-productive here** (measured: recall
//!   0.56 / precision 0.67 vs per-div 0.89 / 0.97). pdf2htmlEX nests wrapped
//!   table cells in sub-containers and **reuses `y` classes** (many unrelated
//!   rows share `.y46`), so clustering divs by absolute `bottom` false-merges
//!   clean statement rows into garbage. The reliable granularity is therefore
//!   **one div = one row**. [`cluster_rows_by_bottom`] implements the clustered
//!   variant for the detached-label case and is kept for completeness + tests,
//!   but [`parse_html_positional`] uses the per-div grid.
//! - **Labels are English** ("Sales revenue", "Operating profit/(loss)", "EQUITY")
//!   and the scale marker is English ("(all amounts in PLN thousand …)"). The
//!   shared dictionary is Polish-only, so this module adds a **module-local**
//!   English map ([`english_metric`]) tried *before* the shared
//!   [`super::pdf::match_dictionary_label`] fallback — no other tier is touched.
//! - **Current-period column selection is automatic** ([`select_current_value`]),
//!   applying the owner's YTD-cumulative convention (ADR 0077 T-B2) without needing
//!   to know which statement produced a row. Measured on the 3 real CDR interims,
//!   two four-column layouts coexist and **both interleave `[current, prior,
//!   current, prior]`**: the Q3 *main* P&L/CF statement prints `[Q3-3M | prior-3M |
//!   9M-YTD | prior-YTD]` (current-YTD at index 2), while the regulatory *"Selected
//!   financial data"* summary prints dual-currency `[YTD-PLN | prior-PLN | YTD-EUR |
//!   prior-EUR]` (current-YTD-PLN at index 0). The current family is therefore the
//!   even indices `{0, 2}` and the wanted current-YTD-PLN figure is the **larger-
//!   magnitude** of that pair (9M cumulative > a single quarter; PLN > EUR).
//!   2- or 3-column interim/balance-sheet rows print the current period leftmost →
//!   index 0. This **per-row magnitude** rule was chosen over the spike's two
//!   alternatives after real-data measurement (see [`select_current_value`]).
//!
//! Pure over `&str` — no network, no DB — so it is fully unit-testable, and its
//! output clears the same [`crate::fundamentals::validation`] gate as every
//! other tier before any fact is trusted. Wired into routing (T-B2): an
//! ESEF-route XHTML that is not valid iXBRL (a pdf2htmlEX render) reaches this
//! parser from [`crate::jobs::structured_extraction`], persisting its facts under
//! the deterministic [`SourceTier::Pdf`] with an identifiable
//! `extraction_method='html_positional'` provenance marker.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use rust_decimal::Decimal;

use super::pdf::{is_per_share, match_dictionary_label, normalize_label, parse_amount, UnitScale};
use super::{ExtractedFact, FactPeriod, SourceTier};

/// The reporting date to stamp on emitted facts. The current-period **column** is
/// no longer a caller parameter — it is selected per row by [`select_current_value`]
/// under the owner's fixed YTD-cumulative convention (ADR 0077 T-B2).
#[derive(Debug, Clone)]
pub struct PositionalColumn {
    /// ISO `YYYY-MM-DD` period-end stamped on every emitted fact.
    pub period_end: String,
}

impl PositionalColumn {
    /// Stamp emitted facts with `period_end`; the current-period value column is
    /// selected automatically per statement row.
    pub fn new(period_end: impl Into<String>) -> Self {
        Self {
            period_end: period_end.into(),
        }
    }
}

/// Selects the current-period value from a row's ordered value columns, applying
/// the owner's YTD-cumulative convention (ADR 0077 T-B2) **without knowing which
/// statement produced the row**.
///
/// GPW interim renders print value columns interleaved `[current, prior, current,
/// prior]`. Two four-column layouts coexist in the same document and were measured
/// on the 3 real CDR interims (33/33 graded):
/// - a **main** Q3/9M P&L or cash-flow statement — `[Q3-3M | prior-3M | 9M-YTD |
///   prior-YTD]`, current-YTD at index 2;
/// - the regulatory **"Selected financial data"** summary — dual-currency
///   `[YTD-PLN | prior-PLN | YTD-EUR | prior-EUR]`, current-YTD-PLN at index 0.
///
/// The current family is the even indices `{0, 2}`; the wanted current-YTD-PLN
/// figure is the **larger-magnitude** of that pair — a 9M cumulative exceeds a
/// single quarter, and a PLN amount exceeds its ~4.3× smaller EUR twin. This
/// per-row rule was chosen over the spike's two candidates because measurement
/// showed each is wrong here: a fixed index-2 mis-reads the summary rows (which
/// win the first-match for most metrics) as their EUR column, and preferring the
/// summary table needs fragile region-detection plus detached-label clustering for
/// the one metric (revenue) whose summary label is a separate positioned div.
/// A 2- or 3-column interim/balance-sheet row prints the current period leftmost →
/// index 0. Returns `None` for an empty row.
///
/// Known limitation (named honestly, ADR 0077 F6b/T-B2): the magnitude rule can
/// pick the 3M quarter over the 9M YTD when a mid-year sign reversal leaves
/// |3M| > |9M| (e.g. a strong Q3 after a loss-making H1 netting near zero).
/// Rare on real flows, and a wrong pick still faces the identity-validation
/// gate + the planned aggregator witness; do not "fix" this by summing columns —
/// composites are a product decision.
pub fn select_current_value(values: &[Decimal]) -> Option<Decimal> {
    match values.len() {
        0 => None,
        // The exact four-column interim signature: pick the larger-magnitude of
        // the current family {0, 2}. Wider rows (8–11-column equity-movement
        // tables) are noise a prior metric match has already claimed; default to
        // the leftmost current column if one ever wins.
        4 => {
            let (a, b) = (values[0], values[2]);
            Some(if b.abs() > a.abs() { b } else { a })
        }
        _ => Some(values[0]),
    }
}

/// A resolved positioned text run: one `<div class="t …">` with its page, its
/// baseline `bottom`, its `left`, and its reconstructed text (spans stripped,
/// entities decoded, inter-run spacing preserved).
#[derive(Debug, Clone)]
pub struct PositionedDiv {
    pub page: u32,
    pub bottom: f64,
    pub left: f64,
    pub text: String,
}

// Whitespace that separates columns / groups thousands: ASCII space, tab, and
// the non-breaking / narrow-no-break spaces pdf2htmlEX emits.
const WS: &[char] = &[' ', '\t', '\u{00a0}', '\u{202f}'];

/// Parses a `.x##{left:…px;}` / `.y##{bottom:…px;}` CSS map for the given axis
/// (`'x'` → the `left` property, `'y'` → `bottom`), keyed by the class number as
/// a string (e.g. `"35"` → `1.3`).
pub fn parse_coord_map(css: &str, axis: char) -> BTreeMap<String, f64> {
    static X_RE: OnceLock<Regex> = OnceLock::new();
    static Y_RE: OnceLock<Regex> = OnceLock::new();
    // pdf2htmlEX names its position classes in **hex** (`.x1a`, `.y4d`), so the
    // class id is `[0-9a-f]+`, matched as an opaque string keyed identically in
    // the map and the div lookup.
    let re = match axis {
        'x' => X_RE.get_or_init(|| Regex::new(r"\.x([0-9a-f]+)\{left:([0-9.]+)px;\}").unwrap()),
        'y' => Y_RE.get_or_init(|| Regex::new(r"\.y([0-9a-f]+)\{bottom:([0-9.]+)px;\}").unwrap()),
        _ => return BTreeMap::new(),
    };
    re.captures_iter(css)
        .filter_map(|c| {
            Some((
                c.get(1)?.as_str().to_string(),
                c.get(2)?.as_str().parse().ok()?,
            ))
        })
        .collect()
}

/// Locates the `id="pf<hex>"` page-marker byte offsets, so each text div can be
/// attributed to its page (pdf2htmlEX numbers pages `pf1`, `pf2`, … in hex).
fn page_markers(html: &str) -> Vec<(usize, u32)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"id="pf([0-9a-f]+)""#).unwrap());
    re.captures_iter(html)
        .filter_map(|c| {
            let m = c.get(0)?;
            let page = u32::from_str_radix(c.get(1)?.as_str(), 16).ok()?;
            Some((m.start(), page))
        })
        .collect()
}

fn page_at(markers: &[(usize, u32)], pos: usize) -> u32 {
    let mut page = 0;
    for &(start, no) in markers {
        if start <= pos {
            page = no;
        } else {
            break;
        }
    }
    page
}

/// Strips inner tags from a div's inner HTML and decodes the entities
/// pdf2htmlEX emits, preserving the inter-run whitespace that encodes columns.
fn reconstruct_text(inner_html: &str) -> String {
    static TAG_RE: OnceLock<Regex> = OnceLock::new();
    let tag = TAG_RE.get_or_init(|| Regex::new(r"<[^>]*>").unwrap());
    let stripped = tag.replace_all(inner_html, "");
    decode_entities(&stripped)
}

fn decode_entities(s: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"&(#x?[0-9a-fA-F]+|[a-zA-Z]+);").unwrap());
    re.replace_all(s, |c: &regex::Captures<'_>| {
        let body = &c[1];
        if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16)
                .ok()
                .and_then(char::from_u32)
                .map(String::from)
                .unwrap_or_else(|| c[0].to_string())
        } else if let Some(dec) = body.strip_prefix('#') {
            dec.parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .map(String::from)
                .unwrap_or_else(|| c[0].to_string())
        } else {
            match body {
                "amp" => "&",
                "lt" => "<",
                "gt" => ">",
                "quot" => "\"",
                "apos" => "'",
                "nbsp" => "\u{00a0}",
                _ => return c[0].to_string(),
            }
            .to_string()
        }
    })
    .into_owned()
}

/// Resolves every `<div class="t …">` in the document to a [`PositionedDiv`],
/// looking up its `x`/`y` classes in the style coordinate maps and its page from
/// the `pf` markers. Divs whose class or coordinate is unknown are dropped.
pub fn positioned_divs(html: &str) -> Vec<PositionedDiv> {
    static DIV_RE: OnceLock<Regex> = OnceLock::new();
    let div_re =
        DIV_RE.get_or_init(|| Regex::new(r#"(?s)<div class="t ([^"]*)">(.*?)</div>"#).unwrap());
    let xs = parse_coord_map(html, 'x');
    let ys = parse_coord_map(html, 'y');
    let markers = page_markers(html);

    let mut out = Vec::new();
    for cap in div_re.captures_iter(html) {
        let class = &cap[1];
        let (Some(xk), Some(yk)) = (class_token(class, 'x'), class_token(class, 'y')) else {
            continue;
        };
        let (Some(&left), Some(&bottom)) = (xs.get(xk), ys.get(yk)) else {
            continue;
        };
        let pos = cap.get(0).map(|m| m.start()).unwrap_or(0);
        out.push(PositionedDiv {
            page: page_at(&markers, pos),
            bottom,
            left,
            text: reconstruct_text(&cap[2]),
        });
    }
    out
}

/// Finds the `x##`/`y##` class token in a space-separated class string.
fn class_token(class: &str, axis: char) -> Option<&str> {
    class.split(WS).find_map(|tok| {
        let rest = tok.strip_prefix(axis)?;
        // Hex class id (`x1a` → "1a"), matched as the coordinate-map key.
        if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_hexdigit()) {
            Some(rest)
        } else {
            None
        }
    })
}

/// Splits a reconstructed line into cells on runs of **2+ whitespace** — the
/// data-derived column boundary (a single interior space is a thousands
/// separator, so a shredded number rejoins into one cell).
fn split_cells(line: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[ \t\u{00a0}\u{202f}]{2,}").unwrap());
    re.split(line.trim())
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string)
        .collect()
}

/// Per-div grid: each positioned text div becomes one row (the reliable
/// granularity for pdf2htmlEX — see module docs), split into cells on 2+
/// whitespace. Rows with fewer than 2 cells are dropped.
pub fn grid_rows(divs: &[PositionedDiv]) -> Vec<Vec<String>> {
    divs.iter()
        .map(|d| split_cells(&d.text))
        .filter(|cells| cells.len() >= 2)
        .collect()
}

/// Bottom-clustered grid: divs on the same page whose baselines fall within
/// `tolerance` are merged into one row, ordered by `left`, then split into
/// cells. **Measured inferior** to [`grid_rows`] on real pdf2htmlEX documents
/// (nested wrapped cells reuse `y` classes and false-merge unrelated rows); kept
/// for the detached-label case and as the contract's clustered variant.
pub fn cluster_rows_by_bottom(divs: &[PositionedDiv], tolerance: f64) -> Vec<Vec<String>> {
    let mut by_page: BTreeMap<u32, Vec<&PositionedDiv>> = BTreeMap::new();
    for d in divs {
        by_page.entry(d.page).or_default().push(d);
    }
    let mut out = Vec::new();
    for items in by_page.values_mut() {
        // Top-to-bottom, then left-to-right.
        items.sort_by(|a, b| {
            b.bottom
                .partial_cmp(&a.bottom)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    a.left
                        .partial_cmp(&b.left)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        let mut cur: Vec<&PositionedDiv> = Vec::new();
        let mut base: Option<f64> = None;
        for d in items.iter() {
            match base {
                Some(b) if (d.bottom - b).abs() <= tolerance => cur.push(d),
                _ => {
                    if !cur.is_empty() {
                        out.push(join_row(&cur));
                    }
                    cur = vec![d];
                    base = Some(d.bottom);
                }
            }
        }
        if !cur.is_empty() {
            out.push(join_row(&cur));
        }
    }
    out.into_iter().filter(|cells| cells.len() >= 2).collect()
}

fn join_row(divs: &[&PositionedDiv]) -> Vec<String> {
    let mut ordered = divs.to_vec();
    ordered.sort_by(|a, b| {
        a.left
            .partial_cmp(&b.left)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // A div boundary is at least a column break, so join with 2 spaces.
    let joined = ordered
        .iter()
        .map(|d| d.text.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("  ");
    split_cells(&joined)
}

/// The module-local English statement dictionary (normalized label → metric),
/// matched longest-first so a specific label beats a prefix of it.
fn english_dictionary() -> &'static [(&'static str, &'static str)] {
    &[
        ("sales revenue", "revenue"),
        ("sales revenues", "revenue"),
        ("net sales of products", "revenue"),
        ("operating profit", "operating_profit"),
        ("net profit", "net_profit"),
        ("total assets", "total_assets"),
        ("equity", "total_equity"),
        ("cash and cash equivalents", "cash"),
        ("net cash from operating activities", "operating_cash_flow"),
        (
            "net cash flows from operating activities",
            "operating_cash_flow",
        ),
        (
            "net cash generated from operating activities",
            "operating_cash_flow",
        ),
        ("net cash from investing activities", "investing_cash_flow"),
        (
            "net cash flows from investing activities",
            "investing_cash_flow",
        ),
        ("net cash from financing activities", "financing_cash_flow"),
        (
            "net cash flows from financing activities",
            "financing_cash_flow",
        ),
        ("basic earnings per share", "eps_basic"),
        ("basic for the reporting period", "eps_basic"),
        ("net earnings/(loss) per share", "eps_basic"),
        ("diluted earnings per share", "eps_diluted"),
        ("diluted for the reporting period", "eps_diluted"),
        ("diluted earnings/(loss) per share", "eps_diluted"),
    ]
}

/// Maps a statement label to a catalog metric key: the module-local English map
/// first (longest-first), then the shared Polish dictionary
/// ([`super::pdf::match_dictionary_label`]) as a fallback — so Polish notes in an
/// English filing still resolve and no other tier's dictionary is touched. The
/// ambiguous "TOTAL EQUITY AND LIABILITIES" (which is *total assets*, not
/// equity) does not start with any equity/asset pattern and returns `None`.
pub fn english_metric(label: &str) -> Option<&'static str> {
    let n = normalize_label(label);
    let mut entries: Vec<&(&str, &str)> = english_dictionary().iter().collect();
    entries.sort_by_key(|(l, _)| std::cmp::Reverse(l.len()));
    for (pattern, metric) in entries {
        // Prefix match (longest-first) — a statement label extends its metric
        // stem with punctuation ("Operating profit/(loss)") or qualifiers ("Net
        // profit/(loss) attributable to owners…"). "TOTAL EQUITY AND
        // LIABILITIES" starts with "total", not "equity", so it never matches.
        if n == *pattern || n.starts_with(pattern) {
            return Some(metric);
        }
    }
    match_dictionary_label(&n)
}

/// English-aware unit-scale detection: recognizes "PLN million" / "in million(s)"
/// as [`UnitScale::Millions`], otherwise delegates to the shared
/// [`super::pdf::detect_unit_scale`] (Polish markers + thousands default, which
/// is also correct for the English "PLN thousand" marker).
pub fn detect_scale(text: &str) -> UnitScale {
    let t = text.to_lowercase();
    if t.contains("pln million") || t.contains("in million") || t.contains("in millions") {
        UnitScale::Millions
    } else {
        super::pdf::detect_unit_scale(text)
    }
}

/// Parses one cell into a magnitude, tolerating the English decimal **dot**
/// (the shared [`super::pdf::parse_amount`] expects a Polish comma; thousands are
/// spaces in these renders, so a lone dot is always the decimal).
fn parse_cell_amount(cell: &str) -> Option<Decimal> {
    parse_amount(&cell.replace('.', ","))
}

/// Classifies a value cell as `(value, grouped, decimal)` — the shape cues used
/// to tell a note-reference (`16`) from a value column (`82 281` / `2.48`).
fn classify_cell(cell: &str) -> Option<(Decimal, bool, bool)> {
    let v = parse_cell_amount(cell)?;
    let t = cell.trim();
    let grouped = t.contains(WS);
    let decimal = t.contains('.') || t.contains(',');
    Some((v, grouped, decimal))
}

/// A row's value columns in order (cell `0` is the label), after dropping a
/// leading note-reference — a bare positive sub-1000 integer, never a
/// parenthesized/negative value. Mirrors the tier-2 `value_tokens` rule.
pub fn value_columns(cells: &[String]) -> Vec<Decimal> {
    let toks: Vec<(Decimal, bool, bool)> = cells
        .iter()
        .skip(1)
        .filter_map(|c| classify_cell(c))
        .collect();
    let has_shaped = toks.iter().any(|(_, g, d)| *g || *d);
    let mut start = 0;
    if has_shaped {
        while let Some(&(v, grouped, decimal)) = toks.get(start) {
            let note_ref = !grouped && !decimal && v > Decimal::ZERO && v < Decimal::from(1000);
            if note_ref {
                start += 1;
            } else {
                break;
            }
        }
    }
    toks[start..].iter().map(|(v, _, _)| *v).collect()
}

/// Parses pdf2htmlEX positional XHTML into extracted facts. Each metric's first
/// matching row wins; the current-period value column is selected per row by
/// [`select_current_value`]; monetary values scale by the document unit, per-share
/// values do not. Every fact carries the deterministic [`SourceTier::Pdf`] (the
/// render is a visual-PDF reconstruction; its distinct sub-tier is recorded via the
/// `extraction_method='html_positional'` provenance marker at persist time — ADR
/// 0077 T-B2) and its normalized label as citation.
pub fn parse_html_positional(html: &str, column: &PositionalColumn) -> Vec<ExtractedFact> {
    let divs = positioned_divs(html);
    let full_text = divs
        .iter()
        .map(|d| d.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let scale_factor = Decimal::from(detect_scale(&full_text).factor());

    let mut facts = Vec::new();
    let mut seen: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    for row in grid_rows(&divs) {
        let Some(metric) = english_metric(&row[0]) else {
            continue;
        };
        if seen.contains(metric) {
            continue;
        }
        let values = value_columns(&row);
        let Some(raw) = select_current_value(&values) else {
            continue;
        };
        let value = if is_per_share(metric) {
            raw
        } else {
            raw * scale_factor
        };
        seen.insert(metric);
        facts.push(ExtractedFact {
            metric_key: metric.to_string(),
            value,
            period: FactPeriod::Instant(column.period_end.clone()),
            basis: None,
            currency: None,
            tier: SourceTier::Pdf,
            citation: normalize_label(&row[0]),
        });
    }
    facts
}

#[cfg(test)]
mod realdata;
#[cfg(test)]
mod tests;
