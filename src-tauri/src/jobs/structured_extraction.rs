//! Structured-first fundamentals extraction service (ADR 0061 S5).
//!
//! Loads a stored report document, runs the deterministic tiered pipeline
//! (ESEF → PDF+profile → HTML witness), and persists the accepted facts with
//! their provenance (source tier + validation verdict + citation) and the
//! learned per-company extraction profile. This is the structured-first path
//! that runs *before* the AI proposal job — AI is only the last resort when no
//! structured tier produces a validated set.
//!
//! Live aggregator (BiznesRadar/Bankier) fetch is gated on a source-specific
//! scraping ADR (see ADR 0061 decision 4), so the witness tier runs with no
//! remote fetch here yet; the pipeline degrades cleanly to ESEF + PDF.

use crate::app_state::AppState;
use crate::fundamentals::extraction::pdf::parse_pdf_text;
use crate::fundamentals::extraction::pipeline::{run_pipeline, Acceptance, PipelineInput};
use crate::fundamentals::extraction::profile::ExtractionProfile;
use crate::fundamentals::extraction::SourceTier;
use crate::report_diff::extraction::{extract_report, SourceFormat};
use crate::storage::StructuredFactInput;

/// The outcome of a structured extraction attempt.
#[derive(Debug, Clone)]
pub struct StructuredExtractionResult {
    pub acceptance: Acceptance,
    /// Which tier produced the accepted (or attempted) facts.
    pub tier: Option<SourceTier>,
    /// Ids of the `financial_facts` this run created.
    pub produced_fact_ids: Vec<String>,
    /// Serialized `DriftReport` when the layout drifted (for the notification).
    pub drift_json: Option<String>,
    /// Whether the pipeline emitted any facts.
    pub emitted: bool,
}

/// Runs the structured pipeline for one report document and persists the
/// result. `confirmation_state` is `auto_unreviewed` for autopilot, `pending`
/// for assist/manual (the existing trust ladder).
pub fn run_structured_extraction(
    state: &AppState,
    company_id: &str,
    report_document_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end: &str,
    confirmation_state: &str,
) -> Result<StructuredExtractionResult, String> {
    // --- Load the document bytes + format -------------------------------
    let document = state
        .get_report_document(report_document_id)
        .map_err(|e| e.to_string())?;
    let local_path = document
        .local_path
        .ok_or_else(|| "the report document has no stored file".to_owned())?;
    let path = state.data_dir().join(&local_path);
    let bytes = std::fs::read(&path).map_err(|e| format!("failed to read report file: {e}"))?;
    let format = SourceFormat::resolve(document.content_type.as_deref(), &local_path);

    // --- Build the pipeline input ---------------------------------------
    let profile = state
        .fundamentals_provenance()
        .get_profile(company_id)
        .map_err(|e| e.to_string())?;

    let (esef_opt, pdf_opt): (Option<Vec<u8>>, Option<String>) = match format {
        SourceFormat::Xhtml => (Some(bytes.clone()), None),
        SourceFormat::Pdf => {
            let extracted = extract_report(&bytes, SourceFormat::Pdf);
            let text = extracted
                .sections
                .iter()
                .map(|s| s.body.clone())
                .collect::<Vec<_>>()
                .join("\n");
            (None, Some(text))
        }
    };

    let input = PipelineInput {
        period_end,
        esef_bytes: esef_opt.as_deref(),
        pdf_text: pdf_opt.as_deref(),
        profile: profile.as_ref(),
        prior: None,
        witness: None,
    };
    let outcome = run_pipeline(&input);

    let drift_json = outcome
        .drift
        .as_ref()
        .and_then(|d| serde_json::to_string(d).ok());

    // --- Persist accepted facts + provenance ----------------------------
    let mut produced_fact_ids = Vec::new();
    if outcome.acceptance.emits() {
        let validation_status = outcome.acceptance.validation_status();
        let tier = outcome.tier.map(|t| t.as_str()).unwrap_or("unknown");
        let store = state.kpi_extraction();
        for fact in &outcome.facts {
            let value = fact.value.to_string();
            let id = store
                .record_structured_fact(StructuredFactInput {
                    company_id,
                    fiscal_year,
                    period_type,
                    period_end: Some(period_end),
                    report_document_id,
                    metric_key: &fact.metric_key,
                    value_numeric: &value,
                    currency: fact.currency.as_deref(),
                    confirmation_state,
                    source_tier: tier,
                    validation_status,
                    citation: Some(&fact.citation),
                })
                .map_err(|e| e.to_string())?;
            if let Some(id) = id {
                produced_fact_ids.push(id);
            }
        }

        // Learn the PDF layout on a clean accept: bootstrap or merge the
        // per-company profile so the next period parses zero-touch.
        if outcome.tier == Some(SourceTier::Pdf) && outcome.acceptance == Acceptance::Accepted {
            if let Some(text) = pdf_opt.as_deref() {
                let parse = parse_pdf_text(text, period_end, profile.as_ref());
                let learned = match &profile {
                    Some(existing) => existing.merge_confirmed(&parse),
                    None => ExtractionProfile::bootstrap(company_id, &parse),
                };
                let _ = state.fundamentals_provenance().upsert_profile(&learned);
            }
        }
    }

    Ok(StructuredExtractionResult {
        acceptance: outcome.acceptance,
        tier: outcome.tier,
        emitted: !produced_fact_ids.is_empty(),
        produced_fact_ids,
        drift_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{open_in_memory_database, CaptureReportDocumentInput, NewCompany};

    const ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
    </html>"#;

    fn seed_esef() -> (AppState, String, String) {
        let dir = std::env::temp_dir().join(format!(
            "brawler-structured-{}-{}",
            std::process::id(),
            "esef"
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = AppState::with_data_dir(connection, dir.clone());
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/annual-2026.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Annual 2026 ESEF".to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join("report.xhtml"), ESEF.as_bytes()).expect("write esef");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("report.xhtml"),
                Some("application/xhtml+xml"),
                None,
                Some(ESEF.len() as i64),
            )
            .expect("mark fetched");
        (state, company.id, document.id)
    }

    #[test]
    fn esef_extraction_persists_facts_with_passed_provenance() {
        let (state, company_id, document_id) = seed_esef();
        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            "auto_unreviewed",
        )
        .expect("structured extraction runs");

        assert!(result.emitted, "ESEF facts should be emitted");
        assert_eq!(result.tier, Some(SourceTier::Esef));
        assert_eq!(result.acceptance, Acceptance::Accepted);
        assert_eq!(result.produced_fact_ids.len(), 3);

        // Every produced fact carries structured provenance: tier + passed status.
        let provenance = state
            .fundamentals_provenance()
            .get_many(&result.produced_fact_ids)
            .expect("provenance");
        assert_eq!(provenance.len(), 3);
        assert!(provenance.iter().all(|p| p.source_tier == "esef"));
        assert!(provenance.iter().all(|p| p.validation_status == "passed"));
    }

    #[test]
    fn missing_file_errors_cleanly() {
        let (state, company_id, document_id) = seed_esef();
        // Remove the file to simulate a broken fetch.
        std::fs::remove_file(state.data_dir().join("report.xhtml")).ok();
        let err = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            "auto_unreviewed",
        );
        assert!(err.is_err());
    }
}
