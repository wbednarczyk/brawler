//! Analyst-recommendations read model (ADR 0073, plan v0.58 A3).
//!
//! Feeds the "Rekomendacje analityków" Spółka workshop tool
//! (`src/shared/components/AnalystRecommendationsSection.tsx`) and the attributed
//! "vs target" readout in Price context. A thin read over the append-only
//! `analyst_recommendations` store (slice A1): the per-company history
//! (newest-first by `published_at`), the newest target-carrying entry for the
//! attributed readout, and the adapter's last successful refresh for the footer's
//! honesty line. No stored projection, so it can never go stale.
//!
//! ADR 0073 hard rule: recommendations are attributed third-party opinions, never
//! advice — the view carries every attribution field (firm, analyst, date) the UI
//! renders inseparably from each number, and nothing here feeds scorecards or
//! app-generated analysis text.

use serde::Serialize;

use crate::app_state::AppState;
use crate::storage::{AnalystRecommendationRow, AnalystRecommendationTarget};

// ============================================================================
// DTO (ts-rs export → ../../src/api/generated/)
// ============================================================================

/// Everything the analyst-recommendations panel renders for one company.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AnalystRecommendationsView {
    pub company_id: String,
    /// The full local history, newest-first by `published_at` (never
    /// `created_at`). Empty when the source has published nothing for the company.
    /// Panel-level counts (entry count, last-change date) derive client-side from
    /// this list — the DTO stays minimal.
    pub entries: Vec<AnalystRecommendationRow>,
    /// The newest recommendation carrying a target price, for the attributed "vs
    /// target" readout beside Price context. Absent when no entry has a target.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub latest_target: Option<AnalystRecommendationTarget>,
    /// The adapter's last successful refresh (`source_adapters.last_success_at`),
    /// for the footer's honest "last refresh" line. Absent before the adapter has
    /// ever run for this install — the footer then omits the line, never fakes a
    /// timestamp (ADR 0073, storyboard footer).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub last_refreshed_at: Option<String>,
}

// ============================================================================
// Read model assembly
// ============================================================================

/// Assemble the analyst-recommendations view for one company: the newest-first
/// history, the newest target-carrying entry, and the adapter's last refresh.
pub fn compute_analyst_recommendations(
    state: &AppState,
    company_id: &str,
) -> Result<AnalystRecommendationsView, String> {
    let store = state.analyst_recommendations();
    let entries = store
        .list_analyst_recommendations(company_id)
        .map_err(|error| error.to_string())?;
    let latest_target = store
        .latest_target(company_id)
        .map_err(|error| error.to_string())?;
    let last_refreshed_at = store
        .last_refreshed_at()
        .map_err(|error| error.to_string())?;

    Ok(AnalystRecommendationsView {
        company_id: company_id.to_owned(),
        entries,
        latest_target,
        last_refreshed_at,
    })
}

// ============================================================================
// Command
// ============================================================================

/// Analyst-recommendations panel view for one company. Offloaded off the UI
/// thread (reads the history table + the adapter catalog row).
#[tauri::command]
pub async fn get_analyst_recommendations(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<AnalystRecommendationsView, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        compute_analyst_recommendations(&state, &company_id)
    })
    .await
    .map_err(|error| format!("analyst recommendations task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AnalystRecommendationEntry, AppState, NewCompany,
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

    #[allow(clippy::too_many_arguments)]
    fn entry(
        firm: &str,
        analyst: Option<&str>,
        rating: &str,
        target: Option<&str>,
        price_at_issue: Option<&str>,
        published_at: &str,
        report_url: Option<&str>,
    ) -> AnalystRecommendationEntry {
        AnalystRecommendationEntry {
            firm: firm.to_owned(),
            analyst: analyst.map(str::to_owned),
            rating: rating.to_owned(),
            target_price: target.map(str::to_owned),
            target_currency: target.map(|_| "PLN".to_owned()),
            price_at_issue: price_at_issue.map(str::to_owned),
            published_at: published_at.to_owned(),
            source_url: "https://www.biznesradar.pl/rekomendacje-spolki/REC".to_owned(),
            report_url: report_url.map(str::to_owned),
        }
    }

    #[test]
    fn empty_company_returns_no_entries_no_target() {
        let s = state();
        let c = company(&s, "EMP");
        let view = compute_analyst_recommendations(&s, &c).expect("view");
        assert_eq!(view.company_id, c);
        assert!(view.entries.is_empty());
        assert!(view.latest_target.is_none());
        // The adapter never ran → no faked refresh timestamp.
        assert!(view.last_refreshed_at.is_none());
    }

    #[test]
    fn view_lists_newest_first_and_surfaces_latest_target() {
        let s = state();
        let c = company(&s, "NEW");
        s.analyst_recommendations()
            .ingest_analyst_recommendations(
                &c,
                &[
                    entry(
                        "Noble Securities",
                        Some("Jan Kowalski"),
                        "trzymaj",
                        Some("230.00"),
                        Some("220.00"),
                        "2026-02-01T09:00:00",
                        Some("https://static.example/rec/n1.pdf"),
                    ),
                    entry(
                        "Noble Securities",
                        Some("Mateusz Chrzanowski"),
                        "akumuluj",
                        Some("250.00"),
                        Some("232.00"),
                        "2026-06-18T08:40:00",
                        Some("https://static.example/rec/n2.pdf"),
                    ),
                ],
            )
            .expect("ingest");

        let view = compute_analyst_recommendations(&s, &c).expect("view");
        assert_eq!(view.entries.len(), 2);
        // Newest-first by published_at.
        assert_eq!(view.entries[0].published_at, "2026-06-18T08:40:00");
        assert_eq!(view.entries[0].rating, "akumuluj");
        // Same-firm upgrade derives its prior rating/target.
        assert_eq!(view.entries[0].direction, "upgrade");
        assert_eq!(view.entries[0].rating_prev.as_deref(), Some("trzymaj"));
        assert_eq!(view.entries[0].target_prev.as_deref(), Some("230.00"));
        // The attributed vs-target readout uses the newest target-carrying entry.
        let target = view.latest_target.expect("latest target");
        assert_eq!(target.firm, "Noble Securities");
        assert_eq!(target.target_price, "250.00");
        assert_eq!(target.published_at, "2026-06-18T08:40:00");
        // Ingest recorded a successful outcome → footer has a refresh timestamp.
        assert!(view.last_refreshed_at.is_some());
    }

    /// Cross-language populated-path contract (mirrors the ownership T6 fixture):
    /// the exact wire JSON of a POPULATED view's `entries` + `latestTarget` is
    /// pinned in a shared fixture the frontend contract test renders through the
    /// real `AnalystRecommendationsSection`. Any DTO drift (field name/casing,
    /// direction/prev derivation, null vs omitted) reddens here; any renderer
    /// drift reddens on the vitest side. The wall-clock `lastRefreshedAt` (a
    /// `now()` outcome stamp) is stripped before comparison — its rendering is
    /// pinned separately in a vitest case with an explicit timestamp.
    #[test]
    fn populated_view_matches_the_shared_wire_fixture() {
        let s = state();
        let c = company(&s, "REC");
        s.analyst_recommendations()
            .ingest_analyst_recommendations(
                &c,
                &[
                    entry(
                        "BM mBank",
                        None,
                        "trzymaj",
                        None,
                        None,
                        "2025-11-26T00:00:00",
                        None,
                    ),
                    entry(
                        "Noble Securities",
                        Some("Jan Kowalski"),
                        "trzymaj",
                        Some("230.00"),
                        Some("220.00"),
                        "2026-02-01T09:00:00",
                        Some("https://static.example/rec/noble-2026-02.pdf"),
                    ),
                    entry(
                        "Noble Securities",
                        Some("Mateusz Chrzanowski"),
                        "akumuluj",
                        Some("250.00"),
                        Some("232.00"),
                        "2026-06-18T08:40:00",
                        Some("https://static.example/rec/noble-2026-06.pdf"),
                    ),
                ],
            )
            .expect("ingest");

        // Company id is deterministic (`company_gpw_rec`); the fixture pins it.
        assert_eq!(c, "company_gpw_rec");
        let view = compute_analyst_recommendations(&s, &c).expect("view");
        let mut actual = serde_json::to_value(&view).expect("serialize");
        // Strip the wall-clock refresh stamp — the rest is deterministic.
        assert!(
            actual
                .as_object_mut()
                .expect("object")
                .remove("lastRefreshedAt")
                .is_some(),
            "populated view should carry a lastRefreshedAt after ingest"
        );
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("BRAWLER_SCENARIOS_DIR"),
            "/analystRecommendationsPopulated.json"
        )))
        .expect("fixture parses");
        assert_eq!(
            actual, fixture,
            "populated AnalystRecommendationsView wire format drifted from the shared fixture \
             (src/test/scenarios/analystRecommendationsPopulated.json) — update BOTH sides \
             deliberately; actual: {actual}"
        );
    }
}
