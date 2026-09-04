//! Tauri command for the BiznesRadar-primary fundamentals pull (ADR 0086
//! decision 2, plan TOR C slice C2).
//!
//! On demand, resolve the three robots-allowed BiznesRadar report pages per
//! tracked company (respecting the per-page daily cadence — cached pages are not
//! refetched) and write core-KPI facts for every reported period. The upcoming
//! `rebuild fundamentals` flow calls the same core; the daily cadence is driven by
//! the durable-queue handler (`jobs::aggregator_fundamentals_pull`). Offloaded to a
//! blocking task so no fetch/parse/DB work runs on the UI thread.

use crate::{app_state, jobs};

/// Run the BiznesRadar-primary fundamentals pull for every tracked company and
/// return a summary of what it wrote / skipped. Idempotent: a re-run inside the
/// daily cadence window fetches nothing (reads the page cache) and re-observes
/// unchanged slots.
#[tauri::command]
pub async fn run_aggregator_fundamentals_pull(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<jobs::aggregator_fundamentals_pull::AggregatorPullSummary, String> {
    let state = state.inner().clone();
    // Serialized entry (issue #132): shares the queue's per-adapter lock so an
    // on-demand run can never race the daily job on the BiznesRadar host.
    jobs::scheduler::run_blocking_task(move || {
        jobs::aggregator_fundamentals_pull::run_aggregator_fundamentals_pull_direct(&state)
    })
    .await
}
