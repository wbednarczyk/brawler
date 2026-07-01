//! Broad PDF-extraction corpus (ADR 0061 S3b).
//!
//! A table-driven behavioural corpus over the real-world problem classes found
//! in Polish financial-report PDFs: unit scales, number formatting, sign
//! conventions, note references, label synonyms/collisions, currency suffixes,
//! duplicate lines, and lines that must be *rejected* (percentages, dates). It
//! doubles as the parser's hardening harness — every class that exposed a gap
//! was fixed in the parser (guardrail-harvest, ADR 0045), and the classes that
//! cannot be closed deterministically (a label wrapped onto the next line) are
//! marked as explicit known-limitations rather than silently passed.

use super::*;

/// One corpus case: parse `text` for a fixed period, assert each `(metric,
/// value)` in `expect` is present and equal, and that no metric in `reject`
/// appears (the "must not misread" set).
struct Case {
    name: &'static str,
    text: &'static str,
    expect: &'static [(&'static str, &'static str)],
    reject: &'static [&'static str],
}

const NBSP: &str = "\u{00A0}";
const ENDASH: &str = "\u{2013}";
const MINUS: &str = "\u{2212}";

fn cases() -> Vec<Case> {
    vec![
        // --- Unit scales -------------------------------------------------
        Case {
            name: "unit_thousands",
            text: "w tys. zł\nAktywa razem  45 000  40 000",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        Case {
            name: "unit_millions",
            text: "w mln zł\nAktywa razem  45  40",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        Case {
            name: "unit_tysiacach_wording",
            text: "dane w tysiącach złotych\nAktywa razem  45 000",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        Case {
            name: "unit_default_is_thousands",
            text: "Sprawozdanie finansowe\nAktywa razem  45 000",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        // --- Number formatting ------------------------------------------
        Case {
            name: "nbsp_thousands_separator",
            text: Box::leak(const_nbsp_line().into_boxed_str()),
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        Case {
            name: "comma_decimal_per_share_unscaled",
            text: "w tys. zł\nZysk na akcję  2,45  2,01",
            expect: &[("eps_basic", "2.45")],
            reject: &[],
        },
        // --- Sign conventions -------------------------------------------
        Case {
            name: "parentheses_negative",
            text: "w tys. zł\nZysk netto  (1 100)  900",
            expect: &[("net_profit", "-1100000")],
            reject: &[],
        },
        Case {
            name: "ascii_minus_negative",
            text: "w tys. zł\nZysk netto  -1 100  900",
            expect: &[("net_profit", "-1100000")],
            reject: &[],
        },
        Case {
            name: "endash_negative",
            text: Box::leak(format!("w tys. zł\nZysk netto  {ENDASH}1 100  900").into_boxed_str()),
            expect: &[("net_profit", "-1100000")],
            reject: &[],
        },
        Case {
            name: "unicode_minus_negative",
            text: Box::leak(format!("w tys. zł\nZysk netto  {MINUS}1 100  900").into_boxed_str()),
            expect: &[("net_profit", "-1100000")],
            reject: &[],
        },
        // --- Note references --------------------------------------------
        Case {
            name: "leading_note_ref_skipped",
            text: "w tys. zł\nKapitał własny  12  25 000  22 000",
            expect: &[("total_equity", "25000000")],
            reject: &[],
        },
        // --- Label synonyms ---------------------------------------------
        Case {
            name: "synonym_suma_aktywow",
            text: "w tys. zł\nSuma aktywów  45 000",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        Case {
            name: "synonym_aktywa_ogolem",
            text: "w tys. zł\nAktywa ogółem  45 000",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        Case {
            name: "loss_wording_zysk_strata_netto",
            text: "w tys. zł\nZysk (strata) netto  (1 100)  900",
            expect: &[("net_profit", "-1100000")],
            reject: &[],
        },
        // --- Label collisions -------------------------------------------
        Case {
            name: "gross_and_net_dont_collide",
            text: "w tys. zł\nZysk brutto ze sprzedaży  3 000  2 500\nZysk netto  1 100  900",
            expect: &[("gross_profit", "3000000"), ("net_profit", "1100000")],
            reject: &[],
        },
        // --- Currency suffix in cell ------------------------------------
        Case {
            name: "currency_suffix_in_cell",
            text: "w tys. zł\nAktywa razem  45 000 zł  40 000 zł",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        // --- Duplicate line (first wins) --------------------------------
        Case {
            name: "duplicate_line_first_wins",
            text: "w tys. zł\nAktywa razem  45 000\nNota 5. Aktywa razem  99 999",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        // --- Case + whitespace ------------------------------------------
        Case {
            name: "uppercase_label",
            text: "w tys. zł\nAKTYWA RAZEM  45 000",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        Case {
            name: "collapsed_extra_whitespace",
            text: "w tys. zł\n  Aktywa   razem     45 000",
            expect: &[("total_assets", "45000000")],
            reject: &[],
        },
        // --- Cash-flow / operating --------------------------------------
        Case {
            name: "operating_cash_flow",
            text: "w tys. zł\nPrzepływy pieniężne netto z działalności operacyjnej  6 000  5 000",
            expect: &[("operating_cash_flow", "6000000")],
            reject: &[],
        },
        // --- Must reject -------------------------------------------------
        Case {
            name: "reject_percentage_as_value",
            text: "w tys. zł\nZysk netto  15,3%",
            expect: &[],
            reject: &["net_profit"],
        },
        Case {
            name: "reject_iso_date_as_value",
            text: "w tys. zł\nAktywa razem  2026-03-31",
            expect: &[],
            reject: &["total_assets"],
        },
    ]
}

/// Built at runtime because a `const` cannot interpolate the NBSP constant.
fn const_nbsp_line() -> String {
    format!("w tys. zł\nAktywa razem  45{NBSP}000  40{NBSP}000")
}

#[test]
fn corpus_covers_real_pdf_problem_classes() {
    let mut failures: Vec<String> = Vec::new();
    for case in cases() {
        let parse = parse_pdf_text(case.text, "2026-03-31", None);
        let got: std::collections::BTreeMap<String, Decimal> = parse
            .facts
            .iter()
            .map(|f| (f.metric_key.clone(), f.value))
            .collect();
        for (metric, expected) in case.expect {
            let want = Decimal::from_str_exact(expected).unwrap();
            match got.get(*metric) {
                Some(v) if *v == want => {}
                Some(v) => failures.push(format!("[{}] {metric}: got {v}, want {want}", case.name)),
                None => failures.push(format!("[{}] {metric}: missing (want {want})", case.name)),
            }
        }
        for metric in case.reject {
            if let Some(v) = got.get(*metric) {
                failures.push(format!(
                    "[{}] {metric}: must be rejected but got {v}",
                    case.name
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "corpus failures:\n{}",
        failures.join("\n")
    );
}

/// Known limitation (ADR 0061): a label printed on its own line with the
/// figures on the next line is not recovered by the line-oriented parser. This
/// is deliberately *not* faked to pass — validation (S1) still refuses a silent
/// wrong value and drift (S3) flags the layout, so the pipeline stays safe. If
/// a deterministic recovery is added, drop the `#[ignore]`.
#[test]
#[ignore = "known limitation: label wrapped onto a separate line from its values"]
fn label_wrapped_to_next_line_is_a_known_gap() {
    let text = "w tys. zł\nPrzychody netto ze sprzedaży\n  12 000  10 500";
    let parse = parse_pdf_text(text, "2026-03-31", None);
    let got: Vec<&str> = parse.facts.iter().map(|f| f.metric_key.as_str()).collect();
    assert!(
        got.contains(&"revenue"),
        "wrapped-label recovery not implemented"
    );
}
