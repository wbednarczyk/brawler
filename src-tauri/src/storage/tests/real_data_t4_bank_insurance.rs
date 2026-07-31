//! Real-data verification harness for epic #277 T4 (banking/insurance
//! extraction chain end-to-end, on a throwaway copy of the owner's DB).
//!
//! Scoped narrowly to three companies whose T1-T3 groundwork this checks
//! against real filings rather than mocks (CLAUDE.md "mocks are never
//! completion evidence"):
//!
//! - **PZU** — a `.zip` ESEF package stored at `doc_kind='other'` (a silent
//!   title, #276's package-shape marker fix should reclassify it once
//!   `reclassify_report_documents` — the same production path the UI's
//!   "Reclassify" action and `financials::reclassify_report_documents`
//!   command run — walks the corpus). Then the production extraction path
//!   for that one document, timed.
//! - **KRU** — a `.xbri`-named file that is actually a ZIP under the hood
//!   (container truth, epic #229 T2). T3 found its instance carries none of
//!   our mapped concepts, so the honest expected outcome is a typed
//!   `reason_code`, not a crash or silent no-op.
//! - **PKO** — already `periodic_ssf`/fetched; a cheap sanity pass over the
//!   same T3 bank-mapping arms PEO exercises, feeding T5's ground truth.
//!
//! **Inert in CI** — like every `real_data_*` harness, skips loudly unless
//! `BRAWLER_REAL_DB` (a throwaway copy) and `BRAWLER_REAL_DATA_DIR` are set.
//! Writes facts/outcomes/doc_kind updates to the copy, so it refuses the
//! master snapshot and the live application database (same guard as
//! [`super::real_data_extraction::deterministic_pipeline_real_data_sweep`]).
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/t4-worktest.sqlite3
//! BRAWLER_REAL_DB=../private/realdata/t4-worktest.sqlite3 \
//!   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
//!   cargo nextest run -p brawler real_data_t4_bank_insurance_chain \
//!     --run-ignored all --no-capture
//! ```

use std::path::PathBuf;
use std::time::Instant;

use super::*;
use crate::jobs::structured_extraction::{derive_report_period, run_structured_extraction};

/// Finds the one report document whose URL ends with `url_suffix`, among a
/// company's stored documents. `ends_with` rather than `contains` on purpose:
/// the ESEF package's `.zip.xades` signature companion shares the whole
/// package filename as a PREFIX of its own URL, so a substring match picks
/// the wrong row (a companion with no stored bytes) whenever it sorts first.
/// Panics (loudly, not silently) if the fixture assumption this harness is
/// built on no longer holds — a harness whose document lookup fails silently
/// would report a false "no facts" outcome.
fn find_document(state: &AppState, company_id: &str, url_suffix: &str) -> ReportDocument {
    let documents = state
        .list_report_documents_by_company(company_id)
        .expect("list report documents");
    documents
        .into_iter()
        .find(|d| d.url.ends_with(url_suffix))
        .unwrap_or_else(|| panic!("no stored document for {company_id} matching {url_suffix:?}"))
}

fn find_company(state: &AppState, ticker: &str) -> crate::storage::Company {
    state
        .list_companies()
        .expect("list companies")
        .into_iter()
        .find(|c| c.ticker == ticker)
        .unwrap_or_else(|| panic!("no company for ticker {ticker}"))
}

/// Runs the production structured-extraction path for one already-fetched
/// document and reports the outcome. `None` fiscal-year derivation is itself
/// reported (not silently skipped) — a document the pipeline cannot place in
/// a period is a measured gap, per the sweep harness's doctrine.
fn extract_and_report(
    state: &AppState,
    company: &crate::storage::Company,
    document: &ReportDocument,
) {
    eprintln!(
        "-- {} doc={} title={:?} doc_kind={:?} local_path={:?}",
        company.ticker, document.id, document.title, document.doc_kind, document.local_path
    );
    let Some((fiscal_year, period_type, period_end)) = derive_report_period(state, document) else {
        eprintln!("   NO PERIOD DERIVED — extraction not attempted");
        return;
    };
    eprintln!("   period: FY{fiscal_year} {period_type} end={period_end}");

    let started = Instant::now();
    let result = run_structured_extraction(
        state,
        &company.id,
        &document.id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    );
    let elapsed = started.elapsed();

    match result {
        Ok(r) => {
            eprintln!(
                "   OUTCOME elapsed={:?} tier={:?} acceptance={:?} reason_code={:?} produced={} skipped={} divergences={}",
                elapsed,
                r.tier,
                r.acceptance,
                r.reason_code,
                r.produced_fact_ids.len(),
                r.skipped_fact_ids.len(),
                r.divergences.len()
            );

            let touched: Vec<String> = r
                .produced_fact_ids
                .iter()
                .chain(&r.skipped_fact_ids)
                .cloned()
                .collect();
            if touched.is_empty() {
                eprintln!("   facts landed: none");
            } else {
                let facts = state
                    .financials()
                    .list_financial_facts(ListFinancialFactsInput {
                        company_id: Some(company.id.clone()),
                        period_id: None,
                        definition_id: None,
                    })
                    .expect("list financial facts");
                let definitions = state
                    .financials()
                    .list_kpi_definitions(ListKpiDefinitionsInput {
                        scope: None,
                        sector: None,
                        company_id: None,
                    })
                    .expect("list kpi definitions");
                let metric_key_of = |definition_id: &str| -> String {
                    definitions
                        .iter()
                        .find(|d| d.id == definition_id)
                        .map(|d| d.metric_key.clone())
                        .unwrap_or_else(|| definition_id.to_owned())
                };
                for fact_id in &touched {
                    if let Some(fact) = facts.iter().find(|f| &f.id == fact_id) {
                        eprintln!(
                            "   facts landed: metric={} value={} currency={:?} basis={} extraction_method={}",
                            metric_key_of(&fact.definition_id),
                            fact.value_numeric,
                            fact.currency,
                            fact.statement_basis,
                            fact.extraction_method
                        );
                    }
                }
            }
        }
        Err(error) => {
            eprintln!("   ERROR elapsed={elapsed:?}: {error}");
        }
    }
}

#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB (a throwaway copy) + BRAWLER_REAL_DATA_DIR"]
fn real_data_t4_bank_insurance_chain() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_t4_bank_insurance_chain: set BRAWLER_REAL_DB to a THROWAWAY copy \
             of the owner's database (see private/realdata/README.md)"
        );
        return;
    };
    let Ok(data_dir) = std::env::var("BRAWLER_REAL_DATA_DIR") else {
        eprintln!(
            "SKIP real_data_t4_bank_insurance_chain: set BRAWLER_REAL_DATA_DIR to the Tauri \
             data dir holding the fetched report files"
        );
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP real_data_t4_bank_insurance_chain: no database at {db_path}");
        return;
    }
    // Same write-guard as every other writing real_data_* harness: this test
    // reclassifies documents and writes facts/outcomes, so it must never run
    // against the master snapshot or the live application database.
    let file_name = std::path::Path::new(&db_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    assert!(
        file_name != "brawler.sqlite3" && !db_path.starts_with("/mnt/d/"),
        "refusing to run: {db_path} is the master snapshot or the live application database. \
         This harness writes — copy it first (private/realdata/README.md)."
    );

    // Opening the connection applies every pending migration (including 0128
    // — the PEO WDF bank-fact repair) via the same `open_database` path the
    // whole app uses.
    let connection = open_database(&db_path).expect("open throwaway real db");
    let state = AppState::with_data_dir(connection, PathBuf::from(&data_dir));

    // ---- 1. Migration 0128: confirm the two wrong PEO facts are gone -------
    eprintln!("== migration 0128: PEO WDF bank-fact repair ==");
    let peo = find_company(&state, "PEO");
    let peo_facts_after = state
        .financials()
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(peo.id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list PEO financial facts");
    eprintln!(
        "   PEO total fact count after open (post-migration): {}",
        peo_facts_after.len()
    );
    let still_wrong: Vec<&FinancialFact> = peo_facts_after
        .iter()
        .filter(|f| {
            f.extraction_method == "espi_cover_note"
                && ((f.value_numeric == "7899000000") || (f.value_numeric == "12000000"))
        })
        .collect();
    eprintln!(
        "   wrong-value facts still present (must be 0): {}",
        still_wrong.len()
    );
    for f in &still_wrong {
        eprintln!("   STILL PRESENT: id={} value={}", f.id, f.value_numeric);
    }

    // ---- 2. PZU: package-shape reclassification --------------------------
    eprintln!("== PZU: package-shape reclassification ==");
    let pzu = find_company(&state, "PZU");
    let pzu_zip_before = find_document(&state, &pzu.id, "pzu-2025-12-31-1-pl.zip");
    eprintln!(
        "   before: doc_kind={:?} title={:?}",
        pzu_zip_before.doc_kind, pzu_zip_before.title
    );
    let summary = state
        .reclassify_report_documents()
        .expect("reclassify_report_documents");
    eprintln!(
        "   reclassify_all: total={} updated={} by_kind={:?}",
        summary.total, summary.updated, summary.by_kind
    );
    let pzu_zip_after = find_document(&state, &pzu.id, "pzu-2025-12-31-1-pl.zip");
    eprintln!("   after: doc_kind={:?}", pzu_zip_after.doc_kind);

    // ---- 3. PZU: production extraction end-to-end + timing ----------------
    eprintln!("== PZU: extraction ==");
    extract_and_report(&state, &pzu, &pzu_zip_after);

    // ---- 4. KRU: .xbri container-truth routing + honest outcome -----------
    eprintln!("== KRU: .xbri container-truth routing ==");
    let kru = find_company(&state, "KRU");
    let kru_doc = find_document(&state, &kru.id, "GRUPAKRUK-2025-12-31-1-pl.xbri");
    eprintln!(
        "   doc_kind={:?} local_path={:?}",
        kru_doc.doc_kind, kru_doc.local_path
    );
    let kru_route = crate::jobs::structured_extraction::is_esef_route(&kru_doc);
    eprintln!("   is_esef_route={kru_route}");
    extract_and_report(&state, &kru, &kru_doc);

    // ---- 5. PKO: sanity pass over T3's bank-mapping arms -------------------
    eprintln!("== PKO: sanity pass ==");
    let pko = find_company(&state, "PKO");
    let pko_doc = find_document(&state, &pko.id, "GKPKOBPSA-2025-12-31-1-pl.zip");
    extract_and_report(&state, &pko, &pko_doc);

    eprintln!("== real_data_t4_bank_insurance_chain done ==");
}
