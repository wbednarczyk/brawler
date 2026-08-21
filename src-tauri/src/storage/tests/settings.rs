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
        .execute("DELETE FROM settings WHERE key = 'backfill_years'", [])
        .expect("delete row");

    let settings = state
        .get_settings()
        .expect("settings should load even without the optional setting row");
    assert_eq!(settings.backfill_years, 3);
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
    assert_eq!(settings.backfill_years, 3);
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
            backfill_years: None,
            youtube_transcription_provider: None,
            youtube_transcription_model: None,
            youtube_transcription_timeout_seconds: Some(600),
            log_level: Some("debug".to_owned()),
            log_max_files: Some(8),
            log_max_file_bytes: Some(10_485_760),
            shortcut_bindings: None,
            db_max_connections: None,
            db_busy_timeout_ms: None,
            db_acquire_timeout_ms: None,
            sources_workers: None,
            autopilot_workers: None,
            pinned_company_ids: None,
            today_reviewed_days: None,
            mcp_enabled: None,
            mcp_port: None,
            mcp_writes_enabled: None,
            kpi_acquisition_enabled: None,
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
            backfill_years: None,
            youtube_transcription_provider: None,
            youtube_transcription_model: None,
            youtube_transcription_timeout_seconds: None,
            log_level: None,
            log_max_files: None,
            log_max_file_bytes: None,
            shortcut_bindings: Some(shortcut_bindings),
            db_max_connections: None,
            db_busy_timeout_ms: None,
            db_acquire_timeout_ms: None,
            sources_workers: None,
            autopilot_workers: None,
            pinned_company_ids: None,
            today_reviewed_days: None,
            mcp_enabled: None,
            mcp_port: None,
            mcp_writes_enabled: None,
            kpi_acquisition_enabled: None,
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
        backfill_years: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        pinned_company_ids: None,
        today_reviewed_days: None,
        mcp_enabled: None,
        mcp_port: None,
        mcp_writes_enabled: None,
        kpi_acquisition_enabled: None,
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
        backfill_years: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        pinned_company_ids: None,
        today_reviewed_days: None,
        mcp_enabled: None,
        mcp_port: None,
        mcp_writes_enabled: None,
        kpi_acquisition_enabled: None,
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
        backfill_years: None,
        youtube_transcription_provider: None,
        youtube_transcription_model: None,
        youtube_transcription_timeout_seconds: None,
        log_level: None,
        log_max_files: None,
        log_max_file_bytes: None,
        shortcut_bindings: None,
        db_max_connections: None,
        db_busy_timeout_ms: None,
        db_acquire_timeout_ms: None,
        sources_workers: None,
        autopilot_workers: None,
        pinned_company_ids: None,
        today_reviewed_days: None,
        mcp_enabled: None,
        mcp_port: None,
        mcp_writes_enabled: None,
        kpi_acquisition_enabled: None,
    });

    assert!(result.is_err());
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
fn update_settings_is_atomic_on_validation_failure() {
    // `update_settings` processes fields sequentially (theme first,
    // capability_providers near the end). Before the fix, a later validation
    // failure returned Err while the earlier theme write stayed committed on
    // the autocommit connection — a torn update. Wrapping the whole body in a
    // transaction must roll back the theme write too (Radicle ef7ad69).
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let before = state.get_settings().expect("settings should load");

    let result = state.update_settings(SettingsUpdate {
        theme: Some("light".to_owned()),
        // An invalid log level fails validation AFTER the theme write in the same
        // update — the whole update must roll back.
        log_level: Some("not_a_level".to_owned()),
        ..SettingsUpdate::default()
    });

    assert!(
        result.is_err(),
        "the invalid log level should fail validation"
    );

    let after = state.get_settings().expect("settings should load");
    assert_eq!(
        after.theme, before.theme,
        "the theme write must roll back when a later field fails validation \
         in the same update — update_settings is atomic"
    );
}

#[test]
fn backfill_years_defaults_tolerantly_when_row_absent() {
    // No seed-row migration (ADR 0077 §3): a database that predates the key must
    // still load settings, reading the default 3 rather than failing.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .checkout()
        .expect("connection")
        .execute("DELETE FROM settings WHERE key = 'backfill_years'", [])
        .expect("delete row");

    let settings = state
        .get_settings()
        .expect("settings should load without the backfill_years row");
    assert_eq!(settings.backfill_years, 3);
}

#[test]
fn backfill_years_clamps_out_of_range_stored_value_on_read() {
    // A hand-edited / absurd stored value is clamped to the supported range on
    // read so it can never drive an unbounded fetch.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    // Inline the checkout so the pooled connection is released before
    // `get_settings` re-checks-out (the in-memory pool is size 1).
    state
        .checkout()
        .expect("connection")
        .execute(
            "INSERT INTO settings (key, value, value_type) VALUES ('backfill_years', '999', 'integer')
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [],
        )
        .expect("seed out-of-range value");

    assert_eq!(state.get_settings().expect("settings").backfill_years, 10);
}

#[test]
fn backfill_years_write_clamps_to_supported_range() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let low = state
        .update_settings(SettingsUpdate {
            backfill_years: Some(0),
            ..Default::default()
        })
        .expect("settings should update");
    assert_eq!(low.backfill_years, 1, "0 clamps up to the minimum");

    let high = state
        .update_settings(SettingsUpdate {
            backfill_years: Some(50),
            ..Default::default()
        })
        .expect("settings should update");
    assert_eq!(high.backfill_years, 10, "50 clamps down to the maximum");

    // The clamped value round-trips through a fresh read (durable in SQLite).
    assert_eq!(state.get_settings().expect("settings").backfill_years, 10);

    let ok = state
        .update_settings(SettingsUpdate {
            backfill_years: Some(5),
            ..Default::default()
        })
        .expect("settings should update");
    assert_eq!(ok.backfill_years, 5, "an in-range value is stored verbatim");
}

#[test]
fn mcp_settings_default_tolerantly_when_rows_absent() {
    // No seed-row migration (ADR 0078 decision 4, established keyless pattern):
    // a database that predates the `mcp_enabled` / `mcp_port` keys must still
    // load settings, reading the defaults (off, port 8317) rather than failing.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let settings = state
        .get_settings()
        .expect("settings should load without the mcp rows");
    assert!(!settings.mcp.enabled, "MCP server defaults to off");
    assert_eq!(settings.mcp.port, 8317, "MCP port defaults to 8317");
    assert!(
        !settings.mcp.writes_enabled,
        "MCP write tier defaults to off (ADR 0088 M3)"
    );
}

#[test]
fn mcp_writes_enabled_upsert_and_round_trip() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // Absent row reads the safe default.
    assert!(!state.get_settings().expect("settings").mcp.writes_enabled);

    let enabled = state
        .update_settings(SettingsUpdate {
            mcp_writes_enabled: Some(true),
            ..Default::default()
        })
        .expect("settings should update");
    assert!(enabled.mcp.writes_enabled);
    // Durable across a fresh read.
    assert!(state.get_settings().expect("settings").mcp.writes_enabled);

    // Toggling back off upserts over the existing row (never dropped).
    let disabled = state
        .update_settings(SettingsUpdate {
            mcp_writes_enabled: Some(false),
            ..Default::default()
        })
        .expect("settings should update");
    assert!(!disabled.mcp.writes_enabled);
}

#[test]
fn kpi_acquisition_enabled_upsert_and_round_trip() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // Absent row reads the safe default (scope off — ADR 0099 dec. 2).
    assert!(
        !state
            .get_settings()
            .expect("settings")
            .mcp
            .kpi_acquisition_enabled
    );
    assert!(!state.settings().kpi_acquisition_gate().expect("gate reads"));

    let enabled = state
        .update_settings(SettingsUpdate {
            kpi_acquisition_enabled: Some(true),
            ..Default::default()
        })
        .expect("settings should update");
    assert!(enabled.mcp.kpi_acquisition_enabled);
    // The dedicated single-row gate read agrees with the full model.
    assert!(state.settings().kpi_acquisition_gate().expect("gate reads"));

    let disabled = state
        .update_settings(SettingsUpdate {
            kpi_acquisition_enabled: Some(false),
            ..Default::default()
        })
        .expect("settings should update");
    assert!(!disabled.mcp.kpi_acquisition_enabled);
    assert!(!state.settings().kpi_acquisition_gate().expect("gate reads"));
}

#[test]
fn mcp_settings_upsert_and_round_trip() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let settings = state
        .update_settings(SettingsUpdate {
            mcp_enabled: Some(true),
            mcp_port: Some(9000),
            ..Default::default()
        })
        .expect("settings should update");
    assert!(settings.mcp.enabled);
    assert_eq!(settings.mcp.port, 9000);

    // The values round-trip through a fresh read (durable in SQLite).
    let persisted = state.get_settings().expect("settings should persist");
    assert!(persisted.mcp.enabled);
    assert_eq!(persisted.mcp.port, 9000);

    // Disabling upserts over the existing row (self-healing, never dropped).
    let disabled = state
        .update_settings(SettingsUpdate {
            mcp_enabled: Some(false),
            ..Default::default()
        })
        .expect("settings should update");
    assert!(!disabled.mcp.enabled);
    assert_eq!(
        disabled.mcp.port, 9000,
        "an enabled-only update leaves the port untouched"
    );
}

#[test]
fn mcp_port_write_clamps_to_supported_range() {
    // Same clamp posture as the pool settings / backfill_years: an out-of-range
    // requested port is corrected, never rejected. Privileged ports (<1024) are
    // clamped up; values beyond the u16 port space are clamped down.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let low = state
        .update_settings(SettingsUpdate {
            mcp_port: Some(80),
            ..Default::default()
        })
        .expect("settings should update");
    assert_eq!(low.mcp.port, 1024, "80 clamps up to the minimum");

    let high = state
        .update_settings(SettingsUpdate {
            mcp_port: Some(70_000),
            ..Default::default()
        })
        .expect("settings should update");
    assert_eq!(high.mcp.port, 65_535, "70000 clamps down to the maximum");

    let ok = state
        .update_settings(SettingsUpdate {
            mcp_port: Some(8317),
            ..Default::default()
        })
        .expect("settings should update");
    assert_eq!(ok.mcp.port, 8317, "an in-range value is stored verbatim");
}

#[test]
fn mcp_port_clamps_out_of_range_stored_value_on_read() {
    // A hand-edited / absurd stored value is clamped on read so it can never
    // drive a bind outside the sane port range; an unparseable value falls
    // back to the default (tolerant `setting_i64_or` posture).
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .checkout()
        .expect("connection")
        .execute(
            "INSERT INTO settings (key, value, value_type) VALUES ('mcp_port', '70000', 'integer')
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [],
        )
        .expect("seed out-of-range value");

    assert_eq!(state.get_settings().expect("settings").mcp.port, 65_535);

    state
        .checkout()
        .expect("connection")
        .execute(
            "UPDATE settings SET value = 'not-a-port' WHERE key = 'mcp_port'",
            [],
        )
        .expect("seed unparseable value");

    assert_eq!(state.get_settings().expect("settings").mcp.port, 8317);
}

#[test]
fn similarity_strategy_reads_map_the_retired_embedding_value_to_static() {
    // ADR 0080: the embedding model is retired, so `static` is the only
    // strategy. An old database that persisted `embedding` must read as
    // `static` — tolerant, never an error — and a missing row still defaults
    // to `static` (ADR 0035 seed-row tolerance).
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));
    assert_eq!(
        state.get_similarity_strategy().expect("default read"),
        "static",
        "missing row defaults to static",
    );

    {
        let connection = state.checkout().expect("database connection");
        connection
            .execute(
                "INSERT INTO settings (key, value, value_type)
                 VALUES ('similarity_strategy', 'embedding', 'string')
                 ON CONFLICT (key) DO UPDATE SET value = 'embedding'",
                [],
            )
            .expect("seed the legacy embedding value");
    }

    assert_eq!(
        state.get_similarity_strategy().expect("legacy read"),
        "static",
        "the retired embedding value maps to static",
    );
}

/// ADR 0084 decision 5 + the data-model tolerant-read rule: the clean cut DELETES
/// the retired AI settings rows, so `get_settings` must still load on a database
/// where they are absent and return the safe defaults for everything that remains.
/// A read that assumed one of those rows existed would abort startup.
#[test]
fn settings_load_with_the_retired_ai_rows_deleted() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // The clean cut's own deletes, replayed: no retired row is left behind.
    state
        .checkout()
        .expect("connection")
        .execute(
            "DELETE FROM settings WHERE key IN (
                'ai_analysis_mode', 'ai_workers', 'ai_provider_concurrency',
                'capability_providers', 'general_analysis_provider',
                'espi_ai_fallback_enabled', 'history_sweep_ai_call_limit',
                'general_analysis_model', 'general_analysis_timeout_seconds',
                'openai_compatible_base_url'
            )",
            [],
        )
        .expect("delete the retired AI settings rows");

    let settings = state
        .get_settings()
        .expect("settings must still load with every retired AI row deleted");

    // The surviving surface reads its documented defaults.
    assert_eq!(settings.theme, "dark");
    assert_eq!(settings.locale, "en");
    assert_eq!(settings.poll_interval_seconds, 900);
    assert_eq!(settings.backfill_years, 3);
    assert_eq!(
        settings.ai_providers.youtube_transcription_provider, "provider_gemini",
        "transcription is KEPT — it is data acquisition, not analysis"
    );

    // And the queue config still resolves to its seeded defaults.
    let queue = state.queue_config();
    assert_eq!(queue.sources_workers, 2);
    assert_eq!(queue.autopilot_workers, 3);
}

// --- Dziś v2 visit anchor + reviewed days (F2 S2, plan decisions 4-5) -----

#[test]
fn today_visit_anchor_and_reviewed_days_default_when_absent() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    assert!(
        state
            .settings()
            .today_last_visit_at()
            .expect("read")
            .is_none(),
        "a never-visited database has no anchor row, not an empty-string one",
    );
    assert!(
        state
            .get_settings()
            .expect("settings")
            .today_reviewed_days
            .is_empty(),
        "reviewed days default to an empty list without a seed row",
    );
}

/// `strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`'s exact shape, matched elsewhere
/// in the codebase (e.g. `storage/tests/migration_safety.rs`'s
/// `"2026-07-20T10:00:00.000Z"`): `YYYY-MM-DDTHH:MM:SS.SSSZ`, 24 bytes.
fn looks_like_a_stamped_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 24 && bytes[10] == b'T' && value.ends_with('Z')
}

#[test]
fn mark_today_visited_stamps_a_parseable_timestamp_and_overwrites_a_previous_one() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // Seed a stale anchor directly (bypassing the write path under test) so
    // the test can prove `mark_today_visited` OVERWRITES an existing row, not
    // merely creates one on first use.
    state
        .checkout()
        .expect("connection")
        .execute(
            "INSERT INTO settings (key, value, value_type)
             VALUES ('todayLastVisitAt', '2020-01-01T00:00:00.000Z', 'string')",
            [],
        )
        .expect("seed stale anchor");

    let stamped = state
        .settings()
        .mark_today_visited()
        .expect("mark_today_visited");

    assert!(
        looks_like_a_stamped_timestamp(&stamped),
        "returned value {stamped} does not look like a stamped timestamp",
    );
    assert_ne!(
        stamped, "2020-01-01T00:00:00.000Z",
        "the stale seeded anchor must be overwritten, not left in place",
    );
    assert_eq!(
        state.settings().today_last_visit_at().expect("read"),
        Some(stamped),
        "the stamped value must be exactly what a fresh read reports back",
    );
}

#[test]
fn today_reviewed_days_round_trips_trims_to_14_and_rejects_bad_format() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // 15 dates in, newest-first order not required from the caller — the
    // write path sorts descending and keeps the 14 newest.
    let mut days: Vec<String> = (1..=15).map(|d| format!("2026-01-{d:02}")).collect();
    days.reverse(); // feed them oldest-first, proving the sort is on the write side

    let settings = state
        .update_settings(SettingsUpdate {
            today_reviewed_days: Some(days),
            ..Default::default()
        })
        .expect("settings should update");

    assert_eq!(settings.today_reviewed_days.len(), 14);
    // The 14 NEWEST survive — 2026-01-01 (the oldest of the 15) is dropped.
    assert!(!settings
        .today_reviewed_days
        .contains(&"2026-01-01".to_owned()));
    assert_eq!(settings.today_reviewed_days[0], "2026-01-15");

    // Round-trips through a fresh read (durable in SQLite).
    assert_eq!(
        state.get_settings().expect("settings").today_reviewed_days,
        settings.today_reviewed_days,
    );

    // An invalid entry is rejected outright (no partial write).
    let rejected = state.update_settings(SettingsUpdate {
        today_reviewed_days: Some(vec!["2026-13-40".to_owned()]),
        ..Default::default()
    });
    assert!(
        rejected.is_err(),
        "an out-of-range calendar entry must be rejected"
    );

    let malformed = state.update_settings(SettingsUpdate {
        today_reviewed_days: Some(vec!["not-a-date".to_owned()]),
        ..Default::default()
    });
    assert!(
        malformed.is_err(),
        "a non-YYYY-MM-DD entry must be rejected"
    );

    // The rejected writes left the prior (trimmed) value untouched.
    assert_eq!(
        state.get_settings().expect("settings").today_reviewed_days,
        settings.today_reviewed_days,
    );
}
