use super::*;

#[test]
fn reads_default_settings_from_sqlite() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let settings = state.get_settings().expect("settings should load");

    assert_eq!(settings.theme, "dark");
    assert_eq!(settings.accent_palette, "night-neon");
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
        "gemini-2.5-flash"
    );
    assert_eq!(
        settings.ai_providers.youtube_transcription_timeout_seconds,
        300
    );
    assert!(settings.ai_providers.general_analysis_provider.is_none());
    assert_eq!(settings.ai_analysis_mode, "source_grounded");
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
            poll_interval_seconds: Some(1800),
            youtube_transcription_provider: None,
            youtube_transcription_model: None,
            youtube_transcription_timeout_seconds: Some(600),
            general_analysis_provider: None,
            ai_analysis_mode: None,
        })
        .expect("settings should update");

    assert_eq!(settings.theme, "light");
    assert_eq!(settings.poll_interval_seconds, 1800);
    assert_eq!(
        settings.ai_providers.youtube_transcription_timeout_seconds,
        600
    );

    let persisted = state.get_settings().expect("settings should persist");

    assert_eq!(persisted.theme, "light");
    assert_eq!(persisted.poll_interval_seconds, 1800);
    assert_eq!(
        persisted.ai_providers.youtube_transcription_timeout_seconds,
        600
    );
}

#[test]
fn rejects_invalid_poll_interval_setting() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.update_settings(SettingsUpdate {
        theme: None,
        poll_interval_seconds: Some(42),
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        general_analysis_provider: None,
        ai_analysis_mode: None,
    });

    assert!(result.is_err());
}

#[test]
fn rejects_invalid_theme_setting() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state.update_settings(SettingsUpdate {
        theme: Some("sepia".to_owned()),
        poll_interval_seconds: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        general_analysis_provider: None,
        ai_analysis_mode: None,
    });

    assert!(result.is_err());
}
