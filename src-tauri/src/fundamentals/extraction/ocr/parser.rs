//! Pure deterministic parser over OCR-emitted markdown (ADR 0077 §4, tier-4).
//!
//! Reads the stitched full-document markdown Mistral OCR produces (per-page
//! markdown with each page's `tables` folded in by the provider), walks the
//! GitHub-style pipe tables, and reads facts through a confirmed
//! [`OcrExtractionProfile`]. It is **pure** — no IO, no state — and shares the
//! tier-2 number/label primitives ([`super::super::pdf`]) so Polish number
//! formatting and scale handling stay identical across tiers.
//!
//! Two hard invariants (proptest-enforced below):
//! 1. every emitted value is a real cell value from the input table times the
//!    profile scale (per-share metrics excepted — never scaled) — the parser can
//!    never fabricate a number absent from a cell;
//! 2. a label absent from the profile's `label_map` is never emitted.
//!
//! The caller supplies the [`SourceTier`] and [`FactPeriod`]; this parser makes
//! no trust decision (T4.3 stamps `SourceTier::AiText` / `source_tier='ai'`).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;
use rust_decimal::Decimal;

use super::super::pdf::{is_per_share, normalize_label, parse_amount};
use super::super::{ExtractedFact, FactPeriod, SourceTier};
use super::profile::{OcrExtractionProfile, ValueColumnLayout};

/// One GitHub-style markdown table: a header row and its body rows, each split
/// into trimmed cell strings.
#[derive(Debug, Clone)]
struct MarkdownTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// Parses OCR-emitted markdown into facts through a confirmed company profile.
///
/// Walks every markdown table, selects the value column per the profile's
/// [`ValueColumnLayout`] (skipping configured header columns), maps the
/// (enumerator-stripped, normalized) row label via `label_map`, parses the cell
/// with the shared Polish-number rules, and applies the profile scale (per-share
/// metrics unscaled). First match per metric wins. Pure and total.
pub fn parse_ocr_markdown(
    markdown: &str,
    profile: &OcrExtractionProfile,
    period: &FactPeriod,
    tier: SourceTier,
) -> Vec<ExtractedFact> {
    let scale_factor = Decimal::from(profile.scale.factor());
    let mut facts = Vec::new();
    let mut seen_metrics: BTreeSet<String> = BTreeSet::new();

    for table in parse_markdown_tables(markdown) {
        let Some(value_col) = select_value_column_index(&table, profile, period) else {
            continue;
        };
        for row in &table.rows {
            let Some(label_cell) = row.first() else {
                continue;
            };
            let label_text = if profile.strip_enumerators {
                strip_enumerator(label_cell)
            } else {
                label_cell.clone()
            };
            let label = normalize_label(&label_text);
            if label.is_empty() {
                continue;
            }
            let Some(metric_key) = profile.label_map.get(&label) else {
                continue;
            };
            // First match per metric wins (statements repeat totals in notes).
            if !seen_metrics.insert(metric_key.clone()) {
                continue;
            }
            let Some(cell) = row.get(value_col) else {
                continue;
            };
            let Some(raw) = parse_amount(cell) else {
                continue;
            };
            // Per-share metrics are printed in absolute zł, never scaled by the
            // document unit (shared rule with the tier-2 PDF parser).
            let value = if is_per_share(metric_key) {
                raw
            } else {
                raw * scale_factor
            };
            facts.push(ExtractedFact {
                metric_key: metric_key.clone(),
                value,
                period: period.clone(),
                basis: None,
                currency: None,
                tier,
                citation: label,
            });
        }
    }

    facts
}

/// Chooses the value-column index for a table under the profile's layout, or
/// `None` when no usable value column exists. Column 0 is the label column;
/// candidate value columns are the remaining headers not matched by
/// `skip_columns`.
fn select_value_column_index(
    table: &MarkdownTable,
    profile: &OcrExtractionProfile,
    period: &FactPeriod,
) -> Option<usize> {
    let n = table.headers.len();
    if n < 2 {
        return None;
    }
    let skip: Vec<String> = profile
        .skip_columns
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let candidates: Vec<usize> = (1..n)
        .filter(|&i| {
            let header = table.headers[i].trim().to_lowercase();
            !skip
                .iter()
                .any(|s| header == *s || header.contains(s.as_str()))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    match profile.value_column {
        ValueColumnLayout::CurrentPeriodFirst | ValueColumnLayout::SingleValue => {
            candidates.first().copied()
        }
        ValueColumnLayout::CurrentPeriodLast => candidates.last().copied(),
        ValueColumnLayout::LabeledByPeriodHeader => {
            let end = period.end_date();
            let year = end.get(..4).unwrap_or(end);
            // Pick the column whose header names the requested period. No
            // confident match yields nothing — never a guessed column.
            candidates.iter().copied().find(|&i| {
                let header = table.headers[i].trim();
                header.contains(end) || (!year.is_empty() && header.contains(year))
            })
        }
    }
}

/// Extracts every GitHub-style pipe table (header row + `---` separator + body
/// rows) from the markdown, in document order.
fn parse_markdown_tables(markdown: &str) -> Vec<MarkdownTable> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut tables = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if is_table_row(lines[i]) && i + 1 < lines.len() && is_separator_row(lines[i + 1]) {
            let headers = split_row(lines[i]);
            let mut rows = Vec::new();
            let mut j = i + 2;
            while j < lines.len() && is_table_row(lines[j]) {
                rows.push(split_row(lines[j]));
                j += 1;
            }
            tables.push(MarkdownTable { headers, rows });
            i = j;
        } else {
            i += 1;
        }
    }
    tables
}

fn is_table_row(line: &str) -> bool {
    line.trim().contains('|')
}

/// A markdown table separator: only `|`, `-`, `:`, whitespace, and at least one
/// dash (`| --- | :---: |`).
fn is_separator_row(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.contains('-')
        && trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Splits one pipe-table row into trimmed cell strings, tolerating optional
/// leading/trailing pipes.
fn split_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);
    trimmed
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// Strips a single leading enumerator token (`"1."`, `"12)"`, `"I."`, `"a)"`)
/// from a row label. Runs before normalization so a "1. Przychody" line maps as
/// "przychody".
fn strip_enumerator(label: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // A digit run, a roman-numeral run, or a single letter, each closed by
        // `.` or `)`, then optional whitespace.
        Regex::new(r"(?i)^\s*(?:\d+[.)]|[ivxlcdm]+[.)]|[a-z][.)])\s*").unwrap()
    });
    re.replace(label, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn period() -> FactPeriod {
        FactPeriod::Instant("2025-09-30".to_string())
    }

    fn label_map() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("przychody ze sprzedaży".to_string(), "revenue".to_string()),
            ("aktywa razem".to_string(), "total_assets".to_string()),
            ("zysk netto".to_string(), "net_profit".to_string()),
            ("zysk na akcję".to_string(), "eps_basic".to_string()),
        ])
    }

    fn profile_with(
        layout: ValueColumnLayout,
        skip: Vec<String>,
        strip: bool,
        scale: UnitScale,
    ) -> OcrExtractionProfile {
        OcrExtractionProfile::bootstrap("GPW:TST", scale, label_map(), layout, skip, strip)
    }

    fn value_of<'a>(facts: &'a [ExtractedFact], metric: &str) -> Option<&'a Decimal> {
        facts
            .iter()
            .find(|f| f.metric_key == metric)
            .map(|f| &f.value)
    }

    use super::super::super::pdf::UnitScale;

    #[test]
    fn layout_current_period_first_picks_first_value_column() {
        let md = "\
| Wyszczególnienie | 2025 | 2024 |
| --- | --- | --- |
| Przychody ze sprzedaży | 1 500 | 1 200 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodFirst,
                Vec::new(),
                false,
                UnitScale::Thousands,
            ),
            &period(),
            SourceTier::AiText,
        );
        assert_eq!(value_of(&facts, "revenue"), Some(&Decimal::from(1_500_000)));
    }

    #[test]
    fn layout_current_period_last_picks_last_value_column() {
        let md = "\
| Wyszczególnienie | 2024 | 2025 |
| --- | --- | --- |
| Przychody ze sprzedaży | 1 200 | 1 500 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodLast,
                Vec::new(),
                false,
                UnitScale::Thousands,
            ),
            &period(),
            SourceTier::AiText,
        );
        assert_eq!(value_of(&facts, "revenue"), Some(&Decimal::from(1_500_000)));
    }

    #[test]
    fn layout_labeled_by_period_header_picks_matching_column() {
        let md = "\
| Pozycja | 30.09.2024 | 30.09.2025 |
| --- | --- | --- |
| Przychody ze sprzedaży | 1 200 | 1 500 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::LabeledByPeriodHeader,
                Vec::new(),
                false,
                UnitScale::Thousands,
            ),
            &period(),
            SourceTier::AiText,
        );
        // Header "30.09.2025" contains the requested period's year → its column wins.
        assert_eq!(value_of(&facts, "revenue"), Some(&Decimal::from(1_500_000)));
    }

    #[test]
    fn layout_single_value_reads_the_only_value_column() {
        let md = "\
| Pozycja | Wartość |
| --- | --- |
| Aktywa razem | 42 000 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::SingleValue,
                Vec::new(),
                false,
                UnitScale::Thousands,
            ),
            &period(),
            SourceTier::AiText,
        );
        assert_eq!(
            value_of(&facts, "total_assets"),
            Some(&Decimal::from(42_000_000))
        );
    }

    #[test]
    fn nota_column_is_skipped_when_configured() {
        let md = "\
| Wyszczególnienie | Nota | 2025 | 2024 |
| --- | --- | --- | --- |
| Przychody ze sprzedaży | 5 | 1 500 | 1 200 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodFirst,
                vec!["Nota".to_string()],
                false,
                UnitScale::Thousands,
            ),
            &period(),
            SourceTier::AiText,
        );
        // Without the skip the parser would read the note ref "5"; with it, the
        // first *value* column (1 500) wins.
        assert_eq!(value_of(&facts, "revenue"), Some(&Decimal::from(1_500_000)));
    }

    #[test]
    fn leading_enumerators_are_stripped_before_label_match() {
        let md = "\
| Wyszczególnienie | 2025 |
| --- | --- |
| 1. Przychody ze sprzedaży | 1 500 |
| II. Zysk netto | 300 |
| a) Aktywa razem | 9 000 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodFirst,
                Vec::new(),
                true,
                UnitScale::Thousands,
            ),
            &period(),
            SourceTier::AiText,
        );
        assert_eq!(value_of(&facts, "revenue"), Some(&Decimal::from(1_500_000)));
        assert_eq!(
            value_of(&facts, "net_profit"),
            Some(&Decimal::from(300_000))
        );
        assert_eq!(
            value_of(&facts, "total_assets"),
            Some(&Decimal::from(9_000_000))
        );
    }

    #[test]
    fn parses_polish_numbers_and_parenthesized_negatives() {
        let md = "\
| Wyszczególnienie | 2025 |
| --- | --- |
| Przychody ze sprzedaży | 1 234 567 |
| Zysk netto | (12 500) |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodFirst,
                Vec::new(),
                false,
                UnitScale::Ones,
            ),
            &period(),
            SourceTier::AiText,
        );
        assert_eq!(value_of(&facts, "revenue"), Some(&Decimal::from(1_234_567)));
        assert_eq!(
            value_of(&facts, "net_profit"),
            Some(&Decimal::from(-12_500))
        );
    }

    #[test]
    fn per_share_metric_is_not_scaled() {
        let md = "\
| Wyszczególnienie | 2025 |
| --- | --- |
| Zysk na akcję | 2,45 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodFirst,
                Vec::new(),
                false,
                UnitScale::Millions,
            ),
            &period(),
            SourceTier::AiText,
        );
        // EPS stays 2.45 despite the Millions document scale.
        assert_eq!(
            value_of(&facts, "eps_basic"),
            Some(&Decimal::from_str_exact("2.45").unwrap())
        );
    }

    #[test]
    fn scale_multiplies_monetary_values() {
        let md = "\
| Wyszczególnienie | 2025 |
| --- | --- |
| Przychody ze sprzedaży | 1 500 |
";
        let millions = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodFirst,
                Vec::new(),
                false,
                UnitScale::Millions,
            ),
            &period(),
            SourceTier::AiText,
        );
        assert_eq!(
            value_of(&millions, "revenue"),
            Some(&Decimal::from(1_500_000_000i64))
        );
    }

    #[test]
    fn unknown_labels_are_never_emitted() {
        let md = "\
| Wyszczególnienie | 2025 |
| --- | --- |
| Tajemnicza pozycja | 999 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodFirst,
                Vec::new(),
                false,
                UnitScale::Thousands,
            ),
            &period(),
            SourceTier::AiText,
        );
        assert!(facts.is_empty());
    }

    #[test]
    fn caller_supplied_tier_is_stamped() {
        let md = "\
| Wyszczególnienie | 2025 |
| --- | --- |
| Przychody ze sprzedaży | 1 500 |
";
        let facts = parse_ocr_markdown(
            md,
            &profile_with(
                ValueColumnLayout::CurrentPeriodFirst,
                Vec::new(),
                false,
                UnitScale::Thousands,
            ),
            &period(),
            SourceTier::AiText,
        );
        assert_eq!(facts[0].tier, SourceTier::AiText);
    }

    // ---- Golden snapshot over a synthetic OCR-v4-shaped document ----

    #[test]
    fn golden_synthetic_ocr_document() {
        // Synthetic (no private/ data): a realistic Polish interim statement,
        // OCR-v4 shape — per-page markdown with the balance-sheet and P&L tables
        // stitched in, a Nota reference column, leading enumerators, mln scale.
        let md = "\
# Skonsolidowane sprawozdanie finansowe

Dane w mln zł.

## Rachunek zysków i strat

| Wyszczególnienie | Nota | 30.09.2025 | 30.09.2024 |
| --- | --- | --- | --- |
| 1. Przychody ze sprzedaży | 4 | 1 842 | 1 610 |
| 2. Zysk netto | 7 | (58) | 121 |
| 3. Zysk na akcję | | 3,15 | 6,54 |

## Sprawozdanie z sytuacji finansowej

| Pozycja | Nota | 30.09.2025 | 31.12.2024 |
| --- | --- | --- | --- |
| I. Aktywa razem | 12 | 9 730 | 9 015 |
| II. Pozycja bez mapowania | 13 | 400 | 380 |
";
        let profile = OcrExtractionProfile::bootstrap(
            "GPW:LPP",
            UnitScale::Millions,
            label_map(),
            ValueColumnLayout::CurrentPeriodFirst,
            vec!["Nota".to_string()],
            true,
        );
        let facts = parse_ocr_markdown(md, &profile, &period(), SourceTier::AiText);
        insta::assert_debug_snapshot!(facts);
    }

    // ---- Property invariants (ADR 0049 / ADR 0077 §4) ----

    use proptest::prelude::*;

    fn parse_all_cell_magnitudes(rows: &[Vec<String>]) -> BTreeSet<Decimal> {
        let mut mags = BTreeSet::new();
        for row in rows {
            for cell in row {
                if let Some(v) = parse_amount(cell) {
                    mags.insert(v);
                }
            }
        }
        mags
    }

    proptest! {
        /// Invariant 1: every emitted value is exactly a cell value from the
        /// input times the profile scale (all generated metrics are monetary /
        /// non-per-share). The parser can never fabricate a number.
        /// Invariant 2: a label absent from the profile's label_map is never
        /// emitted (the "mystery" rows carry unmapped labels).
        #[test]
        fn invariants_no_fabrication_and_no_unknown_labels(
            scale in prop::sample::select(vec![UnitScale::Ones, UnitScale::Thousands, UnitScale::Millions]),
            rows in prop::collection::vec(
                (
                    prop::sample::select(vec![
                        "przychody ze sprzedaży",
                        "aktywa razem",
                        "zysk netto",
                        "tajemnicza pozycja",
                        "inny nieznany wiersz",
                    ]),
                    0u64..1_000_000u64,
                    0u64..1_000_000u64,
                ),
                1..12usize,
            ),
        ) {
            // A label_map with only monetary (non-per-share) metrics.
            let map = BTreeMap::from([
                ("przychody ze sprzedaży".to_string(), "revenue".to_string()),
                ("aktywa razem".to_string(), "total_assets".to_string()),
                ("zysk netto".to_string(), "net_profit".to_string()),
            ]);
            let known_metrics: BTreeSet<String> = map.values().cloned().collect();

            let mut md = String::from("| Pozycja | 2025 | 2024 |\n| --- | --- | --- |\n");
            let mut table_rows: Vec<Vec<String>> = Vec::new();
            for (label, a, b) in &rows {
                md.push_str(&format!("| {label} | {a} | {b} |\n"));
                table_rows.push(vec![label.to_string(), a.to_string(), b.to_string()]);
            }

            let profile = OcrExtractionProfile::bootstrap(
                "GPW:PROP",
                scale,
                map,
                ValueColumnLayout::CurrentPeriodFirst,
                Vec::new(),
                false,
            );
            let facts = parse_ocr_markdown(
                &md,
                &profile,
                &FactPeriod::Instant("2025-09-30".to_string()),
                SourceTier::AiText,
            );

            let scale_dec = Decimal::from(scale.factor());
            let magnitudes = parse_all_cell_magnitudes(&table_rows);
            let scaled: BTreeSet<Decimal> = magnitudes.iter().map(|m| m * scale_dec).collect();

            for fact in &facts {
                prop_assert!(
                    known_metrics.contains(&fact.metric_key),
                    "emitted an unmapped metric: {}",
                    fact.metric_key
                );
                prop_assert!(
                    scaled.contains(&fact.value),
                    "fabricated value {} not present as any cell × scale",
                    fact.value
                );
            }
        }
    }
}
