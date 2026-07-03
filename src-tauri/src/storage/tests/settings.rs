use super::*;
use std::collections::HashMap;

#[test]
fn get_settings_tolerates_missing_optional_setting_row() {
    // A database that recorded a later migration before its seed row existed
    // (e.g. an intermediate dev build) must still load settings — otherwise the
    // startup get_settings call aborts setup and the app closes immediately.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .checkout()
        .expect("connection")
        .execute(
            "DELETE FROM settings WHERE key = 'espi_ai_fallback_enabled'",
            [],
        )
        .expect("delete row");

    let settings = state
        .get_settings()
        .expect("settings should load even without the optional setting row");
    assert!(!settings.espi_ai_fallback_enabled);
}

#[test]
fn persists_and_dedupes_pinned_company_ids() {
    // Sidebar-spine pinned companies (ADR 0054): a JSON list in the settings KV
    // table with a tolerant empty default, overwrite-on-update, dedupe + blank
    // filtering, preserving the user's pin order.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    assert!(
        state
            .get_settings()
            .expect("settings")
            .pinned_company_ids
            .is_empty(),
        "pinned companies default to an empty list without a seed row",
    );

    let settings = state
        .update_settings(SettingsUpdate {
            pinned_company_ids: Some(vec![
                "company_a".to_owned(),
                "company_b".to_owned(),
                "company_a".to_owned(), // duplicate — dropped
                "  ".to_owned(),        // blank — dropped
            ]),
            ..Default::default()
        })
        .expect("settings should update");
    assert_eq!(
        settings.pinned_company_ids,
        vec!["company_a".to_owned(), "company_b".to_owned()],
    );

    // The list round-trips through a fresh read (durable in SQLite).
    assert_eq!(
        state.get_settings().expect("settings").pinned_company_ids,
        vec!["company_a".to_owned(), "company_b".to_owned()],
    );

    // Replacing with an empty list clears the pins.
    let cleared = state
        .update_settings(SettingsUpdate {
            pinned_company_ids: Some(vec![]),
            ..Default::default()
        })
        .expect("settings should update");
    assert!(cleared.pinned_company_ids.is_empty());
}

#[test]
fn reads_default_settings_from_sqlite() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let settings = state.get_settings().expect("settings should load");

    assert_eq!(settings.theme, "dark");
    assert_eq!(settings.locale, "en");
    assert_eq!(settings.accent_palette, "night-neon");
    assert!(!settings.developer_mode);
    assert_eq!(settings.poll_interval_seconds, 900);
    assert_eq!(settings.settings_source, "sqlite");
    assert_eq!(settings.settings_import_export_format, "yaml");
    assert_eq!(settings.yaml_import_export_status, "accepted_deferred");
    assert_eq!(
        settings.ai_providers.youtube_transcription_provider,
        "provider_gemini"
    );
    assert_eq!(
        settings.ai_providers.youtube_transcription_model,
        "gemini-3.5-flash"
    );
    assert_eq!(
        settings.ai_providers.youtube_transcription_timeout_seconds,
        300
    );
    assert_eq!(
        settings.ai_providers.general_analysis_provider.as_deref(),
        Some("provider_gemini")
    );
    assert_eq!(
        settings.ai_providers.general_analysis_model,
        "gemini-3.5-flash"
    );
    assert_eq!(settings.ai_providers.general_analysis_timeout_seconds, 90);
    assert_eq!(settings.ai_analysis_mode, "source_grounded");
    assert_eq!(settings.logs.level, "info");
    assert_eq!(settings.logs.max_files, 5);
    assert_eq!(settings.logs.max_file_bytes, 5_242_880);
    assert!(settings.shortcut_bindings.is_empty());
    assert!(settings.capability_providers.is_empty());
}

#[test]
fn updates_developer_mode_through_dedicated_storage_api() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let enabled = state
        .set_developer_mode_enabled(true)
        .expect("developer mode should enable");
    assert!(enabled.developer_mode);

    let persisted = state.get_settings().expect("settings should persist");
    assert!(persisted.developer_mode);

    let disabled = state
        .set_developer_mode_enabled(false)
        .expect("developer mode should disable");
    assert!(!disabled.developer_mode);
}

#[test]
fn migration_updates_old_gemini_default_model_to_validated_default() {
    let mut connection = open_in_memory_database().expect("database should initialize");
    connection
            .execute(
                "UPDATE settings SET value = 'gemini-2.5-flash-lite' WHERE key = 'youtube_transcription_model'",
                [],
            )
            .expect("old model value should be set");
    connection
        .execute("DELETE FROM schema_migrations WHERE version = 21", [])
        .expect("migration marker should be removable");

    apply_migrations(&mut connection).expect("migration should apply");
    let state = AppState::new(connection);
    let settings = state.get_settings().expect("settings should load");

    assert_eq!(
        settings.ai_providers.youtube_transcription_model,
        "gemini-2.5-flash"
    );
}

#[test]
fn updates_settings_through_storage_api() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let settings = state
        .update_settings(SettingsUpdate {
            theme: Some("light".to_owned()),
            accent_palette: Some("midnight-horizon".to_owned()),
            locale: Some("pl".to_owned()),
            poll_interval_seconds: Some(1800),
            youtube_transcription_provider: None,
            youtube_transcription_model: None,
            youtube_transcription_timeout_seconds: Some(600),
            general_analysis_provider: Some("provider_gemini".to_owned()),
            general_analysis_model: Some("gemini-3.5-flash".to_owned()),
            general_analysis_timeout_seconds: Some(180),
            openai_compatible_base_url: None,
            ai_analysis_mode: None,
            espi_ai_fallback_enabled: None,
            log_level: Some("debug".to_owned()),
            log_max_files: Some(8),
            log_max_file_bytes: Some(10_485_760),
            shortcut_bindings: None,
            capability_providers: None,
            db_max_connections: None,
            db_busy_timeout_ms: None,
            db_acquire_timeout_ms: None,
            sources_workers: None,
            autopilot_workers: None,
            ai_workers: None,
            ai_provider_concurrency: None,
            pinned_company_ids: None,
        })
        .expect("settings should update");

    assert_eq!(settings.theme, "light");
    assert_eq!(settings.accent_palette, "midnight-horizon");
    assert_eq!(settings.locale, "pl");
    assert_eq!(settings.poll_interval_seconds, 1800);
    assert_eq!(
        settings.ai_providers.youtube_transcription_timeout_seconds,
        600
    );
    assert_eq!(
        settings.ai_providers.general_analysis_provider.as_deref(),
        Some("provider_gemini")
    );
    assert_eq!(
        settings.ai_providers.general_analysis_model,
        "gemini-3.5-flash"
    );
    assert_eq!(settings.ai_providers.general_analysis_timeout_seconds, 180);
    assert_eq!(settings.logs.level, "debug");
    assert_eq!(settings.logs.max_files, 8);
    assert_eq!(settings.logs.max_file_bytes, 10_485_760);

    let persisted = state.get_settings().expect("settings should persist");

    assert_eq!(persisted.theme, "light");
    assert_eq!(persisted.accent_palette, "midnight-horizon");
    assert_eq!(persisted.locale, "pl");
    assert_eq!(persisted.poll_interval_seconds, 1800);
    assert_eq!(
        persisted.ai_providers.youtube_transcription_timeout_seconds,
        600
    );
    assert_eq!(
        persisted.ai_providers.general_analysis_provider.as_deref(),
        Some("provider_gemini")
    );
    assert_eq!(
        persisted.ai_providers.general_analysis_model,
        "gemini-3.5-flash"
    );
    assert_eq!(persisted.ai_providers.general_analysis_timeout_seconds, 180);
    assert_eq!(persisted.logs.level, "debug");
    assert_eq!(persisted.logs.max_files, 8);
    assert_eq!(persisted.logs.max_file_bytes, 10_485_760);
}

#[test]
fn updates_shortcut_bindings_through_storage_api() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let mut shortcut_bindings = HashMap::new();

    shortcut_bindings.insert(
        "app.openInbox".to_owned(),
        ShortcutBindingSetting {
            key: "I".to_owned(),
            alt_key: Some(true),
            ctrl_key: None,
            meta_key: None,
            shift_key: Some(true),
            disabled: None,
        },
    );
    shortcut_bindings.insert(
        "app.refreshSources".to_owned(),
        ShortcutBindingSetting {
            key: "R".to_owned(),
            alt_key: None,
            ctrl_key: None,
            meta_key: None,
            shift_key: None,
            disabled: Some(true),
        },
    );

    let settings = state
        .update_settings(SettingsUpdate {
            theme: None,
            accent_palette: None,
            locale: None,
            poll_interval_seconds: None,
            youtube_transcription_provider: None,
            youtube_transcription_model: None,
            youtube_transcription_timeout_seconds: None,
            general_analysis_provider: None,
            general_analysis_model: None,
            general_analysis_timeout_seconds: None,
            openai_compatible_base_url: None,
            ai_analysis_mode: None,
            espi_ai_fallback_enabled: None,
            log_level: None,
            log_max_files: None,
            log_max_file_bytes: None,
            shortcut_bindings: Some(shortcut_bindings),
            capability_providers: None,
            db_max_connections: None,
            db_busy_timeout_ms: None,
            db_acquire_timeout_ms: None,
            sources_workers: None,
            autopilot_workers: None,
            ai_workers: None,
            ai_provider_concurrency: None,
            pinned_company_ids: None,
        })
        .expect("settings should update");

    assert_eq!(settings.shortcut_bindings.len(), 2);
    assert_eq!(
        settings
            .shortcut_bindings
            .get("app.openInbox")
            .expect("shortcut binding should persist")
            .key,
        "I"
    );
    assert_eq!(
        settings
            .shortcut_bindings
            .get("app.refreshSources")
            .expect("shortcut binding should persist")
            .disabled,
        Some(true)
    );
}

#[test]
fn rejects_invalid_poll_interval_setting() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.update_settings(SettingsUpdate {
        theme: None,
        accent_palette: None,
        locale: None,
        poll_interval_seconds: Some(42),
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        general_analysis_provider: None,
        general_analysis_model: None,
        general_analysis_timeout_seconds: None,
        openai_compatible_base_url: None,
        ai_analysis_mode: None,
        espi_ai_fallback_enabled: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        capability_providers: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        ai_workers: None,
        ai_provider_concurrency: None,
        pinned_company_ids: None,
    });

    assert!(result.is_err());
}

#[test]
fn rejects_invalid_theme_setting() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.update_settings(SettingsUpdate {
        theme: Some("sepia".to_owned()),
        accent_palette: None,
        locale: None,
        poll_interval_seconds: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        general_analysis_provider: None,
        general_analysis_model: None,
        general_analysis_timeout_seconds: None,
        openai_compatible_base_url: None,
        ai_analysis_mode: None,
        espi_ai_fallback_enabled: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        capability_providers: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        ai_workers: None,
        ai_provider_concurrency: None,
        pinned_company_ids: None,
    });

    assert!(result.is_err());
}

#[test]
fn rejects_invalid_accent_palette_setting() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.update_settings(SettingsUpdate {
        accent_palette: Some("terminal-green".to_owned()),
        ..SettingsUpdate::default()
    });

    assert!(result.is_err());
}

#[test]
fn rejects_invalid_locale_setting() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.update_settings(SettingsUpdate {
        theme: None,
        accent_palette: None,
        locale: Some("de".to_owned()),
        poll_interval_seconds: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        general_analysis_provider: None,
        general_analysis_model: None,
        general_analysis_timeout_seconds: None,
        openai_compatible_base_url: None,
        ai_analysis_mode: None,
        espi_ai_fallback_enabled: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        capability_providers: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        ai_workers: None,
        ai_provider_concurrency: None,
        pinned_company_ids: None,
    });

    assert!(result.is_err());
}

#[test]
fn rejects_invalid_general_analysis_settings() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let invalid_provider = state.update_settings(SettingsUpdate {
        theme: None,
        accent_palette: None,
        locale: None,
        poll_interval_seconds: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        general_analysis_provider: Some("provider_unknown".to_owned()),
        general_analysis_model: None,
        general_analysis_timeout_seconds: None,
        openai_compatible_base_url: None,
        ai_analysis_mode: None,
        espi_ai_fallback_enabled: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        capability_providers: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        ai_workers: None,
        ai_provider_concurrency: None,
        pinned_company_ids: None,
    });

    assert!(invalid_provider.is_err());

    let invalid_model = state.update_settings(SettingsUpdate {
        theme: None,
        accent_palette: None,
        locale: None,
        poll_interval_seconds: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        general_analysis_provider: None,
        general_analysis_model: Some("gemini-unknown".to_owned()),
        general_analysis_timeout_seconds: None,
        openai_compatible_base_url: None,
        ai_analysis_mode: None,
        espi_ai_fallback_enabled: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        capability_providers: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        ai_workers: None,
        ai_provider_concurrency: None,
        pinned_company_ids: None,
    });

    assert!(invalid_model.is_err());

    let invalid_timeout = state.update_settings(SettingsUpdate {
        theme: None,
        accent_palette: None,
        locale: None,
        poll_interval_seconds: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        general_analysis_provider: None,
        general_analysis_model: None,
        general_analysis_timeout_seconds: Some(12),
        openai_compatible_base_url: None,
        ai_analysis_mode: None,
        espi_ai_fallback_enabled: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        capability_providers: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        ai_workers: None,
        ai_provider_concurrency: None,
        pinned_company_ids: None,
    });

    assert!(invalid_timeout.is_err());
}

#[test]
fn rejects_invalid_log_settings() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let invalid_level = state.update_settings(SettingsUpdate {
        log_level: Some("verbose".to_owned()),
        ..SettingsUpdate::default()
    });
    assert!(invalid_level.is_err());

    let invalid_file_count = state.update_settings(SettingsUpdate {
        log_max_files: Some(0),
        ..SettingsUpdate::default()
    });
    assert!(invalid_file_count.is_err());

    let invalid_file_size = state.update_settings(SettingsUpdate {
        log_max_file_bytes: Some(512),
        ..SettingsUpdate::default()
    });
    assert!(invalid_file_size.is_err());
}

#[test]
fn openai_compatible_base_url_defaults_to_empty_and_roundtrips() {
    // ADR 0060: the base URL for the generic OpenAI-compatible provider has no
    // seed row, so it must tolerate a missing row (default "") and persist
    // through an update like `pinned_company_ids`.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    assert_eq!(
        state
            .get_settings()
            .expect("settings")
            .ai_providers
            .openai_compatible_base_url,
        "",
        "base url defaults to empty without a seed row",
    );

    let updated = state
        .update_settings(SettingsUpdate {
            openai_compatible_base_url: Some("https://example.test/v1".to_owned()),
            ..SettingsUpdate::default()
        })
        .expect("settings should update");
    assert_eq!(
        updated.ai_providers.openai_compatible_base_url,
        "https://example.test/v1"
    );

    let persisted = state.get_settings().expect("settings should persist");
    assert_eq!(
        persisted.ai_providers.openai_compatible_base_url,
        "https://example.test/v1"
    );
}

#[test]
fn rejects_non_http_openai_compatible_base_url() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.update_settings(SettingsUpdate {
        openai_compatible_base_url: Some("ftp://example.test".to_owned()),
        ..SettingsUpdate::default()
    });

    assert!(result.is_err());
}

#[test]
fn accepts_freeform_general_analysis_model_for_openai_compatible_provider() {
    // The compatible provider has no curated model list (ADR 0060) — any
    // non-empty model id is accepted, unlike the curated-provider case
    // covered by `rejects_invalid_general_analysis_settings`.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let settings = state
        .update_settings(SettingsUpdate {
            general_analysis_provider: Some(
                crate::providers::analysis::registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID
                    .to_owned(),
            ),
            general_analysis_model: Some("my-custom-model".to_owned()),
            ..SettingsUpdate::default()
        })
        .expect("freeform model should be accepted for the compatible provider");
    assert_eq!(
        settings.ai_providers.general_analysis_model,
        "my-custom-model"
    );

    // A later model-only update reads the *currently stored* provider (still
    // the compatible one), so it keeps accepting a freeform value.
    let updated = state
        .update_settings(SettingsUpdate {
            general_analysis_model: Some("another-custom-model".to_owned()),
            ..SettingsUpdate::default()
        })
        .expect("freeform model should still be accepted");
    assert_eq!(
        updated.ai_providers.general_analysis_model,
        "another-custom-model"
    );

    let empty_model_rejected = state.update_settings(SettingsUpdate {
        general_analysis_model: Some("   ".to_owned()),
        ..SettingsUpdate::default()
    });
    assert!(empty_model_rejected.is_err());
}

#[test]
fn capability_providers_defaults_to_empty_map_when_row_missing() {
    // No seed row for `capability_providers` (introduced after the initial
    // migration) — a database that predates this key must still load settings
    // with an empty map, like `pinned_company_ids`.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    assert!(
        state
            .get_settings()
            .expect("settings")
            .capability_providers
            .is_empty(),
        "capability providers default to an empty map without a seed row",
    );
}

#[test]
fn capability_providers_roundtrips_via_update() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // `feed_analysis` (a text-kind capability) so the mixed Gemini +
    // OpenAI-compatible pool exercises pure round-trip persistence without
    // tripping the document-support validation gate (Gemini is Native,
    // OpenAI-compatible is TextOnly — both fine for a text-kind capability).
    let mut capability_providers = HashMap::new();
    capability_providers.insert(
        "feed_analysis".to_owned(),
        vec![
            CapabilityProviderEntry {
                provider: crate::providers::analysis::registry::GEMINI_ANALYSIS_PROVIDER_ID
                    .to_owned(),
                model: "gemini-3.5-flash".to_owned(),
            },
            CapabilityProviderEntry {
                provider:
                    crate::providers::analysis::registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID
                        .to_owned(),
                model: "freeform-model".to_owned(),
            },
        ],
    );

    let settings = state
        .update_settings(SettingsUpdate {
            capability_providers: Some(capability_providers),
            ..SettingsUpdate::default()
        })
        .expect("settings should update");

    let entries = settings
        .capability_providers
        .get("feed_analysis")
        .expect("capability entry should persist");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].provider, "provider_gemini");
    assert_eq!(entries[0].model, "gemini-3.5-flash");
    assert_eq!(entries[1].provider, "provider_openai_compatible");
    assert_eq!(entries[1].model, "freeform-model");

    // The order + list round-trip through a fresh read (durable in SQLite).
    let persisted = state.get_settings().expect("settings").capability_providers;
    let persisted_entries = persisted.get("feed_analysis").expect("persisted entry");
    assert_eq!(persisted_entries[0].provider, "provider_gemini");
    assert_eq!(persisted_entries[1].provider, "provider_openai_compatible");
}

#[test]
fn rejects_unknown_capability_key() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut capability_providers = HashMap::new();
    capability_providers.insert("not_a_capability".to_owned(), vec![]);

    let result = state.update_settings(SettingsUpdate {
        capability_providers: Some(capability_providers),
        ..SettingsUpdate::default()
    });

    assert!(result.is_err());
}

#[test]
fn rejects_unknown_capability_provider() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut capability_providers = HashMap::new();
    capability_providers.insert(
        "feed_analysis".to_owned(),
        vec![CapabilityProviderEntry {
            provider: "provider_unknown".to_owned(),
            model: "some-model".to_owned(),
        }],
    );

    let result = state.update_settings(SettingsUpdate {
        capability_providers: Some(capability_providers),
        ..SettingsUpdate::default()
    });

    assert!(result.is_err());
}

#[test]
fn rejects_openai_compatible_provider_for_document_capability() {
    // `kpi_extraction` is a document-kind capability (ADR 0061 decision 5);
    // OpenAI-compatible is TextOnly, so routing it here would hard-fail
    // every extraction and poison pool failover (Radicle 6ea2a8a) — reject it
    // at settings-write time instead of at job-run time.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut capability_providers = HashMap::new();
    capability_providers.insert(
        "kpi_extraction".to_owned(),
        vec![CapabilityProviderEntry {
            provider: crate::providers::analysis::registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID
                .to_owned(),
            model: "custom-model".to_owned(),
        }],
    );

    let result = state.update_settings(SettingsUpdate {
        capability_providers: Some(capability_providers),
        ..SettingsUpdate::default()
    });

    let error =
        result.expect_err("a text-only provider must be rejected for a document capability");
    let message = error.to_string();
    assert!(
        message.contains("kpi_extraction"),
        "error must name the capability: {message}"
    );
    assert!(
        message.contains("provider_openai_compatible"),
        "error must name the provider: {message}"
    );
    assert!(
        message.contains("cannot accept document input"),
        "error must explain why: {message}"
    );
}

#[test]
fn rejects_openai_provider_for_claim_extraction_capability() {
    // Same rule, the other document-kind capability and the other TextOnly
    // provider (OpenAI, not OpenAI-compatible).
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut capability_providers = HashMap::new();
    capability_providers.insert(
        "claim_extraction".to_owned(),
        vec![CapabilityProviderEntry {
            provider: crate::providers::analysis::registry::OPENAI_ANALYSIS_PROVIDER_ID.to_owned(),
            model: "gpt-5.5".to_owned(),
        }],
    );

    let result = state.update_settings(SettingsUpdate {
        capability_providers: Some(capability_providers),
        ..SettingsUpdate::default()
    });

    assert!(result.is_err());
}

#[test]
fn rejects_test_sample_provider_in_capability_providers() {
    // `selectable_analysis_provider_ids` deliberately excludes the test
    // sample provider (it is not a user-selectable analysis provider), so
    // the existing unknown-provider check must already reject it here —
    // pin that behavior so a future change to the selectable-ids list can't
    // silently let the test-sample provider back into a real settings map.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut capability_providers = HashMap::new();
    capability_providers.insert(
        "feed_analysis".to_owned(),
        vec![CapabilityProviderEntry {
            provider: crate::providers::analysis::TEST_SAMPLE_ANALYSIS_PROVIDER_ID.to_owned(),
            model: crate::providers::analysis::TEST_SAMPLE_ANALYSIS_MODEL.to_owned(),
        }],
    );

    let result = state.update_settings(SettingsUpdate {
        capability_providers: Some(capability_providers),
        ..SettingsUpdate::default()
    });

    assert!(
        result.is_err(),
        "the test-sample provider must never be accepted into capability_providers"
    );
}

#[test]
fn rejects_uncurated_model_for_curated_provider() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut capability_providers = HashMap::new();
    capability_providers.insert(
        "feed_analysis".to_owned(),
        vec![CapabilityProviderEntry {
            provider: crate::providers::analysis::registry::GEMINI_ANALYSIS_PROVIDER_ID.to_owned(),
            model: "gemini-unknown".to_owned(),
        }],
    );

    let result = state.update_settings(SettingsUpdate {
        capability_providers: Some(capability_providers),
        ..SettingsUpdate::default()
    });

    assert!(result.is_err());
}

#[test]
fn rejects_empty_capability_model() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut capability_providers = HashMap::new();
    capability_providers.insert(
        "feed_analysis".to_owned(),
        vec![CapabilityProviderEntry {
            provider: crate::providers::analysis::registry::GEMINI_ANALYSIS_PROVIDER_ID.to_owned(),
            model: "   ".to_owned(),
        }],
    );

    let result = state.update_settings(SettingsUpdate {
        capability_providers: Some(capability_providers),
        ..SettingsUpdate::default()
    });

    assert!(result.is_err());
}

#[test]
fn accepts_freeform_model_for_openai_compatible_capability_entry() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // `feed_analysis` (text-kind): OpenAI-compatible is TextOnly, which is
    // only valid for a text-kind capability — see
    // `rejects_openai_compatible_provider_for_document_capability` for the
    // document-kind rejection this same combination now triggers.
    let mut capability_providers = HashMap::new();
    capability_providers.insert(
        "feed_analysis".to_owned(),
        vec![CapabilityProviderEntry {
            provider: crate::providers::analysis::registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID
                .to_owned(),
            model: "my-local-model".to_owned(),
        }],
    );

    let settings = state
        .update_settings(SettingsUpdate {
            capability_providers: Some(capability_providers),
            ..SettingsUpdate::default()
        })
        .expect("freeform model should be accepted for the compatible provider");
    assert_eq!(
        settings
            .capability_providers
            .get("feed_analysis")
            .expect("entry")[0]
            .model,
        "my-local-model"
    );
}

#[test]
fn capability_providers_update_replaces_whole_map() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut first = HashMap::new();
    first.insert(
        "kpi_extraction".to_owned(),
        vec![CapabilityProviderEntry {
            provider: crate::providers::analysis::registry::GEMINI_ANALYSIS_PROVIDER_ID.to_owned(),
            model: "gemini-3.5-flash".to_owned(),
        }],
    );
    state
        .update_settings(SettingsUpdate {
            capability_providers: Some(first),
            ..SettingsUpdate::default()
        })
        .expect("first update should apply");

    let mut second = HashMap::new();
    second.insert(
        "feed_analysis".to_owned(),
        vec![CapabilityProviderEntry {
            provider: crate::providers::analysis::registry::ANTHROPIC_ANALYSIS_PROVIDER_ID
                .to_owned(),
            model: "claude-sonnet-4-6".to_owned(),
        }],
    );
    let settings = state
        .update_settings(SettingsUpdate {
            capability_providers: Some(second),
            ..SettingsUpdate::default()
        })
        .expect("second update should apply");

    assert!(
        !settings.capability_providers.contains_key("kpi_extraction"),
        "the second update replaces the whole map rather than merging"
    );
    assert!(settings.capability_providers.contains_key("feed_analysis"));
}

#[test]
fn accepts_empty_capability_list_as_explicit_fallback() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let mut capability_providers = HashMap::new();
    capability_providers.insert("kpi_extraction".to_owned(), vec![]);

    let settings = state
        .update_settings(SettingsUpdate {
            capability_providers: Some(capability_providers),
            ..SettingsUpdate::default()
        })
        .expect("empty entry list is a valid explicit fallback");
    assert!(settings.capability_providers.contains_key("kpi_extraction"));
    assert!(settings
        .capability_providers
        .get("kpi_extraction")
        .expect("key should persist even with an empty list")
        .is_empty());
}

#[test]
fn update_settings_is_atomic_on_validation_failure() {
    // `update_settings` processes fields sequentially (theme first,
    // capability_providers near the end). Before the fix, a later validation
    // failure returned Err while the earlier theme write stayed committed on
    // the autocommit connection — a torn update. Wrapping the whole body in a
    // transaction must roll back the theme write too (Radicle ef7ad69).
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let before = state.get_settings().expect("settings should load");

    let mut capability_providers = HashMap::new();
    capability_providers.insert("not_a_capability".to_owned(), vec![]);

    let result = state.update_settings(SettingsUpdate {
        theme: Some("light".to_owned()),
        capability_providers: Some(capability_providers),
        ..SettingsUpdate::default()
    });

    assert!(
        result.is_err(),
        "the unknown capability key should fail validation"
    );

    let after = state.get_settings().expect("settings should load");
    assert_eq!(
        after.theme, before.theme,
        "the theme write must roll back when a later field fails validation \
         in the same update — update_settings is atomic"
    );
}
