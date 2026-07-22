//! Tauri command for the one-off `rebuild fundamentals` driver (ADR 0086
//! decision 6, plan TOR C slice C4).
//!
//! Headless-only: there is no UI entry point. After the owner-approved manual
//! wipe of all `financial_facts` on the live DB, this command repopulates the
//! store from the three spec-driven surfaces (BiznesRadar-primary pull + ESEF
//! re-extraction + WDF cover-note re-scan) and returns the before/after per-tier
//! verdict. Invoked once via live-drive. Offloaded to a blocking task so no
//! fetch/parse/DB work runs on the UI thread.

use crate::{app_state, jobs};

/// Run the full three-pass fundamentals rebuild for every tracked company and
/// return the per-pass counts plus the post-rebuild fact totals per source tier.
/// Idempotent: a re-run writes no new facts (every pass re-observes its slots).
#[tauri::command]
pub async fn rebuild_fundamentals(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<jobs::rebuild_fundamentals::RebuildFundamentalsSummary, String> {
    let state = state.inner().clone();
    jobs::scheduler::run_blocking_task(move || {
        jobs::rebuild_fundamentals::run_rebuild_fundamentals(&state)
    })
    .await
}
