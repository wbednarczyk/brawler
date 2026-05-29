use serde::Serialize;
use tauri::Manager;

pub mod storage;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

mod commands {
    use super::{storage, HealthResponse};

    #[tauri::command]
    pub fn health() -> HealthResponse {
        HealthResponse {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[tauri::command]
    pub fn database_status(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::DatabaseStatus, String> {
        state.database_status().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn list_companies(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::Company>, String> {
        state.list_companies().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn create_company(
        input: storage::NewCompany,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::Company, String> {
        state
            .create_company(input)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn lookup_company(
        input: storage::CompanyLookupInput,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Option<storage::CompanyLookupResult>, String> {
        state
            .lookup_company(input)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn delete_company(
        company_id: String,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<(), String> {
        state
            .delete_company(&company_id)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn list_watchlists(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::Watchlist>, String> {
        state.list_watchlists().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn list_watchlist_memberships(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::WatchlistMembership>, String> {
        state
            .list_watchlist_memberships()
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn create_watchlist(
        input: storage::NewWatchlist,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::Watchlist, String> {
        state
            .create_watchlist(input)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn add_company_to_watchlist(
        input: storage::WatchlistCompanyInput,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<(), String> {
        state
            .add_company_to_watchlist(input)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn remove_company_from_watchlist(
        input: storage::WatchlistCompanyInput,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<(), String> {
        state
            .remove_company_from_watchlist(input)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn list_feed_items(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::FeedItem>, String> {
        state.list_feed_items().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn update_feed_item_state(
        input: storage::FeedItemStateInput,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::FeedItem, String> {
        state
            .update_feed_item_state(input)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn list_source_adapters(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::SourceAdapter>, String> {
        state
            .list_source_adapters()
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn get_settings(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::UserSettings, String> {
        state.get_settings().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn update_settings(
        input: storage::SettingsUpdate,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::UserSettings, String> {
        state
            .update_settings(input)
            .map_err(|error| error.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let database_path = app_data_dir.join("brawler.sqlite3");
            let connection = storage::open_database(database_path)?;

            app.manage(storage::AppState::new(connection));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health,
            commands::database_status,
            commands::list_companies,
            commands::create_company,
            commands::lookup_company,
            commands::delete_company,
            commands::list_watchlists,
            commands::list_watchlist_memberships,
            commands::create_watchlist,
            commands::add_company_to_watchlist,
            commands::remove_company_from_watchlist,
            commands::list_feed_items,
            commands::update_feed_item_state,
            commands::list_source_adapters,
            commands::get_settings,
            commands::update_settings
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Brawler application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn health_reports_ok() {
        let response = super::commands::health();

        assert_eq!(response.status, "ok");
        assert_eq!(response.version, "0.1.0");
    }
}
