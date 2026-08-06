use crate::{app_state, storage};

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
