//! T0.1 recall/precision ratchet (guardrail G-3, ADR 0077) for the
//! deterministic fundamentals extraction pipeline over the maintainer's
//! hand-labeled CBF ground truth.
//!
//! Where [`super::t7_cbf_corpus`] is a *structural* regression net (outcome
//! class + fact count per document), this harness grades **values**: each
//! `private/realdata/t7-cbf/ground_truth/*.json` file carries the facts a
//! human read off one (document, period)'s financial statements — catalog
//! `metric_key`s only, values in signed base units (double-pass: the agent
//! proposes, the owner verifies). The test runs the exact same read-only
//! pipeline resolution as the corpus test over each labeled document and
//! computes:
//!
//! - **recall** — labeled facts the pipeline emitted with a matching value;
//! - **precision** — emitted facts for *labeled* metrics that match the label
//!   (emitted facts for metrics nobody labeled are excluded and reported
//!   separately as `unlabeled_emitted`).
//!
//! Values compare as numbers under the pipeline's own [`Tolerance`] (0.5%
//! relative / 1 base-unit absolute), never as strings.
//!
//! **ARCHIVED ratchet (2026-08-05, #182).** ADR 0086 retired the deterministic
//! PDF-positional parser this harness graded (`RECALL_FLOOR`/`PRECISION_FLOOR`
//! below described *that* parser, not the current pipeline). The harness
//! stays runnable and prints its recall/precision, but the floor asserts are
//! gone — they no longer gate anything. Its successor is the #182
//! ESEF/positional ground-truth scorer
//! (`storage::tests::real_data_extraction::esef_positional_ground_truth_scores`,
//! `make realdata-gt-score` / the required-mode closure diagnostic
//! `make realdata-gt-check`) against a much larger hand-labeled corpus — a
//! DIAGNOSTIC over stored DB state, not a fresh-pipeline ratchet either
//! (floors deferred to measurement v2 per a 2026-08-05 methodology audit; see
//! docs/testing.md § "#182 ESEF / positional ground-truth scorer").
//!
//! **Inert in CI** — skips unless `BRAWLER_REAL_DB` + `BRAWLER_REAL_DATA_DIR`
//! point at the throwaway corpus copy and `ground_truth/` exists. Run it:
//!
//! ```text
//! make realdata-extraction-metrics
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

use super::*;
use crate::fundamentals::extraction::esef_package;
use crate::fundamentals::extraction::pipeline::{run_pipeline, PipelineInput};
use crate::fundamentals::validation::Tolerance;
use crate::report_diff::extraction::SourceFormat;

// ---------------------------------------------------------------------------
// ARCHIVED ratchet floors (G-3) — pinned at the measured baseline of
// 2026-07-08 after the owner's pass-2 decisions (EBITDA Operacyjna is a
// company-specific KPI, not plain `ebitda` — 3 labels removed; capex sign +
// net_profit total confirmed): recall 12/37 = 0.3243, precision 12/12 =
// 1.0000 over 3 labeled CBF SSF quarterlies (Q3 2023 flagged→0/11, Q3 2025
// 6/13, Q1 2026 6/13; deterministic PDF tier misses operating_profit/EPS/
// cash-flow lines and capex), minus 0.02 slack.
//
// RETIRED 2026-08-05 (ADR 0086: the PDF-positional parser these floors
// graded no longer exists). Kept only for the informational eprintln below —
// not enforced. Successor ratchet: #182 esef_positional_ground_truth_scores
// (docs/testing.md § "#182 ESEF / positional ground-truth scorer").
// ---------------------------------------------------------------------------
const RECALL_FLOOR: f64 = 0.30;
const PRECISION_FLOOR: f64 = 0.98;

/// One hand-labeled fact. `unit`, `statement`, `page`, and `why_uncertain`
/// exist in the JSON for the owner's second labeling pass; the harness only
/// grades `metric_key` + `value` (signed base units).
#[derive(Debug, Deserialize)]
struct LabeledFact {
    metric_key: String,
    value: String,
    #[serde(default)]
    uncertain: bool,
}

/// One ground-truth file: the facts a human read off one (document, period).
#[derive(Debug, Deserialize)]
struct GroundTruthDocument {
    /// Basename of the stored report file (matched as a suffix of
    /// `report_documents.local_path`).
    document_file: String,
    /// Ticker, matched case-insensitively.
    company: String,
    fiscal_year: i64,
    /// `Q1`/`H1`/`Q3`/`FY` — the `derive_report_period` vocabulary.
    period_type: String,
    facts: Vec<LabeledFact>,
}

/// Mirrors `Tolerance::accepts` / `cross_check_prior`'s residual convention
/// (same 3-line mirror as `super::real_data_extraction`), so a "match" here is
/// exactly what the pipeline's own validation gate would accept.
fn tolerance_accepts(tol: &Tolerance, actual: Decimal, expected: Decimal) -> bool {
    let residual = actual - expected;
    let scale = actual.abs().max(expected.abs());
    let allowed = (scale * tol.relative).max(tol.absolute);
    residual.abs() <= allowed
}

/// The immediately-prior period end (year decremented), same 3-line mirror of
/// the private `jobs::structured_extraction::prior_period_end` helper as
/// `super::real_data_extraction` carries.
fn prior_period_end(period_end: &str) -> Option<String> {
    let year: i64 = period_end.get(0..4)?.parse().ok()?;
    Some(format!("{:04}{}", year - 1, period_end.get(4..)?))
}

fn pct(matched: usize, total: usize) -> String {
    if total == 0 {
        "n/a".to_owned()
    } else {
        format!("{:.1}%", 100.0 * matched as f64 / total as f64)
    }
}

fn ratio(matched: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        matched as f64 / total as f64
    }
}

/// Resolve one stored document to pipeline inputs and run the read-only
/// pipeline — the same source resolution as `t7_cbf_corpus::run_new` (ESEF
/// report package → inner instance; bare xhtml → own bytes; else no
/// surviving tier — the PDF fact-extraction arm is retired, ADR 0086 dec. 1),
/// but returning the emitted facts themselves so values can be graded.
fn run_pipeline_facts(
    state: &AppState,
    company_id: &str,
    document: &ReportDocument,
    fiscal_year: i64,
    period_type: &str,
) -> (String, String, BTreeMap<String, Decimal>) {
    let period_end = match period_type {
        "Q1" => format!("{fiscal_year}-03-31"),
        "H1" => format!("{fiscal_year}-06-30"),
        "Q3" => format!("{fiscal_year}-09-30"),
        _ => format!("{fiscal_year}-12-31"),
    };
    let local_path = document
        .local_path
        .as_deref()
        .expect("labeled document must have a stored file (local_path)");
    let bytes =
        std::fs::read(state.data_dir().join(local_path)).expect("read labeled document bytes");

    // The PDF fact-extraction arm is retired (ADR 0086 dec. 1): a document
    // that is neither an ESEF report package nor bare xhtml has no
    // surviving tier to feed.
    let ct = document.content_type.as_deref();
    let esef: Option<Vec<u8>> = if esef_package::is_report_package(local_path, &bytes) {
        esef_package::extract_instance(&bytes)
    } else if SourceFormat::resolve(ct, local_path) == SourceFormat::Xhtml {
        Some(bytes.clone())
    } else {
        None
    };

    let prior_end = prior_period_end(&period_end);
    let prior = state
        .financials()
        .stored_fact_set(company_id, fiscal_year - 1, period_type)
        .expect("stored fact set");

    let input = PipelineInput {
        period_end: &period_end,
        esef_bytes: esef.as_deref(),
        prior: prior.as_ref(),
        prior_period_end: prior_end.as_deref(),
        expected_keys: None,
    };
    let out = run_pipeline(&input);
    let mut emitted = BTreeMap::new();
    for fact in &out.facts {
        emitted.insert(fact.metric_key.to_string(), fact.value);
    }
    (
        out.acceptance.as_str().to_owned(),
        out.tier
            .map(|t| t.as_str().to_owned())
            .unwrap_or_else(|| "-".to_owned()),
        emitted,
    )
}

/// G-3 recall/precision ratchet over the hand-labeled CBF ground truth
/// (docs/testing.md § Ground-truth metrics). **Inert in CI.**
#[test]
#[ignore = "real-data metrics ratchet; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR + ground_truth/ (see docs/testing.md)"]
fn extraction_metrics_recall_precision_ratchet() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP extraction_metrics: set BRAWLER_REAL_DB to a throwaway corpus copy");
        return;
    };
    let Ok(data_dir) = std::env::var("BRAWLER_REAL_DATA_DIR") else {
        eprintln!("SKIP extraction_metrics: set BRAWLER_REAL_DATA_DIR to the corpus data dir");
        return;
    };
    let ground_truth_dir = PathBuf::from(&data_dir).join("ground_truth");
    let Ok(entries) = std::fs::read_dir(&ground_truth_dir) else {
        eprintln!(
            "SKIP extraction_metrics: no ground-truth dir at {}",
            ground_truth_dir.display()
        );
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    if files.is_empty() {
        eprintln!(
            "SKIP extraction_metrics: ground-truth dir {} holds no *.json",
            ground_truth_dir.display()
        );
        return;
    }

    let connection = open_database(&db_path).expect("open real db");
    let state = AppState::with_data_dir(connection, PathBuf::from(&data_dir));
    let companies = state.list_companies().expect("list companies");
    let tol = Tolerance::default();

    let mut total_labeled = 0usize;
    let mut total_matched = 0usize;
    let mut total_emitted_for_labeled = 0usize;
    let mut total_unlabeled_emitted = 0usize;

    eprintln!(
        "== extraction metrics: {} labeled document(s) in {} ==",
        files.len(),
        ground_truth_dir.display()
    );

    for path in &files {
        let raw = std::fs::read_to_string(path).expect("read ground-truth file");
        let doc: GroundTruthDocument = serde_json::from_str(&raw).unwrap_or_else(|e| {
            panic!("ground-truth {} must parse: {e}", path.display());
        });

        let company = companies
            .iter()
            .find(|c| c.ticker.eq_ignore_ascii_case(&doc.company))
            .unwrap_or_else(|| panic!("no company with ticker {}", doc.company));
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");
        let report_document = documents
            .iter()
            .find(|rd| {
                rd.local_path
                    .as_deref()
                    .is_some_and(|lp| lp.ends_with(&doc.document_file))
            })
            .unwrap_or_else(|| {
                panic!(
                    "no report_documents row with local_path ending in {}",
                    doc.document_file
                )
            });

        // The pipeline's own period derivation must agree with the label —
        // a drift here is an extraction defect, not a harness detail.
        let derived =
            crate::jobs::structured_extraction::derive_report_period(&state, report_document);
        let period_ok = derived
            .as_ref()
            .is_some_and(|(fy, pt, _)| *fy == doc.fiscal_year && *pt == doc.period_type);

        let (acceptance, tier, emitted) = run_pipeline_facts(
            &state,
            &company.id,
            report_document,
            doc.fiscal_year,
            &doc.period_type,
        );

        eprintln!(
            "\n-- {} ({} {} {})  tier={tier} acceptance={acceptance} derived_period={} --",
            doc.document_file,
            doc.company,
            doc.period_type,
            doc.fiscal_year,
            derived
                .as_ref()
                .map(|(fy, pt, _)| format!("{pt} {fy}{}", if period_ok { "" } else { " MISMATCH" }))
                .unwrap_or_else(|| "NONE".to_owned()),
        );
        eprintln!(
            "{:<28} | {:>16} | {:>16} | match",
            "metric_key", "labeled", "extracted"
        );

        let mut labeled = 0usize;
        let mut matched = 0usize;
        let mut emitted_for_labeled = 0usize;
        for fact in &doc.facts {
            labeled += 1;
            let expected = Decimal::from_str(&fact.value).unwrap_or_else(|e| {
                panic!("bad decimal '{}' for {}: {e}", fact.value, fact.metric_key)
            });
            let key_label = if fact.uncertain {
                format!("{} (uncertain)", fact.metric_key)
            } else {
                fact.metric_key.clone()
            };
            match emitted.get(&fact.metric_key) {
                Some(&actual) => {
                    emitted_for_labeled += 1;
                    let ok = tolerance_accepts(&tol, actual, expected);
                    if ok {
                        matched += 1;
                    }
                    eprintln!(
                        "{key_label:<28} | {expected:>16} | {actual:>16} | {}",
                        if ok { "OK" } else { "MISMATCH" }
                    );
                }
                None => {
                    eprintln!("{key_label:<28} | {expected:>16} | {:>16} | MISS", "-");
                }
            }
        }
        let unlabeled: Vec<&String> = emitted
            .keys()
            .filter(|k| !doc.facts.iter().any(|f| &f.metric_key == *k))
            .collect();
        if !unlabeled.is_empty() {
            eprintln!(
                "unlabeled_emitted ({}): {}",
                unlabeled.len(),
                unlabeled
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        eprintln!(
            "doc recall={} ({matched}/{labeled})  precision={} ({matched}/{emitted_for_labeled})",
            pct(matched, labeled),
            pct(matched, emitted_for_labeled),
        );

        total_labeled += labeled;
        total_matched += matched;
        total_emitted_for_labeled += emitted_for_labeled;
        total_unlabeled_emitted += unlabeled.len();
    }

    let recall = ratio(total_matched, total_labeled);
    let precision = ratio(total_matched, total_emitted_for_labeled);
    eprintln!(
        "\n== overall: recall={:.4} ({total_matched}/{total_labeled})  precision={:.4} ({total_matched}/{total_emitted_for_labeled})  unlabeled_emitted={total_unlabeled_emitted} ==",
        recall, precision
    );
    eprintln!(
        "== ARCHIVED floors (retired PDF parser, ADR 0086, no longer enforced): \
         recall>={RECALL_FLOOR}  precision>={PRECISION_FLOOR} — successor ratchet: #182 \
         esef_positional_ground_truth_scores =="
    );

    assert!(
        total_labeled > 0,
        "ground truth resolved but carries no labeled facts"
    );
    // ARCHIVED (ADR 0086 retired the deterministic PDF-positional parser this
    // ratchet graded): the floor asserts are intentionally gone. This harness
    // stays runnable and informational only — it prints recall/precision
    // above but no longer gates on them. See the module doc comment for the
    // successor ratchet.
}
