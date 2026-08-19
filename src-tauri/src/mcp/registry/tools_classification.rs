//! Classification-only rows for every command not carried by an exposed tool
//! — wired into [`super::entries`].

use super::{act, excluded, read, RegistryEntry};

/// Classification of every command NOT already carried by an exposed tool row.
/// Seeded per ADR 0088 dec. 2 (`read` = domain reads; `act` = research writes /
/// workspace actions / job triggers, provenance named where a carrier exists;
/// `excluded` = deletes, undo, settings/credentials, MCP self-management,
/// dev/diagnostic mutations). Reviewed by the orchestrator; the classification
/// gate keeps it exhaustive.
pub(super) fn classifications() -> Vec<RegistryEntry> {
    vec![
        // ---- Reads classified but deliberately NOT exposed -----------------
        // The broad read wave (M2) exposes the LIST + targeted GET per domain
        // via `read_wave_tools()`; the reads below stay `read`-tier but
        // unexposed. Every skip carries its one-line justification — a silent
        // skip is a defect (ADR 0088 dec. 2). Grouped by why.
        //
        // Infra / liveness / diagnostics — no investor-facing data:
        read("health"),                     // process liveness probe
        read("database_status"),            // DB engine/migration diagnostics
        read("backup_status"),              // local-backup infra status
        read("get_history_sweep_progress"), // quote-backfill job progress (UI)
        read("get_backfill_progress"),      // backfill job progress (UI)
        read("get_scheduler_status"),       // scheduler/ops status
        read("list_diagnostic_events"),     // diagnostics/ops event log
        read("get_diagnostic_summary"),     // diagnostics/ops rollup
        read("list_source_reconciliation"), // source-reconciliation diagnostics
        read("get_log_status"),             // log-file status (ops)
        read("list_log_entries"),           // app log lines (ops)
        read("get_local_metrics_snapshot"), // local perf metrics (ops)
        // Settings / config / credentials — sensitive or UI-only config:
        read("get_settings"),       // app settings surface (UI/config)
        read("get_license_status"), // licensing state (config)
        read("get_provider_credential_status"), // credential presence (sensitive)
        read("list_source_adapters"), // source-adapter enable/config catalog
        read("list_cockpit_layouts"), // UI dashboard layout persistence
        read("list_company_autopilot_modes"), // autopilot-mode picker presets (UI)
        read("get_company_autopilot"), // per-company autopilot mode (config; runs exposed)
        // Reference / lookup / autocomplete plumbing:
        read("lookup_company"), // registry autocomplete + directory bootstrap; tools resolve tickers internally
        read("list_company_registry_entries"), // GPW registry directory dump (reference)
        read("get_company_ir_reports_url"), // single config URL (subset of get_company_basic_info intent)
        read("get_company_sector"),         // subset of get_company_basic_info
        read("list_company_sectors"),       // sector-picker preset values (UI)
        read("list_available_metric_keys"), // criterion-editor autocomplete (UI)
        read("validate_criterion_expression"), // criterion-editor validator (authoring aid)
        read("list_kpi_relevance"),         // per-company KPI-relevance config (scorecard editor)
        // Superseded by a richer exposed tool:
        read("list_report_documents"), // superseded by get_report_documents_view (adds period + canonical flag)
        read("get_pre_report_card"), // pre-report UI card; data via list_report_season + list_report_expectations
        read("expectation_review"), // derived per-event review panel; underlying data via list_report_expectations
        read("get_framework_evaluation"), // single stored evaluation; latest-per-framework via get_quality_assessment
        read("get_quality_framework"), // one framework's detail; catalog via list_quality_frameworks
        read("list_fact_provenance"), // folded into list_financial_facts (provenance travels with each fact)
        // Data-management import/export plumbing (destructive counterparts excluded):
        read("export_research_data"), // bulk export dump; content via search + per-domain reads
        read("preview_research_import"), // import-preview plumbing
        read("export_settings_data"), // settings export (config)
        read("preview_settings_import"), // settings import-preview plumbing
        // Research-workspace internals / aggregates already covered elsewhere:
        read("list_research_evidence"), // research timeline aggregates already-exposed per-domain reads
        read("list_company_timeline"),  // company-scoped alias of list_research_evidence
        read("list_watchlist_timeline"), // watchlist-scoped alias of list_research_evidence
        read("list_research_review_state"), // per-scope "reviewed" checkpoints (UI markers)
        read("list_evidence_links"), // internal evidence-graph edges (UI); agent reads the items directly
        read("list_research_reminders"), // personal follow-up reminders (UI workflow)
        // list_alert_rules is now an exposed read (act_wave companions read their
        // own rules) — carried by read_wave_tools().
        // Triage surfaces: list_unmatched_source_items stays classified-only
        // (unmatched-source triage is a separate family, no tool this slice);
        // list_flagged_extraction_outcomes + list_unclassified_filings are now
        // EXPOSED reads — carried by read_wave_tools() (ADR 0088 dec. 4, M4).
        read("list_unmatched_source_items"), // unmatched-source triage (no MCP tool yet)
        // Layer 1 raw-tagged-fact trust surface (ADR 0100, epic #398 final
        // slice). `get_report_tagged_fact_coverage` is EXPOSED (decision 11)
        // — carried by read_wave_tools(); the concept list stays unexposed
        // because it exists only to feed the owner-only promotion action.
        read("list_uncrosswalked_concepts"), // "positions the program doesn't know yet" list (UI)
        // promote_uncrosswalked_concept is the OWNER'S OWN authority (ADR
        // 0100 decision 10): "the owner may promote a captured position into
        // Fundamentals; a machine still may not" — permanently excluded, not
        // merely unexposed.
        excluded("promote_uncrosswalked_concept"),
        // ---- Act: exposed writes -------------------------------------------
        // Every provenance-carrying write, the broad no-provenance research
        // writes, workspace actions, and the light/fail-fast job triggers are
        // now EXPOSED — carried by `act_wave_tools()` (removed from here so each
        // command is classified exactly once). Only the act commands NOT yet
        // exposed remain below, each with its one-line justification (a silent
        // skip is a defect, ADR 0088 dec. 2).
        //
        // ---- Act: writes classified but deliberately NOT exposed -----------
        // Niche period plumbing (facts reference an existing periodId; period
        // create/update is a rare authoring step, UI-driven):
        act("create_financial_period", None),
        act("update_financial_period", None),
        // Framework cloning (whole-framework duplication; niche authoring aid):
        act("clone_framework", None),
        // reset_framework_to_template destroys user customization (delete-class,
        // orchestrator ruling at M1 review) — permanently UI-only.
        excluded("reset_framework_to_template"),
        // Company config edits (single-field URL/sector setters; UI config):
        act("set_company_ir_reports_url", None),
        act("set_company_sector", None),
        act("rename_watchlist", None), // watchlist rename (UI config)
        act("rename_cockpit_layout", None), // saved-view rename (UI config, issue #89)
        act("resolve_transcript_job_company", None), // transcript-triage UI step
        // Report-pipeline job triggers (multi-stage document machinery; UI-driven
        // per-document, not a clean headless agent surface):
        act("extract_report_sections", None),
        act("fetch_report_document", None),
        act("reclassify_report_documents", None),
        act("resolve_ir_report", None),
        act("extract_report_document_data", None),
        // Admin / one-off job triggers — genuinely not part of the agent research
        // surface (whole-corpus rebuilds, quote-history sweeps, registry refresh,
        // and derived-fact backfills that the exposed extraction/refresh tools
        // already cover). (The networked source-refresh / extraction / company
        // backfill triggers ARE exposed — see `act_wave_tools`.)
        act("backfill_company_health_facts", None),
        act("backfill_ownership_extraction", None),
        act("run_history_sweep", None),
        act("rebuild_fundamentals", None),
        act("refresh_gpw_company_registry", None),
        act("refresh_gpw_company_registry_if_stale", None),
        // Video-transcript lifecycle (in-app AI transcript provider; UI-driven
        // enqueue/run, and the only in-app AI dependency — kept off the agent
        // surface):
        act("create_video_transcript_job", None),
        act("update_video_transcript_job", None),
        act("run_video_transcript_job", None),
        // Local-filesystem write (the export save path; issue #106) — an agent
        // must never write arbitrary files on the owner's machine; agents read
        // exports through export_research_data/export_settings_data instead.
        excluded("write_export_file"),
        // ---- Excluded: deletes ---------------------------------------------
        excluded("delete_company"),
        excluded("delete_watchlist"),
        excluded("delete_cockpit_layout"),
        excluded("delete_research_question"),
        excluded("delete_evidence_link"),
        excluded("delete_research_reminder"),
        excluded("delete_notebook_entry"),
        excluded("delete_management_claim"),
        excluded("delete_financial_period"),
        excluded("delete_kpi_relevance"),
        excluded("delete_financial_fact"),
        excluded("delete_quality_framework"),
        excluded("delete_framework_criterion"),
        excluded("delete_framework_evaluation"),
        excluded("delete_video_transcript_job"),
        excluded("delete_alert_rule"),
        // ---- Excluded: undo ------------------------------------------------
        excluded("undo_autopilot_run"),
        // ---- Excluded: bulk import / backups (destructive data management) --
        excluded("apply_research_import"),
        excluded("apply_settings_import"),
        excluded("create_backup"),
        excluded("restore_backup"),
        // ---- Excluded: settings / configuration ----------------------------
        excluded("update_settings"),
        // UI-session semantics: Today's bulk "was on screen" seen-marking for the
        // sidebar badge (ADR 0097 dec. 5). Agents mark individual events via the
        // exposed mark_attention_event_seen.
        excluded("mark_attention_events_seen"),
        excluded("save_cockpit_layout"),
        excluded("set_source_adapter_enabled"),
        excluded("set_company_autopilot"),
        excluded("set_companies_autopilot"),
        // ---- Excluded: credentials / licensing -----------------------------
        excluded("submit_license_key"),
        excluded("clear_license_key"),
        excluded("set_provider_api_key"),
        excluded("clear_provider_api_key"),
        // ---- Excluded: MCP self-management (reads included — sensitive) -----
        excluded("regenerate_mcp_token"),
        excluded("revoke_mcp_token"),
        excluded("mcp_token_status"),
        excluded("set_mcp_enabled"),
        excluded("mcp_status"),
        excluded("regenerate_kpi_acquisition_token"),
        excluded("revoke_kpi_acquisition_token"),
        excluded("kpi_acquisition_token_status"),
        // ---- Excluded: dev / diagnostics mutations + OS side effects -------
        excluded("clear_diagnostic_events"),
        excluded("open_logs_directory"),
        excluded("disable_developer_mode"),
        excluded("unlock_developer_mode"),
    ]
}
