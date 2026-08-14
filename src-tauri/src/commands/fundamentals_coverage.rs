//! Fundamentals **coverage** read model (ADR 0077 §2).
//!
//! Answers "for company X, which fiscal periods have a canonical report, which
//! have extracted facts (and how validated), and what is waiting for review?" —
//! the substrate the F2 Coverage panel (T2.2) renders. This is a **computed**
//! read model (ADR 0044 pattern): every call assembles the answer from the live
//! tables (`report_documents`, `financial_facts`/`financial_periods`/
//! `financial_fact_provenance`). There is NO
//! stored projection and NO table — cardinality is small (a handful of periods
//! per company) and staleness is impossible when nothing is cached.
//!
//! The period key is the union of three sources: the canonical periodic report
//! per period (`classify` taxonomy + `derive_report_period`), periods that carry
//! facts, and periods that carry pending extraction proposals. A period appears
//! in the map if ANY of the three names it.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::fundamentals::extraction::classify::{
    canonical_reports_per_period, CanonicalReportCandidate, DocKind,
};
use crate::jobs::autopilot::{is_structured_document, report_disclosure_key};
use crate::jobs::structured_extraction::derive_report_period;
use crate::{app_state, storage};

// ============================================================================
// DTOs (ts-rs export → ../../src/api/generated/)
// ============================================================================

/// The whole coverage map for one company: one row per fiscal period, newest
/// first.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FundamentalsCoverage {
    pub company_id: String,
    pub periods: Vec<CoveragePeriodRow>,
}

/// One `(fiscal_year, period_type)` period and its coverage across the three
/// axes: the canonical report, the extracted facts, and the review queue.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CoveragePeriodRow {
    pub fiscal_year: i64,
    /// `Q1 | H1 | Q3 | FY` for report-bearing periods; any stored
    /// `financial_periods.period_type` for fact/proposal-only periods.
    pub period_type: String,
    /// The single canonical periodic report for this period, if one exists.
    pub report: Option<CoverageReportCell>,
    pub facts: CoverageFactsCell,
    pub review: CoverageReviewCell,
    /// Whether an AI budget cap stopped this period from being swept: `true` when
    /// the canonical report's latest `history_sweep` run recorded
    /// `reason: "skipped_budget"` (ADR 0077 §6). Cleared naturally by a later
    /// successful extraction (facts appear and the run's delta is overwritten).
    pub skipped_budget: bool,
}

/// The canonical periodic report for a period (ADR 0077 §1 selection).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReportCell {
    pub document_id: String,
    /// `periodic_ssf | periodic_jsf` — the canonical report is always periodic.
    pub doc_kind: String,
    pub title: Option<String>,
    /// A structured ESEF/xhtml document (vs a flat PDF).
    pub structured: bool,
    /// `fetch_status == "fetched"` — a link-only (metadata-only) periodic report
    /// still yields a report cell, but with `fetched = false`.
    pub fetched: bool,
}

/// Fact counts for a period, split by validation state. `total = validated +
/// unvalidated + flagged`.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CoverageFactsCell {
    pub total: i64,
    /// Provenance `passed` or `witness_confirmed`.
    pub validated: i64,
    /// No provenance row, or `unreviewed`/`none` (or any non-passed/non-flagged).
    pub unvalidated: i64,
    /// Provenance `flagged`.
    pub flagged: i64,
}

/// The review-queue view of a period: what a human still needs to look at.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReviewCell {
    /// Mirror of `facts.flagged` — flagged facts are a review surface too.
    /// The `pending_proposals` input is gone with the KPI staging ledger
    /// (ADR 0084 decision 5): flagged deterministic facts are the only review
    /// surface left, so coverage can no longer be inflated by a retired source.
    pub flagged_facts: i64,
}

// ============================================================================
// Computed read model
// ============================================================================

/// Period index in the `report_diff::classify::period_sort_key` shape:
/// `Q1=1, H1=2, Q3=3, FY=4`. The inverse maps a canonical-report period index
/// back to its `period_type` label.
fn period_type_for_index(index: u8) -> Option<&'static str> {
    match index {
        1 => Some("Q1"),
        2 => Some("H1"),
        3 => Some("Q3"),
        4 => Some("FY"),
        _ => None,
    }
}

/// Canonicalize a stored `period_type` label for coverage grouping. Stored
/// data drifts (a legacy out-of-spec `annual` label beside spec `FY`; `q3`
/// vs `Q3`), and keying the period union on raw labels splits one period into
/// two rows. Uppercase everything and alias `ANNUAL` to `FY` (the only
/// observed legacy synonym; data-model.md's label set is the spec). Cells
/// merged under one canonical key are summed by the callers.
fn canonical_period_label(raw: &str) -> String {
    let upper = raw.trim().to_uppercase();
    if upper == "ANNUAL" {
        return "FY".to_owned();
    }
    upper
}

/// A stable descending sort index for arbitrary `period_type` labels so the map
/// reads newest-period-first within a year. The four canonical labels follow the
/// ADR 0077 period index; other stored labels (`Q2`, `H2`, `9M`, …) fall on a
/// reasonable neighbour so they never collapse into `FY`.
fn period_sort_index(period_type: &str) -> u8 {
    match period_type {
        "Q1" => 1,
        "Q2" | "H1" => 2,
        "Q3" | "9M" => 3,
        "Q4" | "H2" | "FY" => 4,
        _ => 0,
    }
}

/// The reporting period of a stored report document for the coverage map:
/// `derive_report_period` first (ESEF self-derives, PDF parses its title/URL),
/// then a title/URL fallback identical to `derive`'s PDF branch so a link-only
/// (metadata-only, no stored file) periodic report still counts. Returns
/// `(fiscal_year, period_type, period_index)`; `None` when no period can be
/// determined.
pub(crate) fn document_period(
    state: &app_state::AppState,
    document: &storage::ReportDocument,
) -> Option<(i64, String, u8)> {
    if let Some((fiscal_year, period_type, _end)) = derive_report_period(state, document) {
        let index = period_sort_index(period_type);
        return Some((fiscal_year, period_type.to_owned(), index));
    }
    // Fallback: parse the title/URL the same way `derive_report_period`'s PDF
    // branch does. Reaches link-only periodic reports (no local file → `derive`
    // returns `None` before it can parse).
    let title = document.title.as_deref().unwrap_or("");
    let (year, index) = crate::report_diff::classify::period_sort_key(title, &document.url)?;
    let period_type = period_type_for_index(index)?;
    Some((i64::from(year), period_type.to_owned(), index))
}

/// Assemble the coverage map for one company (ADR 0077 §2). Testable against
/// `open_in_memory_database`. Reads several tables and (via `derive_report_period`
/// on ESEF documents) may read stored files, so command callers offload it.
pub fn compute_fundamentals_coverage(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<FundamentalsCoverage, String> {
    // (a) Load documents; keep only the two periodic kinds, each with its period.
    let documents = state
        .list_report_documents_by_company(company_id)
        .map_err(|e| e.to_string())?;

    let mut doc_by_id: BTreeMap<String, storage::ReportDocument> = BTreeMap::new();
    let mut candidates: Vec<CanonicalReportCandidate> = Vec::new();
    for document in documents {
        // The read model follows the STORED taxonomy column (ADR 0077 §1) —
        // set-on-write plus the idempotent reclassify command are its freshness
        // mechanism — so coverage can never disagree with what the documents
        // panel (column + filter + index) shows. NULL = not yet classified and
        // is excluded like every non-periodic kind; "Refresh classification"
        // is the user path that converges it.
        let kind = match document.doc_kind.as_deref() {
            Some("periodic_ssf") => DocKind::PeriodicSsf,
            Some("periodic_jsf") => DocKind::PeriodicJsf,
            _ => continue,
        };
        let Some((fiscal_year, _period_type, index)) = document_period(state, &document) else {
            continue;
        };
        // (b) A candidate for canonical-report-per-period selection.
        candidates.push(CanonicalReportCandidate {
            document_id: document.id.clone(),
            doc_kind: kind,
            period: (fiscal_year as i32, index),
            disclosure_key: report_disclosure_key(&document),
            structured: is_structured_document(&document),
        });
        doc_by_id.insert(document.id.clone(), document);
    }

    // Row key: (fiscal_year, period_type). The canonical report per period keys
    // on the period index; convert it back to the label for the row.
    let mut report_cells: BTreeMap<(i64, String), CoverageReportCell> = BTreeMap::new();
    for ((fiscal_year, index), candidate) in canonical_reports_per_period(&candidates) {
        let Some(period_type) = period_type_for_index(index) else {
            continue;
        };
        let Some(document) = doc_by_id.get(&candidate.document_id) else {
            continue;
        };
        report_cells.insert(
            (i64::from(fiscal_year), period_type.to_owned()),
            CoverageReportCell {
                document_id: document.id.clone(),
                doc_kind: candidate.doc_kind.as_str().to_owned(),
                title: document.title.clone(),
                structured: candidate.structured,
                fetched: document.fetch_status == "fetched",
            },
        );
    }

    // (c) Facts aggregated per (fiscal_year, period_type), with provenance.
    let mut fact_cells: BTreeMap<(i64, String), CoverageFactsCell> = BTreeMap::new();
    for row in state
        .financials()
        .facts_coverage_by_period(company_id)
        .map_err(|e| e.to_string())?
    {
        // Merge (sum) under the canonical label — two stored labels (e.g.
        // `annual` + `FY`) can normalize to one period key.
        let cell = fact_cells
            .entry((row.fiscal_year, canonical_period_label(&row.period_type)))
            .or_insert(CoverageFactsCell {
                total: 0,
                validated: 0,
                unvalidated: 0,
                flagged: 0,
            });
        cell.total += row.total;
        cell.validated += row.validated;
        cell.unvalidated += row.unvalidated;
        cell.flagged += row.flagged;
    }

    // (e') Document ids whose latest history-sweep run was budget-denied. Lights
    // a period's `skipped_budget` when its canonical report is one of them
    // (ADR 0077 §6). Computed read model — no stored projection.
    let budget_skipped_docs = state
        .autopilot()
        .documents_skipped_by_budget(company_id)
        .map_err(|e| e.to_string())?;

    // (e) Rows = union of the three key sets.
    let mut keys: BTreeMap<(i64, String), ()> = BTreeMap::new();
    for key in report_cells.keys() {
        keys.insert(key.clone(), ());
    }
    for key in fact_cells.keys() {
        keys.insert(key.clone(), ());
    }

    let mut periods: Vec<CoveragePeriodRow> = keys
        .into_keys()
        .map(|(fiscal_year, period_type)| {
            let facts = fact_cells
                .get(&(fiscal_year, period_type.clone()))
                .cloned()
                .unwrap_or(CoverageFactsCell {
                    total: 0,
                    validated: 0,
                    unvalidated: 0,
                    flagged: 0,
                });
            let report = report_cells
                .get(&(fiscal_year, period_type.clone()))
                .cloned();
            // `skipped_budget` (ADR 0077 §6) lights when this period's canonical
            // report's latest history-sweep run recorded a budget-denied tier-4.
            // A repaired/later successful extraction clears it two ways: it makes
            // `facts.total > 0` (a non-gap), and it overwrites that run's delta
            // (dropping the document from `budget_skipped_docs`).
            let skipped_budget = report
                .as_ref()
                .is_some_and(|cell| budget_skipped_docs.contains(&cell.document_id));
            CoveragePeriodRow {
                report,
                review: CoverageReviewCell {
                    flagged_facts: facts.flagged,
                },
                facts,
                skipped_budget,
                fiscal_year,
                period_type,
            }
        })
        .collect();

    // Newest period first: DESC by (fiscal_year, period index, period_type).
    periods.sort_by(|a, b| {
        b.fiscal_year
            .cmp(&a.fiscal_year)
            .then_with(|| period_sort_index(&b.period_type).cmp(&period_sort_index(&a.period_type)))
            .then_with(|| b.period_type.cmp(&a.period_type))
    });

    Ok(FundamentalsCoverage {
        company_id: company_id.to_owned(),
        periods,
    })
}

/// Coverage map for the company's fundamentals (ADR 0077 §2). Offloaded off the
/// UI thread — assembling the map reads several tables and, for ESEF documents,
/// the stored file (`derive_report_period`).
#[tauri::command]
pub async fn get_fundamentals_coverage(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<FundamentalsCoverage, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_fundamentals_coverage(&state, &company_id))
        .await
        .map_err(|e| format!("coverage task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, NewCompany,
        NewFinancialFact, NewFinancialPeriod,
    };

    const NET_PROFIT: &str = "kpidef_net_profit";

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Test S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    /// A stored periodic report. `fetched` controls whether it carries a file:
    /// `true` → a fetched PDF (period from its title); `false` → a link-only
    /// metadata-only document (no file → coverage falls back to the title parse).
    fn report(state: &AppState, company_id: &str, title: &str, url: &str, fetched: bool) -> String {
        let doc = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.to_owned(),
                source_type: "user_url".to_owned(),
                url: url.to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some(title.to_owned()),
                attribution: None,
            })
            .expect("report document");
        if fetched {
            state
                .mark_report_document_fetched(
                    &doc.id,
                    Some("reports/x.pdf"),
                    Some("application/pdf"),
                    Some("hash"),
                    Some(1024),
                )
                .expect("mark fetched");
        } else {
            state
                .mark_report_document_metadata_only(&doc.id)
                .expect("mark metadata_only");
        }
        doc.id
    }

    fn period(state: &AppState, company_id: &str, fiscal_year: i64, period_type: &str) -> String {
        state
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: company_id.to_owned(),
                fiscal_year,
                period_type: period_type.to_owned(),
                period_end_date: None,
                report_evidence_ref: None,
            })
            .expect("period")
            .id
    }

    /// Seed a period row with a raw out-of-spec label, bypassing the create
    /// boundary that folds legacy labels (F6a) — coverage must stay tolerant of
    /// a DB shape migration 0066 has not visited.
    fn seed_legacy_period_row(
        state: &AppState,
        company_id: &str,
        fiscal_year: i64,
        period_type: &str,
    ) -> String {
        let id = format!("finper_{company_id}_{fiscal_year}_{period_type}");
        state
            .checkout_for_tests()
            .expect("connection")
            .execute(
                "INSERT INTO financial_periods (id, company_id, fiscal_year, period_type)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, company_id, fiscal_year, period_type],
            )
            .expect("legacy period row");
        id
    }

    /// Create a net-profit fact in a period, optionally with a provenance verdict.
    fn fact(
        state: &AppState,
        company_id: &str,
        period_id: &str,
        validation: Option<&str>,
    ) -> String {
        let fact = state
            .financials()
            .create_financial_fact(NewFinancialFact {
                company_id: company_id.to_owned(),
                period_id: period_id.to_owned(),
                definition_id: NET_PROFIT.to_owned(),
                value_numeric: "1000".to_owned(),
                currency: Some("PLN".to_owned()),
                statement_basis: None,
                attribution: None,
                variant: None,
                measure_window: None,
                data_quality: None,
                as_reported_value: None,
                as_reported_scale: None,
                reporting_standard: None,
                extraction_method: None,
                confidence: None,
                confirmation_state: None,
                supersedes_id: None,
                source_document_ref: None,
                annotation: None,
            })
            .expect("fact");
        if let Some(status) = validation {
            state
                .fundamentals_provenance()
                .set_fact_provenance(crate::storage::NewFactProvenance {
                    fact_id: &fact.id,
                    source_tier: "esef",
                    validation_status: status,
                    drift_json: None,
                    citation: None,
                })
                .expect("provenance");
        }
        fact.id
    }

    /// Seed a succeeded extraction job with one pending proposal detected at
    /// `(fiscal_year, period_type)`.
    fn row<'a>(
        coverage: &'a FundamentalsCoverage,
        fiscal_year: i64,
        period_type: &str,
    ) -> &'a CoveragePeriodRow {
        coverage
            .periods
            .iter()
            .find(|p| p.fiscal_year == fiscal_year && p.period_type == period_type)
            .unwrap_or_else(|| panic!("no row for {fiscal_year} {period_type}"))
    }

    #[test]
    fn report_with_validated_facts() {
        let s = state();
        let c = company(&s);
        let doc = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        let p = period(&s, &c, 2025, "FY");
        fact(&s, &c, &p, Some("passed"));

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        let report = r.report.as_ref().expect("report cell");
        assert_eq!(report.document_id, doc);
        assert_eq!(report.doc_kind, "periodic_ssf");
        assert!(report.fetched);
        assert_eq!(r.facts.total, 1);
        assert_eq!(r.facts.validated, 1);
        assert_eq!(r.facts.unvalidated, 0);
        assert_eq!(r.facts.flagged, 0);
    }

    #[test]
    fn report_with_no_facts() {
        let s = state();
        let c = company(&s);
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        assert!(r.report.is_some());
        assert_eq!(r.facts.total, 0);
    }

    /// Period labels must group case-insensitively with `ANNUAL` aliased to
    /// `FY` (data-model allows `FY, H1, H2, Q1–Q4, 9M, M01–M12`), and merged
    /// cells must SUM (not overwrite).
    #[test]
    fn period_labels_group_case_insensitively_and_annual_aliases_to_fy() {
        let connection = open_in_memory_database().expect("db");
        let state = AppState::new(connection);
        let company = company(&state);

        // A legacy out-of-spec 'annual' row can no longer be CREATED (the create
        // boundary folds it to FY, F6a; migration 0066 folds shipped rows), so
        // seed the legacy shape via raw SQL to exercise coverage's tolerant-read
        // grouping.
        let annual = seed_legacy_period_row(&state, &company, 2023, "annual");
        let fy = period(&state, &company, 2023, "FY");
        fact(&state, &company, &annual, Some("passed"));
        fact(&state, &company, &fy, Some("passed"));
        let q3_lower = period(&state, &company, 2025, "q3");
        fact(&state, &company, &q3_lower, None);

        let coverage = compute_fundamentals_coverage(&state, &company).expect("coverage");
        let labels: Vec<(i64, String)> = coverage
            .periods
            .iter()
            .map(|row| (row.fiscal_year, row.period_type.clone()))
            .collect();
        assert_eq!(
            labels,
            vec![(2025, "Q3".to_owned()), (2023, "FY".to_owned())],
            "annual must merge into FY and q3 must canonicalize to Q3 — one row per period"
        );
        let fy_row = &coverage.periods[1];
        assert_eq!(
            fy_row.facts.total, 2,
            "merged cells must sum, not overwrite"
        );
        assert_eq!(fy_row.facts.validated, 2);
    }

    #[test]
    fn facts_with_no_report_still_yield_a_row() {
        let s = state();
        let c = company(&s);
        let p = period(&s, &c, 2024, "FY");
        fact(&s, &c, &p, None);

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2024, "FY");
        assert!(r.report.is_none());
        assert_eq!(r.facts.total, 1);
        assert_eq!(r.facts.unvalidated, 1);
        assert_eq!(r.facts.validated, 0);
    }

    #[test]
    fn flagged_facts_counted_in_facts_and_review() {
        let s = state();
        let c = company(&s);
        let p = period(&s, &c, 2025, "H1");
        fact(&s, &c, &p, Some("flagged"));

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "H1");
        assert_eq!(r.facts.flagged, 1);
        assert_eq!(r.facts.validated, 0);
        assert_eq!(r.facts.unvalidated, 0);
        assert_eq!(r.review.flagged_facts, 1);
    }

    #[test]
    fn metadata_only_report_yields_a_cell_via_fallback() {
        let s = state();
        let c = company(&s);
        // No stored file → `derive_report_period` returns None → title fallback.
        let doc = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "https://x/emitent/2026-03/ssf-2025.pdf",
            false,
        );

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        let report = r.report.as_ref().expect("report cell from fallback");
        assert_eq!(report.document_id, doc);
        assert!(!report.fetched);
    }

    #[test]
    fn governance_document_creates_no_row() {
        let s = state();
        let c = company(&s);
        report(
            &s,
            &c,
            "Sprawozdanie Rady Nadzorczej 2025",
            "x/rn-2025.pdf",
            true,
        );

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        assert!(
            coverage.periods.is_empty(),
            "governance must not produce a period row"
        );
    }

    #[test]
    fn ssf_beats_jsf_for_the_same_period() {
        let s = state();
        let c = company(&s);
        let ssf = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        report(
            &s,
            &c,
            "Jednostkowy raport roczny 2025 JSF",
            "x/jsf-2025.pdf",
            true,
        );

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        let report = r.report.as_ref().expect("report cell");
        assert_eq!(report.document_id, ssf);
        assert_eq!(report.doc_kind, "periodic_ssf");
    }

    #[test]
    fn rows_are_sorted_descending() {
        let s = state();
        let c = company(&s);
        // Every asserted period carries a fact so it is present in the union.
        fact(&s, &c, &period(&s, &c, 2023, "FY"), None);
        fact(&s, &c, &period(&s, &c, 2024, "FY"), None);
        fact(&s, &c, &period(&s, &c, 2024, "H1"), None);
        fact(&s, &c, &period(&s, &c, 2025, "FY"), None);

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let years: Vec<(i64, &str)> = coverage
            .periods
            .iter()
            .map(|p| (p.fiscal_year, p.period_type.as_str()))
            .collect();
        assert_eq!(years[0], (2025, "FY"));
        // 2024 FY before 2024 H1 (higher period index first), before 2023 FY.
        let pos_2024_fy = years.iter().position(|&y| y == (2024, "FY")).unwrap();
        let pos_2024_h1 = years.iter().position(|&y| y == (2024, "H1")).unwrap();
        let pos_2023_fy = years.iter().position(|&y| y == (2023, "FY")).unwrap();
        assert!(pos_2024_fy < pos_2024_h1);
        assert!(pos_2024_h1 < pos_2023_fy);
    }

    #[test]
    fn empty_company_yields_empty_periods() {
        let s = state();
        let c = company(&s);
        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        assert!(coverage.periods.is_empty());
    }

    /// Seed the deterministic autopilot run for a document with a given trigger
    /// and raw `kpi_delta_json`, mirroring what the extract stage writes. Drives
    /// the coverage `skipped_budget` projection (ADR 0077 §6). The run id is
    /// per-`(company, document)` deterministic, so there is at most one run row
    /// per document.
    fn autopilot_run_with_delta(
        state: &AppState,
        company_id: &str,
        document_id: &str,
        trigger: &str,
        kpi_delta_json: &str,
    ) {
        let run_id = format!("autopilot_run:{company_id}:{document_id}");
        state
            .autopilot()
            .create_run_if_absent(&run_id, company_id, document_id, trigger, "autopilot", None)
            .expect("create run");
        state
            .autopilot()
            .set_kpi_delta_json(&run_id, kpi_delta_json)
            .expect("set delta");
    }

    const SKIPPED_BUDGET_DELTA: &str = r#"{"extractionAvailable":false,"reason":"skipped_budget"}"#;

    #[test]
    fn skipped_budget_lights_when_sweep_run_recorded_it() {
        let s = state();
        let c = company(&s);
        let doc = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        autopilot_run_with_delta(&s, &c, &doc, "history_sweep", SKIPPED_BUDGET_DELTA);

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        assert!(
            r.skipped_budget,
            "the canonical doc's history_sweep skipped_budget run must light the flag"
        );
    }

    #[test]
    fn skipped_budget_false_after_successful_rerun() {
        let s = state();
        let c = company(&s);
        let doc = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        // A later successful extraction overwrites the same deterministic run's
        // delta; the flag clears naturally.
        autopilot_run_with_delta(
            &s,
            &c,
            &doc,
            "history_sweep",
            r#"{"extractionAvailable":true,"produced":3}"#,
        );

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        assert!(
            !r.skipped_budget,
            "a success/other-reason delta must not light skipped_budget"
        );
    }

    #[test]
    fn skipped_budget_false_for_detection_run() {
        let s = state();
        let c = company(&s);
        let doc = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        // Only `history_sweep` runs project the flag; a detection/manual run must
        // never light it even if its delta carried the reason.
        autopilot_run_with_delta(&s, &c, &doc, "detection", SKIPPED_BUDGET_DELTA);

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        assert!(
            !r.skipped_budget,
            "a non-sweep run's reason must not light skipped_budget"
        );
    }

    #[test]
    fn skipped_budget_false_when_no_run() {
        let s = state();
        let c = company(&s);
        report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        assert!(!r.skipped_budget, "no run means no skipped_budget");
    }

    #[test]
    fn skipped_budget_false_for_garbled_delta_json() {
        let s = state();
        let c = company(&s);
        let doc = report(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        autopilot_run_with_delta(&s, &c, &doc, "history_sweep", "{not valid json");

        let coverage = compute_fundamentals_coverage(&s, &c).expect("coverage");
        let r = row(&coverage, 2025, "FY");
        assert!(
            !r.skipped_budget,
            "garbled kpi_delta_json is tolerated as not-skipped"
        );
    }

    /// A minimal text-bearing PDF (mirrors the structured-extraction test helper)
    /// whose cover page states the reporting period — the only path where the
    /// coverage read model must extract a PDF to place a document.
    fn cover_page_pdf(lines: &[&str]) -> Vec<u8> {
        let filler = "Nota objasniajaca do sprawozdania finansowego za okres sprawozdawczy.";
        let mut all: Vec<&str> = lines.to_vec();
        while all.iter().map(|l| l.len() + 1).sum::<usize>() < 220 {
            all.push(filler);
        }
        let mut content = String::from("BT /F1 12 Tf 40 750 Td 16 TL\n");
        for (i, line) in all.iter().enumerate() {
            if i > 0 {
                content.push_str("T*\n");
            }
            let escaped = line
                .replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)");
            content.push_str(&format!("({escaped}) Tj\n"));
        }
        content.push_str("ET");
        let objects = [
            "<</Type/Catalog/Pages 2 0 R>>".to_owned(),
            "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_owned(),
            "<</Type/Page/Parent 2 0 R/Resources<</Font<</F1 4 0 R>>>>/MediaBox[0 0 612 792]/Contents 5 0 R>>".to_owned(),
            "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_owned(),
            format!("<</Length {}>>\nstream\n{}\nendstream", content.len(), content),
        ];
        let mut buf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, obj) in objects.iter().enumerate() {
            offsets.push(buf.len());
            buf.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
        }
        let xref_offset = buf.len();
        buf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        buf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in &offsets {
            buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        buf.extend_from_slice(
            format!(
                "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{}\n%%EOF",
                objects.len() + 1,
                xref_offset
            )
            .as_bytes(),
        );
        buf
    }

    #[test]
    fn coverage_derives_the_period_once_then_reads_the_cache() {
        // C4: the Coverage panel derived every document's period on EVERY load,
        // and for a bare-`SSF.pdf` filing that meant a full PDF extraction per load
        // (violating "read persisted derived indexes, don't recompute the corpus").
        // The derivation now persists (migration 0109). Proven by DELETING the
        // file after the first computation: without the cache the document would
        // lose its period and drop from the map; with it, the map is identical.
        let dir = std::env::temp_dir().join(format!(
            "brawler-coverage-c4-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let s = AppState::with_data_dir(
            open_in_memory_database().expect("in-memory db"),
            dir.clone(),
        );
        let c = company(&s);

        // A bare-titled SSF (classify => periodic_ssf) whose ONLY period signal is
        // its cover page.
        let doc = s
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: c.clone(),
                source_type: "espi_attachment".to_owned(),
                url: "https://example.com/SSF.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("SSF.pdf".to_owned()),
                attribution: None,
            })
            .expect("document");
        let bytes = cover_page_pdf(&[
            "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ TST",
            "za okres 6 miesiecy zakonczony 30.06.2025",
        ]);
        std::fs::write(dir.join("ssf.pdf"), &bytes).expect("write pdf");
        s.mark_report_document_fetched(
            &doc.id,
            Some("ssf.pdf"),
            Some("application/pdf"),
            // The real capture path always records the content hash — the
            // provenance-aware cache-hit predicate (migration 0140) needs the
            // realistic shape or the cache can never hit.
            Some(&format!("{:064x}", bytes.len())),
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");

        let first = compute_fundamentals_coverage(&s, &c).expect("coverage");
        assert!(
            row(&first, 2025, "H1").report.is_some(),
            "the cover-page period places the report"
        );

        // Remove the file: a second computation that re-derived would fail to read
        // it and drop the document, changing the map. The cache keeps it identical.
        std::fs::remove_file(dir.join("ssf.pdf")).expect("remove pdf");
        let second = compute_fundamentals_coverage(&s, &c).expect("coverage");
        assert_eq!(
            serde_json::to_value(&second).expect("serialize"),
            serde_json::to_value(&first).expect("serialize"),
            "second coverage computation must read the cached period, not re-extract"
        );
    }
}
