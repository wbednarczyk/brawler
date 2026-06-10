use serde::Serialize;
use tauri::Manager;

pub mod app_state;
pub mod data_directory;
pub mod jobs;
pub mod licensing;
pub mod logging;
pub mod observability;
pub mod providers;
pub mod source_adapters;
pub mod storage;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub(crate) status: &'static str,
    pub(crate) version: &'static str,
}

pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_data_dir = data_directory::runtime_data_dir(app.path().app_data_dir()?)?;
            std::fs::create_dir_all(&app_data_dir)?;

            let database_path = app_data_dir.join("brawler.sqlite3");
            let connection = storage::open_database(database_path)?;
            let state = app_state::AppState::with_data_dir(connection, app_data_dir.clone());
            if developer_mode_requested_from_environment() {
                state.set_developer_mode_enabled(true)?;
            }
            let log_settings = logging::resolve_log_settings(&state.get_settings()?.logs);
            let logs_dir =
                logging::init_file_logger(&app_data_dir, log_settings).map_err(|error| {
                    std::io::Error::other(format!("failed to initialize local logger: {error:?}"))
                })?;
            log::info!("local logging initialized at {}", logs_dir.display());

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::health,
            commands::health::database_status,
            commands::companies::list_companies,
            commands::companies::create_company,
            commands::companies::lookup_company,
            commands::companies::delete_company,
            commands::watchlists::list_watchlists,
            commands::watchlists::list_watchlist_memberships,
            commands::watchlists::create_watchlist,
            commands::watchlists::rename_watchlist,
            commands::watchlists::delete_watchlist,
            commands::watchlists::add_company_to_watchlist,
            commands::watchlists::remove_company_from_watchlist,
            commands::import_export::export_research_data,
            commands::import_export::preview_research_import,
            commands::import_export::apply_research_import,
            commands::import_export::export_settings_data,
            commands::import_export::preview_settings_import,
            commands::import_export::apply_settings_import,
            commands::feed::list_feed_items,
            commands::sources::list_unmatched_source_items,
            commands::feed::update_feed_item_state,
            commands::feed::prune_old_feed_items,
            commands::feed::delete_unsaved_feed_items,
            commands::research::list_research_evidence,
            commands::research::list_company_timeline,
            commands::research::list_watchlist_timeline,
            commands::research::mark_research_scope_reviewed,
            commands::research::list_research_review_state,
            commands::research::create_evidence_link,
            commands::research::delete_evidence_link,
            commands::ai_analysis::start_ai_analysis,
            commands::ai_analysis::list_ai_analysis,
            commands::ai_analysis::retry_ai_analysis,
            commands::notebooks::list_notebook_entries,
            commands::notebooks::create_notebook_entry,
            commands::notebooks::create_note_from_transcript_selection,
            commands::notebooks::update_notebook_entry,
            commands::notebooks::delete_notebook_entry,
            commands::events::list_company_events,
            commands::events::create_company_event,
            commands::transcripts::list_video_transcript_jobs,
            commands::transcripts::delete_video_transcript_job,
            commands::transcripts::create_video_transcript_job,
            commands::transcripts::update_video_transcript_job,
            commands::transcripts::list_transcript_segments,
            commands::transcripts::resolve_transcript_job_company,
            commands::transcripts::run_video_transcript_job,
            commands::sources::list_source_adapters,
            commands::sources::list_company_registry_entries,
            commands::sources::set_source_adapter_enabled,
            commands::sources::refresh_sources,
            commands::sources::refresh_source,
            commands::sources::refresh_gpw_company_registry,
            commands::sources::refresh_gpw_company_registry_if_stale,
            commands::diagnostics::list_diagnostic_events,
            commands::diagnostics::clear_diagnostic_events,
            commands::diagnostics::get_diagnostic_summary,
            commands::logs::get_log_status,
            commands::logs::list_log_entries,
            commands::logs::open_logs_directory,
            commands::metrics::get_local_metrics_snapshot,
            commands::licensing::get_license_status,
            commands::licensing::submit_license_key,
            commands::licensing::clear_license_key,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::disable_developer_mode,
            commands::settings::unlock_developer_mode,
            commands::credentials::get_gemini_transcription_credential_status,
            commands::credentials::set_gemini_transcription_api_key,
            commands::credentials::clear_gemini_transcription_api_key
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Brawler application");
}

fn developer_mode_requested_from_environment() -> bool {
    developer_mode_requested_from_value(std::env::var("BRAWLER_DEVELOPER_MODE").ok().as_deref())
}

fn developer_mode_requested_from_value(value: Option<&str>) -> bool {
    value
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn health_reports_ok() {
        let response = super::commands::health::health();

        assert_eq!(response.status, "ok");
        assert_eq!(response.version, "0.28.2");
    }

    #[test]
    fn parses_developer_mode_environment_values() {
        assert!(super::developer_mode_requested_from_value(Some("true")));
        assert!(super::developer_mode_requested_from_value(Some("1")));
        assert!(super::developer_mode_requested_from_value(Some("YES")));
        assert!(super::developer_mode_requested_from_value(Some(" on ")));
        assert!(!super::developer_mode_requested_from_value(Some("off")));
        assert!(!super::developer_mode_requested_from_value(Some("false")));
        assert!(!super::developer_mode_requested_from_value(None));
    }
}
