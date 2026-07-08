use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::{StorageError, StorageResult};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderSettings {
    pub youtube_transcription_provider: String,
    pub youtube_transcription_model: String,
    pub youtube_transcription_timeout_seconds: i64,
    pub general_analysis_provider: Option<String>,
    pub general_analysis_model: String,
    pub general_analysis_timeout_seconds: i64,
    /// Base URL for the generic OpenAI-compatible provider (ADR 0060). Empty
    /// string when unconfigured; only consulted when
    /// `general_analysis_provider` is `provider_openai_compatible`.
    pub openai_compatible_base_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct LogSettings {
    pub level: String,
    pub max_files: i64,
    pub max_file_bytes: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBindingSetting {
    pub key: String,
    pub alt_key: Option<bool>,
    pub ctrl_key: Option<bool>,
    pub meta_key: Option<bool>,
    pub shift_key: Option<bool>,
    pub disabled: Option<bool>,
}

/// One entry in a capability's ordered provider/model fallback pool (ADR
/// 0060 as amended). Stored as an element of the `capability_providers`
/// settings map, keyed by [`crate::providers::analysis::capabilities::AiCapability::key`].
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProviderEntry {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSettings {
    pub max_connections: u32,
    pub busy_timeout_ms: u64,
    pub acquire_timeout_ms: u64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct QueueSettings {
    /// Worker threads per lane (ADR 0059). Applied at startup (the lanes are
    /// spawned once); a change takes effect on restart, like the pool settings.
    pub sources_workers: i64,
    pub autopilot_workers: i64,
    pub ai_workers: i64,
    /// Max concurrent calls to any one AI provider, shared across the autopilot +
    /// ai lanes — the real AI cost/rate ceiling.
    pub ai_provider_concurrency: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    #[cfg_attr(feature = "ts-export", ts(type = "\"dark\" | \"light\" | \"system\""))]
    pub theme: String,
    #[cfg_attr(feature = "ts-export", ts(type = "\"en\" | \"pl\""))]
    pub locale: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"night-neon\" | \"midnight-horizon\"")
    )]
    pub accent_palette: String,
    pub developer_mode: bool,
    pub poll_interval_seconds: i64,
    pub settings_source: &'static str,
    pub settings_import_export_format: String,
    pub yaml_import_export_status: &'static str,
    pub ai_providers: AiProviderSettings,
    pub ai_analysis_mode: String,
    pub espi_ai_fallback_enabled: bool,
    pub logs: LogSettings,
    pub shortcut_bindings: HashMap<String, ShortcutBindingSetting>,
    /// Per-capability ordered (provider, model) fallback pool (ADR 0060 as
    /// amended), keyed by `AiCapability::key`. An absent key or an empty list
    /// means "use the global fallback" — no seed row, so a database that
    /// predates this key loads an empty map rather than failing.
    pub capability_providers: HashMap<String, Vec<CapabilityProviderEntry>>,
    pub database: DatabaseSettings,
    pub queue: QueueSettings,
    /// Company IDs the user has pinned to the sidebar spine (ADR 0054). A simple
    /// local UI preference stored as a JSON array in the `settings` KV table;
    /// order is the user's pin order. Tolerant default `[]` when the row is absent.
    pub pinned_company_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "UpdateSettingsInput",
        optional_fields
    )
)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    #[cfg_attr(
        feature = "ts-export",
        ts(optional, type = "\"dark\" | \"light\" | \"system\"")
    )]
    pub theme: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(optional, type = "\"night-neon\" | \"midnight-horizon\"")
    )]
    pub accent_palette: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(optional, type = "\"en\" | \"pl\""))]
    pub locale: Option<String>,
    pub poll_interval_seconds: Option<i64>,
    pub youtube_transcription_provider: Option<String>,
    pub youtube_transcription_model: Option<String>,
    pub youtube_transcription_timeout_seconds: Option<i64>,
    pub general_analysis_provider: Option<String>,
    pub general_analysis_model: Option<String>,
    pub general_analysis_timeout_seconds: Option<i64>,
    /// Base URL for the generic OpenAI-compatible provider (ADR 0060).
    pub openai_compatible_base_url: Option<String>,
    pub ai_analysis_mode: Option<String>,
    pub espi_ai_fallback_enabled: Option<bool>,
    pub log_level: Option<String>,
    pub log_max_files: Option<i64>,
    pub log_max_file_bytes: Option<i64>,
    pub shortcut_bindings: Option<HashMap<String, ShortcutBindingSetting>>,
    /// Replace the full `capability_providers` map (ADR 0060 as amended). The
    /// frontend sends the complete desired map, so this overwrites rather than
    /// merges — same contract as `shortcut_bindings`.
    pub capability_providers: Option<HashMap<String, Vec<CapabilityProviderEntry>>>,
    pub db_max_connections: Option<i64>,
    pub db_busy_timeout_ms: Option<i64>,
    pub db_acquire_timeout_ms: Option<i64>,
    pub sources_workers: Option<i64>,
    pub autopilot_workers: Option<i64>,
    pub ai_workers: Option<i64>,
    pub ai_provider_concurrency: Option<i64>,
    /// Replace the full pinned-company list (ADR 0054). The frontend sends the
    /// complete desired order, so this overwrites rather than merges.
    pub pinned_company_ids: Option<Vec<String>>,
}

pub(crate) fn get_settings(connection: &Connection) -> StorageResult<UserSettings> {
    Ok(UserSettings {
        theme: setting_string(connection, "theme")?,
        locale: setting_string(connection, "locale")?,
        accent_palette: setting_string(connection, "accent_palette")?,
        developer_mode: setting_bool(connection, "developer_mode")?,
        poll_interval_seconds: setting_i64(connection, "poll_interval_seconds")?,
        settings_source: "sqlite",
        settings_import_export_format: setting_string(connection, "settings_import_export_format")?,
        yaml_import_export_status: "accepted_deferred",
        ai_providers: AiProviderSettings {
            youtube_transcription_provider: setting_string(
                connection,
                "youtube_transcription_provider",
            )?,
            youtube_transcription_model: setting_string(connection, "youtube_transcription_model")?,
            youtube_transcription_timeout_seconds: setting_i64(
                connection,
                "youtube_transcription_timeout_seconds",
            )?,
            general_analysis_provider: empty_setting_to_none(setting_string(
                connection,
                "general_analysis_provider",
            )?),
            general_analysis_model: setting_string(connection, "general_analysis_model")?,
            general_analysis_timeout_seconds: setting_i64(
                connection,
                "general_analysis_timeout_seconds",
            )?,
            openai_compatible_base_url: setting_string_or(
                connection,
                "openai_compatible_base_url",
                "",
            )?,
        },
        ai_analysis_mode: setting_string(connection, "ai_analysis_mode")?,
        espi_ai_fallback_enabled: setting_bool_or(connection, "espi_ai_fallback_enabled", false)?,
        logs: LogSettings {
            level: setting_string(connection, "log_level")?,
            max_files: setting_i64(connection, "log_max_files")?,
            max_file_bytes: setting_i64(connection, "log_max_file_bytes")?,
        },
        shortcut_bindings: setting_json(connection, "shortcut_bindings")?,
        capability_providers: setting_json_or_default(connection, "capability_providers")?,
        pinned_company_ids: setting_json_or_default(connection, "pinned_company_ids")?,
        database: {
            let config = super::pool::read_pool_config(connection);
            DatabaseSettings {
                max_connections: config.max_connections,
                busy_timeout_ms: config.busy_timeout_ms,
                acquire_timeout_ms: config.acquire_timeout_ms,
            }
        },
        queue: {
            let config = super::queue_config::read_queue_config(connection);
            QueueSettings {
                sources_workers: config.sources_workers,
                autopilot_workers: config.autopilot_workers,
                ai_workers: config.ai_workers,
                ai_provider_concurrency: config.ai_provider_concurrency,
            }
        },
    })
}

pub(crate) fn set_developer_mode_enabled(
    connection: &Connection,
    enabled: bool,
) -> StorageResult<UserSettings> {
    update_setting(
        connection,
        "developer_mode",
        if enabled { "true" } else { "false" },
    )?;

    get_settings(connection)
}

/// The active interpretative-layer similarity strategy (`static` | `embedding`),
/// defaulting to `static` when the row is absent so a database that recorded
/// migration 0049 before the seed row materialized never fails to load (ADR 0035).
pub(crate) fn get_similarity_strategy(connection: &Connection) -> StorageResult<String> {
    match connection.query_row(
        "SELECT value FROM settings WHERE key = 'similarity_strategy'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok("static".to_owned()),
        Err(error) => Err(StorageError::from(error)),
    }
}

/// Persist the active similarity strategy. Upserts (self-healing) and validates
/// the value; readiness checks for the `embedding` model live in the command layer.
pub(crate) fn set_similarity_strategy(
    connection: &Connection,
    strategy: &str,
) -> StorageResult<()> {
    validate_allowed_setting("similarity_strategy", strategy, &["static", "embedding"])?;
    connection.execute(
        "
        INSERT INTO settings (key, value, value_type)
        VALUES ('similarity_strategy', ?1, 'string')
        ON CONFLICT (key) DO UPDATE SET
            value = excluded.value,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![strategy],
    )?;

    Ok(())
}

pub(crate) fn update_settings(
    connection: &Connection,
    input: SettingsUpdate,
) -> StorageResult<UserSettings> {
    // The body below runs many sequential field writes with validation
    // interleaved; an early `?` on a later field's validation must not leave
    // an earlier field's write committed (Radicle ef7ad69). Wrapping the
    // whole update in a transaction makes any early return roll back on
    // `Drop`, and it only persists once every field has validated and written.
    let transaction = connection.unchecked_transaction()?;
    let connection = &transaction;

    if let Some(theme) = input.theme {
        validate_allowed_setting("theme", &theme, &["dark", "light", "system"])?;
        update_setting(connection, "theme", &theme)?;
    }

    if let Some(accent_palette) = input.accent_palette {
        validate_allowed_setting(
            "accent_palette",
            &accent_palette,
            &["night-neon", "midnight-horizon"],
        )?;
        update_setting(connection, "accent_palette", &accent_palette)?;
    }

    if let Some(locale) = input.locale {
        validate_allowed_setting("locale", &locale, &["en", "pl"])?;
        update_setting(connection, "locale", &locale)?;
    }

    if let Some(poll_interval_seconds) = input.poll_interval_seconds {
        validate_allowed_setting_i64(
            "poll_interval_seconds",
            poll_interval_seconds,
            &[300, 900, 1800, 3600],
        )?;
        update_setting(
            connection,
            "poll_interval_seconds",
            &poll_interval_seconds.to_string(),
        )?;
    }

    if let Some(youtube_transcription_provider) = input.youtube_transcription_provider {
        validate_allowed_setting(
            "youtube_transcription_provider",
            &youtube_transcription_provider,
            &["provider_gemini"],
        )?;
        update_setting(
            connection,
            "youtube_transcription_provider",
            &youtube_transcription_provider,
        )?;
    }

    if let Some(youtube_transcription_model) = input.youtube_transcription_model {
        validate_allowed_setting(
            "youtube_transcription_model",
            &youtube_transcription_model,
            &[
                "gemini-2.5-flash-lite",
                "gemini-2.5-flash",
                "gemini-3.1-flash-lite",
                "gemini-3.5-flash",
            ],
        )?;
        update_setting(
            connection,
            "youtube_transcription_model",
            &youtube_transcription_model,
        )?;
    }

    if let Some(youtube_transcription_timeout_seconds) = input.youtube_transcription_timeout_seconds
    {
        validate_allowed_setting_i64(
            "youtube_transcription_timeout_seconds",
            youtube_transcription_timeout_seconds,
            &[45, 90, 180, 300, 600],
        )?;
        update_setting(
            connection,
            "youtube_transcription_timeout_seconds",
            &youtube_transcription_timeout_seconds.to_string(),
        )?;
    }

    // Captured before the provider/model blocks below mutate anything: the
    // provider this update will leave in effect, used to decide whether
    // `general_analysis_model` accepts a freeform value (OpenAI-compatible,
    // ADR 0060) or must be one of the curated `analysis_model_ids()`.
    let effective_general_analysis_provider = input
        .general_analysis_provider
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            setting_string(connection, "general_analysis_provider")
                .ok()
                .and_then(empty_setting_to_none)
        });

    if let Some(general_analysis_provider) = input.general_analysis_provider {
        let mut allowed_providers = vec![""];
        allowed_providers
            .extend(crate::providers::analysis::registry::selectable_analysis_provider_ids());
        validate_allowed_setting(
            "general_analysis_provider",
            &general_analysis_provider,
            &allowed_providers,
        )?;
        update_setting(
            connection,
            "general_analysis_provider",
            &general_analysis_provider,
        )?;
    }

    if let Some(general_analysis_model) = input.general_analysis_model {
        let is_openai_compatible = effective_general_analysis_provider.as_deref()
            == Some(crate::providers::analysis::registry::OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID);
        if is_openai_compatible {
            // The OpenAI-compatible provider has no curated model list (ADR
            // 0060) — any non-empty, user-supplied model id is accepted.
            if general_analysis_model.trim().is_empty() {
                return Err(StorageError::InvalidSettingValue {
                    key: "general_analysis_model",
                    value: general_analysis_model,
                });
            }
        } else {
            validate_allowed_setting(
                "general_analysis_model",
                &general_analysis_model,
                &crate::providers::analysis::registry::analysis_model_ids(),
            )?;
        }
        update_setting(
            connection,
            "general_analysis_model",
            &general_analysis_model,
        )?;
    }

    if let Some(general_analysis_timeout_seconds) = input.general_analysis_timeout_seconds {
        validate_allowed_setting_i64(
            "general_analysis_timeout_seconds",
            general_analysis_timeout_seconds,
            &[45, 90, 180, 300, 600],
        )?;
        update_setting(
            connection,
            "general_analysis_timeout_seconds",
            &general_analysis_timeout_seconds.to_string(),
        )?;
    }

    if let Some(openai_compatible_base_url) = input.openai_compatible_base_url {
        let trimmed = openai_compatible_base_url.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with("http://")
            && !trimmed.starts_with("https://")
        {
            return Err(StorageError::InvalidSettingValue {
                key: "openai_compatible_base_url",
                value: openai_compatible_base_url,
            });
        }
        // No seed row (introduced after the initial migration) — upsert like
        // `pinned_company_ids` so the value is never silently dropped.
        upsert_setting(connection, "openai_compatible_base_url", trimmed, "string")?;
    }

    if let Some(ai_analysis_mode) = input.ai_analysis_mode {
        validate_allowed_setting("ai_analysis_mode", &ai_analysis_mode, &["source_grounded"])?;
        update_setting(connection, "ai_analysis_mode", &ai_analysis_mode)?;
    }

    if let Some(espi_ai_fallback_enabled) = input.espi_ai_fallback_enabled {
        update_setting(
            connection,
            "espi_ai_fallback_enabled",
            if espi_ai_fallback_enabled {
                "true"
            } else {
                "false"
            },
        )?;
    }

    if let Some(log_level) = input.log_level {
        validate_allowed_setting(
            "log_level",
            &log_level,
            &["off", "error", "warn", "info", "debug", "trace"],
        )?;
        update_setting(connection, "log_level", &log_level)?;
    }

    if let Some(log_max_files) = input.log_max_files {
        validate_setting_i64_range("log_max_files", log_max_files, 1, 20)?;
        update_setting(connection, "log_max_files", &log_max_files.to_string())?;
    }

    if let Some(log_max_file_bytes) = input.log_max_file_bytes {
        validate_setting_i64_range(
            "log_max_file_bytes",
            log_max_file_bytes,
            1_048_576,
            104_857_600,
        )?;
        update_setting(
            connection,
            "log_max_file_bytes",
            &log_max_file_bytes.to_string(),
        )?;
    }

    if let Some(shortcut_bindings) = input.shortcut_bindings {
        validate_shortcut_bindings(&shortcut_bindings)?;
        let value = serde_json::to_string(&shortcut_bindings).map_err(StorageError::from)?;
        update_setting(connection, "shortcut_bindings", &value)?;
    }

    if let Some(capability_providers) = input.capability_providers {
        validate_capability_providers(&capability_providers)?;
        let value = serde_json::to_string(&capability_providers).map_err(StorageError::from)?;
        // No seed row (introduced after the initial migration) — upsert like
        // `pinned_company_ids` so the value is never silently dropped.
        upsert_setting(connection, "capability_providers", &value, "json")?;
    }

    if let Some(pinned_company_ids) = input.pinned_company_ids {
        let deduped = dedupe_preserving_order(pinned_company_ids);
        let value = serde_json::to_string(&deduped).map_err(StorageError::from)?;
        upsert_setting(connection, "pinned_company_ids", &value, "json")?;
    }

    // Connection-pool tuning is clamped to safe ranges rather than rejected, so a
    // value out of bounds is corrected instead of failing the update (ADR 0032).
    // Pool sizing takes effect on the next launch.
    if let Some(db_max_connections) = input.db_max_connections {
        let clamped = super::pool::clamp_max_connections(db_max_connections);
        update_setting(
            connection,
            super::pool::MAX_CONNECTIONS_SETTING_KEY,
            &clamped.to_string(),
        )?;
    }

    if let Some(db_busy_timeout_ms) = input.db_busy_timeout_ms {
        let clamped = super::pool::clamp_busy_timeout_ms(db_busy_timeout_ms);
        update_setting(
            connection,
            super::pool::BUSY_TIMEOUT_SETTING_KEY,
            &clamped.to_string(),
        )?;
    }

    if let Some(db_acquire_timeout_ms) = input.db_acquire_timeout_ms {
        let clamped = super::pool::clamp_acquire_timeout_ms(db_acquire_timeout_ms);
        update_setting(
            connection,
            super::pool::ACQUIRE_TIMEOUT_SETTING_KEY,
            &clamped.to_string(),
        )?;
    }

    // Worker-lane counts + per-provider AI concurrency are clamped to safe ranges
    // (never rejected) and take effect on the next launch (ADR 0059).
    if let Some(sources_workers) = input.sources_workers {
        let clamped = super::queue_config::clamp_workers(sources_workers);
        update_setting(
            connection,
            super::queue_config::SOURCES_WORKERS_KEY,
            &clamped.to_string(),
        )?;
    }
    if let Some(autopilot_workers) = input.autopilot_workers {
        let clamped = super::queue_config::clamp_workers(autopilot_workers);
        update_setting(
            connection,
            super::queue_config::AUTOPILOT_WORKERS_KEY,
            &clamped.to_string(),
        )?;
    }
    if let Some(ai_workers) = input.ai_workers {
        let clamped = super::queue_config::clamp_workers(ai_workers);
        update_setting(
            connection,
            super::queue_config::AI_WORKERS_KEY,
            &clamped.to_string(),
        )?;
    }
    if let Some(ai_provider_concurrency) = input.ai_provider_concurrency {
        let clamped = super::queue_config::clamp_provider_concurrency(ai_provider_concurrency);
        update_setting(
            connection,
            super::queue_config::AI_PROVIDER_CONCURRENCY_KEY,
            &clamped.to_string(),
        )?;
    }

    let settings = get_settings(connection)?;
    transaction.commit()?;
    Ok(settings)
}

fn setting_string(connection: &Connection, key: &'static str) -> StorageResult<String> {
    connection
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .map_err(StorageError::from)
}

fn setting_i64(connection: &Connection, key: &'static str) -> StorageResult<i64> {
    let value = setting_string(connection, key)?;

    value
        .parse::<i64>()
        .map_err(|_| StorageError::InvalidSettingValue { key, value })
}

fn setting_bool(connection: &Connection, key: &'static str) -> StorageResult<bool> {
    let value = setting_string(connection, key)?;

    match value.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(StorageError::InvalidSettingValue { key, value }),
    }
}

/// Read a boolean setting, falling back to `default` when the row is absent.
/// Used for settings introduced by later migrations so a database that recorded
/// the migration before the seed row existed never fails to load settings (and,
/// via the startup `get_settings` call, never crashes app launch).
fn setting_bool_or(
    connection: &Connection,
    key: &'static str,
    default: bool,
) -> StorageResult<bool> {
    match connection.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    }) {
        Ok(value) => match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(StorageError::InvalidSettingValue { key, value }),
        },
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default),
        Err(error) => Err(StorageError::from(error)),
    }
}

/// Read a string setting, falling back to `default` when the row is absent.
/// Used for settings introduced without a seed-row migration (e.g. ADR 0060's
/// `openai_compatible_base_url`) so a database that predates the key never
/// fails to load settings.
fn setting_string_or(
    connection: &Connection,
    key: &'static str,
    default: &str,
) -> StorageResult<String> {
    match connection.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    }) {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default.to_owned()),
        Err(error) => Err(StorageError::from(error)),
    }
}

/// The persisted app locale, normalized to a supported code (`pl` | `en`), for
/// resolving bilingual template seeds (ADR 0076 Decision 8). An absent or
/// unrecognized row falls back to the default `pl`.
pub(super) fn seed_locale(connection: &Connection) -> StorageResult<String> {
    let raw = setting_string_or(connection, "locale", "pl")?;
    Ok(match raw.as_str() {
        "en" => "en".to_owned(),
        _ => "pl".to_owned(),
    })
}

fn setting_json<T>(connection: &Connection, key: &'static str) -> StorageResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let value = setting_string(connection, key)?;

    serde_json::from_str::<T>(&value).map_err(StorageError::from)
}

/// Read a JSON setting, falling back to `T::default()` when the row is absent.
/// Used for settings introduced without a seed-row migration (ADR 0054 pinned
/// companies) so a database that predates the key never fails to load settings.
fn setting_json_or_default<T>(connection: &Connection, key: &'static str) -> StorageResult<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    match connection.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    }) {
        Ok(value) => serde_json::from_str::<T>(&value).map_err(StorageError::from),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(T::default()),
        Err(error) => Err(StorageError::from(error)),
    }
}

/// Upsert a setting for keys that may have no seed row (introduced after the
/// initial migration). Unlike [`update_setting`] (UPDATE-only), this inserts the
/// row when absent so the value is never silently dropped.
fn upsert_setting(
    connection: &Connection,
    key: &'static str,
    value: &str,
    value_type: &'static str,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO settings (key, value, value_type)
        VALUES (?1, ?2, ?3)
        ON CONFLICT (key) DO UPDATE SET
            value = excluded.value,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![key, value, value_type],
    )?;

    Ok(())
}

/// Remove duplicate IDs while preserving first-seen order (the user's pin order).
fn dedupe_preserving_order(ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter()
        .filter(|id| !id.trim().is_empty() && seen.insert(id.clone()))
        .collect()
}

fn update_setting(connection: &Connection, key: &'static str, value: &str) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE settings
        SET value = ?2,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE key = ?1
        ",
        params![key, value],
    )?;

    Ok(())
}

fn validate_allowed_setting(key: &'static str, value: &str, allowed: &[&str]) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn validate_allowed_setting_i64(
    key: &'static str,
    value: i64,
    allowed: &[i64],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_string(),
        })
    }
}

fn validate_setting_i64_range(
    key: &'static str,
    value: i64,
    min: i64,
    max: i64,
) -> StorageResult<()> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_string(),
        })
    }
}

fn validate_shortcut_bindings(
    shortcut_bindings: &HashMap<String, ShortcutBindingSetting>,
) -> StorageResult<()> {
    for (shortcut_id, binding) in shortcut_bindings {
        if shortcut_id.trim().is_empty() {
            return Err(StorageError::InvalidSettingValue {
                key: "shortcut_bindings",
                value: "empty shortcut id".to_owned(),
            });
        }

        if binding.disabled.unwrap_or(false) {
            continue;
        }

        if binding.key.trim().is_empty() {
            return Err(StorageError::InvalidSettingValue {
                key: "shortcut_bindings",
                value: format!("{shortcut_id}: empty key"),
            });
        }
    }

    Ok(())
}

/// Validate a `capability_providers` map (ADR 0060 as amended, ADR 0061
/// decision 5): every key must be a recognized
/// [`crate::providers::analysis::capabilities::AiCapability`] key, and every
/// entry's provider/model must be a currently selectable analysis provider /
/// curated model — except the OpenAI-compatible provider, which has no
/// curated model list and accepts any non-empty model id (same exemption as
/// `general_analysis_model`). An empty entry list is valid: it is the
/// explicit "use the global fallback" state. A `document`-kind capability
/// (KPI/claim extraction) additionally requires every entry's provider to
/// have **native** document support: a document-kind call site always sends
/// `AnalysisDocument::Native`, and a `TextOnly` provider (OpenAI,
/// OpenAI-compatible) hard-fails every such call and poisons pool failover
/// (Radicle 6ea2a8a) — so this is rejected here, at settings-write time, not
/// left to surface only when the job actually runs.
fn validate_capability_providers(
    capability_providers: &HashMap<String, Vec<CapabilityProviderEntry>>,
) -> StorageResult<()> {
    use crate::providers::analysis::capabilities::{AiCapability, CapabilityKind};
    use crate::providers::analysis::registry::{
        analysis_model_ids, analysis_provider_document_support, selectable_analysis_provider_ids,
        OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID,
    };
    use crate::providers::analysis::DocumentSupport;

    let selectable_providers = selectable_analysis_provider_ids();
    let curated_models = analysis_model_ids();

    for (capability_key, entries) in capability_providers {
        let capability = match AiCapability::from_key(capability_key) {
            Some(capability) => capability,
            None => {
                let valid_keys = AiCapability::ALL
                    .iter()
                    .map(|capability| capability.key())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(StorageError::InvalidSettingValue {
                    key: "capability_providers",
                    value: format!(
                        "unknown capability key {capability_key} (expected one of {valid_keys})"
                    ),
                });
            }
        };

        for entry in entries {
            if !selectable_providers.contains(&entry.provider.as_str()) {
                return Err(StorageError::InvalidSettingValue {
                    key: "capability_providers",
                    value: format!("{capability_key}: unknown provider {}", entry.provider),
                });
            }

            if entry.model.trim().is_empty() {
                return Err(StorageError::InvalidSettingValue {
                    key: "capability_providers",
                    value: format!(
                        "{capability_key}: empty model for provider {}",
                        entry.provider
                    ),
                });
            }

            let is_openai_compatible = entry.provider == OPENAI_COMPATIBLE_ANALYSIS_PROVIDER_ID;
            if !is_openai_compatible && !curated_models.contains(&entry.model.as_str()) {
                return Err(StorageError::InvalidSettingValue {
                    key: "capability_providers",
                    value: format!(
                        "{capability_key}: uncurated model {} for provider {}",
                        entry.model, entry.provider
                    ),
                });
            }

            if capability.kind() == CapabilityKind::Document
                && analysis_provider_document_support(&entry.provider) != DocumentSupport::Native
            {
                return Err(StorageError::InvalidSettingValue {
                    key: "capability_providers",
                    value: format!(
                        "{capability_key}: provider {} cannot accept document input; route a document-capable provider (Gemini or Claude)",
                        entry.provider
                    ),
                });
            }
        }
    }

    Ok(())
}

fn empty_setting_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

use super::database::Database;
/// settings domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::settings()`.
#[derive(Clone)]
pub struct SettingsStore {
    db: Database,
}

impl SettingsStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn get_settings(&self) -> StorageResult<UserSettings> {
        let connection = self.db.checkout()?;

        get_settings(&connection)
    }

    pub fn update_settings(&self, input: SettingsUpdate) -> StorageResult<UserSettings> {
        let connection = self.db.checkout()?;

        update_settings(&connection, input)
    }

    pub fn set_developer_mode_enabled(&self, enabled: bool) -> StorageResult<UserSettings> {
        let connection = self.db.checkout()?;

        set_developer_mode_enabled(&connection, enabled)
    }

    pub fn get_similarity_strategy(&self) -> StorageResult<String> {
        let connection = self.db.checkout()?;

        get_similarity_strategy(&connection)
    }

    pub fn set_similarity_strategy(&self, strategy: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        set_similarity_strategy(&connection, strategy)
    }
}
