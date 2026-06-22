//! Report-over-report diff commands (v0.47.0, ADR 0052).
//!
//! `extract_report_sections` runs the async, offloaded extraction job for one
//! financial-statement document. `list_report_diff_candidates` and
//! `get_report_diff` are backend read models: the diff is computed on demand from
//! the persisted `report_document_sections` index — never stored. No AI.

use serde::{Deserialize, Serialize};

use crate::report_diff::{
    classify_statement, diff_sections, period_sort_key, Section, SectionsDiff,
};
use crate::storage::ReportDocument;
use crate::{app_state, jobs};

// ============================================================================
// DTOs
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ExtractReportSectionsInput {
    pub report_document_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ExtractReportSectionsResult {
    /// `extracted` | `no_text_layer` | `extraction_failed`.
    pub extraction_state: String,
    /// `pdf` | `xhtml`.
    pub source_format: String,
    pub section_count: i64,
    pub skipped: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportDiffCandidatesInput {
    pub company_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportDiffDocumentRef {
    pub report_document_id: String,
    pub title: Option<String>,
    pub period_label: Option<String>,
    /// `extracted` | `extraction_pending` | `no_text_layer` | `extraction_failed`.
    pub extraction_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportDiffCandidate {
    /// `ssf` | `jsf`.
    pub statement_type: String,
    /// `pdf` | `xhtml` (the newer document's source format).
    pub source_format: String,
    pub older: ReportDiffDocumentRef,
    pub newer: ReportDiffDocumentRef,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportDiffCandidates {
    pub company_id: String,
    pub candidates: Vec<ReportDiffCandidate>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct GetReportDiffInput {
    pub older_report_document_id: String,
    pub newer_report_document_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FetchReportDocumentInput {
    pub report_document_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FetchReportDocumentResult {
    pub report_document_id: String,
    pub fetched: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportDiffResult {
    /// `ssf` | `jsf`.
    pub statement_type: String,
    /// `ok` | `extraction_pending` | `not_diffable`.
    pub status: String,
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub diff: Option<SectionsDiff>,
}

// ============================================================================
// Commands
// ============================================================================

#[tauri::command]
pub async fn extract_report_sections(
    input: ExtractReportSectionsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<ExtractReportSectionsResult, String> {
    // CPU-bound (PDF/xhtml parse); run off the UI thread.
    let app = state.inner().clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        jobs::report_extraction::run_report_extraction(&app, &input.report_document_id)
    })
    .await
    .map_err(|error| error.to_string())??;

    Ok(ExtractReportSectionsResult {
        extraction_state: outcome.state.as_str().to_owned(),
        source_format: outcome.format.as_str().to_owned(),
        section_count: outcome.section_count as i64,
        skipped: outcome.skipped,
    })
}

#[tauri::command]
pub fn list_report_diff_candidates(
    input: ReportDiffCandidatesInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<ReportDiffCandidates, String> {
    let documents = state
        .report_documents()
        .list_report_documents_by_company(&input.company_id)
        .map_err(|error| error.to_string())?;
    let candidates = build_candidates(&state, documents)?;
    Ok(ReportDiffCandidates {
        company_id: input.company_id,
        candidates,
    })
}

#[tauri::command]
pub async fn get_report_diff(
    input: GetReportDiffInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<ReportDiffResult, String> {
    let app = state.inner().clone();
    // The diff over already-extracted sections runs the `similar` text diff; offload it.
    tauri::async_runtime::spawn_blocking(move || build_diff(&app, &input))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn fetch_report_document(
    input: FetchReportDocumentInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<FetchReportDocumentResult, String> {
    // Network + disk IO; run off the UI thread. Idempotent (already-fetched is a no-op).
    let app = state.inner().clone();
    let document_id = input.report_document_id;
    tauri::async_runtime::spawn_blocking(move || {
        let fetcher = crate::document_fetcher::HttpDocumentFetcher::new();
        crate::report_documents_capture::fetch_report_document(&app, &fetcher, &document_id).map(
            |document| FetchReportDocumentResult {
                report_document_id: document.id,
                fetched: document.local_path.is_some(),
            },
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

// ============================================================================
// Read-model assembly
// ============================================================================

/// The persisted extraction status of a document, or `extraction_pending` when it
/// has not been extracted yet.
fn extraction_status(state: &app_state::AppState, document_id: &str) -> Result<String, String> {
    Ok(state
        .report_sections()
        .extraction(document_id)
        .map_err(|error| error.to_string())?
        .map(|e| e.extraction_state)
        .unwrap_or_else(|| "extraction_pending".to_owned()))
}

fn doc_ref(
    state: &app_state::AppState,
    document: &ReportDocument,
) -> Result<ReportDiffDocumentRef, String> {
    let label = document
        .title
        .as_deref()
        .and_then(|t| crate::report_diff::period_label(t, &document.url));
    Ok(ReportDiffDocumentRef {
        report_document_id: document.id.clone(),
        title: document.title.clone(),
        period_label: label,
        extraction_status: extraction_status(state, &document.id)?,
    })
}

/// Group a company's financial statements by type, order each group
/// chronologically, and pair each statement with its immediate predecessor.
fn build_candidates(
    state: &app_state::AppState,
    documents: Vec<ReportDocument>,
) -> Result<Vec<ReportDiffCandidate>, String> {
    // Keep financial statements (downloaded or downloadable on demand), tagged with
    // type + a sort key. `metadata_only` documents are non-periodic by policy
    // (ADR 0036) and never carry a statement file, so they are excluded; `pending`
    // statements are kept and fetched on demand when compared (ADR 0052).
    let mut typed: Vec<(crate::report_diff::StatementType, (i32, u8), ReportDocument)> = documents
        .into_iter()
        .filter(|d| d.fetch_status != "metadata_only")
        .filter_map(|d| {
            let title = d.title.clone().unwrap_or_default();
            let statement = classify_statement(&title, &d.url)?;
            // Period sort key, falling back to created_at when the period is unparseable.
            let key = period_sort_key(&title, &d.url).unwrap_or((0, 0));
            Some((statement, key, d))
        })
        .collect();

    // Order by type, then chronology, with a representative preference within a
    // period: prefer the Polish filing over an English duplicate, then PDF over
    // xhtml, then created_at — so dedup below keeps one document per (type, period)
    // and pairs are consecutive *distinct* periods (no PL-vs-ENG same-period diffs).
    let eng = |d: &ReportDocument| {
        let t = format!("{} {}", d.title.clone().unwrap_or_default(), d.url).to_lowercase();
        i32::from(t.contains("_eng") || t.contains("eng.") || t.contains("english"))
    };
    let xhtml = |d: &ReportDocument| i32::from(d.url.to_lowercase().ends_with(".xhtml"));
    typed.sort_by(|a, b| {
        a.0.as_str()
            .cmp(b.0.as_str())
            .then(a.1.cmp(&b.1))
            .then(eng(&a.2).cmp(&eng(&b.2)))
            .then(xhtml(&a.2).cmp(&xhtml(&b.2)))
            .then(a.2.created_at.cmp(&b.2.created_at))
    });
    // Keep one representative document per (type, period).
    let mut seen = std::collections::HashSet::new();
    typed.retain(|(st, key, _)| seen.insert((*st, *key)));

    let mut candidates = Vec::new();
    for window in typed.windows(2) {
        let (older_type, _, older) = &window[0];
        let (newer_type, _, newer) = &window[1];
        if older_type != newer_type {
            continue; // different statement types never pair
        }
        let source_format =
            crate::report_diff::SourceFormat::resolve(newer.content_type.as_deref(), &newer.url);
        candidates.push(ReportDiffCandidate {
            statement_type: newer_type.as_str().to_owned(),
            source_format: source_format.as_str().to_owned(),
            older: doc_ref(state, older)?,
            newer: doc_ref(state, newer)?,
        });
    }
    // Newest pairs first.
    candidates.reverse();
    Ok(candidates)
}

fn load_sections(state: &app_state::AppState, document_id: &str) -> Result<Vec<Section>, String> {
    Ok(state
        .report_sections()
        .sections(document_id)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|s| Section {
            ordinal: s.ordinal,
            heading: s.heading,
            body: s.body,
        })
        .collect())
}

fn build_diff(
    state: &app_state::AppState,
    input: &GetReportDiffInput,
) -> Result<ReportDiffResult, String> {
    let docs = state.report_documents();
    let older = docs
        .get_report_document(&input.older_report_document_id)
        .map_err(|error| error.to_string())?;
    let newer = docs
        .get_report_document(&input.newer_report_document_id)
        .map_err(|error| error.to_string())?;
    if older.company_id != newer.company_id {
        return Err("company_mismatch".to_owned());
    }
    let older_type = classify_statement(older.title.as_deref().unwrap_or(""), &older.url)
        .ok_or_else(|| "not_a_financial_statement".to_owned())?;
    let newer_type = classify_statement(newer.title.as_deref().unwrap_or(""), &newer.url)
        .ok_or_else(|| "not_a_financial_statement".to_owned())?;
    if older_type != newer_type {
        return Err("statement_type_mismatch".to_owned());
    }

    let older_status = extraction_status(state, &older.id)?;
    let newer_status = extraction_status(state, &newer.id)?;
    let pending = older_status == "extraction_pending" || newer_status == "extraction_pending";
    let blocked = matches!(older_status.as_str(), "no_text_layer" | "extraction_failed")
        || matches!(newer_status.as_str(), "no_text_layer" | "extraction_failed");

    if pending {
        return Ok(ReportDiffResult {
            statement_type: newer_type.as_str().to_owned(),
            status: "extraction_pending".to_owned(),
            diff: None,
        });
    }
    if blocked {
        return Ok(ReportDiffResult {
            statement_type: newer_type.as_str().to_owned(),
            status: "not_diffable".to_owned(),
            diff: None,
        });
    }

    let older_sections = load_sections(state, &older.id)?;
    let newer_sections = load_sections(state, &newer.id)?;
    let diff = diff_sections(&older_sections, &newer_sections);
    Ok(ReportDiffResult {
        statement_type: newer_type.as_str().to_owned(),
        status: "ok".to_owned(),
        diff: Some(diff),
    })
}
