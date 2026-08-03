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
//! `set_ownership_holder_type`) return the FRESHLY recomputed
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
    /// Skin-in-the-game corroboration: present when this holder is matched by a
    /// parsed management-holdings row or an insider transaction (by person name, or
    /// as the vehicle a founder holds through). Drives the Ownership badge.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub skin_in_the_game: Option<SkinInTheGame>,
}

/// The skin-in-the-game evidence behind the Ownership badge: the natural person,
/// and the vehicle when the holding is indirect.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SkinInTheGame {
    /// The natural person behind the stake (a board member or PDMR).
    pub person: String,
    /// The vehicle the person holds through, when the holding is indirect.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub via: Option<String>,
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
    /// Which disclosure this point came from (`espi_filing` / `report_document`
    /// / `aggregator` / `manual`). An `espi_filing` point is a **threshold
    /// crossing**: Polish law only compels that filing when a holder crosses one
    /// of the statutory bands, so the UI marks those dates on the trajectory
    /// (ADR 0072 decision 5). Carries the source of the row that won the
    /// per-`as_of` dedup, i.e. the latest disclosure for that date.
    pub source: String,
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
    /// The tier-4 OCR lifecycle marker (v0.57 T8): absent = eligible for a bulk
    /// OCR pass; `proposed` = a pending OCR proposal awaits review; `rejected` =
    /// the user rejected it; `no_table` = OCR found no shareholders table. The
    /// warnbox uses this to pick the right action/state per residual.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub ocr_state: Option<String>,
}

// ============================================================================
// Read model assembly
// ============================================================================

/// Per-holder history accumulator: `(raw name, holder type, as_of → capital %)`.
/// A `type` alias keeps the grouping map readable (clippy::type_complexity).
/// `(holder display name, holder type, as_of -> (capital %, source))`.
type HolderSeriesAccumulator = (
    String,
    Option<String>,
    BTreeMap<String, (Option<String>, String)>,
);

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
    // Skin-in-the-game corroboration keyed by canonical holder identity (the
    // founder-badge join over the management-holdings + insider substrates).
    let skin = state
        .management_holdings()
        .skin_in_the_game(company_id)
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
            skin_in_the_game: skin
                .get(
                    &crate::fundamentals::ownership::classify::canonical_holder_identity(
                        &stake.holder_name_raw,
                    ),
                )
                .map(|m| SkinInTheGame {
                    person: m.person.clone(),
                    via: m.via.clone(),
                }),
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
            .or_insert_with(|| (row.capital_pct.clone(), row.source.clone()));
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
                    .map(|(as_of, (capital_pct, source))| OwnershipSeriesPoint {
                        as_of,
                        capital_pct,
                        source,
                    })
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
                ocr_state: r.ocr_state,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AppState, CaptureReportDocumentInput, NewCompany,
        NewManagementHolding, NewOwnershipStake,
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

    /// A series point carries the source of the disclosure it came from, which is
    /// what lets the chart mark threshold crossings (ADR 0072 decision 5): an
    /// `espi_filing` is compelled only when a holder crosses a statutory band,
    /// while `report_document` points are ordinary periodic samples. Dropping the
    /// field, or letting the per-`as_of` dedup keep the wrong row's source, would
    /// silently un-mark every crossing in the UI.
    #[test]
    fn series_points_carry_the_source_that_disclosed_them() {
        let s = state();
        let c = company(&s, "CBF");
        let filing = |as_of: &str, pct: &str| NewOwnershipStake {
            company_id: c.clone(),
            holder_name_raw: "Jacek Duch".to_owned(),
            holder_type: Some("founder_insider".to_owned()),
            capital_pct: Some(pct.to_owned()),
            votes_pct: Some(pct.to_owned()),
            as_of: as_of.to_owned(),
            source: "espi_filing".to_owned(),
            report_document_id: None,
            feed_item_id: None,
        };
        // 2024 disclosed by an ESPI filing (a crossing), 2025 by a periodic report.
        s.ownership()
            .append_snapshot(filing("2024-12-31", "24.9"))
            .expect("espi stake");
        stake(
            &s,
            &c,
            "Jacek Duch",
            Some("founder_insider"),
            "25.5",
            "25.5",
            "2025-12-31",
        );

        let overview = compute_ownership_overview(&s, &c).expect("overview");
        let duch = overview
            .history
            .iter()
            .find(|serie| serie.name == "Jacek Duch")
            .expect("duch series");
        assert_eq!(
            duch.points
                .iter()
                .map(|point| (point.as_of.as_str(), point.source.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("2024-12-31", "espi_filing"),
                ("2025-12-31", "report_document"),
            ],
        );
    }

    #[test]
    fn overview_surfaces_skin_in_the_game_for_direct_and_vehicle_holders() {
        let s = state();
        let c = company(&s, "SNT");
        // A direct founder stake and a vehicle stake a founder holds through.
        stake(
            &s,
            &c,
            "Cezary Kozielski",
            Some("founder_insider"),
            "24.0",
            "24.0",
            "2025-09-30",
        );
        stake(
            &s,
            &c,
            "Melhus Company Ltd",
            None,
            "10.0",
            "10.0",
            "2025-09-30",
        );
        let doc = s
            .report_documents()
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: c.clone(),
                source_type: "espi".to_owned(),
                url: "https://example.com/snt-q3.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Raport kwartalny 2025".to_owned()),
                attribution: None,
            })
            .expect("document");
        // Direct person holding + an indirect holding via the vehicle.
        s.management_holdings()
            .upsert_holding(NewManagementHolding {
                company_id: c.clone(),
                report_document_id: doc.id.clone(),
                person_name_raw: "Dariusz Korecki".to_owned(),
                role: Some("management".to_owned()),
                shares: Some("100000".to_owned()),
                indirect_via_raw: None,
                prior_shares: None,
                prior_as_of: None,
                as_of: "2025-09-30".to_owned(),
            })
            .expect("holding");
        s.management_holdings()
            .upsert_holding(NewManagementHolding {
                company_id: c.clone(),
                report_document_id: doc.id.clone(),
                person_name_raw: "Cezary Kozielski".to_owned(),
                role: Some("management".to_owned()),
                shares: Some("2047380".to_owned()),
                indirect_via_raw: Some("Melhus Company Ltd".to_owned()),
                prior_shares: None,
                prior_as_of: None,
                as_of: "2025-09-30".to_owned(),
            })
            .expect("holding via");

        let overview = compute_ownership_overview(&s, &c).expect("overview");
        // The vehicle stake badges via its founder; a direct person match badges too.
        let vehicle = overview
            .holders
            .iter()
            .find(|h| h.name == "Melhus Company Ltd")
            .expect("vehicle holder");
        let skin = vehicle.skin_in_the_game.as_ref().expect("vehicle skin");
        assert_eq!(skin.person, "Cezary Kozielski");
        assert_eq!(skin.via.as_deref(), Some("Melhus Company Ltd"));
        // A holder with no management/insider match carries no badge.
        assert!(overview
            .holders
            .iter()
            .find(|h| h.name == "Cezary Kozielski")
            .map(|h| h.skin_in_the_game.is_some())
            .unwrap_or(false));
    }

    /// Faithful ABE live shape (F-A1, owner dogfooding 2026-07-17): a founder's
    /// most-recent disclosed stake (`founder_insider`, materially above 5%) sits at
    /// an OLDER `as_of` than the newest full-picture basis, because that newest
    /// report's shareholder table was only partially extracted (OFE funds only,
    /// no founders). The disclosure-basis scoping in `current_state` was dropping
    /// the founder entirely, so the read model never surfaced the holder — and the
    /// skin-in-the-game badge, which attaches to a surfaced holder, could never
    /// appear even though the stake is stamped AND corroborated by a matching
    /// management-holdings person row. The corroboration join itself is correct
    /// (see `overview_surfaces_skin_in_the_game_for_direct_and_vehicle_holders`);
    /// the defect is upstream — a founder is not an OFE and does not silently
    /// vanish below 5% (crossing the threshold is itself an ESPI-disclosable
    /// event), so a stamped founder stays in current state (sticky overlay).
    #[test]
    fn founder_below_newest_partial_basis_stays_surfaced_with_skin_badge() {
        let s = state();
        let c = company(&s, "ABE");
        // The founder's newest disclosed stake is 2025-06-30 (8.13% — material).
        stake(
            &s,
            &c,
            "Andrzej Przybyło",
            Some("founder_insider"),
            "8.13",
            "8.13",
            "2025-06-30",
        );
        // A NEWER full-picture basis (2026-03-31) whose shareholder table was only
        // partially parsed — an OFE fund, but NOT the founder.
        stake(
            &s,
            &c,
            "PKO BP Bankowy OFE",
            Some("ofe_pension"),
            "5.10",
            "5.10",
            "2026-03-31",
        );
        // The management-holdings substrate corroborates the founder (T5 join key).
        let doc = s
            .report_documents()
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: c.clone(),
                source_type: "espi".to_owned(),
                url: "https://example.com/abe-annual.xhtml".to_owned(),
                period_id: None,
                origin_ref: None,
                title: Some("Sprawozdanie z działalności 2025".to_owned()),
                attribution: None,
            })
            .expect("document");
        s.management_holdings()
            .upsert_holding(NewManagementHolding {
                company_id: c.clone(),
                report_document_id: doc.id.clone(),
                person_name_raw: "Andrzej Przybyło".to_owned(),
                role: Some("management".to_owned()),
                shares: Some("1316200".to_owned()),
                indirect_via_raw: None,
                prior_shares: None,
                prior_as_of: None,
                as_of: "2025-09-30".to_owned(),
            })
            .expect("holding");

        let overview = compute_ownership_overview(&s, &c).expect("overview");
        // The founder must remain a current holder despite the newer partial basis.
        let founder = overview
            .holders
            .iter()
            .find(|h| h.name == "Andrzej Przybyło")
            .expect("founder still surfaced in current state");
        assert_eq!(founder.holder_type.as_deref(), Some("founder_insider"));
        // And the corroboration badge attaches to that surfaced holder.
        let skin = founder
            .skin_in_the_game
            .as_ref()
            .expect("founder carries the skin-in-the-game badge");
        assert_eq!(skin.person, "Andrzej Przybyło");
        assert!(skin.via.is_none());
        // The OFE from the newest basis is still there too.
        assert!(overview
            .holders
            .iter()
            .any(|h| h.name == "PKO BP Bankowy OFE"));
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
        // The older founder point comes from an ESPI filing — the disclosure a
        // holder is only compelled to make on crossing a statutory band — so the
        // fixture carries one threshold-crossing point beside ordinary
        // periodic-report ones (ADR 0072 decision 5; the chart marks it).
        s.ownership()
            .append_snapshot(NewOwnershipStake {
                company_id: c.clone(),
                holder_name_raw: "Jacek Duch".to_owned(),
                holder_type: Some("founder_insider".to_owned()),
                capital_pct: Some("25.5".to_owned()),
                votes_pct: Some("25.5".to_owned()),
                as_of: "2024-12-31".to_owned(),
                source: "espi_filing".to_owned(),
                report_document_id: None,
                feed_item_id: None,
            })
            .expect("duch espi stake");
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
        // Company id is deterministic (`company_gpw_cbf`); the fixture pins it.
        assert_eq!(c, "company_gpw_cbf");
        let overview = compute_ownership_overview(&s, &c).expect("overview");
        let actual = serde_json::to_value(&overview).expect("serialize");
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("BRAWLER_SCENARIOS_DIR"),
            "/ownershipOverviewPopulated.json"
        )))
        .expect("fixture parses");
        assert_eq!(
            actual, fixture,
            "populated OwnershipOverview wire format drifted from the shared fixture \
             (src/test/scenarios/ownershipOverviewPopulated.json) — update BOTH sides deliberately; \
             actual: {actual}"
        );
    }
}
