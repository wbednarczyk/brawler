use serde::Serialize;
use tauri::Manager;

pub mod app_state;
pub mod jobs;
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
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let database_path = app_data_dir.join("brawler.sqlite3");
            let connection = storage::open_database(database_path)?;

            app.manage(app_state::AppState::new(connection));

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
            commands::watchlists::add_company_to_watchlist,
            commands::watchlists::remove_company_from_watchlist,
            commands::feed::list_feed_items,
            commands::sources::list_unmatched_source_items,
            commands::feed::update_feed_item_state,
            commands::feed::prune_old_feed_items,
            commands::feed::delete_unsaved_feed_items,
            commands::ai_analysis::start_ai_analysis,
            commands::ai_analysis::list_ai_analysis,
            commands::ai_analysis::retry_ai_analysis,
            commands::notebooks::list_notebook_entries,
            commands::notebooks::create_notebook_entry,
            commands::notebooks::create_note_from_transcript_selection,
            commands::notebooks::update_notebook_entry,
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
            commands::sources::refresh_sources,
            commands::sources::refresh_source,
            commands::sources::refresh_gpw_company_registry,
            commands::sources::refresh_gpw_company_registry_if_stale,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::credentials::get_gemini_transcription_credential_status,
            commands::credentials::set_gemini_transcription_api_key,
            commands::credentials::clear_gemini_transcription_api_key
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Brawler application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn health_reports_ok() {
        let response = super::commands::health::health();

        assert_eq!(response.status, "ok");
        assert_eq!(response.version, "0.13.0");
    }
}
