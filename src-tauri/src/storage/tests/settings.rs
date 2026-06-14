use super::*;
use std::collections::HashMap;

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
            ai_analysis_mode: None,
            log_level: Some("debug".to_owned()),
            log_max_files: Some(8),
            log_max_file_bytes: Some(10_485_760),
            shortcut_bindings: None,
            db_max_connections: None,
            db_busy_timeout_ms: None,
            db_acquire_timeout_ms: None,
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
            ai_analysis_mode: None,
            log_level: None,
            log_max_files: None,
            log_max_file_bytes: None,
            shortcut_bindings: Some(shortcut_bindings),
            db_max_connections: None,
            db_busy_timeout_ms: None,
            db_acquire_timeout_ms: None,
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
        ai_analysis_mode: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
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
        ai_analysis_mode: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
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
        ai_analysis_mode: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
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
        ai_analysis_mode: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
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
        ai_analysis_mode: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
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
        ai_analysis_mode: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
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
