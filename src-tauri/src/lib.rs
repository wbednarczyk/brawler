use serde::Serialize;
use tauri::Manager;

pub mod app_state;
pub mod data_directory;
pub mod document_fetcher;
pub mod effects_honesty;
pub mod fundamentals;
pub mod fx;
pub mod interpretation;
pub mod ir_resolution;
pub mod jobs;
pub mod licensing;
pub mod logging;
pub mod mcp;
pub mod observability;
pub mod providers;
pub mod report_diff;
pub mod report_documents_capture;
pub mod report_documents_container;
pub mod report_documents_restore;
pub mod signal_dates;
pub mod source_adapters;
pub mod storage;
pub mod valuation;

/// Reusable invariant assertions for data-transform property tests (ADR 0049).
/// Test-only; never compiled into the shipped binary.
#[cfg(test)]
pub mod transform_invariants;

/// Shared test-only helpers (e.g. the capturing logger). Test-only; never
/// compiled into the shipped binary.
#[cfg(test)]
pub mod test_support;

/// Source-tree guard tests (ADR 0045 harvests, e.g. the cargo-mutants
/// cross-tree include guard). Test-only; never compiled into the shipped
/// binary.
#[cfg(test)]
mod source_tree_guards;

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

            // Repair report documents mis-associated onto the wrong company by
            // tag-listing ingestion (T-A3, card 45fcece). Idempotent and
            // self-healing: safe on every startup, records nothing when there is
            // nothing to fix. Best-effort — a failure is logged, never fatal.
            match state.repair_misassociated_report_documents() {
                Ok(0) => {}
                Ok(removed) => log::warn!(
                    "startup mis-association repair removed {removed} report document(s)"
                ),
                Err(error) => {
                    log::warn!("startup mis-association repair failed: {error}");
                }
            }

            // Restore the documents migration 0107 wrongly deleted (epic #229 T3,
            // owner decision 2026-07-30): four cyber_Folks Q3-2024 filings removed
            // on URL-slug evidence their own bytes contradict. Runs AFTER the
            // mis-association repair (which keeps them — their titles name the
            // owner) and BEFORE the container sniff, which it pre-empts by
            // stamping the container itself. Idempotent, best-effort, never fatal.
            match report_documents_restore::restore_migration_0107_deletions(&state) {
                Ok(summary) if summary.is_noop() => {}
                Ok(summary) => log::warn!(
                    "startup 0107 restore: {} document(s) restored, {} already present, \
                     {} file(s) missing, {} untracked company row(s)",
                    summary.restored,
                    summary.already_present,
                    summary.file_missing,
                    summary.company_absent
                ),
                Err(error) => log::warn!("startup 0107 restore failed: {error}"),
            }

            // Container self-heal (epic #229 T2): stamp `detected_container` on
            // fetched documents stored before migration 0121, reading each file's
            // magic bytes (SQL cannot). Idempotent and best-effort — a row whose
            // file is missing stays NULL and is retried next start.
            match report_documents_container::sniff_missing_containers(&state) {
                Ok(summary) if summary.scanned == 0 => {}
                Ok(summary) => log::info!(
                    "startup container sniff scanned {} document(s): stamped {} {:?}, {} file(s) missing",
                    summary.scanned,
                    summary.stamped,
                    summary.by_container,
                    summary.file_missing
                ),
                Err(error) => log::warn!("startup container sniff failed: {error}"),
            }

            // Start the durable-queue worker as isolated lanes (ADR 0059): reclaim
            // crash residue, then drain each lane's kinds on its own threads off the
            // UI thread, so a slow source refresh cannot starve autopilot (ADR 0050).
            jobs::queue::spawn_pools(
                std::sync::Arc::new(jobs::handlers::build_worker(state.clone())),
                jobs::handlers::pool_layout(state.queue_config()),
            );

            // Ownership extraction catch-up (ADR 0072 T3): a cold or
            // persisted-but-stale DB may hold fetched periodic reports whose
            // shareholders table was never parsed (neither a stake nor a residual
            // recorded). Enqueue extraction for exactly those, off the UI thread on
            // the durable queue. Best-effort — a failure is logged, never fatal.
            let catch_up =
                jobs::ownership_extraction::enqueue_ownership_extraction_catch_up(&state, None);
            if catch_up > 0 {
                log::info!("startup ownership extraction catch-up enqueued {catch_up} document(s)");
            }

            // Management-holdings extraction catch-up (ADR 0083 T5): the same cold-DB
            // gap-filling for the insider-substrate holdings section.
            let mgmt_catch_up =
                jobs::management_holdings_extraction::enqueue_management_extraction_catch_up(
                    &state, None,
                );
            if mgmt_catch_up > 0 {
                log::info!(
                    "startup management-holdings extraction catch-up enqueued {mgmt_catch_up} document(s)"
                );
            }

            // Report-history backfill catch-up (v0.57, ADR 0077 amendment): a cold
            // or persisted-but-stale DB may hold automated companies with NO fetched
            // periodic report (the owner's live DB: 33/50). Enqueue one automatic
            // backfill each, off the UI thread on the durable queue. Idempotent
            // (coverage predicate + stable per-company job id) and `off`-mode-aware.
            // Best-effort — a failure is logged, never fatal.
            let backfill_catch_up =
                jobs::backfill::enqueue_company_backfill_catch_up(&state, None);
            if backfill_catch_up > 0 {
                log::info!(
                    "startup report-history backfill catch-up enqueued {backfill_catch_up} company backfill(s)"
                );
            }

            // Insider cover-note parse catch-up (F-B): the deterministic art. 19
            // cover-note parse only runs inside a source refresh, so a cold or
            // persisted-but-stale DB may hold confirmed `insider_transaction`
            // signals (ingested by a prior version) whose cover notes were never
            // parsed — leaving the insider timeline empty until a manual refresh.
            // Parse those pending filings now, mirroring the ownership and
            // management-holdings catch-ups above. `parse_pending` is idempotent
            // (already-parsed or parked filings are skipped), so it writes zero
            // duplicate rows. Best-effort — a failure is logged, never fatal.
            match state.insider().parse_pending() {
                Ok(0) => {}
                Ok(written) => log::info!(
                    "startup insider cover-note parse catch-up wrote {written} transaction row(s)"
                ),
                Err(error) => {
                    log::warn!("startup insider cover-note parse catch-up failed: {error}");
                }
            }

            // Start the Rust-side source scheduler (ADR 0055 / AV5): it owns the
            // refresh cadence, re-arming source/registry refresh jobs on the durable
            // queue. Replaces the frontend timer, which the webview throttles when
            // hidden. Runs only while the app is open.
            jobs::scheduler::spawn(state.clone());

            // Read-only MCP server (ADR 0078): app-open-only lifetime, off by
            // default. Start only when Settings enable it AND a token exists; a
            // missing token or a bind failure is captured in `mcp_status`, never
            // a crash. Enable/disable later takes effect live via `set_mcp_enabled`.
            let mcp_lifecycle = mcp::lifecycle::McpLifecycle::new();
            if state
                .get_settings()
                .map(|settings| settings.mcp.enabled)
                .unwrap_or(false)
            {
                let status = mcp_lifecycle.ensure_started(&state);
                if let Some(error) = &status.error {
                    log::warn!("MCP server did not start: {error}");
                }
            }
            app.manage(mcp_lifecycle);

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
            commands::companies::get_company_sector,
            commands::companies::set_company_sector,
            commands::companies::list_company_sectors,
            commands::companies::get_company_basic_info,
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
            commands::import_export::write_export_file,
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
            commands::research::list_research_reminders,
            commands::research::create_research_reminder,
            commands::research::update_research_reminder,
            commands::research::delete_research_reminder,
            commands::search::search,
            commands::backups::backup_status,
            commands::backups::create_backup,
            commands::backups::restore_backup,
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
            commands::decision_journal::create_decision_entry,
            commands::decision_journal::list_decision_entries,
            commands::report_expectations::create_report_expectation,
            commands::report_expectations::update_report_expectation,
            commands::report_expectations::list_report_expectations,
            commands::report_expectations::expectation_review,
            commands::report_expectations::record_expectation_resolution,
            commands::report_diff::extract_report_sections,
            commands::report_diff::fetch_report_document,
            commands::report_diff::list_report_diff_candidates,
            commands::report_diff::get_report_diff,
            commands::events::list_company_events,
            commands::events::create_company_event,
            commands::short_positions::list_short_positions,
            commands::signals::list_company_signals,
            commands::signals::confirm_company_signal,
            commands::signals::reject_company_signal,
            commands::signals::confirm_derived_event,
            commands::signals::list_unclassified_filings,
            commands::signals::classify_filing,
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
            commands::financials::reclassify_report_documents,
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
            commands::quality_frameworks::set_qualitative_verdicts,
            commands::fundamentals_extraction::run_structured_extraction,
            commands::fundamentals_extraction::extract_report_document_data,
            commands::fundamentals_extraction::list_fact_provenance,
            commands::fundamentals_extraction::list_flagged_fact_provenance,
            commands::fundamentals_extraction::list_flagged_extraction_outcomes,
            commands::fundamentals_extraction::rerun_extraction_outcome,
            commands::fundamentals_coverage::get_fundamentals_coverage,
            commands::company_health::get_company_health,
            commands::company_health::backfill_company_health_facts,
            commands::company_health::get_red_flags,
            commands::company_health::acknowledge_red_flag,
            commands::insider::get_insider_overview,
            commands::market_data::get_price_context,
            commands::comparison::get_kpi_comparison,
            commands::sector_percentiles::get_sector_percentiles,
            commands::valuation::compute_comparative_valuation,
            commands::valuation::list_valuation_runs,
            commands::analyst_recommendations::get_analyst_recommendations,
            commands::ownership::get_ownership_overview,
            commands::ownership::backfill_ownership_extraction,
            commands::ownership::set_ownership_holder_type,
            commands::history_sweep::run_history_sweep,
            commands::history_sweep::get_history_sweep_progress,
            commands::aggregator_fundamentals::run_aggregator_fundamentals_pull,
            commands::rebuild_fundamentals::rebuild_fundamentals,
            commands::report_documents_view::get_report_documents_view,
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
            commands::attention::create_alert_rule,
            commands::attention::list_alert_rules,
            commands::attention::update_alert_rule,
            commands::attention::set_alert_rule_enabled,
            commands::attention::delete_alert_rule,
            commands::attention::list_attention_events,
            commands::attention::mark_attention_event_seen,
            commands::attention::dismiss_attention_event,
            commands::morning_briefing::generate_morning_briefing,
            commands::morning_briefing::get_latest_morning_briefing,
            commands::diagnostics::list_diagnostic_events,
            commands::diagnostics::list_source_reconciliation,
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
            commands::credentials::get_provider_credential_status,
            commands::credentials::set_provider_api_key,
            commands::credentials::clear_provider_api_key,
            commands::mcp::regenerate_mcp_token,
            commands::mcp::revoke_mcp_token,
            commands::mcp::mcp_token_status,
            commands::mcp::set_mcp_enabled,
            commands::mcp::mcp_status,
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
        assert_eq!(response.version, "0.62.9");
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
