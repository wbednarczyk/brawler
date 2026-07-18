//! Real-data coverage probe for the v0.57 health-score extraction concepts
//! (T1, ADR 0083 Decision 5). **Inert in CI** — skips unless `BRAWLER_REAL_DB`
//! points at a throwaway copy of the maintainer's real DB, exactly like
//! [`super::real_data_extraction`].
//!
//! It runs the *real* [`parse_esef`] parser over every stored ESEF report
//! package (`.xbri`/`.zip`, extracted via [`esef_package::extract_instance`])
//! and reports, per company and per period, which of the four new concepts
//! (`current_assets`, `current_liabilities`, `retained_earnings`,
//! `long_term_debt`) actually extract. This is the coverage table the score UI
//! stays behind until it is honest (ADR 0083 Decision 5 / Decision 10).
//!
//! Run it manually:
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
//!   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
//!   cargo test -p brawler --lib real_data_health_concept_coverage -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;
use crate::fundamentals::extraction::esef::parse_esef;
use crate::fundamentals::extraction::esef_package;

/// The four v0.57 health-score inputs, in report order.
const HEALTH_CONCEPTS: [&str; 4] = [
    "current_assets",
    "current_liabilities",
    "retained_earnings",
    "long_term_debt",
];

#[derive(Default)]
struct Coverage {
    /// period-end -> which of HEALTH_CONCEPTS extracted.
    periods: BTreeMap<String, Vec<&'static str>>,
    packages: usize,
}

#[test]
#[ignore = "real-data coverage probe; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR"]
fn real_data_health_concept_coverage() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_health_concept_coverage: set BRAWLER_REAL_DB to a throwaway copy"
        );
        return;
    };

    let connection = open_database(&db_path).expect("open real db");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };

    let companies = state.list_companies().expect("list companies");
    let mut by_company: BTreeMap<String, Coverage> = BTreeMap::new();

    for company in &companies {
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");
        for document in &documents {
            let Some(local_path) = document.local_path.as_deref() else {
                continue;
            };
            let path = state.data_dir().join(local_path);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if !esef_package::is_report_package(local_path, &bytes) {
                continue;
            }
            let Some(instance) = esef_package::extract_instance(&bytes) else {
                continue;
            };
            let Ok(facts) = parse_esef(&instance) else {
                continue;
            };

            let entry = by_company.entry(company.ticker.clone()).or_default();
            entry.packages += 1;
            // Group extracted health concepts by their period end.
            let mut per_period: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
            for fact in &facts {
                if let Some(&concept) = HEALTH_CONCEPTS.iter().find(|c| **c == fact.metric_key) {
                    per_period
                        .entry(fact.period.end_date().to_owned())
                        .or_default()
                        .push(concept);
                }
            }
            for (period, mut concepts) in per_period {
                concepts.sort_unstable();
                concepts.dedup();
                entry.periods.entry(period).or_default().extend(concepts);
            }
        }
    }

    eprintln!("== v0.57 health-concept coverage over real ESEF packages ==");
    eprintln!("(concepts: CA=current_assets CL=current_liabilities RE=retained_earnings LTD=long_term_debt)");
    let mut companies_with_all_four = 0usize;
    for (ticker, cov) in &by_company {
        // Union across the newest period for a compact per-company verdict.
        let newest = cov.periods.keys().next_back().cloned();
        eprintln!(
            "{ticker:<8} packages={:<3} periods={}",
            cov.packages,
            cov.periods.len()
        );
        for (period, concepts) in &cov.periods {
            let flags: Vec<&str> = HEALTH_CONCEPTS
                .iter()
                .map(|c| if concepts.contains(c) { "Y" } else { "." })
                .collect();
            eprintln!(
                "    {period}  CA={} CL={} RE={} LTD={}",
                flags[0], flags[1], flags[2], flags[3]
            );
        }
        if let Some(newest) = newest {
            if HEALTH_CONCEPTS
                .iter()
                .all(|c| cov.periods[&newest].contains(c))
            {
                companies_with_all_four += 1;
            }
        }
    }
    eprintln!(
        "-- {} companies with ESEF packages; {} have all four concepts in their newest period --",
        by_company.len(),
        companies_with_all_four
    );

    // Sanity only (no quality floor): the probe must reach real packages.
    assert!(
        !by_company.is_empty(),
        "expected at least one company with a parseable ESEF package \
         (check BRAWLER_REAL_DATA_DIR)"
    );
}
