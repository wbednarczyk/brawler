use serde::Serialize;
use tauri::Manager;

pub mod app_state;
pub mod data_directory;
pub mod document_fetcher;
pub mod entity_resolution;
pub mod fundamentals;
pub mod interpretation;
pub mod ir_resolution;
pub mod jobs;
pub mod licensing;
pub mod logging;
pub mod observability;
pub mod providers;
pub mod report_diff;
pub mod report_documents_capture;
pub mod signal_dates;
pub mod source_adapters;
pub mod storage;

/// Reusable invariant assertions for data-transform property tests (ADR 0049).
/// Test-only; never compiled into the shipped binary.
#[cfg(test)]
pub mod transform_invariants;

/// TypeScript marker enums for API string-literal unions (ADR 0048); compiled
/// only for `make types`. See the module docs.
#[cfg(feature = "ts-export")]
mod api_ts_unions;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub(crate) status: &'static str,
    pub(crate) version: &'static str,
}

pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    configure_linux_webkit_runtime_environment();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_data_dir = data_directory::runtime_data_dir(app.path().app_data_dir()?)?;
            std::fs::create_dir_all(&app_data_dir)?;

            let database_path = app_data_dir.join("brawler.sqlite3");
            let state = storage::open_pool(database_path, app_data_dir.clone())?;
            if developer_mode_requested_from_environment() {
                state.set_developer_mode_enabled(true)?;
            }
            let log_settings = logging::resolve_log_settings(&state.get_settings()?.logs);
            let logs_dir =
                logging::init_file_logger(&app_data_dir, log_settings).map_err(|error| {
                    std::io::Error::other(format!("failed to initialize local logger: {error:?}"))
                })?;
            log::info!("local logging initialized at {}", logs_dir.display());

            // One-key-per-provider migration (ADR 0028): drop legacy purpose-scoped
            // keychain entries so no orphaned secrets linger. Best-effort, no fallback.
            providers::credentials::clear_legacy_credentials();

            // Keep the disposable embedding index fresh: when the embedding
            // similarity strategy is active and the model is available, refresh the
            // embed/re-embed job at startup (ADR 0035). Now enqueued onto the
            // durable job queue (ADR 0050) instead of fire-and-forget, so a crash
            // mid-embed resumes; the work is idempotent (skips unchanged content).
            // Uses the stable id `CONTENT_EMBEDDING_KIND` every startup, so
            // `reschedule` (not plain `enqueue`) is required: after the first
            // successful run this row is terminal (`succeeded`), and plain
            // `INSERT OR IGNORE` would silently no-op on every later startup — the
            // "keep it fresh" refresh would fire exactly once, ever (bug class
            // dce9ce8). `reschedule` re-arms it and leaves an in-flight run alone.
            if matches!(
                state.get_similarity_strategy().as_deref(),
                Ok(interpretation::EMBEDDING_STRATEGY)
            ) {
                if let Err(error) = state.jobs().reschedule(
                    jobs::handlers::CONTENT_EMBEDDING_KIND,
                    jobs::handlers::CONTENT_EMBEDDING_KIND,
                    "{}",
                    3,
                ) {
                    log::warn!("failed to enqueue startup content embedding job: {error}");
                }
            }

            // Start the durable-queue worker as isolated lanes (ADR 0059): reclaim
            // crash residue, then drain each lane's kinds on its own threads off the
            // UI thread, so a slow source refresh cannot starve autopilot (ADR 0050).
            jobs::queue::spawn_pools(
                std::sync::Arc::new(jobs::handlers::build_worker(state.clone())),
                jobs::handlers::pool_layout(state.queue_config()),
            );

            // Start the Rust-side source scheduler (ADR 0055 / AV5): it owns the
            // refresh cadence, re-arming source/registry refresh jobs on the durable
            // queue. Replaces the frontend timer, which the webview throttles when
            // hidden. Runs only while the app is open.
            jobs::scheduler::spawn(state.clone());

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
            commands::companies::get_company_ir_reports_url,
            commands::companies::set_company_ir_reports_url,
            commands::watchlists::list_watchlists,
            commands::watchlists::list_watchlist_memberships,
            commands::watchlists::create_watchlist,
            commands::watchlists::rename_watchlist,
            commands::watchlists::delete_watchlist,
            commands::watchlists::add_company_to_watchlist,
            commands::watchlists::remove_company_from_watchlist,
            commands::cockpit_layouts::list_cockpit_layouts,
            commands::cockpit_layouts::save_cockpit_layout,
            commands::cockpit_layouts::rename_cockpit_layout,
            commands::cockpit_layouts::delete_cockpit_layout,
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
            commands::research::list_research_questions,
            commands::research::create_research_question,
            commands::research::update_research_question,
            commands::research::delete_research_question,
            commands::research::create_evidence_link,
            commands::research::list_evidence_links,
            commands::research::delete_evidence_link,
            commands::research::start_research_brief,
            commands::research::list_research_briefs,
            commands::research::list_research_reminders,
            commands::research::create_research_reminder,
            commands::research::update_research_reminder,
            commands::research::delete_research_reminder,
            commands::research::start_research_digest,
            commands::research::list_research_digests,
            commands::search::search,
            commands::backups::backup_status,
            commands::backups::create_backup,
            commands::backups::restore_backup,
            commands::ai_analysis::start_ai_analysis,
            commands::ai_analysis::list_ai_analysis,
            commands::ai_analysis::retry_ai_analysis,
            commands::notebooks::list_notebook_entries,
            commands::notebooks::create_notebook_entry,
            commands::notebooks::create_note_from_transcript_selection,
            commands::notebooks::update_notebook_entry,
            commands::notebooks::delete_notebook_entry,
            commands::management_claims::list_management_claims,
            commands::management_claims::create_management_claim,
            commands::management_claims::update_management_claim,
            commands::management_claims::set_claim_verdict,
            commands::management_claims::delete_management_claim,
            commands::management_claims::list_claims_to_verify,
            commands::report_season::list_report_season,
            commands::report_season::get_pre_report_card,
            commands::report_season::mark_report_prepared,
            commands::report_season::mark_report_processed,
            commands::report_diff::extract_report_sections,
            commands::report_diff::fetch_report_document,
            commands::report_diff::list_report_diff_candidates,
            commands::report_diff::get_report_diff,
            commands::claim_extraction::start_claim_extraction,
            commands::claim_extraction::retry_claim_extraction,
            commands::claim_extraction::list_claim_extraction,
            commands::claim_extraction::confirm_claim_proposal,
            commands::claim_extraction::reject_claim_proposal,
            commands::events::list_company_events,
            commands::events::create_company_event,
            commands::signals::list_company_signals,
            commands::signals::confirm_company_signal,
            commands::signals::reject_company_signal,
            commands::signals::confirm_derived_event,
            commands::signals::run_ai_signal_classification,
            commands::signals::run_ai_event_derivation,
            commands::financials::list_kpi_definitions,
            commands::financials::create_kpi_definition,
            commands::financials::list_financial_periods,
            commands::financials::create_financial_period,
            commands::financials::update_financial_period,
            commands::financials::delete_financial_period,
            commands::financials::list_kpi_relevance,
            commands::financials::create_kpi_relevance,
            commands::financials::update_kpi_relevance,
            commands::financials::delete_kpi_relevance,
            commands::financials::list_financial_facts,
            commands::financials::create_financial_fact,
            commands::financials::update_financial_fact,
            commands::financials::delete_financial_fact,
            commands::financials::capture_report_document,
            commands::financials::list_report_documents,
            commands::financials::resolve_ir_report,
            commands::quality_frameworks::list_quality_frameworks,
            commands::quality_frameworks::get_quality_framework,
            commands::quality_frameworks::create_quality_framework,
            commands::quality_frameworks::update_quality_framework,
            commands::quality_frameworks::delete_quality_framework,
            commands::quality_frameworks::clone_framework,
            commands::quality_frameworks::reset_framework_to_template,
            commands::quality_frameworks::create_framework_criterion,
            commands::quality_frameworks::update_framework_criterion,
            commands::quality_frameworks::delete_framework_criterion,
            commands::quality_frameworks::validate_criterion_expression,
            commands::quality_frameworks::evaluate_framework,
            commands::quality_frameworks::list_framework_evaluations,
            commands::quality_frameworks::get_framework_evaluation,
            commands::quality_frameworks::delete_framework_evaluation,
            commands::quality_frameworks::list_available_metric_keys,
            commands::kpi_extraction::start_kpi_extraction,
            commands::kpi_extraction::retry_kpi_extraction,
            commands::kpi_extraction::list_kpi_extraction,
            commands::kpi_extraction::confirm_kpi_proposal,
            commands::kpi_extraction::reject_kpi_proposal,
            commands::fundamentals_extraction::run_structured_extraction,
            commands::fundamentals_extraction::list_fact_provenance,
            commands::fundamentals_extraction::list_flagged_fact_provenance,
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
            commands::sources::backfill_company_history,
            commands::sources::get_backfill_progress,
            commands::sources::refresh_gpw_company_registry,
            commands::sources::refresh_gpw_company_registry_if_stale,
            commands::sources::get_scheduler_status,
            commands::diagnostics::list_diagnostic_events,
            commands::diagnostics::clear_diagnostic_events,
            commands::diagnostics::get_diagnostic_summary,
            commands::interpretation::get_embedding_model_status,
            commands::interpretation::download_embedding_model,
            commands::interpretation::set_similarity_strategy,
            commands::interpretation::rebuild_embedding_index,
            commands::interpretation::find_similar_content,
            commands::logs::get_log_status,
            commands::logs::list_log_entries,
            commands::logs::open_logs_directory,
            commands::metrics::get_local_metrics_snapshot,
            commands::licensing::get_license_status,
            commands::licensing::submit_license_key,
            commands::licensing::clear_license_key,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::list_ai_provider_catalog,
            commands::settings::disable_developer_mode,
            commands::settings::unlock_developer_mode,
            commands::credentials::get_provider_credential_status,
            commands::credentials::set_provider_api_key,
            commands::credentials::clear_provider_api_key,
            commands::autopilot::get_company_autopilot,
            commands::autopilot::set_company_autopilot,
            commands::autopilot::list_company_autopilot_modes,
            commands::autopilot::set_companies_autopilot,
            commands::autopilot::list_autopilot_runs,
            commands::autopilot::get_autopilot_run,
            commands::autopilot::set_autopilot_run_notification_state,
            commands::autopilot::undo_autopilot_run,
            commands::autopilot::trigger_autopilot_run
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Brawler application");
}

#[cfg(target_os = "linux")]
fn configure_linux_webkit_runtime_environment() {
    if should_disable_webkit_dmabuf_renderer(
        std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some(),
        std::env::var_os("WSL_INTEROP").is_some(),
        std::env::var_os("WSL_DISTRO_NAME").is_some(),
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .ok()
            .as_deref(),
    ) {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_webkit_runtime_environment() {}

#[cfg(target_os = "linux")]
fn should_disable_webkit_dmabuf_renderer(
    already_configured: bool,
    has_wsl_interop: bool,
    has_wsl_distro_name: bool,
    kernel_osrelease: Option<&str>,
) -> bool {
    !already_configured
        && (has_wsl_interop
            || has_wsl_distro_name
            || kernel_osrelease
                .map(|release| {
                    let release = release.to_ascii_lowercase();
                    release.contains("microsoft") || release.contains("wsl")
                })
                .unwrap_or(false))
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
        assert_eq!(response.version, "0.48.0");
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

    #[cfg(target_os = "linux")]
    #[test]
    fn disables_webkit_dmabuf_renderer_on_wsl_when_unconfigured() {
        assert!(super::should_disable_webkit_dmabuf_renderer(
            false, true, false, None
        ));
        assert!(super::should_disable_webkit_dmabuf_renderer(
            false, false, true, None
        ));
        assert!(super::should_disable_webkit_dmabuf_renderer(
            false,
            false,
            false,
            Some("6.6.87.2-microsoft-standard-WSL2")
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn keeps_webkit_dmabuf_renderer_default_outside_wsl_or_when_configured() {
        assert!(!super::should_disable_webkit_dmabuf_renderer(
            false,
            false,
            false,
            Some("6.8.0-60-generic")
        ));
        assert!(!super::should_disable_webkit_dmabuf_renderer(
            true,
            true,
            true,
            Some("6.6.87.2-microsoft-standard-WSL2")
        ));
    }
}
