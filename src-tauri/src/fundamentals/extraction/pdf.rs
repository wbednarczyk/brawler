//! Deterministic PDF parser — tier 2 of the pipeline (ADR 0061).
//!
//! When no ESEF/iXBRL instance exists (the common case for GPW interim reports
//! and NewConnect issuers), the figures live only in a human-typeset PDF. This
//! parser reads the **already-extracted text** (see
//! [`crate::report_diff::extraction::extract_report`]) line by line, matches
//! each line's leading label against a Polish financial-statement dictionary,
//! and reads the current-period value column — applying the document's unit
//! scale (`w tys. zł` → ×1000, `w mln zł` → ×1e6), Polish number formatting
//! (space/NBSP thousands, comma decimal, accounting parentheses) and note-ref
//! skipping.
//!
//! It is **not** a source of truth: PDF layout is unschematized, so every fact
//! it produces must clear the [`crate::fundamentals::validation`] gate before
//! it is trusted, and a per-company [`super::profile::ExtractionProfile`]
//! records the confirmed label→metric mapping so drift is detectable. It is
//! pure over `&str` (text, not bytes), so it is fully unit-testable.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use super::profile::ExtractionProfile;
use super::{ExtractedFact, FactPeriod, SourceTier};

/// The default Polish label → `metric_key` dictionary. Each entry is a
/// normalized label (lowercase, whitespace-collapsed) and the metric it maps
/// to. Longer labels are matched first so `zysk z działalności operacyjnej`
/// wins over `zysk`. A per-company profile can override/extend this.
fn default_dictionary() -> &'static [(&'static str, &'static str)] {
    // Ordered longest-first at match time; declaration order is not relied on.
    &[
        // Balance sheet.
        ("aktywa razem", "total_assets"),
        ("suma aktywów", "total_assets"),
        ("aktywa ogółem", "total_assets"),
        ("zobowiązania razem", "total_liabilities"),
        ("zobowiązania ogółem", "total_liabilities"),
        (
            "zobowiązania i rezerwy na zobowiązania",
            "total_liabilities",
        ),
        ("kapitał własny", "total_equity"),
        ("kapitał własny razem", "total_equity"),
        ("razem kapitał własny", "total_equity"),
        ("środki pieniężne i ich ekwiwalenty", "cash"),
        ("środki pieniężne i ekwiwalenty środków pieniężnych", "cash"),
        // Income statement.
        ("przychody netto ze sprzedaży", "revenue"),
        ("przychody ze sprzedaży", "revenue"),
        ("przychody z umów z klientami", "revenue"),
        ("zysk brutto ze sprzedaży", "gross_profit"),
        ("zysk z działalności operacyjnej", "operating_profit"),
        (
            "zysk (strata) z działalności operacyjnej",
            "operating_profit",
        ),
        ("zysk operacyjny", "operating_profit"),
        ("zysk netto", "net_profit"),
        ("zysk (strata) netto", "net_profit"),
        (
            "zysk netto przypisany akcjonariuszom jednostki dominującej",
            "net_profit",
        ),
        // Cash-flow statement.
        (
            "przepływy pieniężne netto z działalności operacyjnej",
            "operating_cash_flow",
        ),
        (
            "przepływy środków pieniężnych z działalności operacyjnej",
            "operating_cash_flow",
        ),
        (
            "przepływy pieniężne netto z działalności inwestycyjnej",
            "investing_cash_flow",
        ),
        (
            "przepływy pieniężne netto z działalności finansowej",
            "financing_cash_flow",
        ),
        // Per-share (never scaled by the document unit).
        ("zysk na jedną akcję", "eps_basic"),
        ("zysk na akcję", "eps_basic"),
        ("rozwodniony zysk na akcję", "eps_diluted"),
    ]
}

/// Normalizes a label for dictionary matching: lowercased, whitespace
/// collapsed, surrounding punctuation and note-reference digits trimmed.
pub fn normalize_label(raw: &str) -> String {
    let lowered = raw.to_lowercase();
    let collapsed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .trim_matches(|c: char| c == ':' || c == '.' || c == '-' || c.is_whitespace())
        .to_string()
}

/// The unit scale a document reports its monetary figures in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnitScale {
    Ones,
    Thousands,
    Millions,
}

impl UnitScale {
    pub fn factor(self) -> i64 {
        match self {
            UnitScale::Ones => 1,
            UnitScale::Thousands => 1_000,
            UnitScale::Millions => 1_000_000,
        }
    }
    pub fn as_i64(self) -> i64 {
        self.factor()
    }
}

/// Detects the document-wide monetary unit from phrases like `w tys. zł` /
/// `dane w tysiącach` / `w mln zł`. Defaults to thousands — the overwhelmingly
/// common convention for Polish statements — when nothing is found.
pub fn detect_unit_scale(text: &str) -> UnitScale {
    let t = text.to_lowercase();
    if t.contains("w mln") || t.contains("w milionach") || t.contains("mln zł") {
        UnitScale::Millions
    } else {
        // Thousands is the dominant Polish convention and the default; the
        // explicit `w tys.` wording resolves here too.
        UnitScale::Thousands
    }
}

/// A single numeric token read from a statement line, with the shape cues used
/// to tell a note-reference (`5`) from a value column (`1 234` / `2,45`).
#[derive(Debug, Clone, Copy)]
struct NumToken {
    value: Decimal,
    grouped: bool,
    decimal: bool,
}

/// Locates where the numeric region of a line begins: the first digit,
/// including a leading `(` or `-` sign directly before it.
fn first_number_start() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // A leading `(`, ASCII `-`, or a Unicode minus/dash keeps the sign attached
    // to the value column rather than stranding it in the label.
    RE.get_or_init(|| Regex::new(r"[(\u{2013}\u{2014}\u{2212}-]?\d").unwrap())
}

/// Matches a single value column: an optional sign/paren, then a grouped or
/// plain digit run with an optional decimal comma. Within a column, a single
/// space (or NBSP) is a thousands separator; columns themselves are separated
/// by 2+ spaces or a tab (see [`split_columns`]), which removes the
/// "1 100 900" grouping ambiguity that single-space tables create.
fn column_number_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\(?-?(?:\d{1,3}(?:[ \u{00A0}\u{202F}]\d{3})+|\d+)(?:,\d+)?\)?$").unwrap()
    })
}

/// Splits a line's numeric region into column strings on runs of 2+ whitespace
/// or a tab — the tabular-PDF column boundary — while a single interior space
/// stays as a thousands separator.
fn split_columns(region: &str) -> Vec<&str> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[ \u{00A0}\u{202F}]{2,}|\t").unwrap());
    re.split(region.trim())
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// Strips a trailing per-cell currency word (`"1 234 zł"` → `"1 234"`). Scale
/// words (`tys.`/`mln`) and `%` are intentionally *not* stripped: `%` marks a
/// non-monetary ratio that must be rejected, and cell-level scale words are a
/// rare edge left to the document-level unit.
fn strip_currency_suffix(s: &str) -> String {
    let mut out = s.trim().to_string();
    loop {
        let lower = out.to_lowercase();
        let mut stripped = false;
        for suf in [" zł", " zl", " pln", " eur", "zł", "zl", "pln", "eur"] {
            if lower.ends_with(suf) {
                out.truncate(out.len() - suf.len());
                out = out.trim_end().to_string();
                stripped = true;
                break;
            }
        }
        if !stripped {
            break;
        }
    }
    out
}

/// Parses one column string into a numeric token, or `None` if the column is
/// not a clean number. Handles accounting parentheses, ASCII and Unicode
/// minus/dash (`-`, `–`, `—`, `−`), and a trailing currency word; a bare dash
/// (`–`, "no data") yields `None`.
fn parse_token(tok: &str) -> Option<NumToken> {
    // Normalize a leading Unicode minus/dash to ASCII '-'.
    let mut s = tok.trim().to_string();
    if let Some(rest) = s
        .strip_prefix('\u{2013}')
        .or_else(|| s.strip_prefix('\u{2014}'))
        .or_else(|| s.strip_prefix('\u{2212}'))
    {
        s = format!("-{}", rest.trim_start());
    }
    s = strip_currency_suffix(&s);
    let s = s.trim();
    if !column_number_regex().is_match(s) {
        return None;
    }
    let mut negative = false;
    let mut body = s.to_string();
    if body.starts_with('(') && body.ends_with(')') {
        negative = true;
        body = body[1..body.len() - 1].to_string();
    }
    if let Some(stripped) = body.strip_prefix('-') {
        negative = true;
        body = stripped.to_string();
    }
    let grouped = body.contains(' ') || body.contains('\u{00A0}') || body.contains('\u{202F}');
    let decimal = body.contains(',');
    body.retain(|c| !c.is_whitespace());
    let normalized = body.replace(',', ".");
    let mut value = Decimal::from_str(&normalized).ok()?;
    if negative {
        value.set_sign_negative(true);
    }
    Some(NumToken {
        value,
        grouped,
        decimal,
    })
}

/// Splits a statement line into its leading label and the value tokens after
/// it. Returns `None` when the line has no label or no numbers. Columns are
/// separated on 2+ whitespace / tab; a single interior space is a thousands
/// separator.
fn split_label_and_values(line: &str) -> Option<(String, Vec<NumToken>)> {
    let first = first_number_start().find(line)?;
    let label_raw = &line[..first.start()];
    let label = normalize_label(label_raw);
    if label.is_empty() {
        return None;
    }
    let numeric_region = &line[first.start()..];
    let tokens: Vec<NumToken> = split_columns(numeric_region)
        .into_iter()
        .filter_map(parse_token)
        .collect();
    if tokens.is_empty() {
        return None;
    }
    Some((label, tokens))
}

/// Drops leading bare small integers (note refs) from `tokens` until the
/// first value-shaped (grouped or decimal) token, so callers can take the
/// Nth *value* column positionally regardless of a note reference prefix.
fn value_tokens(tokens: &[NumToken]) -> impl Iterator<Item = &NumToken> {
    let has_value_shaped = tokens.iter().any(|t| t.grouped || t.decimal);
    let mut iter = tokens.iter().peekable();
    if has_value_shaped {
        while let Some(t) = iter.peek() {
            let looks_like_note = !t.grouped && !t.decimal && t.value.abs() < Decimal::from(1000);
            if looks_like_note {
                iter.next();
            } else {
                break;
            }
        }
    }
    iter
}

/// Chooses the current-period value from a line's tokens, skipping a leading
/// note-reference. Polish statements print the current period in the first
/// value column.
fn current_period_value(tokens: &[NumToken]) -> Option<Decimal> {
    value_tokens(tokens).next().map(|t| t.value)
}

/// Chooses the prior-period (comparative) value from a line's tokens — the
/// value column immediately after the current-period one, which Polish
/// statements conventionally print alongside it. `None` when the line carries
/// no second value column (e.g. a company's first reporting period, or a
/// note-reference-only line).
fn comparative_period_value(tokens: &[NumToken]) -> Option<Decimal> {
    let mut iter = value_tokens(tokens);
    iter.next(); // the current-period value.
    iter.next().map(|t| t.value)
}

/// The result of parsing a report's text: the extracted facts plus the set of
/// labels that matched a metric — the raw material for drift detection against
/// a company profile.
#[derive(Debug, Clone)]
pub struct PdfParse {
    pub facts: Vec<ExtractedFact>,
    /// Normalized labels that matched a metric, for profile/drift comparison.
    pub matched_labels: BTreeSet<String>,
    pub unit_scale: UnitScale,
}

impl PdfParse {
    /// The confirmed label → `metric_key` mapping this parse implies, built
    /// from the emitted facts (each carries its matched label as citation).
    pub fn label_metric_map(&self) -> std::collections::BTreeMap<String, String> {
        self.facts
            .iter()
            .map(|f| (f.citation.clone(), f.metric_key.clone()))
            .collect()
    }
}

/// Parses report text for the given period end. When `profile` is supplied its
/// label map is consulted first (a confirmed company-specific mapping), then
/// the default dictionary. Every emitted fact carries [`SourceTier::Pdf`] and
/// the matched label as its citation; values are scaled to base units.
pub fn parse_pdf_text(
    text: &str,
    period_end: &str,
    profile: Option<&ExtractionProfile>,
) -> PdfParse {
    parse_pdf_text_impl(text, period_end, None, profile)
}

/// Same as [`parse_pdf_text`], but also reads the report's **second** value
/// column — the prior-period comparative Polish statements conventionally
/// print alongside the current figure — and emits it as additional facts
/// stamped with `prior_period_end` (ADR 0061 dec. 4b). These comparative facts
/// are distinguishable purely by period end: [`fact_set_for_period`] with
/// `period_end` yields the same current-period set [`parse_pdf_text`] would,
/// and with `prior_period_end` yields the comparative set for the pipeline's
/// cross-check against the already-stored prior period.
pub fn parse_pdf_text_with_comparatives(
    text: &str,
    period_end: &str,
    prior_period_end: &str,
    profile: Option<&ExtractionProfile>,
) -> PdfParse {
    parse_pdf_text_impl(text, period_end, Some(prior_period_end), profile)
}

fn parse_pdf_text_impl(
    text: &str,
    period_end: &str,
    prior_period_end: Option<&str>,
    profile: Option<&ExtractionProfile>,
) -> PdfParse {
    let unit_scale = profile
        .map(|p| p.unit_scale)
        .unwrap_or_else(|| detect_unit_scale(text));
    let scale_factor = Decimal::from(unit_scale.factor());

    let mut facts = Vec::new();
    let mut matched_labels = BTreeSet::new();
    let mut seen_metrics = BTreeSet::new();

    for line in text.lines() {
        let Some((label, tokens)) = split_label_and_values(line) else {
            continue;
        };
        let Some(metric_key) = match_label(&label, profile) else {
            continue;
        };
        // First match per metric wins (statements repeat totals in notes).
        if !seen_metrics.insert(metric_key.to_string()) {
            continue;
        }
        let Some(raw_value) = current_period_value(&tokens) else {
            continue;
        };
        // Per-share metrics are printed in absolute zł, never scaled by the
        // document unit.
        let value = if is_per_share(metric_key) {
            raw_value
        } else {
            raw_value * scale_factor
        };
        matched_labels.insert(label.clone());
        facts.push(ExtractedFact {
            metric_key: metric_key.to_string(),
            value,
            period: FactPeriod::Instant(period_end.to_string()),
            basis: None,
            currency: None,
            tier: SourceTier::Pdf,
            citation: label.clone(),
        });

        if let Some(prior_end) = prior_period_end {
            if let Some(raw_comparative) = comparative_period_value(&tokens) {
                let comparative_value = if is_per_share(metric_key) {
                    raw_comparative
                } else {
                    raw_comparative * scale_factor
                };
                facts.push(ExtractedFact {
                    metric_key: metric_key.to_string(),
                    value: comparative_value,
                    period: FactPeriod::Instant(prior_end.to_string()),
                    basis: None,
                    currency: None,
                    tier: SourceTier::Pdf,
                    citation: label,
                });
            }
        }
    }

    PdfParse {
        facts,
        matched_labels,
        unit_scale,
    }
}

pub(super) fn is_per_share(metric_key: &str) -> bool {
    matches!(
        metric_key,
        "eps_basic" | "eps_diluted" | "dividend_per_share"
    )
}

/// Parses a single value column into its base magnitude (no unit scaling),
/// reusing the tier-2 number rules. Shared with the HTML aggregator (tier 3).
pub(super) fn parse_amount(col: &str) -> Option<Decimal> {
    parse_token(col).map(|t| t.value)
}

/// Matches a normalized label against the default Polish dictionary
/// (longest-label-first). Shared with the HTML aggregator (tier 3).
pub(super) fn match_dictionary_label(label: &str) -> Option<&'static str> {
    let mut entries: Vec<&(&str, &str)> = default_dictionary().iter().collect();
    entries.sort_by_key(|(l, _)| std::cmp::Reverse(l.len()));
    for (pattern, metric) in entries {
        if label == *pattern || label.starts_with(pattern) {
            return Some(metric);
        }
    }
    None
}

/// Matches a normalized label to a metric key: the company profile first (its
/// confirmed mapping), then the default dictionary, longest-label-first so a
/// specific label beats a prefix of it.
fn match_label(label: &str, profile: Option<&ExtractionProfile>) -> Option<&'static str> {
    if let Some(profile) = profile {
        if let Some(metric) = profile.label_map.get(label) {
            // Profile values are validated against the catalog on write, so a
            // stored mapping resolves to a stable static key here.
            if let Some(k) = canonical_metric_key(metric) {
                return Some(k);
            }
        }
    }
    match_dictionary_label(label)
}

/// Resolves a stored metric-key string back to the static catalog key, so a
/// profile-driven match yields the same `&'static str` as a dictionary match.
fn canonical_metric_key(key: &str) -> Option<&'static str> {
    default_dictionary()
        .iter()
        .map(|(_, m)| *m)
        .find(|m| *m == key)
}

#[cfg(test)]
mod corpus;
#[cfg(test)]
mod tests;
