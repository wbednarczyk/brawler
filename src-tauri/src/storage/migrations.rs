use super::*;
use std::path::Path;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!("../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "feed_item_display_company",
        sql: include_str!("../../migrations/0002_feed_item_display_company.sql"),
    },
    Migration {
        version: 3,
        name: "notebook_entry_origins",
        sql: include_str!("../../migrations/0003_notebook_entry_origins.sql"),
    },
    Migration {
        version: 4,
        name: "notebook_follow_ups",
        sql: include_str!("../../migrations/0004_notebook_follow_ups.sql"),
    },
    Migration {
        version: 5,
        name: "feed_item_attachments",
        sql: include_str!("../../migrations/0005_feed_item_attachments.sql"),
    },
    Migration {
        version: 6,
        name: "company_registry",
        sql: include_str!("../../migrations/0006_company_registry.sql"),
    },
    Migration {
        version: 7,
        name: "bankier_market_rss",
        sql: include_str!("../../migrations/0007_bankier_market_rss.sql"),
    },
    Migration {
        version: 8,
        name: "portal_analiz_source_placeholder",
        sql: include_str!("../../migrations/0008_portal_analiz_source_placeholder.sql"),
    },
    Migration {
        version: 9,
        name: "feed_item_duplicate_signatures",
        sql: include_str!("../../migrations/0009_feed_item_duplicate_signatures.sql"),
    },
    Migration {
        version: 10,
        name: "bankier_company_komunikaty",
        sql: include_str!("../../migrations/0010_bankier_company_komunikaty.sql"),
    },
    Migration {
        version: 11,
        name: "disable_gpw_espi_ebi",
        sql: include_str!("../../migrations/0011_disable_gpw_espi_ebi.sql"),
    },
    Migration {
        version: 12,
        name: "bankier_reviewed_rss_placeholders",
        sql: include_str!("../../migrations/0012_bankier_reviewed_rss_placeholders.sql"),
    },
    Migration {
        version: 13,
        name: "company_events",
        sql: include_str!("../../migrations/0013_company_events.sql"),
    },
    Migration {
        version: 14,
        name: "gpw_market_events_rss",
        sql: include_str!("../../migrations/0014_gpw_market_events_rss.sql"),
    },
    Migration {
        version: 15,
        name: "event_source_candidates",
        sql: include_str!("../../migrations/0015_event_source_candidates.sql"),
    },
    Migration {
        version: 16,
        name: "enable_bankier_kalendarium",
        sql: include_str!("../../migrations/0016_enable_bankier_kalendarium.sql"),
    },
    Migration {
        version: 17,
        name: "transcript_storage_foundation",
        sql: include_str!("../../migrations/0017_transcript_storage_foundation.sql"),
    },
    Migration {
        version: 18,
        name: "youtube_transcription_provider_id",
        sql: include_str!("../../migrations/0018_youtube_transcription_provider_id.sql"),
    },
    Migration {
        version: 19,
        name: "youtube_transcription_model",
        sql: include_str!("../../migrations/0019_youtube_transcription_model.sql"),
    },
    Migration {
        version: 20,
        name: "youtube_transcription_timeout",
        sql: include_str!("../../migrations/0020_youtube_transcription_timeout.sql"),
    },
    Migration {
        version: 21,
        name: "gemini_default_model_to_validated_flash",
        sql: include_str!("../../migrations/0021_gemini_default_model_to_validated_flash.sql"),
    },
    Migration {
        version: 22,
        name: "app_locale",
        sql: include_str!("../../migrations/0022_app_locale.sql"),
    },
    Migration {
        version: 23,
        name: "shortcut_bindings",
        sql: include_str!("../../migrations/0023_shortcut_bindings.sql"),
    },
    Migration {
        version: 24,
        name: "ai_analysis_jobs",
        sql: include_str!("../../migrations/0024_ai_analysis_jobs.sql"),
    },
    Migration {
        version: 25,
        name: "developer_mode_setting",
        sql: include_str!("../../migrations/0025_developer_mode_setting.sql"),
    },
    Migration {
        version: 26,
        name: "diagnostic_events",
        sql: include_str!("../../migrations/0026_diagnostic_events.sql"),
    },
    Migration {
        version: 27,
        name: "log_settings",
        sql: include_str!("../../migrations/0027_log_settings.sql"),
    },
    Migration {
        version: 28,
        name: "license_metadata",
        sql: include_str!("../../migrations/0028_license_metadata.sql"),
    },
    Migration {
        version: 29,
        name: "newconnect_company_directory",
        sql: include_str!("../../migrations/0029_newconnect_company_directory.sql"),
    },
    Migration {
        version: 30,
        name: "research_evidence_boundary",
        sql: include_str!("../../migrations/0030_research_evidence_boundary.sql"),
    },
    Migration {
        version: 31,
        name: "research_questions",
        sql: include_str!("../../migrations/0031_research_questions.sql"),
    },
    Migration {
        version: 32,
        name: "ai_research_briefs",
        sql: include_str!("../../migrations/0032_ai_research_briefs.sql"),
    },
    Migration {
        version: 33,
        name: "research_reminders_and_digests",
        sql: include_str!("../../migrations/0033_research_reminders_and_digests.sql"),
    },
    Migration {
        version: 34,
        name: "company_fundamentals",
        sql: include_str!("../../migrations/0034_company_fundamentals.sql"),
    },
    Migration {
        version: 35,
        name: "report_documents",
        sql: include_str!("../../migrations/0035_report_documents.sql"),
    },
    Migration {
        version: 36,
        name: "multi_provider_ai_defaults",
        sql: include_str!("../../migrations/0036_multi_provider_ai_defaults.sql"),
    },
    Migration {
        version: 37,
        name: "kpi_extraction",
        sql: include_str!("../../migrations/0037_kpi_extraction.sql"),
    },
    Migration {
        version: 38,
        name: "company_ir_reports_url",
        sql: include_str!("../../migrations/0038_company_ir_reports_url.sql"),
    },
    Migration {
        version: 39,
        name: "search_index",
        sql: include_str!("../../migrations/0039_search_index.sql"),
    },
    Migration {
        version: 40,
        name: "database_pool_settings",
        sql: include_str!("../../migrations/0040_database_pool_settings.sql"),
    },
    Migration {
        version: 41,
        name: "company_signals",
        sql: include_str!("../../migrations/0041_company_signals.sql"),
    },
    Migration {
        version: 42,
        name: "company_signals_seed_fixup",
        sql: include_str!("../../migrations/0042_company_signals_seed_fixup.sql"),
    },
    Migration {
        version: 43,
        name: "company_signals_self_heal",
        sql: include_str!("../../migrations/0043_company_signals_self_heal.sql"),
    },
    Migration {
        version: 44,
        name: "generalize_own_shares_signal",
        sql: include_str!("../../migrations/0044_generalize_own_shares_signal.sql"),
    },
    Migration {
        version: 45,
        name: "management_claims",
        sql: include_str!("../../migrations/0045_management_claims.sql"),
    },
    Migration {
        version: 46,
        name: "claim_extraction",
        sql: include_str!("../../migrations/0046_claim_extraction.sql"),
    },
    Migration {
        version: 47,
        name: "report_preparations",
        sql: include_str!("../../migrations/0047_report_preparations.sql"),
    },
    Migration {
        version: 48,
        name: "quality_frameworks",
        sql: include_str!("../../migrations/0048_quality_frameworks.sql"),
    },
    Migration {
        version: 49,
        name: "content_embeddings",
        sql: include_str!("../../migrations/0049_content_embeddings.sql"),
    },
    Migration {
        version: 50,
        name: "roic_roce_resolvable",
        sql: include_str!("../../migrations/0050_roic_roce_resolvable.sql"),
    },
    Migration {
        version: 51,
        name: "job_queue",
        sql: include_str!("../../migrations/0051_job_queue.sql"),
    },
    Migration {
        version: 52,
        name: "feed_item_story_key",
        sql: include_str!("../../migrations/0052_feed_item_story_key.sql"),
    },
    Migration {
        version: 53,
        name: "report_document_sections",
        sql: include_str!("../../migrations/0053_report_document_sections.sql"),
    },
    Migration {
        version: 54,
        name: "cockpit_layouts",
        sql: include_str!("../../migrations/0054_cockpit_layouts.sql"),
    },
    Migration {
        version: 55,
        name: "autopilot",
        sql: include_str!("../../migrations/0055_autopilot.sql"),
    },
    Migration {
        version: 56,
        name: "queue_worker_settings",
        sql: include_str!("../../migrations/0056_queue_worker_settings.sql"),
    },
    Migration {
        version: 57,
        name: "fundamentals_provenance",
        sql: include_str!("../../migrations/0057_fundamentals_provenance.sql"),
    },
    Migration {
        version: 58,
        name: "structured_attachment_fetch_gate",
        sql: include_str!("../../migrations/0058_structured_attachment_fetch_gate.sql"),
    },
    Migration {
        version: 59,
        name: "qualitative_criteria",
        sql: include_str!("../../migrations/0059_qualitative_criteria.sql"),
    },
    Migration {
        version: 60,
        name: "qualitative_assessment_index",
        sql: include_str!("../../migrations/0060_qualitative_assessment_index.sql"),
    },
    Migration {
        version: 61,
        name: "report_document_kind",
        sql: include_str!("../../migrations/0061_report_document_kind.sql"),
    },
    Migration {
        version: 62,
        name: "history_sweeps",
        sql: include_str!("../../migrations/0062_history_sweeps.sql"),
    },
    Migration {
        version: 63,
        name: "ocr_extraction_profile",
        sql: include_str!("../../migrations/0063_ocr_extraction_profile.sql"),
    },
    Migration {
        version: 64,
        name: "kpi_extraction_committed_facts",
        sql: include_str!("../../migrations/0064_kpi_extraction_committed_facts.sql"),
    },
    Migration {
        version: 65,
        name: "sweep_ai_budget",
        sql: include_str!("../../migrations/0065_sweep_ai_budget.sql"),
    },
    Migration {
        version: 66,
        name: "period_type_annual_to_fy",
        sql: include_str!("../../migrations/0066_period_type_annual_to_fy.sql"),
    },
    Migration {
        version: 67,
        name: "decision_entries",
        sql: include_str!("../../migrations/0067_decision_entries.sql"),
    },
    Migration {
        version: 68,
        name: "report_expectations",
        sql: include_str!("../../migrations/0068_report_expectations.sql"),
    },
    Migration {
        version: 69,
        name: "drop_content_embeddings",
        sql: include_str!("../../migrations/0069_drop_content_embeddings.sql"),
    },
    Migration {
        version: 70,
        name: "drop_feed_item_story_key",
        sql: include_str!("../../migrations/0070_drop_feed_item_story_key.sql"),
    },
    Migration {
        version: 71,
        name: "daily_quotes_and_sector",
        sql: include_str!("../../migrations/0071_daily_quotes_and_sector.sql"),
    },
    Migration {
        version: 72,
        name: "level0_market_ratios",
        sql: include_str!("../../migrations/0072_level0_market_ratios.sql"),
    },
    Migration {
        version: 73,
        name: "registry_entry_sector",
        sql: include_str!("../../migrations/0073_registry_entry_sector.sql"),
    },
    Migration {
        version: 74,
        name: "pe_ratio_from_net_profit",
        sql: include_str!("../../migrations/0074_pe_ratio_from_net_profit.sql"),
    },
    Migration {
        version: 75,
        name: "ratio_fallback_recipes",
        sql: include_str!("../../migrations/0075_ratio_fallback_recipes.sql"),
    },
    Migration {
        version: 76,
        name: "remove_twelvedata_adapter",
        sql: include_str!("../../migrations/0076_remove_twelvedata_adapter.sql"),
    },
    Migration {
        version: 77,
        name: "alert_rules_attention_events",
        sql: include_str!("../../migrations/0077_alert_rules_attention_events.sql"),
    },
    Migration {
        version: 78,
        name: "morning_briefings",
        sql: include_str!("../../migrations/0078_morning_briefings.sql"),
    },
    Migration {
        version: 79,
        name: "auditor_opinion_signal",
        sql: include_str!("../../migrations/0079_auditor_opinion_signal.sql"),
    },
    Migration {
        version: 80,
        name: "knf_short_positions",
        sql: include_str!("../../migrations/0080_knf_short_positions.sql"),
    },
    Migration {
        version: 81,
        name: "espi_witness_reconciliation",
        sql: include_str!("../../migrations/0081_espi_witness_reconciliation.sql"),
    },
    Migration {
        version: 82,
        name: "ownership_stakes",
        sql: include_str!("../../migrations/0082_ownership_stakes.sql"),
    },
    Migration {
        version: 83,
        name: "ownership_extraction_residual",
        sql: include_str!("../../migrations/0083_ownership_extraction_residual.sql"),
    },
    Migration {
        version: 84,
        name: "ownership_holder_type_proposals",
        sql: include_str!("../../migrations/0084_ownership_holder_type_proposals.sql"),
    },
    Migration {
        version: 85,
        name: "major_holdings_and_ownership_witness",
        sql: include_str!("../../migrations/0085_major_holdings_and_ownership_witness.sql"),
    },
    Migration {
        version: 86,
        name: "ownership_holder_identity_aliases",
        sql: include_str!("../../migrations/0086_ownership_holder_identity_aliases.sql"),
    },
    Migration {
        version: 87,
        name: "ownership_adapter_source_type",
        sql: include_str!("../../migrations/0087_ownership_adapter_source_type.sql"),
    },
    Migration {
        version: 88,
        name: "ownership_aggregator_reset",
        sql: include_str!("../../migrations/0088_ownership_aggregator_reset.sql"),
    },
    Migration {
        version: 89,
        name: "company_health_kpi_definitions",
        sql: include_str!("../../migrations/0089_company_health_kpi_definitions.sql"),
    },
    Migration {
        version: 90,
        name: "insider_transactions",
        sql: include_str!("../../migrations/0090_insider_transactions.sql"),
    },
    Migration {
        version: 91,
        name: "management_holdings",
        sql: include_str!("../../migrations/0091_management_holdings.sql"),
    },
    Migration {
        version: 92,
        name: "red_flag_acks",
        sql: include_str!("../../migrations/0092_red_flag_acks.sql"),
    },
    Migration {
        version: 93,
        name: "ownership_ocr_proposals",
        sql: include_str!("../../migrations/0093_ownership_ocr_proposals.sql"),
    },
    Migration {
        version: 94,
        name: "insider_attachment_attempts",
        sql: include_str!("../../migrations/0094_insider_attachment_attempts.sql"),
    },
    Migration {
        version: 95,
        name: "financial_statement_type_from_sector",
        sql: include_str!("../../migrations/0095_financial_statement_type_from_sector.sql"),
    },
    Migration {
        version: 96,
        name: "dismiss_stale_attention_events",
        sql: include_str!("../../migrations/0096_dismiss_stale_attention_events.sql"),
    },
    Migration {
        version: 97,
        name: "backfill_attention_trigger_type",
        sql: include_str!("../../migrations/0097_backfill_attention_trigger_type.sql"),
    },
    Migration {
        version: 98,
        name: "kru_statement_type_specialty_finance",
        sql: include_str!("../../migrations/0098_kru_statement_type_specialty_finance.sql"),
    },
    Migration {
        version: 99,
        name: "repair_cdr_q3_2023_misscaled_facts",
        sql: include_str!("../../migrations/0099_repair_cdr_q3_2023_misscaled_facts.sql"),
    },
    Migration {
        version: 100,
        name: "analyst_recommendations",
        sql: include_str!("../../migrations/0100_analyst_recommendations.sql"),
    },
    Migration {
        version: 101,
        name: "purge_retired_ai_job_kinds",
        sql: include_str!("../../migrations/0101_purge_retired_ai_job_kinds.sql"),
    },
    Migration {
        version: 102,
        name: "clean_cut_ai_artifacts",
        sql: include_str!("../../migrations/0102_clean_cut_ai_artifacts.sql"),
    },
    Migration {
        version: 103,
        name: "fundamentals_extraction_outcomes",
        sql: include_str!("../../migrations/0103_fundamentals_extraction_outcomes.sql"),
    },
    Migration {
        version: 104,
        name: "fundamentals_witness",
        sql: include_str!("../../migrations/0104_fundamentals_witness.sql"),
    },
    Migration {
        version: 105,
        name: "witness_fallback_reason",
        sql: include_str!("../../migrations/0105_witness_fallback_reason.sql"),
    },
    Migration {
        version: 106,
        name: "seed_core_kpi_relevance",
        sql: include_str!("../../migrations/0106_seed_core_kpi_relevance.sql"),
    },
    Migration {
        version: 107,
        name: "repair_misassociation_and_note_ref_facts",
        sql: include_str!("../../migrations/0107_repair_misassociation_and_note_ref_facts.sql"),
    },
    Migration {
        version: 108,
        name: "esef_anchored_refill_misscaled_pdf_facts",
        sql: include_str!("../../migrations/0108_esef_anchored_refill_misscaled_pdf_facts.sql"),
    },
    Migration {
        version: 109,
        name: "document_derived_periods",
        sql: include_str!("../../migrations/0109_document_derived_periods.sql"),
    },
    Migration {
        version: 110,
        name: "aggregator_fundamentals_pages",
        sql: include_str!("../../migrations/0110_aggregator_fundamentals_pages.sql"),
    },
    Migration {
        version: 111,
        name: "parent_attributable_kpi_definitions",
        sql: include_str!("../../migrations/0111_parent_attributable_kpi_definitions.sql"),
    },
    Migration {
        version: 112,
        name: "wdf_issuer_row_kpi_definitions",
        sql: include_str!("../../migrations/0112_wdf_issuer_row_kpi_definitions.sql"),
    },
    Migration {
        version: 113,
        name: "company_signals_agent_classified_by",
        sql: include_str!("../../migrations/0113_company_signals_agent_classified_by.sql"),
    },
    Migration {
        version: 114,
        name: "attention_events_evidence_title",
        sql: include_str!("../../migrations/0114_attention_events_evidence_title.sql"),
    },
    Migration {
        version: 115,
        name: "fx_rates",
        sql: include_str!("../../migrations/0115_fx_rates.sql"),
    },
    Migration {
        version: 116,
        name: "valuation_runs",
        sql: include_str!("../../migrations/0116_valuation_runs.sql"),
    },
    Migration {
        version: 117,
        name: "financial_facts_annotation",
        sql: include_str!("../../migrations/0117_financial_facts_annotation.sql"),
    },
    Migration {
        version: 118,
        name: "attention_events_nullable_company",
        sql: include_str!("../../migrations/0118_attention_events_nullable_company.sql"),
    },
    Migration {
        version: 119,
        name: "extraction_outcomes_recount_and_superseded",
        sql: include_str!("../../migrations/0119_extraction_outcomes_recount_and_superseded.sql"),
    },
    Migration {
        version: 120,
        name: "repair_eps_shares_currency",
        sql: include_str!("../../migrations/0120_repair_eps_shares_currency.sql"),
    },
    Migration {
        version: 121,
        name: "report_documents_detected_container",
        sql: include_str!("../../migrations/0121_report_documents_detected_container.sql"),
    },
    Migration {
        version: 122,
        name: "fact_provenance_witness_corroboration",
        sql: include_str!("../../migrations/0122_fact_provenance_witness_corroboration.sql"),
    },
    Migration {
        version: 123,
        name: "extraction_outcomes_value_divergence",
        sql: include_str!("../../migrations/0123_extraction_outcomes_value_divergence.sql"),
    },
    Migration {
        version: 124,
        name: "heal_missing_core_kpi_relevance",
        sql: include_str!("../../migrations/0124_heal_missing_core_kpi_relevance.sql"),
    },
    Migration {
        version: 125,
        name: "scope_discriminated_kpi_definition_ids",
        sql: include_str!("../../migrations/0125_scope_discriminated_kpi_definition_ids.sql"),
    },
    Migration {
        version: 126,
        name: "seed_statement_pack_kpi_relevance",
        sql: include_str!("../../migrations/0126_seed_statement_pack_kpi_relevance.sql"),
    },
    Migration {
        version: 127,
        name: "split_brokerage_from_specialty_finance",
        sql: include_str!("../../migrations/0127_split_brokerage_from_specialty_finance.sql"),
    },
    Migration {
        version: 128,
        name: "repair_peo_wdf_bank_misclassified_facts",
        sql: include_str!("../../migrations/0128_repair_peo_wdf_bank_misclassified_facts.sql"),
    },
    Migration {
        version: 129,
        name: "kpi_definition_origin",
        sql: include_str!("../../migrations/0129_kpi_definition_origin.sql"),
    },
    Migration {
        version: 130,
        name: "kpi_definition_statement_group",
        sql: include_str!("../../migrations/0130_kpi_definition_statement_group.sql"),
    },
    Migration {
        version: 131,
        name: "wdf_per_share_and_ebitda_kpi_definitions",
        sql: include_str!("../../migrations/0131_wdf_per_share_and_ebitda_kpi_definitions.sql"),
    },
    Migration {
        version: 132,
        name: "prune_banking_core_relevance",
        sql: include_str!("../../migrations/0132_prune_banking_core_relevance.sql"),
    },
    Migration {
        version: 133,
        name: "insider_tx_provenance",
        sql: include_str!("../../migrations/0133_insider_tx_provenance.sql"),
    },
    Migration {
        version: 134,
        name: "repair_positional_esef_tier_mismatch",
        sql: include_str!("../../migrations/0134_repair_positional_esef_tier_mismatch.sql"),
    },
    Migration {
        version: 135,
        name: "retire_html_positional_facts",
        sql: include_str!("../../migrations/0135_retire_html_positional_facts.sql"),
    },
    Migration {
        version: 136,
        name: "purge_retired_search_rows",
        sql: include_str!("../../migrations/0136_purge_retired_search_rows.sql"),
    },
    Migration {
        version: 137,
        name: "kpi_ingest_runs",
        sql: include_str!("../../migrations/0137_kpi_ingest_runs.sql"),
    },
    Migration {
        version: 138,
        name: "kpi_ingest_staging",
        sql: include_str!("../../migrations/0138_kpi_ingest_staging.sql"),
    },
    Migration {
        version: 139,
        name: "kpi_ingest_validation_attempts",
        sql: include_str!("../../migrations/0139_kpi_ingest_validation_attempts.sql"),
    },
    Migration {
        version: 140,
        name: "derived_period_content_hash",
        sql: include_str!("../../migrations/0140_derived_period_content_hash.sql"),
    },
];

pub fn open_database(path: impl AsRef<Path>) -> StorageResult<Connection> {
    let mut connection = Connection::open(path)?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

pub fn open_in_memory_database() -> StorageResult<Connection> {
    let mut connection = Connection::open_in_memory()?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

pub(super) fn database_status(connection: &Connection) -> StorageResult<DatabaseStatus> {
    Ok(DatabaseStatus {
        applied_migrations: count_rows(connection, "schema_migrations")?,
        companies: count_rows(connection, "companies")?,
        source_adapters: count_rows(connection, "source_adapters")?,
        settings: count_rows(connection, "settings")?,
    })
}

#[cfg(test)]
pub(super) fn expected_migration_count() -> i64 {
    MIGRATIONS.len() as i64
}

pub(super) fn migration_count() -> i64 {
    MIGRATIONS.len() as i64
}

/// Number of applied migrations, or 0 when the database has no migration table
/// yet (a brand-new database).
pub(super) fn count_applied_migrations(connection: &Connection) -> StorageResult<i64> {
    let table_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
        [],
        |row| row.get(0),
    )?;

    if !table_exists {
        return Ok(0);
    }

    count_rows(connection, "schema_migrations")
}

pub(super) fn apply_migrations(connection: &mut Connection) -> StorageResult<()> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        ",
    )?;

    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    for migration in MIGRATIONS {
        let already_applied: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [migration.version],
            |row| row.get(0),
        )?;

        if already_applied {
            continue;
        }

        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            (migration.version, migration.name),
        )?;
    }

    transaction.commit()?;
    Ok(())
}

pub(super) fn count_rows(connection: &Connection, table_name: &str) -> StorageResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");

    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(StorageError::from)
}

/// Test-only: apply migrations up to (and including) `max_version`, leaving the
/// database at a historical schema so an upgrade path can be exercised without
/// shipping binary `.sqlite` snapshots (ADR 0048 migration-safety coverage).
#[cfg(test)]
pub(super) fn apply_migrations_up_to(
    connection: &mut Connection,
    max_version: i64,
) -> StorageResult<()> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        ",
    )?;

    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    for migration in MIGRATIONS {
        if migration.version > max_version {
            break;
        }

        let already_applied: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [migration.version],
            |row| row.get(0),
        )?;

        if already_applied {
            continue;
        }

        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            (migration.version, migration.name),
        )?;
    }

    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod migration_invariants {
    use super::MIGRATIONS;

    #[test]
    fn versions_are_contiguous_unique_and_ordered() {
        // Migrations are append-only and immutable once shipped (CLAUDE.md): a
        // reused, out-of-order, or gapped version number is the mistake this
        // guards. They must be exactly 1..=N in declaration order.
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            let expected = index as i64 + 1;
            assert_eq!(
                migration.version, expected,
                "migration #{index} ('{}') has version {} but must be {} (contiguous, ordered, unique)",
                migration.name, migration.version, expected,
            );
        }
    }

    /// Guardrail harvest, epic #229 T3 (#140/#171): **no migration may delete
    /// report documents on URL evidence.**
    ///
    /// `0107_repair_misassociation_and_note_ref_facts.sql` did exactly that —
    /// `company_id = 'company_gpw_cbf' AND url LIKE '%energa%'` — on the belief
    /// that a `Grupy-Energa` filename meant an Energa filing. The bytes say
    /// otherwise: all four deleted files are cyber_Folks' own Q3-2024 filing
    /// (PDF `/Author: cyber_Folks`, body "Raport kwartalny Grupy cyber_Folks_"),
    /// and the attachment host simply reuses one issuer's filename across
    /// unrelated same-day filings. Four legitimate periodic documents were lost.
    ///
    /// Only document **content** can settle an association, and SQL cannot read
    /// bytes — so a migration is structurally the wrong instrument for this
    /// class. 0107 is grandfathered (immutable once applied); every later
    /// migration must fail here instead of repeating it.
    #[test]
    fn no_migration_deletes_report_documents_by_url_pattern() {
        const GRANDFATHERED: i64 = 107;
        let mut offenders = Vec::new();
        for migration in MIGRATIONS {
            if migration.version == GRANDFATHERED {
                continue;
            }
            let sql = migration.sql.to_lowercase();
            for statement in sql.split(';') {
                let Some(start) = statement.find("delete from report_documents") else {
                    continue;
                };
                let predicate = &statement[start..];
                if predicate.contains("url") && predicate.contains("like") {
                    offenders.push(format!(
                        "  {:04}_{} deletes report_documents on a URL pattern",
                        migration.version, migration.name
                    ));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "a report document's owner cannot be decided from its URL — the host reuses one \
             issuer's filename across unrelated filings, and migration 0107 destroyed four \
             legitimate cyber_Folks statements that way. Verify content, not slugs:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<&str> = MIGRATIONS.iter().map(|migration| migration.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "migration names must be unique");
    }

    /// Guardrail harvest (#358, sol F7): the contiguity/count checks above only
    /// ever see what IS registered in `MIGRATIONS`, so a `.sql` file dropped into
    /// `migrations/` and never wired into this const passes every existing gate
    /// silently — it never applies, and no test catches the drift. This walks
    /// the actual `migrations/*.sql` directory and asserts the file inventory and
    /// `MIGRATIONS` agree exactly, in both directions.
    #[test]
    fn every_migration_file_is_registered_and_vice_versa() {
        let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let mut file_versions: Vec<i64> =
            std::fs::read_dir(&migrations_dir)
                .expect("read migrations dir")
                .filter_map(|entry| {
                    let entry = entry.expect("dir entry");
                    let name = entry.file_name();
                    let name = name.to_str().expect("utf8 filename").to_owned();
                    if !name.ends_with(".sql") {
                        return None;
                    }
                    let prefix = &name[..4];
                    Some(prefix.parse::<i64>().unwrap_or_else(|_| {
                        panic!("migration file '{name}' has no numeric prefix")
                    }))
                })
                .collect();
        file_versions.sort_unstable();

        let mut registered_versions: Vec<i64> = MIGRATIONS
            .iter()
            .map(|migration| migration.version)
            .collect();
        registered_versions.sort_unstable();

        assert_eq!(
            file_versions, registered_versions,
            "every migrations/*.sql file must have exactly one MIGRATIONS entry, and vice versa \
             (a file present but unregistered silently never applies)"
        );
    }
}
