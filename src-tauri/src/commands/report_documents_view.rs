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
            ReportDocumentViewRow {
                document,
                fiscal_year,
                period_type,
                canonical,
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
