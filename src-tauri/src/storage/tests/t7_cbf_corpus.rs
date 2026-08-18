//! T7-C permanent corpus regression harness for the structured fundamentals
//! extraction pipeline over the maintainer's real CBF filings (owner T7 blocker:
//! the per-document "Wyciągnij dane" button yielded zero FY facts on the CBF
//! 2024/2025 annual reports — ESEF `.xbri` packages were misclassified as PDF and
//! never unpacked, and the dimension-blind iXBRL parse could not validate).
//!
//! **Inert in CI** — like [`super::real_data_extraction`] and
//! [`super::autopilot::autopilot_real_data_validation`], it skips unless
//! `BRAWLER_REAL_DB` + `BRAWLER_REAL_DATA_DIR` point at a throwaway copy of the
//! corpus. It reuses the read-only [`run_pipeline`] path (no writes), so it is
//! idempotent and order-independent.
//!
//! Unlike the recall/precision harness (which grades against hand-labeled fact
//! values), this one is a **structural regression net**: for every CBF document
//! it records the extraction *outcome class* (facts / empty / flagged /
//! no_period), the emitted fact count, and the derived period, and compares them
//! against a committed expectations baseline
//! (`t7_cbf_corpus_expectations.json`, next to this file — identifiers/titles +
//! outcome only, **no report content**).
//!
//! Semantics (same philosophy as the Playwright visual baseline):
//! - **Regression → FAIL**: a document that used to yield facts now yields fewer
//!   or none, an accepted document is now flagged/empty, a derived period drifts,
//!   or a pinned document disappeared.
//! - **Improvement → soft report**: a previously-empty document now yields facts
//!   (or more of them). The run prints the refreshed table and passes; re-run
//!   with `BRAWLER_UPDATE_EXPECTATIONS=1` to deliberately re-pin the baseline.
//!
//! Run it / refresh the baseline:
//!
//! ```text
//! make realdata-extraction-check              # compare against the baseline
//! BRAWLER_UPDATE_EXPECTATIONS=1 make realdata-extraction-check   # re-pin
//! ```
//!
//! See `docs/testing.md` (corpus layout + refresh procedure).

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;
use crate::fundamentals::extraction::esef::parse_esef;
use crate::fundamentals::extraction::pipeline::{run_pipeline, PipelineInput};
use crate::fundamentals::extraction::{esef_package, primary_period_end};
use crate::jobs::structured_extraction::{compute_layer1_generation, DocumentRoute};
use crate::report_diff::classify::period_sort_key;
use crate::report_diff::extraction::SourceFormat;

const CORPUS_TICKER: &str = "CBF";

/// One document's extraction outcome (the `new`, post-fix code path). `period`
/// is `"FY 2025"`-style or `"-"`; `outcome` is `no_period` or an
/// `Acceptance::as_str()` value; `facts` is the emitted count.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Outcome {
    period: String,
    outcome: String,
    tier: String,
    facts: usize,
}

/// A committed expectation row: content-free identifiers + the pinned outcome.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Expectation {
    id: String,
    title: String,
    period: String,
    outcome: String,
    facts: usize,
}

const ACCEPTED: &[&str] = &["accepted", "accepted_via_witness", "accepted_unreviewed"];

fn is_accepted(outcome: &str) -> bool {
    ACCEPTED.contains(&outcome)
}

fn period_label(p: &Option<(i64, &'static str, String)>) -> String {
    match p {
        Some((year, ptype, _)) => format!("{ptype} {year}"),
        None => "-".to_owned(),
    }
}

/// Build the read-only pipeline inputs for one document exactly as production
/// resolves them, then run the pure pipeline. Returns `(tier, acceptance, facts)`.
fn run_new(state: &AppState, company_id: &str, document: &ReportDocument) -> Outcome {
    let derived = crate::jobs::structured_extraction::derive_report_period(state, document);
    let Some((fiscal_year, period_type, period_end)) = derived.clone() else {
        return Outcome {
            period: period_label(&derived),
            outcome: "no_period".to_owned(),
            tier: "-".to_owned(),
            facts: 0,
        };
    };

    let Some(local_path) = document.local_path.as_deref() else {
        return Outcome {
            period: period_label(&derived),
            outcome: "no_period".to_owned(),
            tier: "-".to_owned(),
            facts: 0,
        };
    };
    let Ok(bytes) = std::fs::read(state.data_dir().join(local_path)) else {
        return Outcome {
            period: period_label(&derived),
            outcome: "no_period".to_owned(),
            tier: "-".to_owned(),
            facts: 0,
        };
    };

    // Same route resolution as production `route_document`/Layer 1 capture: a
    // ZIP report package reads its presentation linkbase + every instance; a
    // bare xhtml has no linkbase to read (so its facts get no role rows and
    // never survive Layer 2 projection — ADR 0100 decision 3); everything
    // else has no surviving tier (the PDF fact-extraction arm is retired,
    // ADR 0086 dec. 1).
    let ct = document.content_type.as_deref();
    let route = if esef_package::is_report_package(local_path, &bytes) {
        Some(DocumentRoute::ZipPackage)
    } else if SourceFormat::resolve(ct, local_path) == SourceFormat::Xhtml {
        Some(DocumentRoute::IxbrlInstance)
    } else {
        None
    };
    let generation = route.map(|r| compute_layer1_generation(&bytes, r));
    let has_presentation_linkbase = generation
        .as_ref()
        .is_some_and(|g| g.has_presentation_linkbase);
    let layer1_facts = generation.map(|g| g.facts);

    let outcome = run_pipeline_for(
        state,
        company_id,
        fiscal_year,
        period_type,
        &period_end,
        layer1_facts,
        has_presentation_linkbase,
    );
    Outcome {
        period: period_label(&derived),
        ..outcome
    }
}

/// The legacy (pre-T7-C) resolution, for the BEFORE column only: `SourceFormat`
/// alone (no ZIP unpacking), period from the raw xhtml/instance or the title.
fn run_legacy(state: &AppState, company_id: &str, document: &ReportDocument) -> Outcome {
    let Some(local_path) = document.local_path.as_deref() else {
        return Outcome {
            period: "-".into(),
            outcome: "no_period".into(),
            tier: "-".into(),
            facts: 0,
        };
    };
    let Ok(bytes) = std::fs::read(state.data_dir().join(local_path)) else {
        return Outcome {
            period: "-".into(),
            outcome: "no_period".into(),
            tier: "-".into(),
            facts: 0,
        };
    };
    let ct = document.content_type.as_deref();
    let format = SourceFormat::resolve(ct, local_path);

    // Legacy period derivation (the pre-T7 `derive_report_period`).
    let derived: Option<(i64, &'static str, String)> = match format {
        SourceFormat::Xhtml => parse_esef(&bytes).ok().and_then(|f| {
            let pe = primary_period_end(&f)?;
            let fy = pe.get(0..4).and_then(|y| y.parse::<i64>().ok())?;
            Some((fy, "FY", pe))
        }),
        SourceFormat::Pdf => {
            let title = document.title.as_deref().unwrap_or("");
            period_sort_key(title, &document.url).and_then(|(year, idx)| {
                let (pt, pe): (&'static str, String) = match idx {
                    1 => ("Q1", format!("{year}-03-31")),
                    2 => ("H1", format!("{year}-06-30")),
                    3 => ("Q3", format!("{year}-09-30")),
                    4 => ("FY", format!("{year}-12-31")),
                    _ => return None,
                };
                Some((i64::from(year), pt, pe))
            })
        }
    };
    let Some((fiscal_year, period_type, period_end)) = derived.clone() else {
        return Outcome {
            period: period_label(&derived),
            outcome: "no_period".into(),
            tier: "-".into(),
            facts: 0,
        };
    };
    // The PDF fact-extraction arm is retired (ADR 0086 dec. 1): a PDF-format
    // document has no surviving tier to feed. The legacy path never unpacked
    // a ZIP package (that was the T7-C fix) — bare-instance route mirrors it.
    let layer1_facts = match format {
        SourceFormat::Xhtml => {
            Some(compute_layer1_generation(&bytes, DocumentRoute::IxbrlInstance).facts)
        }
        SourceFormat::Pdf => None,
    };
    // `DocumentRoute::IxbrlInstance` never reads a linkbase (a bare instance
    // has none to read) — this legacy shim always routes bare, so it is
    // always `false`, matching the pre-epic selection this path models.
    let outcome = run_pipeline_for(
        state,
        company_id,
        fiscal_year,
        period_type,
        &period_end,
        layer1_facts,
        false,
    );
    Outcome {
        period: period_label(&derived),
        ..outcome
    }
}

fn run_pipeline_for(
    state: &AppState,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    layer1_facts: Option<Vec<crate::storage::NewTaggedFact>>,
    has_presentation_linkbase: bool,
) -> Outcome {
    let prior_end = {
        let y: i64 = period_end
            .get(0..4)
            .and_then(|s| s.parse().ok())
            .unwrap_or(fiscal_year);
        period_end
            .get(4..)
            .map(|rest| format!("{:04}{}", y - 1, rest))
    };
    let prior = state
        .financials()
        .stored_fact_set(company_id, fiscal_year - 1, period_type)
        .expect("stored fact set");
    let input = PipelineInput {
        period_end,
        layer1_facts: layer1_facts.as_deref(),
        has_presentation_linkbase,
        prior: prior.as_ref(),
        prior_period_end: prior_end.as_deref(),
        expected_keys: None,
    };
    let out = run_pipeline(&input);
    Outcome {
        period: String::new(), // filled by the caller
        outcome: out.acceptance.as_str().to_owned(),
        tier: out
            .tier
            .map(|t| t.as_str().to_owned())
            .unwrap_or_else(|| "-".to_owned()),
        facts: out.facts.len(),
    }
}

fn expectations_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/storage/tests/t7_cbf_corpus_expectations.json")
}

#[test]
#[ignore = "real-data corpus regression; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR (see docs/testing.md)"]
fn t7_cbf_corpus_extraction_regression() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP t7_cbf_corpus_extraction_regression: set BRAWLER_REAL_DB to a throwaway copy"
        );
        return;
    };
    let connection = open_database(&db_path).expect("open real db");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => {
            eprintln!("SKIP t7_cbf_corpus_extraction_regression: set BRAWLER_REAL_DATA_DIR to the corpus data dir");
            return;
        }
    };

    let companies = state.list_companies().expect("list companies");
    let Some(company) = companies
        .iter()
        .find(|c| c.ticker.eq_ignore_ascii_case(CORPUS_TICKER))
    else {
        eprintln!(
            "SKIP t7_cbf_corpus_extraction_regression: no company with ticker {CORPUS_TICKER}"
        );
        return;
    };
    let documents = state
        .list_report_documents_by_company(&company.id)
        .expect("list report documents");
    assert!(
        !documents.is_empty(),
        "expected the corpus to hold CBF report documents"
    );

    // Compute the current table (legacy BEFORE + new AFTER), sorted by id.
    let mut rows: Vec<(String, String, Outcome, Outcome)> = documents
        .iter()
        .map(|d| {
            let title = d.title.clone().unwrap_or_default();
            (
                d.id.clone(),
                title,
                run_legacy(&state, &company.id, d),
                run_new(&state, &company.id, d),
            )
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    // Print the BEFORE/AFTER table (only interesting rows: any doc that either
    // path resolved a period for, to keep the announcement noise out).
    eprintln!("== T7-C CBF corpus extraction (BEFORE vs AFTER) ==");
    eprintln!(
        "{:<52} | {:<22} | {:<22}",
        "document", "BEFORE (legacy)", "AFTER (fixed)"
    );
    for (_, title, legacy, new) in &rows {
        if legacy.outcome == "no_period" && new.outcome == "no_period" {
            continue;
        }
        let short: String = title.chars().take(50).collect();
        eprintln!(
            "{short:<52} | {:<22} | {:<22}",
            format!("{} {} f={}", legacy.period, legacy.outcome, legacy.facts),
            format!(
                "{} {}/{} f={}",
                new.period, new.outcome, new.tier, new.facts
            ),
        );
    }

    let current: Vec<Expectation> = rows
        .iter()
        .map(|(id, title, _, new)| Expectation {
            id: id.clone(),
            title: title.clone(),
            period: new.period.clone(),
            outcome: new.outcome.clone(),
            facts: new.facts,
        })
        .collect();

    let path = expectations_path();
    let update = std::env::var("BRAWLER_UPDATE_EXPECTATIONS")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let baseline: Option<Vec<Expectation>> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok());

    if update || baseline.is_none() {
        let json = serde_json::to_string_pretty(&current).expect("serialize expectations");
        std::fs::write(&path, json).expect("write expectations baseline");
        eprintln!(
            "\n{} baseline at {} ({} documents). Review the diff before committing.",
            if update { "REFRESHED" } else { "WROTE new" },
            path.display(),
            current.len()
        );
        return;
    }

    // ---- Compare live outcomes against the pinned baseline ----------------
    let baseline = baseline.unwrap();
    let expected: BTreeMap<&str, &Expectation> =
        baseline.iter().map(|e| (e.id.as_str(), e)).collect();
    let live: BTreeMap<&str, &Expectation> = current.iter().map(|e| (e.id.as_str(), e)).collect();

    let mut regressions: Vec<String> = Vec::new();
    let mut improvements: Vec<String> = Vec::new();

    for (id, exp) in &expected {
        match live.get(id) {
            None => regressions.push(format!(
                "{} [{}]: pinned document disappeared",
                exp.title, id
            )),
            Some(now) => {
                // Period drift on a doc that had one is a regression.
                if exp.period != "-" && exp.period != now.period {
                    regressions.push(format!(
                        "{} [{}]: period drifted {} -> {}",
                        exp.title, id, exp.period, now.period
                    ));
                }
                if now.facts < exp.facts {
                    regressions.push(format!(
                        "{} [{}]: fact count dropped {} -> {}",
                        exp.title, id, exp.facts, now.facts
                    ));
                } else if now.facts > exp.facts {
                    improvements.push(format!(
                        "{} [{}]: fact count grew {} -> {}",
                        exp.title, id, exp.facts, now.facts
                    ));
                }
                if is_accepted(&exp.outcome) && !is_accepted(&now.outcome) {
                    regressions.push(format!(
                        "{} [{}]: {} -> {} (accepted document downgraded)",
                        exp.title, id, exp.outcome, now.outcome
                    ));
                } else if !is_accepted(&exp.outcome) && is_accepted(&now.outcome) {
                    improvements.push(format!(
                        "{} [{}]: {} -> {} (newly accepted)",
                        exp.title, id, exp.outcome, now.outcome
                    ));
                }
            }
        }
    }
    let new_docs: Vec<&&Expectation> = live
        .iter()
        .filter(|(id, _)| !expected.contains_key(**id))
        .map(|(_, e)| e)
        .collect();

    if !improvements.is_empty() {
        eprintln!("\n-- IMPROVEMENTS (soft; re-pin with BRAWLER_UPDATE_EXPECTATIONS=1) --");
        for i in &improvements {
            eprintln!("  + {i}");
        }
    }
    if !new_docs.is_empty() {
        eprintln!("\n-- NEW documents not in the baseline (soft; re-pin to track) --");
        for e in &new_docs {
            eprintln!(
                "  + {} [{}]: {} {} f={}",
                e.title, e.id, e.period, e.outcome, e.facts
            );
        }
    }
    if !regressions.is_empty() {
        eprintln!("\n-- REGRESSIONS --");
        for r in &regressions {
            eprintln!("  ! {r}");
        }
    }

    // Anchor assertion: the FY2025 ESEF `.xbri` annual filing must yield facts —
    // this is the exact owner-reported blocker; if it ever stops, fail loudly.
    let xbri_fy2025 = current.iter().find(|e| {
        e.title.to_lowercase().contains("2025-12-31") && e.title.to_lowercase().ends_with(".xbri")
    });
    if let Some(e) = xbri_fy2025 {
        assert!(
            e.facts > 0 && e.period == "FY 2025",
            "the CBF FY2025 ESEF .xbri annual filing must yield FY2025 facts (got {e:?})"
        );
    }

    assert!(
        regressions.is_empty(),
        "{} extraction regression(s) vs the pinned baseline (see the list above)",
        regressions.len()
    );
}

/// T7-F anchor on the real corpus: extracting the same stored document twice
/// through the full **write path** (`run_structured_extraction`, the exact code
/// behind the "Wyciągnij dane" button) must be idempotent — the owner's second
/// click failed with `UNIQUE constraint failed: financial_facts...` because the
/// insert had no re-observation policy. The second run must succeed, create
/// nothing new, report every slot as skipped, and leave the fact table
/// unchanged. Writes to the DB — the harness already mandates a THROWAWAY copy.
#[test]
#[ignore = "real-data corpus regression; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR (see docs/testing.md)"]
fn t7_cbf_double_extraction_is_idempotent_on_the_real_corpus() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP t7_cbf_double_extraction: set BRAWLER_REAL_DB to a throwaway copy");
        return;
    };
    let connection = open_database(&db_path).expect("open real db");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => {
            eprintln!("SKIP t7_cbf_double_extraction: set BRAWLER_REAL_DATA_DIR to the corpus dir");
            return;
        }
    };
    let companies = state.list_companies().expect("list companies");
    let Some(company) = companies
        .iter()
        .find(|c| c.ticker.eq_ignore_ascii_case(CORPUS_TICKER))
    else {
        eprintln!("SKIP t7_cbf_double_extraction: no company with ticker {CORPUS_TICKER}");
        return;
    };
    // The owner's exact re-click target: the FY2025 ESEF `.xbri` annual filing.
    let documents = state
        .list_report_documents_by_company(&company.id)
        .expect("list report documents");
    let Some(document) = documents.iter().find(|d| {
        d.local_path.is_some()
            && d.title
                .as_deref()
                .is_some_and(|t| t.to_lowercase().ends_with(".xbri"))
    }) else {
        eprintln!("SKIP t7_cbf_double_extraction: no stored .xbri document in the corpus");
        return;
    };
    let (fiscal_year, period_type, period_end) =
        crate::jobs::structured_extraction::derive_report_period(&state, document)
            .expect("the .xbri filing must derive a period");

    let run = |label: &str| {
        crate::jobs::structured_extraction::run_structured_extraction(
            &state,
            &company.id,
            &document.id,
            fiscal_year,
            period_type,
            &period_end,
            MODE_AUTOPILOT,
        )
        .unwrap_or_else(|e| panic!("{label} extraction must not error (owner T7-F): {e}"))
    };

    let count_facts = || {
        state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company.id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list financial facts")
            .len()
    };

    let first = run("first");
    let facts_after_first = count_facts();

    let second = run("second");
    let facts_after_second = count_facts();

    assert!(
        second.produced_fact_ids.is_empty(),
        "a re-extraction must create no new facts (got {})",
        second.produced_fact_ids.len()
    );
    assert!(
        !second.skipped_fact_ids.is_empty(),
        "a re-extraction must report its re-observed slots as skipped"
    );
    assert_eq!(
        facts_after_first, facts_after_second,
        "the fact table must be unchanged by a re-extraction"
    );
    eprintln!(
        "double-extraction OK: first produced={} skipped={}, second produced=0 skipped={} divergent={}",
        first.produced_fact_ids.len(),
        first.skipped_fact_ids.len(),
        second.skipped_fact_ids.len(),
        second.divergences.len(),
    );
}
