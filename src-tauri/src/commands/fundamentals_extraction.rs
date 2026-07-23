//! Tauri commands for structured-first fundamentals extraction (ADR 0061 S5).
//!
//! Exposes the deterministic pipeline as an explicit user action over a stored
//! report document (the reachable entry point for the PDF tier, whose period is
//! user-supplied), plus a read for the per-fact provenance the KPI display
//! badges. Extraction touches the filesystem and DB, so it is offloaded off the
//! UI thread.

use serde::{Deserialize, Serialize};

use crate::{app_state, jobs, storage};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct RunStructuredExtractionInput {
    pub company_id: String,
    pub report_document_id: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end: String,
    /// Trust-ladder mode: `autopilot` | `assist`. Defaults to `autopilot` for a
    /// user-invoked deterministic run. The per-fact confirmation state is
    /// derived from the validation outcome (Accepted/witness → `confirmed`;
    /// unproven → `auto_unreviewed`/`pending` by mode), never from a
    /// caller-chosen literal (ADR 0061 dec. 3/8/9).
    pub mode: Option<String>,
}

/// Serializable summary of a structured extraction run for the UI.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct StructuredExtractionSummary {
    /// accepted | accepted_via_witness | accepted_unreviewed | flagged | empty
    pub acceptance: String,
    /// esef | pdf | html_aggregator | … (the tier that produced the facts).
    pub tier: Option<String>,
    pub emitted: bool,
    pub produced_fact_ids: Vec<String>,
    /// Facts already present at their slot on a re-extraction (re-observations):
    /// same value, or a value that diverges from the stored one. Lets the UI
    /// report "already recorded" honestly instead of a bare "no new values".
    pub skipped_fact_ids: Vec<String>,
    /// How many re-observed slots carried a value that disagrees with the stored
    /// fact (never silently overwritten — surfaced for ratification).
    pub divergent_count: i64,
}

/// Normalizes the optional trust-ladder `mode` input (default `autopilot`;
/// rejects anything other than `autopilot`/`assist`) — shared by both extraction
/// entry points so they validate identically.
pub(crate) fn normalize_mode(raw: Option<&str>) -> Result<String, String> {
    match raw.map(str::trim) {
        None | Some("") => Ok(storage::MODE_AUTOPILOT.to_owned()),
        Some(value) if value == storage::MODE_AUTOPILOT || value == storage::MODE_ASSIST => {
            Ok(value.to_owned())
        }
        Some(other) => Err(format!(
            "unknown extraction mode '{other}' (expected 'autopilot' or 'assist')"
        )),
    }
}

/// Maps the internal pipeline result to the serializable UI summary.
pub(crate) fn summarize(
    result: jobs::structured_extraction::StructuredExtractionResult,
) -> StructuredExtractionSummary {
    StructuredExtractionSummary {
        acceptance: result.acceptance.as_str().to_owned(),
        tier: result.tier.map(|t| t.as_str().to_owned()),
        emitted: result.emitted,
        produced_fact_ids: result.produced_fact_ids,
        skipped_fact_ids: result.skipped_fact_ids,
        divergent_count: result.divergences.len() as i64,
    }
}

#[tauri::command]
pub async fn run_structured_extraction(
    input: RunStructuredExtractionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<StructuredExtractionSummary, String> {
    let state = state.inner().clone();
    let mode = normalize_mode(input.mode.as_deref())?;

    // Offload the filesystem read + parse + DB writes off the UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        let result = jobs::structured_extraction::run_structured_extraction(
            &state,
            &input.company_id,
            &input.report_document_id,
            input.fiscal_year,
            &input.period_type,
            &input.period_end,
            &mode,
        )?;
        detect_score_deterioration_after_extraction(&state, &input.company_id, result.emitted);
        Ok(summarize(result))
    })
    .await
    .map_err(|e| format!("structured extraction task failed: {e}"))?
}

/// Red-flag detection at the fact-confirmation seam (ADR 0083 D8, T7): a fresh
/// batch of confirmed facts can flip a company's Piotroski F / Altman Z″ enough to
/// raise a `score_deterioration`. Best-effort — a detection failure never fails
/// the extraction — and only when the run actually emitted facts.
pub(crate) fn detect_score_deterioration_after_extraction(
    state: &app_state::AppState,
    company_id: &str,
    emitted: bool,
) {
    if !emitted {
        return;
    }
    if let Err(error) = state.red_flags().detect_score_deterioration(company_id) {
        log::warn!("score_deterioration detection failed for {company_id}: {error}");
    }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ExtractReportDocumentDataInput {
    pub company_id: String,
    pub report_document_id: String,
    /// Trust-ladder mode (`autopilot` | `assist`, default `autopilot`) — same
    /// semantics as `run_structured_extraction`; the per-fact confirmation state
    /// is derived from the validation outcome, never a caller-chosen literal.
    pub mode: Option<String>,
}

/// The reachable, one-click entry point for the structured pipeline over a
/// **single stored report document** (closes the ADR 0061 S5 live-path gap: the
/// deterministic pipeline previously had no UI caller outside autopilot). Unlike
/// `run_structured_extraction`, the caller supplies no period — it is derived
/// server-side by [`jobs::structured_extraction::derive_report_period`], exactly
/// as the autopilot stage does, so the UI never invents the reporting period.
/// Errors when the period can't be derived (no stored file, unparsable ESEF, or
/// a PDF whose title/URL carries no classifiable period). Confirmation semantics
/// are unchanged from the pipeline — facts land per `mode` + validation outcome.
#[tauri::command]
pub async fn extract_report_document_data(
    input: ExtractReportDocumentDataInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<StructuredExtractionSummary, String> {
    let state = state.inner().clone();
    let mode = normalize_mode(input.mode.as_deref())?;

    tauri::async_runtime::spawn_blocking(move || {
        let document_id = input.report_document_id.clone();
        // Diagnostics lifecycle (T7-A pattern, owner T7): a zero-fact run must
        // never be invisible. Developer-mode gated inside `record_diagnostic_event`,
        // best-effort, so these are no-ops in normal use.
        record_extraction_diagnostic(
            &state,
            &document_id,
            "started",
            "info",
            "Report-document extraction started.",
            serde_json::json!({
                "companyId": input.company_id,
                "reportDocumentId": document_id,
                "mode": mode,
            }),
        );

        let document = state
            .get_report_document(&document_id)
            .map_err(|e| e.to_string())?;
        let Some((fiscal_year, period_type, period_end)) =
            jobs::structured_extraction::derive_report_period(&state, &document)
        else {
            // Honest failure: an unrecognised format or a non-iXBRL render (e.g.
            // a pdf2htmlEX visual `.xhtml`) — never a silent empty-success.
            let reason = "could not determine the reporting period for this document — its \
                 title/URL carries no parseable period, or the stored file is not a \
                 recognised report format (a non-tagged visual .xhtml render, or a \
                 report package with no inline-XBRL instance)"
                .to_owned();
            // Durable record, not just a developer-mode diagnostic: a document
            // the pipeline tried and could not place is precisely the "flagged
            // with a typed reason, never silently absent" case (ADR 0084 dec. 4).
            // There is no period to key the slot by — that IS the failure — so
            // the sentinel period (`0` / `""` / `""`) keys it by document alone.
            record_no_period_outcome(&state, &input.company_id, &document_id, &reason);
            record_extraction_diagnostic(
                &state,
                &document_id,
                "no_period",
                "warning",
                "Extraction skipped: reporting period could not be derived.",
                serde_json::json!({
                    "reportDocumentId": document_id,
                    "contentType": document.content_type,
                    "reason": reason,
                }),
            );
            return Err(reason);
        };

        let result = jobs::structured_extraction::run_structured_extraction(
            &state,
            &input.company_id,
            &document_id,
            fiscal_year,
            period_type,
            &period_end,
            &mode,
        );

        match &result {
            Ok(res) => {
                let (stage, severity, message) = extraction_outcome_diagnostic(res.emitted);
                record_extraction_diagnostic(
                    &state,
                    &document_id,
                    stage,
                    severity,
                    message,
                    serde_json::json!({
                        "reportDocumentId": document_id,
                        "fiscalYear": fiscal_year,
                        "periodType": period_type,
                        "periodEnd": period_end,
                        "acceptance": res.acceptance.as_str(),
                        "tier": res.tier.map(|t| t.as_str()),
                        "emitted": res.emitted,
                        "factCount": res.produced_fact_ids.len(),
                        "skippedCount": res.skipped_fact_ids.len(),
                        "divergentCount": res.divergences.len(),
                    }),
                );
                // Each re-observed slot whose fresh value disagrees with the
                // stored (possibly confirmed) fact is never silently overwritten
                // (owner T7) — surface it as a warning for ratification.
                for divergence in &res.divergences {
                    record_extraction_diagnostic(
                        &state,
                        &document_id,
                        "value_divergence",
                        "warning",
                        "Re-extraction observed a different value than the stored fact — kept the stored value.",
                        serde_json::json!({
                            "reportDocumentId": document_id,
                            "factId": divergence.fact_id,
                            "metricKey": divergence.metric_key,
                            "storedValue": divergence.existing,
                            "extractedValue": divergence.incoming,
                        }),
                    );
                }
            }
            Err(error) => record_extraction_diagnostic(
                &state,
                &document_id,
                "failed",
                "error",
                "Report-document extraction failed.",
                serde_json::json!({
                    "reportDocumentId": document_id,
                    "error": error,
                }),
            ),
        }

        if let Ok(res) = &result {
            detect_score_deterioration_after_extraction(&state, &input.company_id, res.emitted);
        }
        result.map(summarize)
    })
    .await
    .map_err(|e| format!("structured extraction task failed: {e}"))?
}

/// The sentinel period for an outcome recorded when the reporting period could
/// **not** be derived. A period-less failure has no `(year, type, end)` to key
/// its slot by — that absence is the failure itself — so the slot is keyed by
/// document alone and the period fields carry these sentinels. Readers must
/// treat `PERIOD_TYPE`/`PERIOD_END` empty as "period unknown", never as a real
/// period (documented in data-model.md).
pub(crate) mod no_period_sentinel {
    pub const FISCAL_YEAR: i64 = 0;
    pub const PERIOD_TYPE: &str = "";
    pub const PERIOD_END: &str = "";
}

/// A reporting period at the extraction-outcome boundary. `Unknown` is the typed
/// counterpart of the [`no_period_sentinel`] DB encoding (fiscal year `0`, empty
/// type/end): a period that could not be derived. Keeping it an enum in Rust
/// means no caller can mistake the `0` sentinel for a real fiscal year — the
/// sentinel encoding exists only inside [`PeriodSlot::to_sentinel`] /
/// [`PeriodSlot::from_stored`], never in flight through construction or read code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PeriodSlot {
    Known {
        fiscal_year: i64,
        period_type: String,
        period_end: String,
    },
    Unknown,
}

impl PeriodSlot {
    /// The stored `(fiscal_year, period_type, period_end)` triple. The sentinel
    /// encoding for `Unknown` lives here and nowhere else.
    fn to_sentinel(&self) -> (i64, &str, &str) {
        match self {
            PeriodSlot::Known {
                fiscal_year,
                period_type,
                period_end,
            } => (*fiscal_year, period_type.as_str(), period_end.as_str()),
            PeriodSlot::Unknown => (
                no_period_sentinel::FISCAL_YEAR,
                no_period_sentinel::PERIOD_TYPE,
                no_period_sentinel::PERIOD_END,
            ),
        }
    }

    /// Interprets a stored outcome's period triple. The sentinel (empty
    /// type/end) reads back as `Unknown`; anything else is a `Known` period —
    /// so a stored fiscal year is only ever surfaced through the `Known` arm.
    #[cfg_attr(not(test), allow(dead_code))]
    fn from_stored(fiscal_year: i64, period_type: &str, period_end: &str) -> Self {
        if period_type.is_empty() && period_end.is_empty() {
            PeriodSlot::Unknown
        } else {
            PeriodSlot::Known {
                fiscal_year,
                period_type: period_type.to_owned(),
                period_end: period_end.to_owned(),
            }
        }
    }
}

/// Records the durable `no_period_derived` outcome for a document whose
/// reporting period could not be derived. Best-effort: the caller's error is the
/// more important signal, so a bookkeeping failure is logged, never propagated.
fn record_no_period_outcome(
    state: &app_state::AppState,
    company_id: &str,
    report_document_id: &str,
    reason: &str,
) {
    log::info!(
        "module=structured_extraction stage=outcome company={company_id} \
         document={report_document_id} acceptance=empty reason=no_period_derived"
    );
    let detail = serde_json::to_string(&serde_json::json!({ "error": reason })).ok();
    // The typed `Unknown` period converts to the DB sentinel only here, at the
    // storage boundary — no other code in this file handles the raw `0`/`""`.
    let unknown_period = PeriodSlot::Unknown;
    let (fiscal_year, period_type, period_end) = unknown_period.to_sentinel();
    if let Err(error) =
        state
            .fundamentals_provenance()
            .record_extraction_outcome(storage::NewExtractionOutcome {
                company_id,
                report_document_id,
                fiscal_year,
                period_type,
                period_end,
                tier: None,
                acceptance: "empty",
                reason_code: "no_period_derived",
                detail_json: detail.as_deref(),
                drift_json: None,
                structure_changed: false,
                fact_count: 0,
            })
    {
        log::warn!(
            "module=structured_extraction stage=outcome_record_failed \
             company={company_id} document={report_document_id} error={error}"
        );
    }
}

/// The `(stage, severity, message)` for a **completed** on-demand extraction
/// run: an issuer emit or an empty run. (The witness-fallback middle state is
/// retired with ADR 0086 — the aggregator sources facts through its own primary
/// pull, never through an extraction run.) Messages follow this command's
/// existing idiom — raw English diagnostic strings, not translation codes.
fn extraction_outcome_diagnostic(emitted: bool) -> (&'static str, &'static str, &'static str) {
    if emitted {
        (
            "completed",
            "info",
            "Report-document extraction completed with facts.",
        )
    } else {
        (
            "empty",
            "warning",
            "Report-document extraction produced no facts.",
        )
    }
}

/// Record one report-document extraction diagnostic event (developer-mode
/// gated, best-effort — mirrors `qualitative_assessment::record_qualitative_diagnostic`
/// and `ai_analysis::record_ai_analysis_diagnostic`). Scope id is the report
/// document id, so the Diagnostics view groups a document's whole extraction
/// lifecycle (`started` → `completed`/`empty`/`no_period`/`failed`).
fn record_extraction_diagnostic(
    state: &app_state::AppState,
    report_document_id: &str,
    stage: &str,
    severity: &str,
    message: &str,
    metadata: serde_json::Value,
) {
    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "fundamentals_extraction".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "extract_report_document".to_owned(),
            id: Some(report_document_id.to_owned()),
        }),
        stage: stage.to_owned(),
        severity: severity.to_owned(),
        message: message.to_owned(),
        metadata: Some(metadata),
    });
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFactProvenanceInput {
    pub fact_ids: Vec<String>,
}

#[tauri::command]
pub fn list_fact_provenance(
    input: ListFactProvenanceInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::FactProvenance>, String> {
    state
        .fundamentals_provenance()
        .get_many(&input.fact_ids)
        .map_err(|error| error.to_string())
}

/// Every fact currently flagged by the pipeline (drift / contradiction) — the
/// "structure changed" review surface.
#[tauri::command]
pub fn list_flagged_fact_provenance(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::FactProvenance>, String> {
    state
        .fundamentals_provenance()
        .list_flagged()
        .map_err(|error| error.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFlaggedExtractionOutcomesInput {
    pub company_id: String,
}

/// A company's **non-emitting** extraction outcomes — the periods where the
/// deterministic pipeline ran and refused to emit, with the typed reason and
/// failing-check detail (ADR 0061 decision 2, ADR 0084 decision 4/6).
///
/// This is the counterpart to `list_flagged_fact_provenance`: that one lists
/// flagged *facts*, which by construction only exist where something WAS
/// emitted. This lists the periods where nothing was — the gaps the user most
/// needs to see, and which were previously invisible. Clean periods are
/// excluded; absence of a row still means "never attempted".
#[tauri::command]
pub fn list_flagged_extraction_outcomes(
    input: ListFlaggedExtractionOutcomesInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::ExtractionOutcome>, String> {
    state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&input.company_id)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct RerunExtractionOutcomeInput {
    /// The recorded outcome slot to re-run (`ExtractionOutcome.id`).
    pub outcome_id: String,
    /// Trust-ladder mode (`autopilot` | `assist`, default `autopilot`) — same
    /// semantics as `run_structured_extraction`.
    pub mode: Option<String>,
}

/// Re-runs the deterministic pipeline for a **recorded outcome slot** — the
/// "try again" action on a flagged period.
///
/// Takes the company/document/period from the stored outcome rather than asking
/// the caller to re-derive them, so the retry can never target a different slot
/// than the one shown. The re-run updates that same row in place: a period whose
/// cause has been fixed leaves the flagged list instead of leaving a stale flag
/// beside a fresh success. Offloaded (`spawn_blocking`) — it reads the stored
/// file, parses it, and writes to the DB.
#[tauri::command]
pub async fn rerun_extraction_outcome(
    input: RerunExtractionOutcomeInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<StructuredExtractionSummary, String> {
    let state = state.inner().clone();
    let mode = normalize_mode(input.mode.as_deref())?;

    tauri::async_runtime::spawn_blocking(move || {
        let company_id = state
            .fundamentals_provenance()
            .get_extraction_outcome(&input.outcome_id)
            .map_err(|e| e.to_string())?
            .map(|outcome| outcome.company_id);
        let result = jobs::structured_extraction::rerun_extraction_outcome(
            &state,
            &input.outcome_id,
            &mode,
        )?;
        if let Some(company_id) = company_id.as_deref() {
            detect_score_deterioration_after_extraction(&state, company_id, result.emitted);
        }
        Ok(summarize(result))
    })
    .await
    .map_err(|e| format!("structured extraction task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use crate::storage::{open_in_memory_database, CaptureReportDocumentInput, NewCompany};

    /// This module's own source, for the structural offloading guard below.
    const SOURCE: &str = include_str!("fundamentals_extraction.rs");

    /// Every extraction command reads a stored file, parses it, and writes to
    /// the database — meaningful blocking IO that must never run on the UI
    /// thread (CLAUDE.md "keep non-trivial work off the UI thread"; Definition
    /// of Done §C). A future edit that drops `spawn_blocking`, or turns one of
    /// these back into a synchronous command, freezes the app for the duration
    /// of a PDF parse. Nothing else catches that — it is invisible to a
    /// behavioural test and to clippy — so it is caught here, at the source.
    ///
    /// Deliberately narrow: only the commands that do extraction work. Pure
    /// indexed reads in this file (`list_fact_provenance`,
    /// `list_flagged_fact_provenance`, `list_flagged_extraction_outcomes`) are
    /// single queries and are correctly synchronous.
    #[test]
    fn every_extraction_command_is_offloaded_off_the_ui_thread() {
        for command in [
            "run_structured_extraction",
            "extract_report_document_data",
            "rerun_extraction_outcome",
        ] {
            let signature = format!("pub async fn {command}(");
            let start = SOURCE.find(&signature).unwrap_or_else(|| {
                panic!(
                    "`{command}` must be declared `pub async fn` — a synchronous \
                     extraction command blocks the UI thread"
                )
            });
            // Scope to THIS function's body: from its signature to the first
            // top-level closing brace (`\n}`), which is the end of the fn. Note
            // the earlier version of this guard scoped to the next
            // `#[tauri::command]` and so ran past the last command in the file
            // into this very test module — whose assert message contains the
            // literal `spawn_blocking` — and passed while the offload was gone.
            // A guard that cannot fail is worse than no guard.
            let rest = &SOURCE[start..];
            let body_end = rest.find("\n}").map(|i| i + 2).unwrap_or(rest.len());
            assert!(
                rest[..body_end].contains("spawn_blocking"),
                "`{command}` must offload its filesystem/DB work via \
                 `tauri::async_runtime::spawn_blocking`"
            );
        }
    }

    /// ADR 0084 decision 4: a document the pipeline cannot place is flagged with
    /// a typed reason, **never silently absent**. A period that cannot be derived
    /// used to leave only a developer-mode diagnostic — invisible in normal use,
    /// and gone in 7 days. It must leave a durable outcome row instead.
    #[test]
    fn undecidable_period_records_a_durable_no_period_outcome() {
        let dir = std::env::temp_dir().join(format!(
            "brawler-noperiod-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let connection = open_in_memory_database().expect("db");
        let state = crate::app_state::AppState::with_data_dir(connection, dir.clone());
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
        // A title and URL carrying no parseable period at all.
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "user_url".to_owned(),
                url: "https://example.com/document.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Some untitled attachment".to_owned()),
                attribution: None,
            })
            .expect("document");
        std::fs::write(dir.join("doc.pdf"), b"%PDF-1.4\n").expect("write pdf");
        state
            .mark_report_document_fetched(
                &document.id,
                Some("doc.pdf"),
                Some("application/pdf"),
                None,
                Some(9),
            )
            .expect("mark fetched");

        // Nothing recorded yet — absence still means "never attempted".
        assert!(state
            .fundamentals_provenance()
            .list_flagged_extraction_outcomes(&company.id)
            .expect("list outcomes")
            .is_empty());

        let stored = state.get_report_document(&document.id).expect("document");
        assert!(
            crate::jobs::structured_extraction::derive_report_period(&state, &stored).is_none(),
            "the fixture must genuinely have no derivable period"
        );
        super::record_no_period_outcome(
            &state,
            &company.id,
            &document.id,
            "could not determine the reporting period for this document",
        );

        let outcomes = state
            .fundamentals_provenance()
            .list_flagged_extraction_outcomes(&company.id)
            .expect("list outcomes");
        assert_eq!(
            outcomes.len(),
            1,
            "an undecidable period must leave a durable record, not just a diagnostic"
        );
        assert_eq!(outcomes[0].reason_code, "no_period_derived");
        assert_eq!(outcomes[0].report_document_id, document.id);
        assert_eq!(outcomes[0].tier, None);
        // The sentinel period marks "period unknown" — never a real period.
        assert_eq!(
            outcomes[0].fiscal_year,
            super::no_period_sentinel::FISCAL_YEAR
        );
        assert_eq!(
            outcomes[0].period_type,
            super::no_period_sentinel::PERIOD_TYPE
        );
        assert_eq!(
            outcomes[0].period_end,
            super::no_period_sentinel::PERIOD_END
        );
        // Read back through the typed boundary: the stored sentinel resolves to
        // the typed `Unknown`, so no reader treats the `0` as a real fiscal year.
        assert_eq!(
            super::PeriodSlot::from_stored(
                outcomes[0].fiscal_year,
                &outcomes[0].period_type,
                &outcomes[0].period_end,
            ),
            super::PeriodSlot::Unknown
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The typed period boundary round-trips: `Unknown` ↔ the DB sentinel, and a
    /// real period ↔ `Known` — the `0` sentinel is only ever produced by
    /// `Unknown` and only ever read back as `Unknown`, never as a `Known` year.
    #[test]
    fn period_slot_round_trips_without_leaking_the_sentinel() {
        use super::PeriodSlot;

        let (fy, pt, pe) = PeriodSlot::Unknown.to_sentinel();
        assert_eq!(fy, super::no_period_sentinel::FISCAL_YEAR);
        assert_eq!(PeriodSlot::from_stored(fy, pt, pe), PeriodSlot::Unknown);

        let known = PeriodSlot::Known {
            fiscal_year: 2025,
            period_type: "annual".to_owned(),
            period_end: "2025-12-31".to_owned(),
        };
        let (fy, pt, pe) = known.to_sentinel();
        assert_eq!(fy, 2025);
        assert_eq!(PeriodSlot::from_stored(fy, pt, pe), known);
    }

    /// An issuer emit and an empty run stay distinguishable (the witness-fallback
    /// middle state is retired with ADR 0086 — the aggregator sources facts
    /// through its own primary pull, never through an extraction run).
    #[test]
    fn extraction_outcome_diagnostic_is_two_way() {
        let issuer = super::extraction_outcome_diagnostic(true);
        let empty = super::extraction_outcome_diagnostic(false);

        assert_eq!(issuer.0, "completed");
        assert_eq!(empty.0, "empty");
        assert_ne!(issuer.2, empty.2);
        assert_ne!(issuer.1, empty.1); // severity differs from the info emit
    }
}
