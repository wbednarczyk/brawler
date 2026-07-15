//! Morning-briefing commands (ADR 0068 decision 4, plan v0.54-attention-routing-briefing §T5).
//! Thin `#[tauri::command]` wrappers: `generate_morning_briefing` enqueues an
//! on-demand compose on the durable queue (the `morning_briefing` ai-lane job runs
//! the deterministic composer + optional narrative); `get_latest_morning_briefing`
//! returns the most recently composed briefing (or `null` when none exists yet).
//! A once-per-day auto compose is enqueued by the Rust scheduler while the app is
//! open (`jobs::scheduler`).

use crate::{app_state, jobs, storage};

/// Enqueue an on-demand morning-briefing compose (forces a fresh compose even if
/// today's briefing already exists). Returns once queued; poll
/// `get_latest_morning_briefing` for the result.
#[tauri::command]
pub fn generate_morning_briefing(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    jobs::morning_briefing::enqueue_on_demand_briefing(&state);
    Ok(())
}

/// The most recently composed morning briefing (its structured item list plus the
/// AI narrative when one was produced), or `null` when none has been composed yet.
#[tauri::command]
pub fn get_latest_morning_briefing(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<storage::MorningBriefing>, String> {
    state
        .morning_briefings()
        .latest_morning_briefing()
        .map_err(|error| error.to_string())
}
