use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::{StorageError, StorageResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderSettings {
    pub youtube_transcription_provider: String,
    pub youtube_transcription_model: String,
    pub youtube_transcription_timeout_seconds: i64,
    pub general_analysis_provider: Option<String>,
    pub general_analysis_model: String,
    pub general_analysis_timeout_seconds: i64,
}

#[derive(Debug, Deserialize, Serialize)]
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
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    pub theme: String,
    pub locale: String,
    pub accent_palette: String,
    pub poll_interval_seconds: i64,
    pub settings_source: &'static str,
    pub settings_import_export_format: String,
    pub yaml_import_export_status: &'static str,
    pub ai_providers: AiProviderSettings,
    pub ai_analysis_mode: String,
    pub shortcut_bindings: HashMap<String, ShortcutBindingSetting>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    pub theme: Option<String>,
    pub locale: Option<String>,
    pub poll_interval_seconds: Option<i64>,
    pub youtube_transcription_provider: Option<String>,
    pub youtube_transcription_model: Option<String>,
    pub youtube_transcription_timeout_seconds: Option<i64>,
    pub general_analysis_provider: Option<String>,
    pub general_analysis_model: Option<String>,
    pub general_analysis_timeout_seconds: Option<i64>,
    pub ai_analysis_mode: Option<String>,
    pub shortcut_bindings: Option<HashMap<String, ShortcutBindingSetting>>,
}

pub(crate) fn get_settings(connection: &Connection) -> StorageResult<UserSettings> {
    Ok(UserSettings {
        theme: setting_string(connection, "theme")?,
        locale: setting_string(connection, "locale")?,
        accent_palette: setting_string(connection, "accent_palette")?,
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
        },
        ai_analysis_mode: setting_string(connection, "ai_analysis_mode")?,
        shortcut_bindings: setting_json(connection, "shortcut_bindings")?,
    })
}

pub(crate) fn update_settings(
    connection: &Connection,
    input: SettingsUpdate,
) -> StorageResult<UserSettings> {
    if let Some(theme) = input.theme {
        validate_allowed_setting("theme", &theme, &["dark", "light", "system"])?;
        update_setting(connection, "theme", &theme)?;
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

    if let Some(general_analysis_provider) = input.general_analysis_provider {
        validate_allowed_setting(
            "general_analysis_provider",
            &general_analysis_provider,
            &["", "provider_gemini"],
        )?;
        update_setting(
            connection,
            "general_analysis_provider",
            &general_analysis_provider,
        )?;
    }

    if let Some(general_analysis_model) = input.general_analysis_model {
        validate_allowed_setting(
            "general_analysis_model",
            &general_analysis_model,
            &[
                "gemini-2.5-flash-lite",
                "gemini-2.5-flash",
                "gemini-3.1-flash-lite",
                "gemini-3.5-flash",
            ],
        )?;
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

    if let Some(ai_analysis_mode) = input.ai_analysis_mode {
        validate_allowed_setting("ai_analysis_mode", &ai_analysis_mode, &["source_grounded"])?;
        update_setting(connection, "ai_analysis_mode", &ai_analysis_mode)?;
    }

    if let Some(shortcut_bindings) = input.shortcut_bindings {
        validate_shortcut_bindings(&shortcut_bindings)?;
        let value = serde_json::to_string(&shortcut_bindings).map_err(StorageError::from)?;
        update_setting(connection, "shortcut_bindings", &value)?;
    }

    get_settings(connection)
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

fn setting_json<T>(connection: &Connection, key: &'static str) -> StorageResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let value = setting_string(connection, key)?;

    serde_json::from_str::<T>(&value).map_err(StorageError::from)
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

fn empty_setting_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
