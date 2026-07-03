//! Real-data recall/precision harness for the structured fundamentals
//! extraction pipeline (Radicle 380024f; ADR 0061 guardrail — "a `#[ignore]`
//! real-data harness measures recall/precision on the owner's filings before
//! any default flip"; also the gate for the ADR 0060 decision-3 document-tier
//! default-model change).
//!
//! **Inert in CI** — like [`super::autopilot::autopilot_real_data_validation`],
//! it skips unless `BRAWLER_REAL_DB` points at a throwaway copy of the
//! maintainer's real DB, so `make check` never runs it.
//!
//! Where `autopilot_real_data_validation` is a pipeline *smoke* (drains the
//! durable queue, asserts a terminal state), this harness measures *accuracy*:
//! for a small hand-labeled ground-truth set of real report documents, it
//! calls the pure [`run_pipeline`] directly (no job queue, no persistence,
//! same as [`crate::jobs::structured_extraction::run_structured_extraction`]
//! minus the write side) and compares the emitted facts against the labels.
//!
//! Run it manually:
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
//!   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
//!   cargo test -p brawler --lib real_data_extraction_recall_precision -- --ignored --nocapture
//! ```
//!
//! See `docs/testing.md` for the ground-truth JSON format and the env-var
//! contract.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

use super::*;
use crate::fundamentals::extraction::pipeline::{run_pipeline, PipelineInput};
use crate::fundamentals::validation::Tolerance;
use crate::report_diff::extraction::{extract_report, SourceFormat};

// ---------------------------------------------------------------------------
// Ground-truth JSON shape
// ---------------------------------------------------------------------------

/// One labeled report document. See `docs/testing.md` for the on-disk shape
/// and a worked example.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruthDocument {
    /// Company ticker (matched case-insensitively against `companies.ticker`).
    ticker: String,
    #[serde(rename = "match")]
    doc_match: DocumentMatch,
    /// ISO `YYYY-MM-DD` period end the labeled facts belong to.
    period_end: String,
    /// `Q1`/`Q2`/`Q3`/`H1`/`FY` etc — matches `financial_periods.period_type`.
    period_type: String,
    fiscal_year: i64,
    /// `metric_key` -> expected value, as a decimal string in signed base units.
    facts: BTreeMap<String, String>,
}

/// Identifies which `report_documents` row a ground-truth entry labels.
/// Either field may be set; at least one must be for the entry to resolve.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DocumentMatch {
    url_contains: Option<String>,
    content_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruth {
    documents: Vec<GroundTruthDocument>,
}

fn document_matches(document: &ReportDocument, spec: &DocumentMatch) -> bool {
    if let Some(needle) = spec.url_contains.as_deref() {
        if document.url.contains(needle) {
            return true;
        }
    }
    if let Some(hash) = spec.content_hash.as_deref() {
        if document.content_hash.as_deref() == Some(hash) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Small local mirrors of private pipeline helpers
// ---------------------------------------------------------------------------

/// Mirrors `jobs::structured_extraction::prior_period_end` (private to that
/// module): the immediately-prior period's end date, by decrementing the
/// leading year. Duplicated here rather than exported, since it is a 3-line
/// pure helper and this harness must not widen the production API surface.
fn prior_period_end(period_end: &str) -> Option<String> {
    let year: i64 = period_end.get(0..4)?.parse().ok()?;
    Some(format!("{:04}{}", year - 1, period_end.get(4..)?))
}

/// Mirrors `Tolerance::accepts` (private to `fundamentals::validation`) and
/// `cross_check_prior`'s residual/scale convention (`residual = actual -
/// expected`, `scale = actual.abs().max(expected.abs())`) exactly, so a
/// "match" here means what the pipeline's own validation gate would accept.
fn tolerance_accepts(tol: &Tolerance, actual: Decimal, expected: Decimal) -> bool {
    let residual = actual - expected;
    let scale = actual.abs().max(expected.abs());
    let allowed = (scale * tol.relative).max(tol.absolute);
    residual.abs() <= allowed
}

fn pct(matched: usize, total: usize) -> String {
    if total == 0 {
        "n/a".to_owned()
    } else {
        format!("{:.1}%", 100.0 * matched as f64 / total as f64)
    }
}

#[derive(Default)]
struct TierStats {
    matched: usize,
    labeled: usize,
    emitted_for_labeled: usize,
}

/// Real-data recall/precision harness (CLAUDE.md real-data-validation-
/// precedes-implementation guardrail; ADR 0061 guardrail). **Inert in CI** —
/// skips unless `BRAWLER_REAL_DB` and a ground-truth file are present.
///
/// `BRAWLER_REAL_DATA_DIR` points at the Tauri data dir holding the actual
/// fetched report files (same convention as `autopilot_real_data_validation`);
/// without it, `AppState::new` falls back to a temp data dir, so every
/// document fails to resolve (the sanity assert below then fails loudly
/// rather than silently reporting 0/0).
///
/// `BRAWLER_GROUND_TRUTH` overrides the ground-truth path (default
/// `private/realdata/ground_truth.json`, resolved relative to the repo root).
///
/// This is a harness-sanity gate only (no quality floor): it asserts the
/// ground truth is reachable and *some* labeled fact matched. Precision/
/// recall thresholds are a deliberate follow-up before any default flip
/// (ADR 0061 guardrail; ADR 0060 decision 3) — see `docs/testing.md`.
#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR + a ground-truth file"]
fn real_data_extraction_recall_precision() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_extraction_recall_precision: set BRAWLER_REAL_DB to a throwaway copy"
        );
        return;
    };

    let ground_truth_path = std::env::var("BRAWLER_GROUND_TRUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("private/realdata/ground_truth.json")
        });
    let Ok(raw) = std::fs::read_to_string(&ground_truth_path) else {
        eprintln!(
            "SKIP real_data_extraction_recall_precision: no ground-truth file at {}",
            ground_truth_path.display()
        );
        return;
    };
    let ground_truth: GroundTruth =
        serde_json::from_str(&raw).expect("ground-truth JSON must parse (see docs/testing.md)");
    if ground_truth.documents.is_empty() {
        eprintln!(
            "SKIP real_data_extraction_recall_precision: ground truth at {} has no documents",
            ground_truth_path.display()
        );
        return;
    }

    // open_database applies migrations, same as autopilot_real_data_validation.
    let connection = open_database(&db_path).expect("open real db");
    // Point at the real Tauri data dir so report bytes can be read (same
    // convention as autopilot_real_data_validation's BRAWLER_REAL_DATA_DIR).
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };

    let companies = state.list_companies().expect("list companies");
    let tol = Tolerance::default();

    let mut resolved = 0usize;
    let mut total_labeled = 0usize;
    let mut total_matched = 0usize;
    let mut total_emitted_for_labeled = 0usize;
    let mut total_unlabeled_emitted = 0usize;
    let mut tier_stats: BTreeMap<&'static str, TierStats> = BTreeMap::new();

    eprintln!(
        "== real-data extraction recall/precision: {} labeled document(s) ==",
        ground_truth.documents.len()
    );

    for doc in &ground_truth.documents {
        let label = format!("{} {}", doc.ticker, doc.period_end);

        if doc.doc_match.url_contains.is_none() && doc.doc_match.content_hash.is_none() {
            eprintln!("SKIP {label}: ground-truth entry has neither urlContains nor contentHash");
            continue;
        }

        let Some(company) = companies
            .iter()
            .find(|c| c.ticker.eq_ignore_ascii_case(&doc.ticker))
        else {
            eprintln!("SKIP {label}: no company with ticker {}", doc.ticker);
            continue;
        };

        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");
        let Some(report_document) = documents
            .iter()
            .find(|rd| document_matches(rd, &doc.doc_match))
        else {
            eprintln!(
                "SKIP {label}: no report_documents row matched {:?}",
                doc.doc_match
            );
            continue;
        };

        let Some(local_path) = report_document.local_path.as_deref() else {
            eprintln!("SKIP {label}: matched report document has no stored file (local_path)");
            continue;
        };
        let path = state.data_dir().join(local_path);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("SKIP {label}: failed to read {}: {err}", path.display());
                continue;
            }
        };

        let format = SourceFormat::resolve(report_document.content_type.as_deref(), local_path);
        let (esef_bytes, pdf_text): (Option<Vec<u8>>, Option<String>) = match format {
            SourceFormat::Xhtml => (Some(bytes), None),
            SourceFormat::Pdf => {
                let extracted = extract_report(&bytes, SourceFormat::Pdf);
                let text = extracted
                    .sections
                    .iter()
                    .map(|s| s.body.clone())
                    .collect::<Vec<_>>()
                    .join("\n");
                (None, Some(text))
            }
        };

        // The honest live configuration: whatever profile/prior-period data
        // this company already has, exactly as run_structured_extraction reads it.
        let profile = state
            .fundamentals_provenance()
            .get_profile(&company.id)
            .expect("get profile");
        let prior_end = prior_period_end(&doc.period_end);
        let prior = state
            .financials()
            .stored_fact_set(&company.id, doc.fiscal_year - 1, &doc.period_type)
            .expect("stored fact set");

        let input = PipelineInput {
            period_end: &doc.period_end,
            esef_bytes: esef_bytes.as_deref(),
            pdf_text: pdf_text.as_deref(),
            profile: profile.as_ref(),
            prior: prior.as_ref(),
            prior_period_end: prior_end.as_deref(),
            expected_keys: None,
            witness: None,
        };
        let outcome = run_pipeline(&input);

        let mut emitted: BTreeMap<&str, Decimal> = BTreeMap::new();
        for fact in &outcome.facts {
            emitted.insert(&fact.metric_key, fact.value);
        }

        let mut labeled = 0usize;
        let mut matched = 0usize;
        let mut emitted_for_labeled = 0usize;
        for (key, expected_str) in &doc.facts {
            labeled += 1;
            let expected = Decimal::from_str(expected_str)
                .unwrap_or_else(|e| panic!("bad decimal '{expected_str}' for {key}: {e}"));
            match emitted.get(key.as_str()) {
                Some(&actual) => {
                    emitted_for_labeled += 1;
                    if tolerance_accepts(&tol, actual, expected) {
                        matched += 1;
                    } else {
                        eprintln!("  MISMATCH {key}: expected {expected} got {actual}");
                    }
                }
                None => eprintln!("  MISSING  {key}: expected {expected}"),
            }
        }
        let unlabeled_emitted = emitted
            .keys()
            .filter(|k| !doc.facts.contains_key(**k))
            .count();

        let tier = outcome.tier.map(|t| t.as_str()).unwrap_or("none");
        eprintln!(
            "{label:<24} tier={tier:<16} acceptance={:<20} recall={:>7} ({matched}/{labeled})  precision={:>7} ({matched}/{emitted_for_labeled})  unlabeled_emitted={unlabeled_emitted}",
            outcome.acceptance.as_str(),
            pct(matched, labeled),
            pct(matched, emitted_for_labeled),
        );

        let entry = tier_stats.entry(tier).or_default();
        entry.matched += matched;
        entry.labeled += labeled;
        entry.emitted_for_labeled += emitted_for_labeled;

        resolved += 1;
        total_labeled += labeled;
        total_matched += matched;
        total_emitted_for_labeled += emitted_for_labeled;
        total_unlabeled_emitted += unlabeled_emitted;
    }

    eprintln!("-- per tier --");
    for (tier, stats) in &tier_stats {
        eprintln!(
            "{tier:<16} recall={:>7} ({}/{})  precision={:>7} ({}/{})",
            pct(stats.matched, stats.labeled),
            stats.matched,
            stats.labeled,
            pct(stats.matched, stats.emitted_for_labeled),
            stats.matched,
            stats.emitted_for_labeled,
        );
    }
    eprintln!(
        "-- overall: resolved={}/{} documents  recall={} ({total_matched}/{total_labeled})  precision={} ({total_matched}/{total_emitted_for_labeled})  unlabeled_emitted={total_unlabeled_emitted} --",
        resolved,
        ground_truth.documents.len(),
        pct(total_matched, total_labeled),
        pct(total_matched, total_emitted_for_labeled),
    );

    // Harness-sanity only (no quality floor yet — see docs/testing.md).
    assert!(
        resolved > 0,
        "expected at least one ground-truth document to resolve to a real report file \
         (check BRAWLER_REAL_DATA_DIR, tickers, and match specs)"
    );
    assert!(
        total_matched > 0,
        "overall recall must be > 0 across the resolved documents (got {total_matched}/{total_labeled})"
    );
}
