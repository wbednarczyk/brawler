//! Report-documents **view** read model (ADR 0077 §1/§2, Panel B).
//!
//! Answers "for company X, list every stored report document, and for each one:
//! which fiscal period does it belong to, and is it the canonical report for
//! that period?" — the substrate the redesigned Report Documents panel groups by
//! period and stars the canonical report in.
//!
//! Like the coverage map it is a **computed** read model (ADR 0044): every call
//! derives the answer from the live `report_documents` rows. The period of each
//! document is [`crate::commands::fundamentals_coverage::document_period`] (ESEF
//! self-derives, PDF/link parses its title/URL) — the SAME function coverage
//! uses, so the two panels can never disagree about a document's period. The
//! canonical flag is exactly the set chosen by
//! [`canonical_reports_per_period`] over the periodic documents — again the same
//! inputs coverage feeds it — so the ★ in the panel and the report cell in the
//! coverage map name the same document.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::commands::fundamentals_coverage::document_period;
use crate::fundamentals::extraction::classify::{
    canonical_reports_per_period, CanonicalReportCandidate, DocKind,
};
use crate::jobs::autopilot::{is_structured_document, report_disclosure_key};
use crate::{app_state, storage};

// ============================================================================
// DTOs (ts-rs export → ../../src/api/generated/)
// ============================================================================

/// Every stored report document for one company, each tagged with its fiscal
/// period (if derivable) and whether it is that period's canonical report.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportDocumentsView {
    pub company_id: String,
    pub rows: Vec<ReportDocumentViewRow>,
}

/// One stored document, with its derived period and canonical flag. `fiscal_year`
/// and `period_type` are `null` together when no period can be derived from the
/// document (the common case for non-periodic filings). `canonical` is only ever
/// `true` for a periodic document chosen by the canonical-report selection.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportDocumentViewRow {
    pub document: storage::ReportDocument,
    pub fiscal_year: Option<i64>,
    pub period_type: Option<String>,
    pub canonical: bool,
    /// Extraction verdict aggregated from `fundamentals_extraction_outcomes`
    /// (#155). `None` = the pipeline never attempted this document, so the user
    /// cannot yet know whether extraction is worth running.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub extraction: Option<DocumentExtractionStatus>,
}

/// The per-document "contains extractable financial data" indicator (#155),
/// aggregated over every outcome slot recorded for the document: any emitting
/// slot -> `has_data` (with the summed fact count), else any flagged slot ->
/// `flagged`, else `empty` (attempted, nothing extractable found).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct DocumentExtractionStatus {
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"has_data\" | \"flagged\" | \"empty\"")
    )]
    pub status: String,
    pub fact_count: i64,
}

// ============================================================================
// Computed read model
// ============================================================================

/// A stored document paired with its derived period `(fiscal_year, period_type,
/// period_index)`, or `None` when no period can be derived from it.
type DocumentWithPeriod = (storage::ReportDocument, Option<(i64, String, u8)>);

/// Assemble the report-documents view for one company (ADR 0077 §1/§2). Testable
/// against `open_in_memory_database`. Reads the documents table and, for ESEF
/// documents, may read stored files (`document_period`), so command callers
/// offload it.
pub fn compute_report_documents_view(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<ReportDocumentsView, String> {
    let documents = state
        .list_report_documents_by_company(company_id)
        .map_err(|e| e.to_string())?;

    // Extraction indicator (#155): aggregate every outcome slot per document.
    // No rows for a document -> None (never attempted) — reads tolerate the
    // table predating a document, per the data-model rule.
    let mut extraction_by_document: std::collections::BTreeMap<String, DocumentExtractionStatus> =
        std::collections::BTreeMap::new();
    for outcome in state
        .fundamentals_provenance()
        .list_extraction_outcomes(company_id)
        .map_err(|e| e.to_string())?
    {
        let entry = extraction_by_document
            .entry(outcome.report_document_id.clone())
            .or_insert_with(|| DocumentExtractionStatus {
                status: "empty".to_owned(),
                fact_count: 0,
            });
        entry.fact_count += outcome.fact_count.max(0);
        // `facts_superseded` (repair migration 0119, issue #243): the recorded
        // emission's facts are no longer at the slot — an emitting acceptance
        // alone must not render "contains data" beside a zero count.
        let emitted = outcome.fact_count > 0
            || (outcome.reason_code != "facts_superseded"
                && matches!(
                    outcome.acceptance.as_str(),
                    "accepted" | "accepted_via_witness" | "accepted_unreviewed"
                ));
        if emitted {
            entry.status = "has_data".to_owned();
        } else if outcome.acceptance == "flagged" && entry.status != "has_data" {
            entry.status = "flagged".to_owned();
        }
    }

    // Derive each document's period ONCE (ESEF file reads are not free), keeping
    // it beside its document for both canonical selection and the row output.
    let prepared: Vec<DocumentWithPeriod> = documents
        .into_iter()
        .map(|document| {
            let period = document_period(state, &document);
            (document, period)
        })
        .collect();

    // Canonical selection over the periodic documents only — the SAME inputs the
    // coverage map feeds `canonical_reports_per_period`, so the ★ and the
    // coverage report cell agree.
    let mut candidates: Vec<CanonicalReportCandidate> = Vec::new();
    for (document, period) in &prepared {
        let kind = match document.doc_kind.as_deref() {
            Some("periodic_ssf") => DocKind::PeriodicSsf,
            Some("periodic_jsf") => DocKind::PeriodicJsf,
            _ => continue,
        };
        let Some((fiscal_year, _period_type, index)) = period else {
            continue;
        };
        candidates.push(CanonicalReportCandidate {
            document_id: document.id.clone(),
            doc_kind: kind,
            period: (*fiscal_year as i32, *index),
            disclosure_key: report_disclosure_key(document),
            structured: is_structured_document(document),
        });
    }
    let canonical_ids: BTreeSet<String> = canonical_reports_per_period(&candidates)
        .values()
        .map(|c| c.document_id.clone())
        .collect();

    let rows = prepared
        .into_iter()
        .map(|(document, period)| {
            let canonical = canonical_ids.contains(&document.id);
            let (fiscal_year, period_type) = match period {
                Some((fiscal_year, period_type, _index)) => (Some(fiscal_year), Some(period_type)),
                None => (None, None),
            };
            let extraction = extraction_by_document.get(&document.id).cloned();
            ReportDocumentViewRow {
                document,
                fiscal_year,
                period_type,
                canonical,
                extraction,
            }
        })
        .collect();

    Ok(ReportDocumentsView {
        company_id: company_id.to_owned(),
        rows,
    })
}

/// Report-documents view for one company (ADR 0077 §1/§2). Offloaded off the UI
/// thread — deriving periods reads the documents table and, for ESEF documents,
/// the stored file.
#[tauri::command]
pub async fn get_report_documents_view(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<ReportDocumentsView, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_report_documents_view(&state, &company_id))
        .await
        .map_err(|e| format!("report documents view task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, NewCompany,
    };

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

    /// Store a report document. `fetched` → a stored PDF; otherwise a link-only
    /// (metadata-only) document.
    fn document(
        state: &AppState,
        company_id: &str,
        title: &str,
        url: &str,
        fetched: bool,
    ) -> String {
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
        // Classify on write so the view sees a real doc_kind (mirrors ingestion).
        state.reclassify_report_documents().expect("reclassify");
        doc.id
    }

    fn row<'a>(view: &'a ReportDocumentsView, document_id: &str) -> &'a ReportDocumentViewRow {
        view.rows
            .iter()
            .find(|r| r.document.id == document_id)
            .unwrap_or_else(|| panic!("no row for {document_id}"))
    }

    #[test]
    fn periodic_document_gets_period_fields_and_is_canonical() {
        let s = state();
        let c = company(&s);
        let id = document(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );

        let view = compute_report_documents_view(&s, &c).expect("view");
        let r = row(&view, &id);
        assert_eq!(r.fiscal_year, Some(2025));
        assert_eq!(r.period_type.as_deref(), Some("FY"));
        assert!(
            r.canonical,
            "the only periodic report for its period is canonical"
        );
    }

    #[test]
    fn only_one_document_is_canonical_per_period() {
        let s = state();
        let c = company(&s);
        let ssf = document(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        let jsf = document(
            &s,
            &c,
            "Jednostkowy raport roczny 2025 JSF",
            "x/jsf-2025.pdf",
            true,
        );

        let view = compute_report_documents_view(&s, &c).expect("view");
        let ssf_row = row(&view, &ssf);
        let jsf_row = row(&view, &jsf);
        // Both belong to 2025 FY; only the consolidated report is canonical.
        assert_eq!(ssf_row.period_type.as_deref(), Some("FY"));
        assert_eq!(jsf_row.period_type.as_deref(), Some("FY"));
        assert!(ssf_row.canonical, "ssf beats jsf for the period");
        assert!(!jsf_row.canonical, "the standalone report is not canonical");
    }

    #[test]
    fn governance_document_gets_null_period_and_is_not_canonical() {
        let s = state();
        let c = company(&s);
        let id = document(
            &s,
            &c,
            "Sprawozdanie Rady Nadzorczej cyber_Folks",
            "x/rn.pdf",
            true,
        );

        let view = compute_report_documents_view(&s, &c).expect("view");
        let r = row(&view, &id);
        assert_eq!(r.fiscal_year, None);
        assert_eq!(r.period_type, None);
        assert!(!r.canonical);
    }

    /// #155: the extraction indicator aggregates outcome slots per document —
    /// no rows = None (never attempted), an emitting slot wins over a flagged
    /// one, a flagged slot wins over empty, and fact counts sum across slots.
    #[test]
    fn extraction_indicator_aggregates_outcome_slots_per_document() {
        let s = state();
        let c = company(&s);
        let attempted = document(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        let untouched = document(
            &s,
            &c,
            "Skonsolidowany raport roczny 2024 SSF",
            "x/ssf-2024.pdf",
            true,
        );
        let outcome = |fiscal_year: i64, acceptance: &'static str, fact_count: i64| {
            crate::storage::NewExtractionOutcome {
                company_id: &c,
                report_document_id: &attempted,
                fiscal_year,
                period_type: "FY",
                period_end: "2025-12-31",
                tier: Some("esef"),
                acceptance,
                reason_code: if fact_count > 0 {
                    "emitted"
                } else {
                    "validation_failed"
                },
                detail_json: None,
                drift_json: None,
                structure_changed: false,
                fact_count,
            }
        };
        s.fundamentals_provenance()
            .record_extraction_outcome(outcome(2025, "accepted", 7))
            .expect("emitting outcome");
        s.fundamentals_provenance()
            .record_extraction_outcome(outcome(2024, "flagged", 0))
            .expect("flagged outcome");

        let view = compute_report_documents_view(&s, &c).expect("view");
        let attempted_row = row(&view, &attempted);
        let status = attempted_row
            .extraction
            .as_ref()
            .expect("attempted document carries an indicator");
        assert_eq!(status.status, "has_data", "an emitting slot wins");
        assert_eq!(status.fact_count, 7);
        assert!(
            row(&view, &untouched).extraction.is_none(),
            "no outcome rows means never attempted, not empty"
        );
    }

    /// #155: attempted-but-nothing-found and flagged documents are distinct
    /// from has-data ones.
    #[test]
    fn extraction_indicator_reports_flagged_when_nothing_emitted() {
        let s = state();
        let c = company(&s);
        let doc = document(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        s.fundamentals_provenance()
            .record_extraction_outcome(crate::storage::NewExtractionOutcome {
                company_id: &c,
                report_document_id: &doc,
                fiscal_year: 2025,
                period_type: "FY",
                period_end: "2025-12-31",
                tier: Some("esef"),
                acceptance: "flagged",
                reason_code: "validation_failed",
                detail_json: None,
                drift_json: None,
                structure_changed: false,
                fact_count: 0,
            })
            .expect("flagged outcome");

        let view = compute_report_documents_view(&s, &c).expect("view");
        let status = row(&view, &doc).extraction.as_ref().expect("indicator");
        assert_eq!(status.status, "flagged");
        assert_eq!(status.fact_count, 0);
    }

    /// Issue #243: a `facts_superseded` row (repair migration 0119 — recorded
    /// emission whose facts are no longer at the slot) must not render
    /// "contains data" beside a zero count; without another emitting outcome
    /// the document reads `empty`.
    #[test]
    fn extraction_indicator_does_not_claim_data_for_superseded_facts() {
        let s = state();
        let c = company(&s);
        let doc = document(
            &s,
            &c,
            "Skonsolidowany raport roczny 2025 SSF",
            "x/ssf-2025.pdf",
            true,
        );
        s.fundamentals_provenance()
            .record_extraction_outcome(crate::storage::NewExtractionOutcome {
                company_id: &c,
                report_document_id: &doc,
                fiscal_year: 2025,
                period_type: "FY",
                period_end: "2025-12-31",
                tier: Some("pdf"),
                acceptance: "accepted",
                reason_code: "facts_superseded",
                detail_json: None,
                drift_json: None,
                structure_changed: false,
                fact_count: 0,
            })
            .expect("superseded outcome");

        let view = compute_report_documents_view(&s, &c).expect("view");
        let status = row(&view, &doc).extraction.as_ref().expect("indicator");
        assert_eq!(
            status.status, "empty",
            "an emitting acceptance with superseded facts is not data"
        );
        assert_eq!(status.fact_count, 0);
    }

    #[test]
    fn unparseable_document_with_no_file_gets_null_period() {
        let s = state();
        let c = company(&s);
        let id = document(&s, &c, "Some internal memo", "x/memo", false);

        let view = compute_report_documents_view(&s, &c).expect("view");
        let r = row(&view, &id);
        assert_eq!(r.fiscal_year, None);
        assert_eq!(r.period_type, None);
        assert!(!r.canonical);
    }

    #[test]
    fn empty_company_yields_no_rows() {
        let s = state();
        let c = company(&s);
        let view = compute_report_documents_view(&s, &c).expect("view");
        assert!(view.rows.is_empty());
    }
}
