//! Real-data probe for the domain-recency rule (#496; data-model.md § Model
//! principles, guardrail `d60305c`): on a copy of the maintainer's database
//! every "newest first" read must be monotonic in its DOMAIN key, and the
//! canonical-fact preference must pick the `total` figure where an
//! `owners_of_parent` sibling was inserted later (the CBF / GPW / VRC
//! H1-2026 `total_equity` shape that mis-fed the expectation review).
//!
//! **Inert in CI** — skips unless `BRAWLER_REAL_DB` points at a throwaway copy
//! of the owner's DB (never the live file). Opens it read-only: a probe that
//! migrated the copy would prove nothing about the shipped schema.

use crate::storage::financials::CANONICAL_FACT_PREFERENCE_ORDER;
use crate::storage::{open_database_readonly, AppState, ListFinancialFactsInput};
use std::collections::HashMap;

fn real_db_path(probe: &str) -> Option<String> {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP {probe}: set BRAWLER_REAL_DB to a COPY of the owner's database");
        return None;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP {probe}: no database at {db_path}");
        return None;
    }
    Some(db_path)
}

/// Site 1: `list_report_documents_by_company` is newest-disclosure-first for
/// every company (the Documents tool's "No period" rows render this order).
#[test]
fn report_documents_list_is_monotonic_in_disclosure_key() {
    let Some(db_path) = real_db_path("report_documents_list_is_monotonic_in_disclosure_key") else {
        return;
    };
    let state = AppState::new(open_database_readonly(&db_path).expect("open real db read-only"));
    let companies = state.list_companies().expect("companies");
    let (mut docs_total, mut companies_with_docs) = (0usize, 0usize);
    for company in &companies {
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("documents");
        if documents.is_empty() {
            continue;
        }
        companies_with_docs += 1;
        docs_total += documents.len();
        for pair in documents.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let (ka, kb) = (a.disclosure_key(), b.disclosure_key());
            assert!(
                ka > kb || (ka == kb && a.id < b.id),
                "{}: {} ({ka}) listed before {} ({kb}) — not newest-disclosure-first",
                company.id,
                a.id,
                b.id
            );
        }
    }
    eprintln!(
        "recency probe: {docs_total} documents across {companies_with_docs} companies ordered by disclosure key"
    );
    assert!(docs_total > 0, "an empty corpus proves nothing");
}

/// Site 2: `list_financial_facts` (company-wide) is newest-period-first with
/// the canonical fact first inside a period, for every company.
#[test]
fn financial_facts_list_is_monotonic_in_period_key() {
    let Some(db_path) = real_db_path("financial_facts_list_is_monotonic_in_period_key") else {
        return;
    };
    let keys: HashMap<String, (String, i64)> = {
        let connection = open_database_readonly(&db_path).expect("open real db read-only");
        let mut statement = connection
            .prepare(
                "SELECT id, IFNULL(period_end_date, fiscal_year || '-12-31'), fiscal_year
                 FROM financial_periods",
            )
            .expect("prepare");
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, (row.get(1)?, row.get(2)?)))
            })
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows")
    };
    let state = AppState::new(open_database_readonly(&db_path).expect("open real db read-only"));
    let companies = state.list_companies().expect("companies");
    let (mut facts_total, mut inversions) = (0usize, 0usize);
    for company in &companies {
        let facts = state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company.id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("facts");
        facts_total += facts.len();
        for pair in facts.windows(2) {
            let ka = keys.get(&pair[0].period_id).expect("period of fact");
            let kb = keys.get(&pair[1].period_id).expect("period of fact");
            if ka < kb {
                inversions += 1;
                eprintln!(
                    "{}: {} ({ka:?}) before {} ({kb:?})",
                    company.id, pair[0].id, pair[1].id
                );
            }
        }
    }
    eprintln!("recency probe: {facts_total} facts, {inversions} period-key inversions");
    assert_eq!(inversions, 0, "facts must be newest-period-first");
    assert!(facts_total > 0, "an empty corpus proves nothing");
}

/// Site 3 (+ the KPI-table cell): the one canonical preference picks `total`
/// over a later-inserted `owners_of_parent` sibling in every confirmed slot
/// that holds both — the exact shape that mis-fed the expectation review.
#[test]
fn canonical_preference_picks_total_over_later_owners_of_parent() {
    let Some(db_path) =
        real_db_path("canonical_preference_picks_total_over_later_owners_of_parent")
    else {
        return;
    };
    let connection = open_database_readonly(&db_path).expect("open real db read-only");
    let sql = format!(
        "WITH slot AS (
             SELECT p.company_id, p.fiscal_year, p.period_type, k.metric_key
             FROM financial_facts f
             JOIN financial_periods p ON p.id = f.period_id
             JOIN kpi_definitions k ON k.id = f.definition_id
             WHERE f.confirmation_state = 'confirmed'
             GROUP BY 1, 2, 3, 4
             HAVING SUM(f.attribution = 'total') > 0
                AND SUM(f.attribution = 'owners_of_parent') > 0
         )
         SELECT s.company_id, s.fiscal_year, s.period_type, s.metric_key,
                (SELECT f.attribution
                 FROM financial_facts f
                 JOIN financial_periods p ON p.id = f.period_id
                 JOIN kpi_definitions k ON k.id = f.definition_id
                 WHERE p.company_id = s.company_id AND p.fiscal_year = s.fiscal_year
                   AND p.period_type = s.period_type AND k.metric_key = s.metric_key
                   AND f.confirmation_state = 'confirmed'
                 ORDER BY {CANONICAL_FACT_PREFERENCE_ORDER}, f.id
                 LIMIT 1) AS picked
         FROM slot s"
    );
    let mut statement = connection.prepare(&sql).expect("prepare");
    let picks: Vec<(String, i64, String, String, String)> = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    eprintln!(
        "recency probe: {} confirmed slots hold both total and owners_of_parent",
        picks.len()
    );
    for (company, year, period, metric, picked) in &picks {
        assert_eq!(
            picked, "total",
            "{company} {year} {period} {metric}: canonical preference picked {picked}"
        );
    }
    assert!(
        !picks.is_empty(),
        "expected the CBF/GPW/VRC H1-2026 total_equity slots on the owner's corpus"
    );
}
