//! Tier-4 OCR extraction over ownership residuals (v0.57 T8, ADR 0077 over
//! ADR 0072 decision 2a).
//!
//! The deterministic shareholders parser (T3) writes stakes directly and finally;
//! only its residual — a glyph-mangled text layer or an image-rendered table it
//! refuses to guess at — is parked in `ownership_extraction_residual`. This job
//! reuses the ADR 0077 tier-4 **vision/OCR** capability over those residual
//! documents: it OCRs the stored PDF through the routable `vision_extraction`
//! pool, parses the shareholders table out of the OCR markdown with the SAME
//! deterministic parser ([`parse_ocr_shareholders`] — OCR defeats the glyph
//! encoding by reading pixels), and lands the result as a **proposal in the
//! company review surface** (`ownership_ocr_proposals`). Nothing is ever
//! auto-applied: confirmation is the only door to a stake (ADR 0072 decision 2a).
//!
//! **Provider routing.** The `VisionExtraction` capability routes **only** to an
//! explicitly-configured document-native provider (Mistral OCR) — never the
//! general-analysis fallback (Gemini was rejected for tier-4, ADR 0077 verdict).
//! With no vision provider configured, a residual-bearing run is a **clean
//! no-op** (residuals counted as `skipped`, honest messaging), never an error —
//! the AI-boundary rule for an unconfigured provider.
//!
//! **Re-propose rule** (the residual `ocr_state` marker gates re-selection):
//! - `NULL` — never attempted → the **bulk** and manual per-company passes both select it.
//! - `proposed` — a pending OCR proposal awaits review → never re-selected.
//! - `rejected` — the user rejected the proposal → never re-selected.
//! - `no_table` — OCR ran (or the doc is un-OCRable, e.g. an ESEF/iXBRL route) and yielded no shareholders table → the **bulk** pass skips it (no re-spend); the **manual per-company** pass re-arms it (an explicit user retry).
//!
//! Confirm CLEARS the residual entirely (the gap is filled); reject parks it
//! `rejected`. A provider error creates no proposal and leaves the residual
//! `NULL` (retryable) — never a stamp.

use crate::app_state::AppState;
use crate::fundamentals::ownership::{parse_ocr_shareholders, OwnershipParseState};
use crate::jobs::structured_extraction::derive_report_period;
use crate::providers::analysis::{capabilities::AiCapability, AiAnalysisProvider};
use crate::storage::{
    NewOwnershipOcrProposal, OcrHolderRow, ResidualNeedingOcr, OCR_STATE_NO_TABLE,
};

/// Outcome of one OCR-extraction pass over residual documents.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipOcrExtractionSummary {
    /// Residual documents examined this run.
    pub examined: usize,
    /// New/refreshed OCR proposals created (a shareholders table was read).
    pub proposed: usize,
    /// Documents OCR (or the format gate) yielded no shareholders table for —
    /// marked `no_table`, never a stamp.
    pub no_table: usize,
    /// Residuals skipped because no vision provider is configured — a clean
    /// no-op, never an error (they stay eligible for a later run).
    pub skipped: usize,
    /// Provider/read failures — no proposal, surfaced (logged + counted).
    pub errors: usize,
}

impl OwnershipOcrExtractionSummary {
    fn zero() -> Self {
        Self {
            examined: 0,
            proposed: 0,
            no_table: 0,
            skipped: 0,
            errors: 0,
        }
    }
}

/// Bulk pass: OCR every eligible residual across all companies (`ocr_state
/// IS NULL` only — a `no_table` doc is not re-spent). Mirrors
/// `run_ownership_classification`'s global shape.
pub async fn run_ownership_ocr_extraction(
    state: &AppState,
) -> Result<OwnershipOcrExtractionSummary, String> {
    // Bulk pass targets the card's population — `table_unparsable` +
    // `glyph_encoded` residuals never attempted (`ocr_state IS NULL`).
    // `section_missing` is excluded (a document genuinely lacking the section is
    // not worth a provider call); `no_table` is not re-spent.
    let residuals = state
        .ownership()
        .residuals_needing_ocr(None, false, false)
        .map_err(|error| error.to_string())?;
    run_over(state, residuals).await
}

/// Manual per-company pass (the residual warnbox action): OCR the company's
/// eligible residuals AND re-arm its `no_table` ones (an explicit user retry).
pub async fn run_company_ownership_ocr_extraction(
    state: &AppState,
    company_id: &str,
) -> Result<OwnershipOcrExtractionSummary, String> {
    // The manual retry is broader (an explicit user action): re-arm this
    // company's `no_table` residuals AND include `section_missing` ones.
    let residuals = state
        .ownership()
        .residuals_needing_ocr(Some(company_id), true, true)
        .map_err(|error| error.to_string())?;
    run_over(state, residuals).await
}

/// Whether an explicit `VisionExtraction` provider is configured. Mirrors the
/// tier-4 fundamentals path: the vision pool never falls back to the
/// general-analysis provider (it cannot OCR), so an EMPTY explicit config means
/// no provider — checked directly, not via `resolve_capability_members`.
fn has_vision_provider(state: &AppState) -> Result<bool, String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    Ok(settings
        .capability_providers
        .get(AiCapability::VisionExtraction.key())
        .is_some_and(|entries| !entries.is_empty()))
}

/// Drive the pass over a resolved residual set. Isolated from residual selection
/// so tests can pass a fixed set; provider build is short-circuited when there is
/// nothing to do or no vision provider (clean no-op).
async fn run_over(
    state: &AppState,
    residuals: Vec<ResidualNeedingOcr>,
) -> Result<OwnershipOcrExtractionSummary, String> {
    if residuals.is_empty() {
        return Ok(OwnershipOcrExtractionSummary::zero());
    }
    // No vision provider → a clean no-op: count the eligible residuals as skipped
    // (they stay NULL, eligible for a later run once a provider exists).
    if !has_vision_provider(state)? {
        log::info!(
            "module=ownership_ocr stage=no_provider residuals={} — clean no-op",
            residuals.len()
        );
        return Ok(OwnershipOcrExtractionSummary {
            skipped: residuals.len(),
            ..OwnershipOcrExtractionSummary::zero()
        });
    }

    let timeout_seconds = state
        .get_settings()
        .map_err(|error| error.to_string())?
        .ai_providers
        .general_analysis_timeout_seconds;
    let provider = crate::jobs::build_capability_provider(
        state,
        AiCapability::VisionExtraction,
        timeout_seconds,
    )?;

    process_residuals(state, provider.as_ref(), &residuals).await
}

/// The provider-driven core, split out so tests inject a scripted OCR provider
/// without a live pool.
pub(crate) async fn process_residuals(
    state: &AppState,
    provider: &dyn AiAnalysisProvider,
    residuals: &[ResidualNeedingOcr],
) -> Result<OwnershipOcrExtractionSummary, String> {
    let mut summary = OwnershipOcrExtractionSummary::zero();
    for residual in residuals {
        summary.examined += 1;
        match process_one(state, provider, residual).await {
            Ok(true) => summary.proposed += 1,
            Ok(false) => summary.no_table += 1,
            Err(error) => {
                log::warn!(
                    "module=ownership_ocr stage=error companyId={} document={} error={}",
                    residual.company_id,
                    residual.report_document_id,
                    error
                );
                summary.errors += 1;
            }
        }
    }
    log::info!(
        "module=ownership_ocr stage=done providerId={} examined={} proposed={} noTable={} errors={}",
        provider.provider_id(),
        summary.examined,
        summary.proposed,
        summary.no_table,
        summary.errors
    );
    Ok(summary)
}

/// OCR one residual document. `Ok(true)` = a proposal was created; `Ok(false)` =
/// no shareholders table (marked `no_table`); `Err` = a provider/read failure
/// (no proposal, residual left eligible).
async fn process_one(
    state: &AppState,
    provider: &dyn AiAnalysisProvider,
    residual: &ResidualNeedingOcr,
) -> Result<bool, String> {
    let document = state
        .get_report_document(&residual.report_document_id)
        .map_err(|error| error.to_string())?;

    // Resolve WHICH file to OCR (orchestrator gate, T8 real-data): a PDF residual
    // OCRs itself; an xhtml pdf2htmlEX container (unreadable text layer — the very
    // reason it is residual) OCRs its fetched **PDF sibling** of the same company
    // + period; an ESEF package or an xhtml with no PDF sibling is un-OCRable →
    // `no_table` (no provider call, no spend).
    let Some(ocr_document) = resolve_ocr_target(state, &document) else {
        log::info!(
            "module=ownership_ocr stage=no_ocr_target document={} — no OCRable PDF (no sibling / esef package), marking no_table",
            residual.report_document_id
        );
        state
            .ownership()
            .set_residual_ocr_state(&residual.report_document_id, Some(OCR_STATE_NO_TABLE))
            .map_err(|error| error.to_string())?;
        return Ok(false);
    };
    let Some(ocr_local_path) = ocr_document.local_path.clone() else {
        state
            .ownership()
            .set_residual_ocr_state(&residual.report_document_id, Some(OCR_STATE_NO_TABLE))
            .map_err(|error| error.to_string())?;
        return Ok(false);
    };

    let bytes = std::fs::read(state.data_dir().join(&ocr_local_path))
        .map_err(|error| format!("read report file failed: {error}"))?;
    // Servers often deliver report PDFs as octet-stream; the OCR MIME is fixed.
    let markdown = provider
        .ocr_document(&bytes, "application/pdf")
        .await
        .map_err(|error| error.to_string())?;

    let outcome = parse_ocr_shareholders(&markdown);
    if outcome.state != OwnershipParseState::Found || outcome.rows.is_empty() {
        log::info!(
            "module=ownership_ocr stage=no_table document={} parseState={:?}",
            residual.report_document_id,
            outcome.state
        );
        state
            .ownership()
            .set_residual_ocr_state(&residual.report_document_id, Some(OCR_STATE_NO_TABLE))
            .map_err(|error| error.to_string())?;
        return Ok(false);
    }

    // Date the disclosure without ever fabricating one: the residual's resolved
    // date first (T3 order), else the document-period derivation.
    let Some(as_of) = residual
        .detected_as_of
        .clone()
        .or_else(|| derive_report_period(state, &document).map(|(_, _, end)| end))
    else {
        log::warn!(
            "module=ownership_ocr stage=no_as_of document={} — {} row(s) read but no resolvable date, marking no_table",
            residual.report_document_id,
            outcome.rows.len()
        );
        state
            .ownership()
            .set_residual_ocr_state(&residual.report_document_id, Some(OCR_STATE_NO_TABLE))
            .map_err(|error| error.to_string())?;
        return Ok(false);
    };

    let rows = outcome
        .rows
        .iter()
        .map(|row| OcrHolderRow {
            holder_name_raw: row.holder_raw.clone(),
            capital_pct: row.capital_pct.clone(),
            votes_pct: row.votes_pct.clone(),
        })
        .collect();
    state
        .ownership()
        .record_ocr_proposal(NewOwnershipOcrProposal {
            // The residual document (its coverage gap closes on confirm).
            report_document_id: residual.report_document_id.clone(),
            // The document actually OCR'd (the PDF sibling for an xhtml residual).
            source_document_id: ocr_document.id.clone(),
            company_id: residual.company_id.clone(),
            as_of,
            matched_heading: outcome.matched_heading.clone(),
            provider_id: Some(provider.provider_id().to_owned()),
            model: Some(provider.model().to_owned()),
            rows,
        })
        .map_err(|error| error.to_string())?;
    log::info!(
        "module=ownership_ocr stage=proposed document={} source={} rows={}",
        residual.report_document_id,
        ocr_document.id,
        outcome.rows.len()
    );
    Ok(true)
}

/// Resolve the PDF to OCR for a residual document. `None` when nothing is
/// OCRable (an xhtml container with no PDF sibling, or an ESEF report package).
/// A PDF residual OCRs itself; an xhtml pdf2htmlEX container OCRs its fetched PDF
/// sibling of the same company + period ([`find_pdf_sibling`], shared with T5).
fn resolve_ocr_target(
    state: &AppState,
    document: &crate::storage::ReportDocument,
) -> Option<crate::storage::ReportDocument> {
    let local_path = document.local_path.as_deref()?;
    let format = crate::report_diff::extraction::SourceFormat::resolve(
        document.content_type.as_deref(),
        local_path,
    );
    match format {
        // pdf2htmlEX xhtml container → its companion PDF, not the xhtml.
        crate::report_diff::extraction::SourceFormat::Xhtml => {
            crate::jobs::structured_extraction::find_pdf_sibling(state, document)
        }
        // A real PDF OCRs itself — unless it is an ESEF report package (.xbri/.zip
        // resolves to the Pdf branch), which the ESEF tier owns and OCR cannot read.
        crate::report_diff::extraction::SourceFormat::Pdf => {
            if crate::jobs::structured_extraction::is_esef_route(
                document.content_type.as_deref(),
                local_path,
            ) {
                None
            } else {
                Some(document.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::analysis::{
        AnalysisDocument, AnalysisProviderError, AnalysisProviderOutput, AnalysisRequest,
        ResearchBriefProviderOutput, ResearchBriefRequest, ResearchDigestRequest,
    };
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, NewCompany,
        OwnershipExtractionResidual,
    };
    use async_trait::async_trait;
    use tauri::async_runtime::block_on;

    /// A scripted OCR provider: `ocr_document` returns fixed markdown so a test
    /// exercises the parse/proposal path with no network. `Err` variants let a
    /// test drive the provider-error branch.
    struct ScriptedOcr {
        markdown: Result<String, ()>,
    }

    #[async_trait]
    impl AiAnalysisProvider for ScriptedOcr {
        fn provider_id(&self) -> &'static str {
            "test_ocr"
        }
        fn model(&self) -> &str {
            "ocr-v1"
        }
        async fn analyze(
            &self,
            _request: &AnalysisRequest,
        ) -> Result<AnalysisProviderOutput, AnalysisProviderError> {
            unimplemented!("not used by ownership OCR")
        }
        async fn generate_research_brief(
            &self,
            _request: &ResearchBriefRequest,
        ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
            unimplemented!("not used")
        }
        async fn generate_research_digest(
            &self,
            _request: &ResearchDigestRequest,
        ) -> Result<ResearchBriefProviderOutput, AnalysisProviderError> {
            unimplemented!("not used")
        }
        async fn complete_document(
            &self,
            _prompt: &str,
            _document: &AnalysisDocument,
        ) -> Result<String, AnalysisProviderError> {
            unimplemented!("not used")
        }
        async fn ocr_document(
            &self,
            _bytes: &[u8],
            _mime_type: &str,
        ) -> Result<String, AnalysisProviderError> {
            self.markdown
                .clone()
                .map_err(|()| AnalysisProviderError::ProviderError("scripted OCR error".to_owned()))
        }
    }

    /// A synthetic OCR-shaped shareholders table (markdown, never copied from
    /// `private/`). Padded prose so the parser reaches a real anchor window.
    fn ocr_shareholders_markdown() -> String {
        let filler = "Niniejszy raport okresowy zawiera dane oraz komentarz zarzadu. ".repeat(40);
        format!(
            "# Sprawozdanie\n\n{filler}\n\n\
             ## Akcjonariusze posiadajacy co najmniej 5% ogolnej liczby glosow na WZ\n\n\
             | Akcjonariusz | Liczba akcji | % kapitalu | Liczba glosow | % glosow |\n\
             |---|---|---|---|---|\n\
             | Jan Kowalski | 1 234 567 | 12,34 | 2 000 000 | 15,00 |\n\
             | Aviva OFE | 987 654 | 9,88 | 987 654 | 7,41 |\n\
             | Pozostali (free float) | 7 777 779 | 77,78 | 10 000 000 | 77,59 |\n"
        )
    }

    fn unique_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("brawler-ownocr-{}-{n}", std::process::id()))
    }

    fn state_with_dir() -> AppState {
        let dir = unique_dir();
        std::fs::create_dir_all(&dir).expect("temp dir");
        AppState::with_data_dir(open_in_memory_database().expect("in-memory db"), dir)
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

    /// Create a fetched periodic PDF report + park a residual for it. Returns the
    /// document id.
    fn residual_pdf(state: &AppState, company_id: &str, file_name: &str) -> String {
        let doc = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.to_owned(),
                source_type: "user_url".to_owned(),
                url: format!("https://example.com/{file_name}"),
                period_id: None,
                origin_ref: None,
                title: Some("Skonsolidowany raport roczny 2025 SSF".to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(state.data_dir().join(file_name), b"%PDF-1.4 residual").expect("write");
        state
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some("application/pdf"),
                Some("hash"),
                Some(17),
            )
            .expect("fetch");
        state
            .ownership()
            .record_extraction_residual(OwnershipExtractionResidual {
                report_document_id: doc.id.clone(),
                company_id: company_id.to_owned(),
                parse_state: "glyph_encoded".to_owned(),
                detected_as_of: Some("2025-12-31".to_owned()),
                matched_heading: Some("Akcjonariusze".to_owned()),
                ocr_state: None,
                created_at: String::new(),
                updated_at: String::new(),
            })
            .expect("residual");
        doc.id
    }

    #[test]
    fn ocr_run_creates_proposal_leaves_residual_and_writes_no_stakes() {
        let state = state_with_dir();
        let c = company(&state);
        let doc = residual_pdf(&state, &c, "ssf-2025.pdf");

        let residuals = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .expect("residuals");
        assert_eq!(residuals.len(), 1);

        let provider = ScriptedOcr {
            markdown: Ok(ocr_shareholders_markdown()),
        };
        let summary = block_on(process_residuals(&state, &provider, &residuals)).expect("run");
        assert_eq!(summary.proposed, 1, "one table read → one proposal");
        assert_eq!(summary.no_table, 0);
        assert_eq!(summary.errors, 0);

        // A proposal exists with the parsed holder rows; NO stakes were written.
        let proposal = state
            .ownership()
            .get_ocr_proposal(&doc)
            .expect("get")
            .expect("a proposal");
        assert_eq!(proposal.as_of, "2025-12-31");
        assert!(
            proposal
                .rows
                .iter()
                .any(|r| r.holder_name_raw == "Jan Kowalski"),
            "the parsed holder is proposed"
        );
        assert!(
            state
                .ownership()
                .current_state(&c)
                .expect("state")
                .is_empty(),
            "an OCR proposal writes ZERO stakes (confirm is the only door)"
        );

        // The residual is intact but now marked `proposed`, so a re-run does not
        // re-propose (idempotent, no re-spend).
        let residual = state
            .ownership()
            .get_extraction_residual(&doc)
            .expect("residual")
            .expect("still parked");
        assert_eq!(residual.ocr_state.as_deref(), Some("proposed"));
        let rerun = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .expect("rerun");
        assert!(rerun.is_empty(), "a proposed residual is not re-selected");
    }

    #[test]
    fn confirm_applies_stakes_and_clears_residual() {
        let state = state_with_dir();
        let c = company(&state);
        let doc = residual_pdf(&state, &c, "ssf-2025.pdf");
        let residuals = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .unwrap();
        let provider = ScriptedOcr {
            markdown: Ok(ocr_shareholders_markdown()),
        };
        block_on(process_residuals(&state, &provider, &residuals)).expect("run");

        let written = state
            .ownership()
            .confirm_ocr_proposal(&doc)
            .expect("confirm");
        assert!(written >= 1, "confirm writes the proposed rows as stakes");

        let stakes = state.ownership().current_state(&c).expect("state");
        let jan = stakes
            .iter()
            .find(|s| s.holder_name_raw == "Jan Kowalski")
            .expect("Jan stake applied");
        assert_eq!(jan.source, "report_document");
        assert_eq!(jan.report_document_id.as_deref(), Some(doc.as_str()));
        assert_eq!(jan.as_of, "2025-12-31");
        assert_eq!(jan.capital_pct.as_deref(), Some("12.34"));

        // The residual and the proposal are gone; the deterministic pass stamped
        // the OFE type on ingest.
        assert!(
            state
                .ownership()
                .get_extraction_residual(&doc)
                .unwrap()
                .is_none(),
            "confirm clears the residual"
        );
        assert!(
            state.ownership().get_ocr_proposal(&doc).unwrap().is_none(),
            "confirm deletes the proposal"
        );
        let aviva = stakes.iter().find(|s| s.holder_name_raw == "Aviva OFE");
        assert_eq!(
            aviva.and_then(|s| s.holder_type.as_deref()),
            Some("ofe_pension"),
            "deterministic classification stamps the OFE on confirm"
        );
    }

    #[test]
    fn reject_parks_residual_and_is_not_re_proposed() {
        let state = state_with_dir();
        let c = company(&state);
        let doc = residual_pdf(&state, &c, "ssf-2025.pdf");
        let residuals = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .unwrap();
        let provider = ScriptedOcr {
            markdown: Ok(ocr_shareholders_markdown()),
        };
        block_on(process_residuals(&state, &provider, &residuals)).expect("run");

        state.ownership().reject_ocr_proposal(&doc).expect("reject");
        assert!(
            state.ownership().get_ocr_proposal(&doc).unwrap().is_none(),
            "reject deletes the proposal"
        );
        let residual = state
            .ownership()
            .get_extraction_residual(&doc)
            .unwrap()
            .unwrap();
        assert_eq!(residual.ocr_state.as_deref(), Some("rejected"));
        assert!(
            state.ownership().current_state(&c).unwrap().is_empty(),
            "reject writes no stakes"
        );

        // Neither the bulk pass nor the manual per-company pass re-proposes it.
        assert!(state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .unwrap()
            .is_empty());
        assert!(
            state
                .ownership()
                .residuals_needing_ocr(Some(&c), true, true)
                .unwrap()
                .is_empty(),
            "a rejected residual is never re-proposed, even by the manual retry"
        );
    }

    #[test]
    fn empty_ocr_marks_no_table_and_bulk_skips_it_but_manual_rearms() {
        let state = state_with_dir();
        let c = company(&state);
        let doc = residual_pdf(&state, &c, "ssf-2025.pdf");
        let residuals = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .unwrap();
        // OCR returns prose with no shareholders table.
        let provider = ScriptedOcr {
            markdown: Ok("Sprawozdanie finansowe. Bilans i rachunek wynikow.".to_owned()),
        };
        let summary = block_on(process_residuals(&state, &provider, &residuals)).expect("run");
        assert_eq!(summary.no_table, 1);
        assert_eq!(summary.proposed, 0);

        let residual = state
            .ownership()
            .get_extraction_residual(&doc)
            .unwrap()
            .unwrap();
        assert_eq!(residual.ocr_state.as_deref(), Some("no_table"));
        assert!(
            state
                .ownership()
                .residuals_needing_ocr(None, false, false)
                .unwrap()
                .is_empty(),
            "the bulk pass does not re-spend a no_table residual"
        );
        assert_eq!(
            state
                .ownership()
                .residuals_needing_ocr(Some(&c), true, true)
                .unwrap()
                .len(),
            1,
            "the manual per-company pass re-arms a no_table residual"
        );
    }

    #[test]
    fn provider_error_yields_no_proposal_and_leaves_residual_eligible() {
        let state = state_with_dir();
        let c = company(&state);
        let doc = residual_pdf(&state, &c, "ssf-2025.pdf");
        let residuals = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .unwrap();
        let provider = ScriptedOcr { markdown: Err(()) };
        let summary = block_on(process_residuals(&state, &provider, &residuals)).expect("run");
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.proposed, 0);
        assert!(state.ownership().get_ocr_proposal(&doc).unwrap().is_none());
        let residual = state
            .ownership()
            .get_extraction_residual(&doc)
            .unwrap()
            .unwrap();
        assert_eq!(
            residual.ocr_state, None,
            "a provider error leaves the residual eligible for a retry"
        );
    }

    #[test]
    fn no_vision_provider_is_a_clean_no_op_not_an_error() {
        let state = state_with_dir();
        let c = company(&state);
        residual_pdf(&state, &c, "ssf-2025.pdf");
        // No VisionExtraction provider configured.
        let summary = block_on(run_ownership_ocr_extraction(&state)).expect("no-op, not error");
        assert_eq!(summary.skipped, 1, "the residual is counted as skipped");
        assert_eq!(summary.proposed, 0);
        assert_eq!(summary.errors, 0, "no provider is never an error");
    }

    #[test]
    fn no_residuals_is_a_zero_no_op_without_provider_resolution() {
        let state = state_with_dir();
        // No residuals at all → zero summary even with no provider (no resolution).
        let summary = block_on(run_ownership_ocr_extraction(&state)).expect("zero");
        assert_eq!(summary, OwnershipOcrExtractionSummary::zero());
    }

    // ---- Real-data gap (orchestrator T8): xhtml residuals via PDF sibling ----

    /// Create a fetched periodic document of an arbitrary format (no residual).
    fn fetched_periodic_doc(
        state: &AppState,
        company_id: &str,
        file_name: &str,
        content_type: &str,
    ) -> String {
        let doc = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company_id.to_owned(),
                source_type: "user_url".to_owned(),
                url: format!("https://example.com/{file_name}"),
                period_id: None,
                origin_ref: None,
                // Same title on both siblings → same derived report period.
                title: Some("Skonsolidowany raport roczny 2025 SSF".to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(state.data_dir().join(file_name), b"%PDF-1.4 body").expect("write");
        state
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some(content_type),
                Some("hash"),
                Some(12),
            )
            .expect("fetch");
        doc.id
    }

    fn park_glyph_residual(state: &AppState, company_id: &str, doc_id: &str, parse_state: &str) {
        state
            .ownership()
            .record_extraction_residual(OwnershipExtractionResidual {
                report_document_id: doc_id.to_owned(),
                company_id: company_id.to_owned(),
                parse_state: parse_state.to_owned(),
                detected_as_of: Some("2025-12-31".to_owned()),
                matched_heading: Some("Akcjonariusze".to_owned()),
                ocr_state: None,
                created_at: String::new(),
                updated_at: String::new(),
            })
            .expect("residual");
    }

    #[test]
    fn xhtml_residual_ocrs_pdf_sibling_with_proposal_provenance_and_confirm_clears_xhtml() {
        let state = state_with_dir();
        let c = company(&state);
        // A pdf2htmlEX xhtml container (unreadable text layer) is the residual.
        let xhtml = fetched_periodic_doc(&state, &c, "ssf-2025.xhtml", "application/xhtml+xml");
        park_glyph_residual(&state, &c, &xhtml, "glyph_encoded");
        // Its fetched PDF sibling of the SAME company + period carries the content.
        let pdf = fetched_periodic_doc(&state, &c, "ssf-2025.pdf", "application/pdf");

        let residuals = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .expect("residuals");
        assert_eq!(residuals.len(), 1, "only the xhtml residual is queued");

        let provider = ScriptedOcr {
            markdown: Ok(ocr_shareholders_markdown()),
        };
        let summary = block_on(process_residuals(&state, &provider, &residuals)).expect("run");
        assert_eq!(
            summary.proposed, 1,
            "the xhtml resolved via its PDF sibling"
        );

        // Proposal provenance points at the PDF actually read; the proposal keys on
        // the xhtml residual (so confirm closes ITS gap).
        let proposal = state
            .ownership()
            .get_ocr_proposal(&xhtml)
            .expect("get")
            .expect("proposal");
        assert_eq!(
            proposal.source_document_id, pdf,
            "the OCR run's provenance is the PDF sibling actually read"
        );
        assert_eq!(proposal.report_document_id, xhtml);

        // Confirm writes stakes and clears the ORIGINAL xhtml residual.
        let written = state
            .ownership()
            .confirm_ocr_proposal(&xhtml)
            .expect("confirm");
        assert!(written >= 1);
        assert!(
            state
                .ownership()
                .get_extraction_residual(&xhtml)
                .unwrap()
                .is_none(),
            "confirm clears the xhtml residual"
        );
        // Stakes anchor to the residual (xhtml) document so the deterministic
        // catch-up sees coverage and never re-parks/re-OCRs it (no churn loop).
        let stakes = state.ownership().current_state(&c).expect("state");
        let jan = stakes
            .iter()
            .find(|s| s.holder_name_raw == "Jan Kowalski")
            .expect("Jan applied");
        assert_eq!(jan.report_document_id.as_deref(), Some(xhtml.as_str()));
    }

    #[test]
    fn xhtml_residual_without_pdf_sibling_marks_no_table() {
        let state = state_with_dir();
        let c = company(&state);
        let xhtml = fetched_periodic_doc(&state, &c, "ssf-2025.xhtml", "application/xhtml+xml");
        park_glyph_residual(&state, &c, &xhtml, "glyph_encoded");
        // No PDF sibling exists.

        let residuals = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .unwrap();
        let provider = ScriptedOcr {
            markdown: Ok(ocr_shareholders_markdown()),
        };
        let summary = block_on(process_residuals(&state, &provider, &residuals)).expect("run");
        assert_eq!(
            summary.no_table, 1,
            "no sibling → no_table (no provider call)"
        );
        assert_eq!(summary.proposed, 0);
        assert!(state
            .ownership()
            .get_ocr_proposal(&xhtml)
            .unwrap()
            .is_none());
        let residual = state
            .ownership()
            .get_extraction_residual(&xhtml)
            .unwrap()
            .unwrap();
        assert_eq!(residual.ocr_state.as_deref(), Some("no_table"));
    }

    #[test]
    fn bulk_skips_section_missing_but_manual_per_company_includes_it() {
        let state = state_with_dir();
        let c = company(&state);
        let unparsable = fetched_periodic_doc(&state, &c, "a.pdf", "application/pdf");
        park_glyph_residual(&state, &c, &unparsable, "table_unparsable");
        let missing = fetched_periodic_doc(&state, &c, "b.pdf", "application/pdf");
        park_glyph_residual(&state, &c, &missing, "section_missing");

        // Bulk: only the table_unparsable residual (section_missing excluded).
        let bulk = state
            .ownership()
            .residuals_needing_ocr(None, false, false)
            .unwrap();
        assert_eq!(bulk.len(), 1);
        assert_eq!(bulk[0].report_document_id, unparsable);

        // Manual per-company: both (an explicit user retry includes section_missing).
        let manual = state
            .ownership()
            .residuals_needing_ocr(Some(&c), true, true)
            .unwrap();
        assert_eq!(manual.len(), 2, "manual includes section_missing");
    }
}
