use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::{StorageError, StorageResult};

/// Backfill depth (years of history the on-track backfill covers, ADR 0077 §3).
/// The write path clamps to `[MIN, MAX]` rather than rejecting (like the pool
/// settings, ADR 0032); reads clamp an out-of-range stored value to the same
/// range so a hand-edited database never drives an absurd fetch. No seed-row
/// migration — an absent row reads `DEFAULT` (tolerant, like ADR 0060's
/// `openai_compatible_base_url`).
pub(crate) const BACKFILL_YEARS_MIN: i64 = 1;
pub(crate) const BACKFILL_YEARS_MAX: i64 = 10;
pub(crate) const BACKFILL_YEARS_DEFAULT: i64 = 3;

/// Clamp a requested backfill depth into the supported `[1, 10]` years range.
pub(crate) fn clamp_backfill_years(value: i64) -> i64 {
    value.clamp(BACKFILL_YEARS_MIN, BACKFILL_YEARS_MAX)
}

/// MCP server listen port (ADR 0078 decision 4). Same tolerant/clamp posture as
/// `backfill_years`: the write path clamps to the non-privileged port range
/// `[1024, 65535]` rather than rejecting, and reads clamp an out-of-range (or
/// default an unparseable) stored value so a hand-edited database never drives
/// a privileged or impossible bind. No seed-row migration — an absent row
/// reads `DEFAULT` (established keyless pattern).
pub(crate) const MCP_PORT_MIN: i64 = 1024;
pub(crate) const MCP_PORT_MAX: i64 = 65_535;
pub(crate) const MCP_PORT_DEFAULT: i64 = 8317;

/// Clamp a requested MCP port into the supported `[1024, 65535]` range.
pub(crate) fn clamp_mcp_port(value: i64) -> i64 {
    value.clamp(MCP_PORT_MIN, MCP_PORT_MAX)
}

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
}

/// MCP server settings group (ADR 0078 decisions 2 + 4). The server is off by
/// default and binds `127.0.0.1:<port>` only (the bind address is deliberately
/// not a setting — guardrail G-4). The auth token lives in the OS keychain
/// under the credentials boundary, never in this table.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct McpSettings {
    pub enabled: bool,
    /// Listen port; clamped to `[1024, 65535]`, default `8317`.
    pub port: u16,
    /// Whether the MCP `act` (write) tier is enabled (ADR 0088 M3). Default
    /// `false`: the read wave is always available when the server is on, but
    /// write tools stay gated behind this toggle and reject calls with a typed
    /// `writes_disabled` error until the user opts in. Deliberately absent from
    /// the MCP surface (`update_settings` is `Excluded`), so a connected agent
    /// can never enable its own writes.
    pub writes_enabled: bool,
    /// Whether the `kpi_acquisition` scope is enabled (ADR 0099 dec. 2).
    /// Default `false`: disabled, the acquisition token is rejected at auth
    /// (401) like an unknown token — the kill switch covers reads too. Same
    /// self-enable-impossible posture as `writes_enabled`.
    pub kpi_acquisition_enabled: bool,
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
    /// Years of company history the on-track backfill covers (ADR 0077 §3).
    /// Clamped to `[1, 10]`; an absent row reads the default `3`.
    pub backfill_years: i64,
    pub settings_source: &'static str,
    pub settings_import_export_format: String,
    pub yaml_import_export_status: &'static str,
    pub ai_providers: AiProviderSettings,
    pub logs: LogSettings,
    pub shortcut_bindings: HashMap<String, ShortcutBindingSetting>,
    pub database: DatabaseSettings,
    pub queue: QueueSettings,
    /// Company IDs the user has pinned to the sidebar spine (ADR 0054). A simple
    /// local UI preference stored as a JSON array in the `settings` KV table;
    /// order is the user's pin order. Tolerant default `[]` when the row is absent.
    pub pinned_company_ids: Vec<String>,
    /// Read-only MCP server (ADR 0078): off by default, port default `8317`.
    /// Absent rows read the defaults (no seed-row migration).
    pub mcp: McpSettings,
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
    /// Requested backfill depth in years (ADR 0077 §3); clamped to `[1, 10]` on
    /// write rather than rejected.
    pub backfill_years: Option<i64>,
    pub youtube_transcription_provider: Option<String>,
    pub youtube_transcription_model: Option<String>,
    pub youtube_transcription_timeout_seconds: Option<i64>,
    pub log_level: Option<String>,
    pub log_max_files: Option<i64>,
    pub log_max_file_bytes: Option<i64>,
    pub shortcut_bindings: Option<HashMap<String, ShortcutBindingSetting>>,
    pub db_max_connections: Option<i64>,
    pub db_busy_timeout_ms: Option<i64>,
    pub db_acquire_timeout_ms: Option<i64>,
    pub sources_workers: Option<i64>,
    pub autopilot_workers: Option<i64>,
    /// Replace the full pinned-company list (ADR 0054). The frontend sends the
    /// complete desired order, so this overwrites rather than merges.
    pub pinned_company_ids: Option<Vec<String>>,
    /// Enable/disable persistence for the MCP server (ADR 0078). The live
    /// start/stop lifecycle command is separate (`set_mcp_enabled`, M3).
    pub mcp_enabled: Option<bool>,
    /// Requested MCP listen port (ADR 0078); clamped to `[1024, 65535]` on
    /// write rather than rejected.
    pub mcp_port: Option<i64>,
    /// Enable/disable the MCP `act` (write) tier (ADR 0088 M3). Default `false`.
    /// This is the ONLY path to toggle write access — and `update_settings` is
    /// `Excluded` from the MCP registry, so an agent can never flip it itself.
    pub mcp_writes_enabled: Option<bool>,
    /// Enable/disable the `kpi_acquisition` MCP scope (ADR 0099 dec. 2).
    /// Default `false`; same excluded-from-MCP posture as `mcp_writes_enabled`.
    pub kpi_acquisition_enabled: Option<bool>,
}

pub(crate) fn get_settings(connection: &Connection) -> StorageResult<UserSettings> {
    Ok(UserSettings {
        theme: setting_string(connection, "theme")?,
        locale: setting_string(connection, "locale")?,
        accent_palette: setting_string(connection, "accent_palette")?,
        developer_mode: setting_bool(connection, "developer_mode")?,
        poll_interval_seconds: setting_i64(connection, "poll_interval_seconds")?,
        backfill_years: clamp_backfill_years(setting_i64_or(
            connection,
            "backfill_years",
            BACKFILL_YEARS_DEFAULT,
        )?),
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
        },
        logs: LogSettings {
            level: setting_string(connection, "log_level")?,
            max_files: setting_i64(connection, "log_max_files")?,
            max_file_bytes: setting_i64(connection, "log_max_file_bytes")?,
        },
        shortcut_bindings: setting_json(connection, "shortcut_bindings")?,
        pinned_company_ids: setting_json_or_default(connection, "pinned_company_ids")?,
        mcp: McpSettings {
            enabled: setting_bool_or(connection, "mcp_enabled", false)?,
            // Clamp on read too, so a hand-edited row can never drive a
            // privileged or impossible bind; `as u16` is safe post-clamp.
            port: clamp_mcp_port(setting_i64_or(connection, "mcp_port", MCP_PORT_DEFAULT)?) as u16,
            writes_enabled: setting_bool_or(connection, "mcp_writes_enabled", false)?,
            kpi_acquisition_enabled: setting_bool_or(connection, "kpi_acquisition_enabled", false)?,
        },
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

/// The legacy interpretative-layer similarity-strategy read, kept as a
/// tolerant shim (ADR 0080): a missing row or the legacy `embedding` value
/// both read as `static` — never an error, never a resurrected strategy.
/// There is no setter anymore.
pub(crate) fn get_similarity_strategy(connection: &Connection) -> StorageResult<String> {
    match connection.query_row(
        "SELECT value FROM settings WHERE key = 'similarity_strategy'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) if value == "embedding" => Ok("static".to_owned()),
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok("static".to_owned()),
        Err(error) => Err(StorageError::from(error)),
    }
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
    let transaction =
        rusqlite::Transaction::new_unchecked(connection, rusqlite::TransactionBehavior::Immediate)?;
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

    // Backfill depth is clamped to the safe range rather than rejected (ADR 0077
    // §3, same posture as the pool settings). No seed row — upsert so the value
    // is never silently dropped.
    if let Some(backfill_years) = input.backfill_years {
        let clamped = clamp_backfill_years(backfill_years);
        upsert_setting(
            connection,
            "backfill_years",
            &clamped.to_string(),
            "integer",
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

    if let Some(pinned_company_ids) = input.pinned_company_ids {
        let deduped = dedupe_preserving_order(pinned_company_ids);
        let value = serde_json::to_string(&deduped).map_err(StorageError::from)?;
        upsert_setting(connection, "pinned_company_ids", &value, "json")?;
    }

    // MCP server settings (ADR 0078 decision 4). No seed rows — upsert like
    // `backfill_years` so the values are never silently dropped; the port is
    // clamped to the safe range rather than rejected.
    if let Some(mcp_enabled) = input.mcp_enabled {
        // Booleans are stored as 'string'-typed "true"/"false" rows everywhere
        // else (developer_mode) — keep that.
        upsert_setting(
            connection,
            "mcp_enabled",
            if mcp_enabled { "true" } else { "false" },
            "string",
        )?;
    }

    if let Some(mcp_port) = input.mcp_port {
        let clamped = clamp_mcp_port(mcp_port);
        upsert_setting(connection, "mcp_port", &clamped.to_string(), "integer")?;
    }

    if let Some(mcp_writes_enabled) = input.mcp_writes_enabled {
        upsert_setting(
            connection,
            "mcp_writes_enabled",
            if mcp_writes_enabled { "true" } else { "false" },
            "string",
        )?;
    }

    if let Some(kpi_acquisition_enabled) = input.kpi_acquisition_enabled {
        upsert_setting(
            connection,
            "kpi_acquisition_enabled",
            if kpi_acquisition_enabled {
                "true"
            } else {
                "false"
            },
            "string",
        )?;
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

/// Read an integer setting, tolerant of both an absent row and an unparseable
/// value: either yields `default`. Used for keys introduced without a seed-row
/// migration (ADR 0077 §3 `backfill_years`) so a database that predates the key
/// never fails to load settings; the caller clamps the result to its valid range.
fn setting_i64_or(connection: &Connection, key: &'static str, default: i64) -> StorageResult<i64> {
    match connection.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get::<_, String>(0)
    }) {
        Ok(value) => Ok(value.parse::<i64>().unwrap_or(default)),
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

    /// Single-row read of the `kpi_acquisition` scope gate (ADR 0099 dec. 2).
    /// The MCP server consults this on every authenticated request — a
    /// dedicated one-key read so unauthenticated traffic never materializes
    /// the whole settings model.
    pub fn kpi_acquisition_gate(&self) -> StorageResult<bool> {
        let connection = self.db.checkout()?;

        setting_bool_or(&connection, "kpi_acquisition_enabled", false)
    }

    pub fn set_developer_mode_enabled(&self, enabled: bool) -> StorageResult<UserSettings> {
        let connection = self.db.checkout()?;

        set_developer_mode_enabled(&connection, enabled)
    }

    pub fn get_similarity_strategy(&self) -> StorageResult<String> {
        let connection = self.db.checkout()?;

        get_similarity_strategy(&connection)
    }
}
