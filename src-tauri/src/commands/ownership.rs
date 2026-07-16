//! Ownership overview read model + review commands (v0.56 T6, ADR 0072).
//!
//! Feeds the "Akcjonariat" section of the Basic Info panel
//! (`src/shared/components/OwnershipSection.tsx`): current stakes grouped by
//! holder type, derived free float, per-holder capital history, extraction
//! residuals awaiting OCR/AI, and pending AI holder-type proposals awaiting
//! confirmation. A computed read model over the append-only ownership store
//! (`storage::ownership`) — no stored projection, so it can never go stale.
//!
//! The mutation commands (`set_ownership_holder_type`,
//! `confirm/reject_ownership_holder_type_proposal`) return the FRESHLY recomputed
//! overview so the UI updates in one round-trip; `backfill_ownership_extraction`
//! enqueues the deterministic extraction jobs and returns how many documents were
//! queued; `run_ownership_classification` drives the AI classify-with-confirm job
//! (T5 core) directly, off the UI thread.

use serde::Serialize;
use std::collections::BTreeMap;

use crate::app_state::AppState;

// ============================================================================
// DTOs (ts-rs export → ../../src/api/generated/)
// ============================================================================

/// Everything the Ownership section renders for one company.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipOverview {
    pub company_id: String,
    /// Latest disclosed `as_of` across current holders (the "stan na …" header);
    /// `None` when there are no stakes yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub as_of: Option<String>,
    /// Source tag of the latest-`as_of` stake (e.g. `report_document`), for the
    /// provenance label; `None` when empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub source: Option<String>,
    /// Derived free float = `100 − Σ disclosed capital`, floored at 0
    /// (decimal-exact TEXT). Always present, `"100"` when nothing disclosed.
    pub free_float_pct: String,
    /// Σ of disclosed `capital_pct` across current holders (decimal-exact TEXT).
    pub disclosed_sum: String,
    /// Current state: latest disclosed stake per holder.
    pub holders: Vec<OwnershipHolder>,
    /// Per-current-holder capital-% trajectory over time (chronological).
    pub history: Vec<OwnershipHolderSeries>,
    /// Derived free float per report disclosure basis, chronological
    /// (owner dogfooding round 3: the float joins the stakes-over-time chart).
    pub free_float_history: Vec<OwnershipFreeFloatPoint>,
    /// Documents whose shareholders table the deterministic parser could not read
    /// (glyph-mangled / image table / missing section) — awaiting OCR/AI.
    pub residuals: Vec<OwnershipResidual>,
    /// Pending AI holder-type proposals awaiting user confirmation.
    pub pending_proposals: Vec<OwnershipProposal>,
}

/// One current holder (latest disclosed stake).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipHolder {
    /// Normalized name — the stable key for `set_ownership_holder_type`.
    pub holder_key: String,
    /// Raw display name.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub holder_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub capital_pct: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub votes_pct: Option<String>,
    pub as_of: String,
    pub source: String,
}

/// A holder's capital-% trajectory over time.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipHolderSeries {
    pub holder_key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub holder_type: Option<String>,
    /// Chronological (oldest → newest) `as_of` points.
    pub points: Vec<OwnershipSeriesPoint>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipSeriesPoint {
    pub as_of: String,
    /// Decimal-exact capital %; `None` when only votes were disclosed that period.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub capital_pct: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipFreeFloatPoint {
    pub as_of: String,
    /// Derived float = `100 − Σ` disclosed capital of that report basis.
    pub pct: String,
}

/// A document awaiting OCR/AI extraction (nothing was written for it).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipResidual {
    pub report_document_id: String,
    /// `section_missing | table_unparsable | glyph_encoded`.
    pub parse_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub detected_as_of: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub matched_heading: Option<String>,
}

/// A pending AI holder-type proposal (never auto-applied).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipProposal {
    pub id: String,
    /// Normalized holder name this proposal classifies.
    pub holder_key: String,
    pub proposed_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub rationale: Option<String>,
}

/// Outcome of one AI classification run (ts-rs mirror of the T5 job summary).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipClassificationResult {
    pub examined: u32,
    pub proposed: u32,
    pub skipped: u32,
    pub errors: u32,
}

// ============================================================================
// Read model assembly
// ============================================================================

/// Per-holder history accumulator: `(raw name, holder type, as_of → capital %)`.
/// A `type` alias keeps the grouping map readable (clippy::type_complexity).
type HolderSeriesAccumulator = (String, Option<String>, BTreeMap<String, Option<String>>);

/// Assemble the ownership overview for one company. Reads the current state
/// (+ derived free float), the full snapshot history (grouped per holder into a
/// capital-% series), the extraction residuals, and pending AI proposals.
pub fn compute_ownership_overview(
    state: &AppState,
    company_id: &str,
) -> Result<OwnershipOverview, String> {
    let ownership = state.ownership();
    let current = ownership
        .current_state_with_free_float(company_id)
        .map_err(|error| error.to_string())?;
    let history_rows = ownership
        .history(company_id, None)
        .map_err(|error| error.to_string())?;
    let residuals = ownership
        .list_extraction_residuals(company_id)
        .map_err(|error| error.to_string())?;
    let proposals = ownership
        .list_holder_type_proposals(company_id, true)
        .map_err(|error| error.to_string())?;

    let holders: Vec<OwnershipHolder> = current
        .stakes
        .iter()
        .map(|stake| OwnershipHolder {
            holder_key: stake.holder_name_normalized.clone(),
            name: stake.holder_name_raw.clone(),
            holder_type: stake.holder_type.clone(),
            capital_pct: stake.capital_pct.clone(),
            votes_pct: stake.votes_pct.clone(),
            as_of: stake.as_of.clone(),
            source: stake.source.clone(),
        })
        .collect();

    // Header provenance: the latest `as_of` across current holders, and the
    // source tag of a stake disclosed at that date.
    let as_of = current.stakes.iter().map(|s| s.as_of.clone()).max();
    let source = as_of.as_ref().and_then(|latest| {
        current
            .stakes
            .iter()
            .find(|s| &s.as_of == latest)
            .map(|s| s.source.clone())
    });

    // Per-current-holder capital series. `history` is newest-first; we key each
    // holder's points by `as_of` (keeping the first — newest-created — row for a
    // given date), then emit chronological (oldest → newest).
    let current_keys: std::collections::HashSet<&str> = current
        .stakes
        .iter()
        .map(|s| s.holder_name_normalized.as_str())
        .collect();
    // holder_key -> (name, holder_type, as_of -> capital_pct)
    let mut series_map: BTreeMap<String, HolderSeriesAccumulator> = BTreeMap::new();
    for row in &history_rows {
        if !current_keys.contains(row.holder_name_normalized.as_str()) {
            continue;
        }
        let entry = series_map
            .entry(row.holder_name_normalized.clone())
            .or_insert_with(|| {
                (
                    row.holder_name_raw.clone(),
                    row.holder_type.clone(),
                    BTreeMap::new(),
                )
            });
        // History rows arrive newest-first; the first row seen for an `as_of`
        // wins (its latest disclosure), so only insert when absent.
        entry
            .2
            .entry(row.as_of.clone())
            .or_insert_with(|| row.capital_pct.clone());
    }
    let history: Vec<OwnershipHolderSeries> = series_map
        .into_iter()
        .map(
            |(holder_key, (name, holder_type, points))| OwnershipHolderSeries {
                holder_key,
                name,
                holder_type,
                points: points
                    .into_iter()
                    .map(|(as_of, capital_pct)| OwnershipSeriesPoint { as_of, capital_pct })
                    .collect(),
            },
        )
        .collect();

    Ok(OwnershipOverview {
        company_id: company_id.to_owned(),
        as_of,
        source,
        free_float_pct: current.free_float_pct,
        disclosed_sum: current.disclosed_capital_sum,
        holders,
        history,
        free_float_history: state
            .ownership()
            .free_float_history(company_id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|(as_of, pct)| OwnershipFreeFloatPoint { as_of, pct })
            .collect(),
        residuals: residuals
            .into_iter()
            .map(|r| OwnershipResidual {
                report_document_id: r.report_document_id,
                parse_state: r.parse_state,
                detected_as_of: r.detected_as_of,
                matched_heading: r.matched_heading,
            })
            .collect(),
        pending_proposals: proposals
            .into_iter()
            .map(|p| OwnershipProposal {
                id: p.id,
                holder_key: p.holder_name_normalized,
                proposed_type: p.proposed_type,
                confidence: p.confidence,
                rationale: p.rationale,
            })
            .collect(),
    })
}

// ============================================================================
// Commands
// ============================================================================

/// Ownership overview for the Basic Info panel's Akcjonariat section. Offloaded
/// off the UI thread (reads several ownership tables + groups history).
#[tauri::command]
pub async fn get_ownership_overview(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<OwnershipOverview, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_ownership_overview(&state, &company_id))
        .await
        .map_err(|error| format!("ownership overview task failed: {error}"))?
}

/// Force-enqueue deterministic ownership extraction for every fetched periodic
/// document of the company (the "Wydobądź z raportów" CTA). Returns how many
/// documents were queued. Offloaded — it lists documents and writes queue rows.
#[tauri::command]
pub async fn backfill_ownership_extraction(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::jobs::ownership_extraction::backfill_company_ownership_extraction(
            &state,
            &company_id,
        ) as u32
    })
    .await
    .map_err(|error| format!("ownership backfill task failed: {error}"))
}

/// Manual re-type: set (or clear, with `None`) a holder's classification across
/// their snapshot rows, then return the refreshed overview. A manual label is
/// authoritative — the deterministic pass never overwrites it.
#[tauri::command]
pub async fn set_ownership_holder_type(
    company_id: String,
    holder_key: String,
    holder_type: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<OwnershipOverview, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .ownership()
            .set_holder_type(&company_id, &holder_key, holder_type.as_deref())
            .map_err(|error| error.to_string())?;
        compute_ownership_overview(&state, &company_id)
    })
    .await
    .map_err(|error| format!("set holder type task failed: {error}"))?
}

/// Confirm a pending AI holder-type proposal (applies the type across the
/// holder's rows), then return the refreshed overview.
#[tauri::command]
pub async fn confirm_ownership_holder_type_proposal(
    company_id: String,
    proposal_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<OwnershipOverview, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .ownership()
            .confirm_holder_type_proposal(&proposal_id)
            .map_err(|error| error.to_string())?;
        compute_ownership_overview(&state, &company_id)
    })
    .await
    .map_err(|error| format!("confirm proposal task failed: {error}"))?
}

/// Reject a pending AI holder-type proposal (leaves the holder unclassified),
/// then return the refreshed overview.
#[tauri::command]
pub async fn reject_ownership_holder_type_proposal(
    company_id: String,
    proposal_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<OwnershipOverview, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .ownership()
            .reject_holder_type_proposal(&proposal_id)
            .map_err(|error| error.to_string())?;
        compute_ownership_overview(&state, &company_id)
    })
    .await
    .map_err(|error| format!("reject proposal task failed: {error}"))?
}

/// Run the AI holder-type classify-with-confirm job over every company's residual
/// holders (T5 core). Already async (provider IO); awaited directly.
#[tauri::command]
pub async fn run_ownership_classification(
    state: tauri::State<'_, AppState>,
) -> Result<OwnershipClassificationResult, String> {
    let state = state.inner().clone();
    let summary =
        crate::jobs::ownership_classification::run_ai_ownership_classification(&state).await?;
    Ok(OwnershipClassificationResult {
        examined: summary.examined as u32,
        proposed: summary.proposed as u32,
        skipped: summary.skipped as u32,
        errors: summary.errors as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, NewCompany,
        NewHolderTypeProposal, NewOwnershipStake, OwnershipExtractionResidual,
    };

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState, ticker: &str) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: ticker.to_owned(),
                display_name: format!("{ticker} S.A."),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    fn stake(
        state: &AppState,
        company_id: &str,
        holder: &str,
        holder_type: Option<&str>,
        capital: &str,
        votes: &str,
        as_of: &str,
    ) {
        state
            .ownership()
            .append_snapshot(NewOwnershipStake {
                company_id: company_id.to_owned(),
                holder_name_raw: holder.to_owned(),
                holder_type: holder_type.map(str::to_owned),
                capital_pct: Some(capital.to_owned()),
                votes_pct: Some(votes.to_owned()),
                as_of: as_of.to_owned(),
                source: "report_document".to_owned(),
                report_document_id: None,
                feed_item_id: None,
            })
            .expect("snapshot");
    }

    #[test]
    fn empty_company_reports_full_free_float_and_no_holders() {
        let s = state();
        let c = company(&s, "EMPT");

        let overview = compute_ownership_overview(&s, &c).expect("overview");
        assert_eq!(overview.company_id, c);
        assert!(overview.holders.is_empty());
        assert!(overview.history.is_empty());
        assert!(overview.residuals.is_empty());
        assert!(overview.pending_proposals.is_empty());
        assert_eq!(overview.free_float_pct, "100");
        assert_eq!(overview.disclosed_sum, "0");
        assert!(overview.as_of.is_none());
        assert!(overview.source.is_none());
    }

    #[test]
    fn populated_overview_groups_current_holders_history_and_free_float() {
        let s = state();
        let c = company(&s, "CBF");
        // Two years of a founder plus one OFE entering the latest year.
        stake(
            &s,
            &c,
            "Jacek Duch",
            Some("founder_insider"),
            "25.5",
            "25.5",
            "2024-12-31",
        );
        stake(
            &s,
            &c,
            "Jacek Duch",
            Some("founder_insider"),
            "25.5",
            "25.5",
            "2025-12-31",
        );
        stake(
            &s,
            &c,
            "NN PTE",
            Some("ofe_pension"),
            "6.0",
            "6.0",
            "2025-12-31",
        );

        let overview = compute_ownership_overview(&s, &c).expect("overview");
        // Current state = latest per holder → 2 holders.
        assert_eq!(overview.holders.len(), 2);
        assert_eq!(overview.as_of.as_deref(), Some("2025-12-31"));
        assert_eq!(overview.source.as_deref(), Some("report_document"));
        // Disclosed 25.5 + 6.0 = 31.5 → free float 68.5.
        assert_eq!(overview.disclosed_sum, "31.5");
        assert_eq!(overview.free_float_pct, "68.5");
        // Founder has a 2-point chronological series; OFE has one.
        let duch = overview
            .history
            .iter()
            .find(|serie| serie.name == "Jacek Duch")
            .expect("duch series");
        assert_eq!(duch.points.len(), 2);
        assert_eq!(duch.points[0].as_of, "2024-12-31");
        assert_eq!(duch.points[1].as_of, "2025-12-31");
    }

    #[test]
    fn overview_surfaces_residuals_and_pending_proposals() {
        let s = state();
        let c = company(&s, "VRC");
        stake(
            &s,
            &c,
            "cyber_Folks S.A.",
            Some("parent_company"),
            "50.0",
            "50.2",
            "2023-05-01",
        );
        // A holder awaiting AI classification (unclassified stake + proposal).
        stake(
            &s,
            &c,
            "Itema Ventures UAB",
            None,
            "5.5",
            "5.5",
            "2025-12-31",
        );
        s.ownership()
            .upsert_holder_type_proposal(NewHolderTypeProposal {
                company_id: c.clone(),
                holder_name_normalized: "ITEMA VENTURES UAB".to_owned(),
                proposed_type: "other_institutional".to_owned(),
                confidence: Some(0.7),
                rationale: Some("foreign holding".to_owned()),
                provider_id: Some("test".to_owned()),
                model: Some("scripted".to_owned()),
            })
            .expect("proposal");
        let document = s
            .report_documents()
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: c.clone(),
                source_type: "espi".to_owned(),
                url: "https://example.com/vrc-2023.pdf".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Raport roczny 2023".to_owned()),
                attribution: None,
            })
            .expect("document");
        s.ownership()
            .record_extraction_residual(OwnershipExtractionResidual {
                report_document_id: document.id.clone(),
                company_id: c.clone(),
                parse_state: "glyph_encoded".to_owned(),
                detected_as_of: Some("2023-12-31".to_owned()),
                matched_heading: Some("Akcjonariat".to_owned()),
                created_at: String::new(),
                updated_at: String::new(),
            })
            .expect("residual");

        let overview = compute_ownership_overview(&s, &c).expect("overview");
        assert_eq!(overview.residuals.len(), 1);
        assert_eq!(overview.residuals[0].parse_state, "glyph_encoded");
        assert_eq!(overview.pending_proposals.len(), 1);
        assert_eq!(
            overview.pending_proposals[0].holder_key,
            "ITEMA VENTURES UAB"
        );
        assert_eq!(
            overview.pending_proposals[0].proposed_type,
            "other_institutional"
        );
    }

    #[test]
    fn set_holder_type_relabels_and_returns_refreshed_overview() {
        let s = state();
        let c = company(&s, "ACP");
        stake(
            &s,
            &c,
            "TSS Europe B.V.",
            Some("other_institutional"),
            "10.0",
            "10.0",
            "2025-12-31",
        );

        let overview = compute_ownership_overview(&s, &c).expect("overview");
        let holder = &overview.holders[0];
        assert_eq!(holder.holder_type.as_deref(), Some("other_institutional"));

        // Re-type across the holder's rows (the command's storage effect).
        let affected = s
            .ownership()
            .set_holder_type(&c, &holder.holder_key, Some("parent_company"))
            .expect("set");
        assert_eq!(affected, 1);
        let refreshed = compute_ownership_overview(&s, &c).expect("refreshed");
        assert_eq!(
            refreshed.holders[0].holder_type.as_deref(),
            Some("parent_company")
        );
    }

    /// Cross-language populated-path contract (mirrors the price-context M1
    /// fixture): the exact wire-format JSON of a POPULATED `OwnershipOverview` is
    /// pinned in a shared fixture that the frontend contract test renders through
    /// the real `OwnershipSection`. Any DTO drift (field name/casing, omitted
    /// optionals, series grouping) reddens here; any renderer drift reddens on the
    /// vitest side — closing the gap the mock-fidelity corpus (empty-only) leaves.
    #[test]
    fn populated_ownership_overview_matches_the_shared_wire_fixture() {
        let s = state();
        let c = company(&s, "CBF");
        // Two years of a founder (a 2-point trajectory) + one OFE + one holder
        // awaiting AI classification (no capital, so it stays in the free float).
        stake(
            &s,
            &c,
            "Jacek Duch",
            Some("founder_insider"),
            "25.5",
            "25.5",
            "2024-12-31",
        );
        stake(
            &s,
            &c,
            "Jacek Duch",
            Some("founder_insider"),
            "25.5",
            "25.5",
            "2025-12-31",
        );
        stake(
            &s,
            &c,
            "NN PTE",
            Some("ofe_pension"),
            "6.0",
            "6.0",
            "2025-12-31",
        );
        s.ownership()
            .append_snapshot(NewOwnershipStake {
                company_id: c.clone(),
                holder_name_raw: "Itema Ventures UAB".to_owned(),
                holder_type: None,
                capital_pct: None,
                votes_pct: None,
                as_of: "2025-12-31".to_owned(),
                source: "report_document".to_owned(),
                report_document_id: None,
                feed_item_id: None,
            })
            .expect("itema stake");
        s.ownership()
            .upsert_holder_type_proposal(NewHolderTypeProposal {
                company_id: c.clone(),
                holder_name_normalized: "ITEMA VENTURES UAB".to_owned(),
                proposed_type: "other_institutional".to_owned(),
                confidence: Some(0.7),
                rationale: Some("foreign holding entity".to_owned()),
                provider_id: Some("test".to_owned()),
                model: Some("scripted".to_owned()),
            })
            .expect("proposal");

        // Company id is deterministic (`company_gpw_cbf`); the fixture pins it.
        assert_eq!(c, "company_gpw_cbf");
        let overview = compute_ownership_overview(&s, &c).expect("overview");
        let actual = serde_json::to_value(&overview).expect("serialize");
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/test/scenarios/ownershipOverviewPopulated.json"
        ))
        .expect("fixture parses");
        assert_eq!(
            actual, fixture,
            "populated OwnershipOverview wire format drifted from the shared fixture \
             (src/test/scenarios/ownershipOverviewPopulated.json) — update BOTH sides deliberately; \
             actual: {actual}"
        );
    }
}
