//! Unit tests for the pdf2htmlEX positional parser, over a small synthetic
//! fragment that mirrors the real CD PROJEKT anatomy: a `<style>` coordinate
//! map, a page container, self-contained statement-line divs, one number
//! **shredded across adjacent spans**, a leading note-reference, and an English
//! unit-scale marker.

use super::*;
use rust_decimal::Decimal;

/// A dozen spans / a handful of positioned divs forming the statement rows we
/// grade. Faithful to pdf2htmlEX: numbers are split across `<span>`s with a
/// single space between thousands groups and 2+ spaces between columns; the
/// balance-sheet row carries a leading note-reference (`16`); the scale marker
/// is English.
const FRAGMENT: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head>
<style type="text/css">
.x0{left:56.000000px;}
.x1{left:300.000000px;}
.y0{bottom:700.000000px;}
.y1{bottom:680.000000px;}
.y2{bottom:660.000000px;}
.y3{bottom:640.000000px;}
.y4{bottom:620.000000px;}
</style></head><body>
<div id="pf1" class="pf w0 h0" style="content-visibility: auto;">
<div class="t m0 x0 h4 y0 ff2 fs1 fc0 sc0 ls0 ws0">(all amounts in PLN thousand, unless stated otherwise) </div>
<div class="t m0 x0 hb y1 ff2 fs4 fc0 sc0 ls7 ws15">Sales revenue<span class="_ _0"> </span> <span class="lsa">227</span> <span class="ls15">555</span> <span class="_ _1"> </span>442 <span class="lsf">682</span> <span class="_ _1"> </span><span class="lsf">652</span> <span class="ls12">375</span> <span class="_ _1"> </span><span class="ls15">767</span> <span class="_ _0">692</span></div>
<div class="t m0 x0 hb y2 ff2 fs4 fc0 sc0 ls8 wsf">Cash and cash equivalents  16  82 281  148 980  178 054 </div>
<div class="t m0 x0 hb y3 ff3 fs4 fc0 sc0 lsb wsb">EQUITY    2 570 916  2 484 354  2 403 223 </div>
<div class="t m0 x0 hb y4 ff2 fs4 fc0 sc0 ls7 ws15">Basic earnings per share (in PLN)  2.48  2.89 </div>
</div>
</body></html>"#;

#[test]
fn parses_x_and_y_coordinate_maps() {
    let xs = parse_coord_map(FRAGMENT, 'x');
    let ys = parse_coord_map(FRAGMENT, 'y');
    assert_eq!(xs.get("0"), Some(&56.0));
    assert_eq!(xs.get("1"), Some(&300.0));
    assert_eq!(ys.get("0"), Some(&700.0));
    assert_eq!(ys.get("4"), Some(&620.0));
}

#[test]
fn resolves_divs_with_page_bottom_left_and_reconstructed_text() {
    let divs = positioned_divs(FRAGMENT);
    // 5 text divs on page 1.
    assert_eq!(divs.len(), 5);
    assert!(divs.iter().all(|d| d.page == 1));
    let revenue = divs
        .iter()
        .find(|d| d.text.starts_with("Sales revenue"))
        .expect("revenue div");
    assert_eq!(revenue.left, 56.0);
    assert_eq!(revenue.bottom, 680.0);
    // The shredded spans reconstruct with a single space inside a number and
    // 2+ spaces between columns.
    assert_eq!(
        revenue.text.trim(),
        "Sales revenue  227 555  442 682  652 375  767 692"
    );
}

#[test]
fn grid_rows_split_columns_on_two_space_runs_and_rejoin_shredded_numbers() {
    let rows = grid_rows(&positioned_divs(FRAGMENT));
    let revenue = rows
        .iter()
        .find(|r| r[0] == "Sales revenue")
        .expect("revenue row");
    // A single interior space stays inside the cell (shredded number rejoined);
    // 2+ spaces are column breaks.
    assert_eq!(
        revenue,
        &vec![
            "Sales revenue".to_string(),
            "227 555".to_string(),
            "442 682".to_string(),
            "652 375".to_string(),
            "767 692".to_string(),
        ]
    );
}

#[test]
fn value_columns_skip_leading_note_reference_but_keep_parenthesized_small_values() {
    // "16" is a bare positive sub-1000 note-ref → skipped.
    let cols = value_columns(&[
        "Cash and cash equivalents".into(),
        "16".into(),
        "82 281".into(),
        "148 980".into(),
    ]);
    assert_eq!(cols, vec![Decimal::from(82281), Decimal::from(148980)]);
    // A parenthesized "(950)" is a real negative value, never a note-ref.
    let cf = value_columns(&[
        "Net cash from financing activities".into(),
        "(950)".into(),
        "1 234".into(),
    ]);
    assert_eq!(cf, vec![Decimal::from(-950), Decimal::from(1234)]);
}

#[test]
fn english_metric_maps_labels_and_guards_ambiguous_totals() {
    assert_eq!(english_metric("Sales revenue"), Some("revenue"));
    assert_eq!(
        english_metric("Operating profit/(loss)"),
        Some("operating_profit")
    );
    assert_eq!(english_metric("EQUITY"), Some("total_equity"));
    assert_eq!(
        english_metric("Net cash from operating activities"),
        Some("operating_cash_flow")
    );
    assert_eq!(
        english_metric("Basic earnings per share (in PLN)"),
        Some("eps_basic")
    );
    assert_eq!(
        english_metric("Diluted earnings/(loss) per share"),
        Some("eps_diluted")
    );
    // "TOTAL EQUITY AND LIABILITIES" is total *assets*, never total_equity.
    assert_eq!(english_metric("TOTAL EQUITY AND LIABILITIES"), None);
    // Polish still resolves via the shared dictionary fallback.
    assert_eq!(english_metric("aktywa razem"), Some("total_assets"));
}

#[test]
fn detect_scale_handles_english_and_polish_markers() {
    assert_eq!(
        detect_scale("(all amounts in PLN thousand, unless stated otherwise)"),
        UnitScale::Thousands
    );
    assert_eq!(
        detect_scale("all amounts in PLN million"),
        UnitScale::Millions
    );
    assert_eq!(detect_scale("dane w mln zł"), UnitScale::Millions);
}

#[test]
fn parse_html_positional_emits_scaled_facts_from_the_grid() {
    let facts = parse_html_positional(FRAGMENT, &PositionalColumn::new("2024-09-30"));
    let by_key: std::collections::BTreeMap<_, _> = facts
        .iter()
        .map(|f| (f.metric_key.as_str(), f.value))
        .collect();
    // The revenue row is a Q3 main income statement `[3M | 3M' | 9M-YTD | 9M-YTD']`
    // (`227 555  442 682  652 375  767 692`): the current-YTD (larger-magnitude of
    // the {0, 2} current family) is selected and scaled ×1000 ("PLN thousand").
    assert_eq!(by_key.get("revenue"), Some(&Decimal::from(652375000i64)));
    // A 3-column balance-sheet row (after the note-ref skip) → current is leftmost.
    assert_eq!(by_key.get("cash"), Some(&Decimal::from(82281000i64)));
    assert_eq!(
        by_key.get("total_equity"),
        Some(&Decimal::from(2570916000i64))
    );
    // EPS is per-share — never scaled by the document unit (2-column row → col 0).
    assert_eq!(by_key.get("eps_basic"), Some(&Decimal::new(248, 2)));
    // Facts carry the deterministic Pdf sub-tier, period and citation.
    let rev = facts.iter().find(|f| f.metric_key == "revenue").unwrap();
    assert_eq!(rev.tier, SourceTier::Pdf);
    assert_eq!(rev.period.end_date(), "2024-09-30");
    assert_eq!(rev.citation, "sales revenue");
}

#[test]
fn select_current_value_picks_ytd_in_both_four_column_layouts() {
    let decs = |xs: &[i64]| xs.iter().map(|&x| Decimal::from(x)).collect::<Vec<_>>();
    // Main P&L/CF `[Q3-3M | prior-3M | 9M-YTD | prior-YTD]` → current-YTD (col 2,
    // the larger-magnitude current column).
    assert_eq!(
        select_current_value(&decs(&[227555, 442682, 652375, 767692])),
        Some(Decimal::from(652375))
    );
    // "Selected financial data" summary `[YTD-PLN | prior-PLN | YTD-EUR | prior-EUR]`
    // → current-YTD-PLN (col 0, the larger-magnitude current column).
    assert_eq!(
        select_current_value(&decs(&[220874, 281152, 51340, 61423])),
        Some(Decimal::from(220874))
    );
    // Signed cash-flow row: magnitude, not numeric max, drives the pick.
    assert_eq!(
        select_current_value(&decs(&[-361046, -183968, -83921, -40191])),
        Some(Decimal::from(-361046))
    );
    // 2-column interim `[current | prior]` and 3-column balance sheet → col 0.
    assert_eq!(
        select_current_value(&decs(&[181114, 49341])),
        Some(Decimal::from(181114))
    );
    assert_eq!(
        select_current_value(&decs(&[82281, 148980, 178054])),
        Some(Decimal::from(82281))
    );
    assert_eq!(select_current_value(&[]), None);
}

/// The clustered variant (kept for the detached-label case): a label div and a
/// value div sharing a baseline are merged into one row, ordered by `left`.
const DETACHED_FRAGMENT: &str = r#"<html><head><style type="text/css">
.x0{left:56.000000px;}
.x1{left:300.000000px;}
.y0{bottom:700.000000px;}
.y1{bottom:680.000000px;}
</style></head><body>
<div id="pf1" class="pf">
<div class="t m0 x0 hb y0 ff2">TOTAL ASSETS</div>
<div class="t m0 x1 hb y0 ff2">2 755 416  2 660 007 </div>
<div class="t m0 x0 hb y1 ff2">EQUITY    2 570 916 </div>
</div></body></html>"#;

#[test]
fn cluster_rows_by_bottom_joins_a_detached_label_and_value() {
    let divs = positioned_divs(DETACHED_FRAGMENT);
    let rows = cluster_rows_by_bottom(&divs, 1.0);
    // The label + value divs at bottom 700 merge (ordered by left); EQUITY at
    // bottom 680 stays its own row.
    assert!(rows.contains(&vec![
        "TOTAL ASSETS".to_string(),
        "2 755 416".to_string(),
        "2 660 007".to_string(),
    ]));
    assert!(rows.contains(&vec!["EQUITY".to_string(), "2 570 916".to_string()]));
}

/// Insta golden of the full parse output for the synthetic fragment (ADR 0049) —
/// pins the emitted facts (metric, value, YTD column, tier, citation) so any
/// silent change to the grid reconstruction or column selection reddens.
#[test]
fn parse_output_golden_snapshot() {
    let facts = parse_html_positional(FRAGMENT, &PositionalColumn::new("2024-09-30"));
    insta::assert_debug_snapshot!(facts);
}

// ---- Property invariants (ADR 0049) ----

mod invariants {
    use super::super::{cluster_rows_by_bottom, select_current_value, split_cells, PositionedDiv};
    use proptest::prelude::*;
    use rust_decimal::Decimal;

    /// Formats a magnitude with pdf2htmlEX-style single-space thousands groups
    /// (`1234567` → `"1 234 567"`), the shredding [`split_cells`] must rejoin.
    fn group_thousands(v: u64) -> String {
        let digits = v.to_string();
        let bytes = digits.as_bytes();
        let mut out = String::new();
        for (i, b) in bytes.iter().enumerate() {
            if i > 0 && (bytes.len() - i).is_multiple_of(3) {
                out.push(' ');
            }
            out.push(*b as char);
        }
        out
    }

    proptest! {
        /// Grid reconstruction round-trips: a row assembled from a label plus
        /// space-shredded numbers, columns separated by 2+ spaces, splits back into
        /// exactly those cells — a single interior space (thousands) never breaks a
        /// cell, and [`split_cells`] fabricates no column.
        #[test]
        fn shredded_row_round_trips_to_its_cells(
            label in "[A-Za-z]{1,8}( [A-Za-z]{1,8}){0,2}",
            values in prop::collection::vec(0u64..100_000_000u64, 1..6usize),
        ) {
            let cells: Vec<String> = std::iter::once(label.clone())
                .chain(values.iter().map(|v| group_thousands(*v)))
                .collect();
            // 2-space runs are the column boundary the generator emits.
            let row = cells.join("  ");
            prop_assert_eq!(split_cells(&row), cells);
        }

        /// Clustering never merges rows with distinct y-classes: divs on
        /// well-separated baselines (gaps far exceeding the tolerance) each stay
        /// their own row — the false-merge failure mode the module avoids.
        #[test]
        fn clustering_never_merges_distinct_baselines(n in 1usize..8) {
            let divs: Vec<PositionedDiv> = (0..n)
                .map(|i| PositionedDiv {
                    page: 1,
                    bottom: 100.0 + i as f64 * 10.0,
                    left: 56.0,
                    text: format!("Row{i}  {}  {}", i + 1, i + 2),
                })
                .collect();
            let rows = cluster_rows_by_bottom(&divs, 1.0);
            prop_assert_eq!(rows.len(), n);
        }

        /// Column selection never fabricates: the current-period value is always
        /// one of the input columns (never a synthesized number).
        #[test]
        fn selected_value_is_always_an_input_column(
            values in prop::collection::vec(-1_000_000i64..1_000_000i64, 1..7usize),
        ) {
            let decs: Vec<Decimal> = values.iter().map(|&v| Decimal::from(v)).collect();
            if let Some(picked) = select_current_value(&decs) {
                prop_assert!(decs.contains(&picked));
            }
        }
    }
}
