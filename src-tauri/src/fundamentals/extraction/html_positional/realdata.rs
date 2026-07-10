//! F6b real-data grading harness (guardrail G-3 shape, ADR 0077) for the
//! pdf2htmlEX positional parser, graded against the maintainer's hand-labeled CD
//! PROJEKT ground truth (`private/realdata/t7-cdr/ground_truth/*.json`).
//!
//! **Inert in CI** — `#[ignore]` and env-gated: it skips unless
//! `BRAWLER_CDR_DOCS_DIR` points at a directory holding the three CDR interim
//! XHTML renders (the owner's data dir; the files are too large to vendor into
//! the corpus copy). Ground truth is read from `BRAWLER_CDR_GT_DIR`, or the
//! repo's `private/realdata/t7-cdr/ground_truth` by default.
//!
//! Run it (from `src-tauri/`):
//! ```text
//! BRAWLER_CDR_DOCS_DIR=/mnt/d/Brawler/Builds/latest/data/report_documents \
//!   cargo nextest run -p brawler html_positional::realdata --run-ignored all --no-capture
//! ```
//!
//! It grades each file with the parser's automatic current-period column
//! selection (ADR 0077 T-B2) — no caller-supplied value index — printing per-file
//! and overall recall/precision. `metric_key`s whose ground-truth value is a
//! *computed composite* not printed as a line (`total_liabilities` = non-current +
//! current liabilities) are reported separately as structurally-unreachable, not
//! counted against the printed-line recall.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

use super::{parse_html_positional, PositionalColumn};

/// Composites the parser maps no printed line to; reported, not counted as a
/// printed-line miss.
const COMPOSITE_METRICS: &[&str] = &["total_liabilities"];

#[derive(Debug, Deserialize)]
struct LabeledFact {
    metric_key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct GroundTruthDocument {
    document_file: String,
    #[allow(dead_code)]
    company: String,
    fiscal_year: i64,
    period_type: String,
    facts: Vec<LabeledFact>,
}

fn gt_dir() -> PathBuf {
    if let Ok(d) = std::env::var("BRAWLER_CDR_GT_DIR") {
        return PathBuf::from(d);
    }
    // src-tauri/ manifest → repo root → private/realdata/...
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("private/realdata/t7-cdr/ground_truth")
}

fn period_end(fiscal_year: i64, period_type: &str) -> String {
    match period_type {
        "Q1" => format!("{fiscal_year}-03-31"),
        "H1" => format!("{fiscal_year}-06-30"),
        "Q3" => format!("{fiscal_year}-09-30"),
        _ => format!("{fiscal_year}-12-31"),
    }
}

/// 0.5% relative / 1 base-unit absolute — the pipeline's own `Tolerance`.
fn matches(actual: Decimal, expected: Decimal) -> bool {
    let residual = (actual - expected).abs();
    let scale = actual.abs().max(expected.abs());
    let allowed = (scale * Decimal::new(5, 3)).max(Decimal::ONE);
    residual <= allowed
}

struct Graded {
    recall_printed: (usize, usize),
    precision: (usize, usize),
    hits: Vec<String>,
    misses: Vec<String>,
    wrong: Vec<(String, String, String)>,
    composites: Vec<String>,
}

fn grade(html: &str, gt: &GroundTruthDocument) -> Graded {
    let column = PositionalColumn::new(period_end(gt.fiscal_year, &gt.period_type));
    let facts = parse_html_positional(html, &column);
    let got: BTreeMap<String, Decimal> =
        facts.into_iter().map(|f| (f.metric_key, f.value)).collect();

    let labeled: BTreeMap<String, Decimal> = gt
        .facts
        .iter()
        .map(|f| (f.metric_key.clone(), Decimal::from_str(&f.value).unwrap()))
        .collect();

    let (mut hits, mut misses, mut wrong, mut composites) = (vec![], vec![], vec![], vec![]);
    let mut printed_total = 0usize;
    for (mk, ev) in &labeled {
        if COMPOSITE_METRICS.contains(&mk.as_str()) {
            composites.push(mk.clone());
            continue;
        }
        printed_total += 1;
        match got.get(mk) {
            Some(av) if matches(*av, *ev) => hits.push(mk.clone()),
            Some(av) => wrong.push((mk.clone(), av.to_string(), ev.to_string())),
            None => misses.push(mk.clone()),
        }
    }
    // Precision over emitted facts for *labeled* metrics.
    let emitted_labeled: Vec<&String> = got.keys().filter(|k| labeled.contains_key(*k)).collect();
    let precision_hits = emitted_labeled
        .iter()
        .filter(|k| matches(got[**k], labeled[**k]))
        .count();

    Graded {
        recall_printed: (hits.len(), printed_total),
        precision: (precision_hits, emitted_labeled.len()),
        hits,
        misses,
        wrong,
        composites,
    }
}

#[test]
#[ignore = "real-data spike harness; needs BRAWLER_CDR_DOCS_DIR"]
fn grade_cdr_positional_extraction() {
    let Ok(docs_dir) = std::env::var("BRAWLER_CDR_DOCS_DIR") else {
        eprintln!("SKIP: set BRAWLER_CDR_DOCS_DIR to the dir holding the CDR interim XHTML files");
        return;
    };
    let docs_dir = PathBuf::from(docs_dir);
    let gt_dir = gt_dir();
    assert!(
        gt_dir.is_dir(),
        "ground-truth dir not found: {}",
        gt_dir.display()
    );

    let files = [
        "cdr-2024-q3-ssf.json",
        "cdr-2025-h1-ssf.json",
        "cdr-2025-q1-ssf.json",
    ];

    let (mut ov_rh, mut ov_rt, mut ov_ph, mut ov_pt) = (0usize, 0usize, 0usize, 0usize);
    for gt_name in files {
        let gt: GroundTruthDocument =
            serde_json::from_str(&std::fs::read_to_string(gt_dir.join(gt_name)).unwrap()).unwrap();
        let doc_path = docs_dir.join(&gt.document_file);
        assert!(
            doc_path.is_file(),
            "document not found: {}",
            doc_path.display()
        );
        let html = std::fs::read_to_string(&doc_path).unwrap();

        println!(
            "\n=== {} ({} {}) ===",
            gt_name, gt.period_type, gt.fiscal_year
        );
        // Current-period column selection is automatic (ADR 0077 T-B2): the Q3
        // main-statement (index 2) and summary-table (index 0) layouts both
        // resolve to the current-YTD without a caller-supplied index.
        let g = grade(&html, &gt);
        let (rh, rt) = g.recall_printed;
        let (ph, pt) = g.precision;
        println!(
            "  recall(printed) {rh}/{rt}={:.2}  precision {ph}/{pt}={:.2}",
            rh as f64 / rt.max(1) as f64,
            ph as f64 / pt.max(1) as f64,
        );
        println!("    HIT:   {:?}", g.hits);
        println!("    MISS:  {:?}", g.misses);
        if !g.wrong.is_empty() {
            println!("    WRONG: {:?}", g.wrong);
        }
        println!(
            "    COMPOSITE (structurally unreachable): {:?}",
            g.composites
        );
        ov_rh += rh;
        ov_rt += rt;
        ov_ph += ph;
        ov_pt += pt;
    }
    let recall = ov_rh as f64 / ov_rt.max(1) as f64;
    let precision = ov_ph as f64 / ov_pt.max(1) as f64;
    println!(
        "\n=== OVERALL (printed lines) === recall {ov_rh}/{ov_rt}={recall:.3}  precision {ov_ph}/{ov_pt}={precision:.3}"
    );

    // Production ratchet (guardrail G-3, ADR 0077 T-B2). The F6b spike floored
    // these at 0.90 (graded 32/33 = 0.970 with a global column); statement-aware
    // selection lifts the measured result to 33/33 = 1.000 on the 3 CDR interims,
    // so the floor promotes to 0.99 — a single regressed line (→0.970) reddens.
    // Only move deliberately (CLAUDE.md rule 3).
    assert!(recall >= 0.99, "printed-line recall regressed: {recall:.3}");
    assert!(precision >= 0.99, "precision regressed: {precision:.3}");
}
