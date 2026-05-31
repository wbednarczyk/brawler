use serde::Serialize;
use tauri::Manager;

pub mod source_adapters;
pub mod storage;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

mod commands {
    use super::{source_adapters, storage, HealthResponse};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RefreshSourcesInput {
        trigger: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RefreshRegistryIfStaleInput {
        trigger: Option<String>,
        stale_after_seconds: Option<i64>,
    }

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
        let first_result = state
            .lookup_company(input.clone())
            .map_err(|error| error.to_string())?;
        if first_result.is_some() || !should_bootstrap_gpw_registry(&input, &state)? {
            return Ok(first_result);
        }

        refresh_gpw_company_registry_for_trigger(&state, "lookup")?;

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
    pub fn list_unmatched_source_items(
        adapter_id: String,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::UnmatchedSourceItem>, String> {
        state
            .list_unmatched_source_items(&adapter_id)
            .map_err(|error| error.to_string())
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
    pub fn list_notebook_entries(
        company_id: String,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::NotebookEntry>, String> {
        state
            .list_notebook_entries(&company_id)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn create_notebook_entry(
        input: storage::NewNotebookEntry,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::NotebookEntry, String> {
        state
            .create_notebook_entry(input)
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn update_notebook_entry(
        input: storage::NotebookEntryUpdate,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::NotebookEntry, String> {
        state
            .update_notebook_entry(input)
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
    pub fn list_company_registry_entries(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::CompanyRegistryEntry>, String> {
        state
            .list_company_registry_entries()
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn refresh_sources(
        input: Option<RefreshSourcesInput>,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::SourceIngestionResult, String> {
        let trigger = input
            .and_then(|input| input.trigger)
            .filter(|trigger| trigger == "manual" || trigger == "scheduler")
            .unwrap_or_else(|| "manual".to_owned());
        let _ = state
            .record_source_adapter_attempt(source_adapters::gpw_espi_ebi::ADAPTER_ID, &trigger);

        let fetcher = source_adapters::gpw_espi_ebi::HttpGpwPageFetcher;
        let mut listings = match source_adapters::gpw_espi_ebi::fetch_report_listings(&fetcher) {
            Ok(listings) => listings,
            Err(error) => {
                let message = error.to_string();
                let _ = state.record_source_adapter_error(
                    source_adapters::gpw_espi_ebi::ADAPTER_ID,
                    &message,
                );

                return Err(message);
            }
        };
        let detail_policy = source_adapters::gpw_espi_ebi::detail_fetch_policy();
        let mut details_fetched = 0usize;
        let mut details_stored = 0usize;
        let mut details_failed = 0usize;
        let mut last_detail_warning: Option<String> = None;

        if detail_policy.enabled_by_default {
            for listing in &mut listings {
                if details_fetched >= detail_policy.max_details_per_refresh {
                    break;
                }

                if detail_policy.matched_items_only
                    && !state
                        .tracks_gpw_listing_company(&listing.company_ticker, &listing.isin)
                        .map_err(|error| error.to_string())?
                {
                    continue;
                }

                if details_fetched > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(
                        detail_policy.min_delay_between_requests_seconds,
                    ));
                }

                match source_adapters::gpw_espi_ebi::fetch_report_detail(
                    &fetcher,
                    &listing.detail_url,
                ) {
                    Ok(detail) => {
                        let evaluation =
                            source_adapters::gpw_espi_ebi::evaluate_report_detail(&detail);
                        if evaluation.usable_for_ingestion {
                            listing.body_text = Some(detail.body_text);
                            listing.attachments = detail.attachments;
                            details_stored += 1;
                        } else {
                            last_detail_warning = Some(format!(
                                "{}: {}",
                                listing.title,
                                evaluation.warnings.join(", ")
                            ));
                            details_failed += 1;
                        }
                    }
                    Err(error) => {
                        last_detail_warning = Some(format!("{}: {}", listing.title, error));
                        let _ = state.record_source_adapter_error(
                            source_adapters::gpw_espi_ebi::ADAPTER_ID,
                            &error.to_string(),
                        );
                        details_failed += 1;
                    }
                }

                details_fetched += 1;
            }
        }

        let _ = state.record_source_adapter_state(
            source_adapters::gpw_espi_ebi::ADAPTER_ID,
            "last_detail_items_attempted",
            &details_fetched.to_string(),
        );
        let _ = state.record_source_adapter_state(
            source_adapters::gpw_espi_ebi::ADAPTER_ID,
            "last_detail_items_stored",
            &details_stored.to_string(),
        );
        let _ = state.record_source_adapter_state(
            source_adapters::gpw_espi_ebi::ADAPTER_ID,
            "last_detail_items_failed",
            &details_failed.to_string(),
        );
        let _ = state.record_source_adapter_state(
            source_adapters::gpw_espi_ebi::ADAPTER_ID,
            "last_detail_warning",
            last_detail_warning.as_deref().unwrap_or(""),
        );

        let mut result = state
            .ingest_gpw_report_listings(&listings)
            .map_err(|error| error.to_string())?;
        result.detail_items_attempted = details_fetched;
        result.detail_items_stored = details_stored;
        result.detail_items_failed = details_failed;

        Ok(result)
    }

    #[tauri::command]
    pub fn refresh_gpw_company_registry(
        input: Option<RefreshSourcesInput>,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::CompanyRegistryRefreshResult, String> {
        let trigger = input
            .and_then(|input| input.trigger)
            .filter(|trigger| trigger == "manual" || trigger == "scheduler" || trigger == "lookup")
            .unwrap_or_else(|| "manual".to_owned());

        refresh_gpw_company_registry_for_trigger(&state, &trigger)
    }

    #[tauri::command]
    pub fn refresh_gpw_company_registry_if_stale(
        input: Option<RefreshRegistryIfStaleInput>,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Option<storage::CompanyRegistryRefreshResult>, String> {
        let trigger = input
            .as_ref()
            .and_then(|input| input.trigger.clone())
            .filter(|trigger| trigger == "scheduler")
            .unwrap_or_else(|| "scheduler".to_owned());
        let stale_after_seconds = input
            .and_then(|input| input.stale_after_seconds)
            .unwrap_or(86_400);

        if !state
            .gpw_company_registry_is_stale(stale_after_seconds)
            .map_err(|error| error.to_string())?
        {
            return Ok(None);
        }

        refresh_gpw_company_registry_for_trigger(&state, &trigger).map(Some)
    }

    fn should_bootstrap_gpw_registry(
        input: &storage::CompanyLookupInput,
        state: &storage::AppState,
    ) -> Result<bool, String> {
        let has_lookup_value = input
            .ticker
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || input
                .isin
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            || input
                .display_name
                .as_deref()
                .is_some_and(|value| value.trim().chars().count() >= 3);

        if input.exchange.trim().to_uppercase() != "GPW" || !has_lookup_value {
            return Ok(false);
        }

        state
            .gpw_company_registry_needs_bootstrap_refresh()
            .map_err(|error| error.to_string())
    }

    fn refresh_gpw_company_registry_for_trigger(
        state: &storage::AppState,
        trigger: &str,
    ) -> Result<storage::CompanyRegistryRefreshResult, String> {
        let _ = state.record_source_adapter_attempt(
            source_adapters::gpw_company_registry::ADAPTER_ID,
            trigger,
        );

        let fetcher = source_adapters::gpw_company_registry::HttpGpwCompanyRegistryFetcher;
        let (entries, fetched_at) =
            match source_adapters::gpw_company_registry::fetch_company_registry_entries(&fetcher) {
                Ok(result) => result,
                Err(error) => {
                    let message = error.to_string();
                    let _ = state.record_source_adapter_error(
                        source_adapters::gpw_company_registry::ADAPTER_ID,
                        &message,
                    );

                    return Err(message);
                }
            };

        state
            .refresh_gpw_company_registry(&entries, &fetched_at)
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
        .plugin(tauri_plugin_opener::init())
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
            commands::list_unmatched_source_items,
            commands::update_feed_item_state,
            commands::list_notebook_entries,
            commands::create_notebook_entry,
            commands::update_notebook_entry,
            commands::list_source_adapters,
            commands::list_company_registry_entries,
            commands::refresh_sources,
            commands::refresh_gpw_company_registry,
            commands::refresh_gpw_company_registry_if_stale,
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
        assert_eq!(response.version, "0.7.0");
    }
}
