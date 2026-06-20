use crate::{app_state, storage};

#[tauri::command]
pub fn list_watchlists(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::Watchlist>, String> {
    state
        .watchlists()
        .list_watchlists()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_watchlist_memberships(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::WatchlistMembership>, String> {
    state
        .watchlists()
        .list_watchlist_memberships()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_watchlist(
    input: storage::NewWatchlist,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::Watchlist, String> {
    state
        .watchlists()
        .create_watchlist(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn rename_watchlist(
    input: storage::WatchlistUpdate,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::Watchlist, String> {
    state
        .watchlists()
        .rename_watchlist(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_watchlist(
    watchlist_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .watchlists()
        .delete_watchlist(&watchlist_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn add_company_to_watchlist(
    input: storage::WatchlistCompanyInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .watchlists()
        .add_company_to_watchlist(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn remove_company_from_watchlist(
    input: storage::WatchlistCompanyInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .watchlists()
        .remove_company_from_watchlist(input)
        .map_err(|error| error.to_string())
}
