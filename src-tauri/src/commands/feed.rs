use serde::Deserialize;

use crate::{app_state, jobs, storage};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PruneOldFeedItemsInput {
    #[cfg_attr(feature = "ts-export", ts(type = "number"))]
    retention_days: Option<i64>,
}

#[tauri::command]
pub fn list_feed_items(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::FeedItem>, String> {
    state.list_feed_items().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_feed_item_state(
    input: storage::FeedItemStateInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FeedItem, String> {
    state
        .update_feed_item_state(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn prune_old_feed_items(
    input: Option<PruneOldFeedItemsInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FeedPruneResult, String> {
    let state = state.inner().clone();
    let retention_days = input.and_then(|input| input.retention_days).unwrap_or(30);

    jobs::scheduler::run_blocking_task(move || {
        jobs::feed_cleanup::prune_old_feed_items(&state, retention_days)
    })
    .await
}

#[tauri::command]
pub async fn delete_unsaved_feed_items(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::FeedDeleteResult, String> {
    let state = state.inner().clone();

    jobs::scheduler::run_blocking_task(move || {
        jobs::feed_cleanup::delete_unsaved_feed_items(&state)
    })
    .await
}
