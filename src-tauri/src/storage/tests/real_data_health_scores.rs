//! Real-data **score** probe for the v0.57 health engine (T3, ADR 0083
//! Decision 10 — the validation gate). **Inert in CI** — skips unless
//! `BRAWLER_REAL_DB` points at a throwaway copy of the maintainer's real DB,
//! exactly like [`super::real_data_health_coverage`].
//!
//! Where the coverage probe reports which *concepts* extract, this one runs the
//! *real* [`compute_company_health`] read model over every tracked company and
//! prints the **latest-FY** Piotroski F and Altman Z″ result — headline value,
//! `insufficient_data` (which signals/components computed + the missing-input
//! list), or `not_applicable`. This is the table cross-checked against
//! BiznesRadar's published F-Score / Altman EM-Score (T3 record in the plan
//! doc). It asserts nothing about the *values* (there is no quality floor here —
//! the triage lives in the plan doc); it only proves the engine reaches real
//! facts.
//!
//! Run it manually:
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
//!   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
//!   cargo test -p brawler --lib real_data_health_scores -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use super::*;
use crate::commands::company_health::compute_company_health;
use crate::fundamentals::health::{AltmanScore, PiotroskiScore};

/// Compact one-line render of a Piotroski result.
fn render_piotroski(p: &PiotroskiScore) -> String {
    match p {
        PiotroskiScore::Headline { score, .. } => format!("F={score}/9 (headline)"),
        PiotroskiScore::InsufficientData { signals, missing } => {
            let computed: Vec<String> = signals
                .iter()
                .map(|s| format!("{}={}", s.code, s.points))
                .collect();
            let miss: Vec<String> = missing
                .iter()
                .map(|m| format!("{}@{}", m.metric, m.period))
                .collect();
            format!(
                "insufficient_data [computed {} pts over {}: {} | missing: {}]",
                signals.iter().map(|s| s.points as u32).sum::<u32>(),
                signals.len(),
                computed.join(","),
                miss.join(","),
            )
        }
        PiotroskiScore::NotApplicable { reason } => format!("not_applicable ({reason})"),
    }
}

/// Compact one-line render of an Altman result.
fn render_altman(a: &AltmanScore) -> String {
    match a {
        AltmanScore::Headline {
            z_score,
            band,
            components,
        } => {
            let terms: Vec<String> = components
                .iter()
                .map(|c| format!("{}={}", c.code, c.contribution))
                .collect();
            format!("Z''={z_score} band={band:?} [{}]", terms.join(" "))
        }
        AltmanScore::InsufficientData {
            components,
            missing,
        } => {
            let have: Vec<String> = components.iter().map(|c| c.code.clone()).collect();
            let miss: Vec<String> = missing
                .iter()
                .map(|m| format!("{}@{}", m.metric, m.period))
                .collect();
            format!(
                "insufficient_data [have: {} | missing: {}]",
                have.join(","),
                miss.join(","),
            )
        }
        AltmanScore::NotApplicable { reason } => format!("not_applicable ({reason})"),
    }
}

#[test]
#[ignore = "real-data score probe; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR"]
fn real_data_health_scores() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP real_data_health_scores: set BRAWLER_REAL_DB to a throwaway copy");
        return;
    };

    let connection = open_database(&db_path).expect("open real db");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };

    let mut companies = state.list_companies().expect("list companies");
    companies.sort_by(|a, b| a.ticker.cmp(&b.ticker));

    eprintln!("== v0.57 health scores over the real DB (latest FY per company) ==");
    let mut reached = 0usize;
    let mut f_headline = 0usize;
    let mut z_headline = 0usize;

    for company in &companies {
        let health = match compute_company_health(&state, &company.id) {
            Ok(h) => h,
            Err(error) => {
                eprintln!("{:<6} ERROR: {error}", company.ticker);
                continue;
            }
        };
        let Some(latest) = &health.latest else {
            eprintln!(
                "{:<6} stmt={:<11} (no FY period)",
                company.ticker, health.statement_type
            );
            continue;
        };
        reached += 1;
        if matches!(latest.piotroski, PiotroskiScore::Headline { .. }) {
            f_headline += 1;
        }
        if matches!(latest.altman, AltmanScore::Headline { .. }) {
            z_headline += 1;
        }
        eprintln!(
            "{:<6} stmt={:<11} FY{}",
            company.ticker, health.statement_type, latest.fiscal_year
        );
        eprintln!("        Piotroski: {}", render_piotroski(&latest.piotroski));
        eprintln!("        Altman   : {}", render_altman(&latest.altman));
    }

    eprintln!(
        "-- {} companies; {} reached a latest FY; F headline {} / Z'' headline {} --",
        companies.len(),
        reached,
        f_headline,
        z_headline
    );

    assert!(
        reached > 0,
        "expected at least one company with a scored FY period"
    );
}

/// The T3 gate re-run driver (ADR 0083 D4 amendment (d)): force-re-extract the
/// health facts over the real stored ESEF packages, so the four new concepts land
/// as confirmed facts before the score harness runs. Mutates the throwaway DB —
/// run it BEFORE `real_data_health_scores` on the same copy. Needs the real data
/// dir (the stored packages) via `BRAWLER_REAL_DATA_DIR`.
#[test]
#[ignore = "real-data backfill driver; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR"]
fn real_data_backfill_health_facts() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP real_data_backfill_health_facts: set BRAWLER_REAL_DB to a throwaway copy");
        return;
    };
    let connection = open_database(&db_path).expect("open real db");
    let data_dir = std::env::var("BRAWLER_REAL_DATA_DIR")
        .expect("BRAWLER_REAL_DATA_DIR must point at the stored report packages");
    let state = AppState::with_data_dir(connection, PathBuf::from(data_dir));

    let mut companies = state.list_companies().expect("list companies");
    companies.sort_by(|a, b| a.ticker.cmp(&b.ticker));

    eprintln!("== health-facts backfill over the real stored packages ==");
    let mut total_created = 0usize;
    let mut total_divergent = 0usize;
    for company in &companies {
        match crate::jobs::health_facts_backfill::backfill_company_health_facts(&state, &company.id)
        {
            Ok(r) => {
                total_created += r.facts_created;
                total_divergent += r.divergences;
                if r.documents_processed > 0 {
                    eprintln!(
                        "{:<6} docs={} created={} reobserved={} divergent={} no_period={}",
                        company.ticker,
                        r.documents_processed,
                        r.facts_created,
                        r.facts_reobserved,
                        r.divergences,
                        r.documents_skipped_no_period
                    );
                }
            }
            Err(error) => eprintln!("{:<6} BACKFILL ERROR: {error}", company.ticker),
        }
    }
    eprintln!(
        "-- backfill created {total_created} new fact(s); {total_divergent} divergence(s) --"
    );
}

/// Migration 0095 (ADR 0083 D4 amendment) must classify the real DB's financial
/// issuers off `'industrial'` so the `NotApplicable` health gate fires. Opening
/// the throwaway copy applies the migration; PKO/PZU/XTB must land non-industrial.
#[test]
#[ignore = "real-data classification probe; needs BRAWLER_REAL_DB"]
fn real_data_financial_classification() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_financial_classification: set BRAWLER_REAL_DB to a throwaway copy"
        );
        return;
    };
    let connection = open_database(&db_path).expect("open real db (applies migration 0095)");
    let state = AppState::new(connection);
    let companies = state.list_companies().expect("list companies");

    for ticker in ["PKO", "PZU", "XTB"] {
        let company = companies
            .iter()
            .find(|c| c.ticker == ticker)
            .unwrap_or_else(|| panic!("{ticker} present in the real DB"));
        let statement_type = state
            .companies()
            .get_statement_type(&company.id)
            .expect("statement_type");
        eprintln!("{ticker}: statement_type={statement_type}");
        assert_ne!(
            statement_type, "industrial",
            "{ticker} is a financial issuer — migration 0095 must map it off the industrial default"
        );
    }
}
