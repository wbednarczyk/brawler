use super::*;

pub(super) fn preview_settings_import(contents: &str) -> StorageResult<ImportPreview> {
    let document = parse_settings_document(contents)?;
    let mut preview = plan_settings_import(&document);
    preview.warnings.extend(retired_setting_warnings(contents));
    Ok(preview)
}

pub(super) fn apply_settings_import(
    connection: &Connection,
    contents: &str,
) -> StorageResult<ImportApplyResult> {
    let document = parse_settings_document(contents)?;
    let mut preview = plan_settings_import(&document);
    preview.warnings.extend(retired_setting_warnings(contents));
    if !preview.valid {
        return Err(StorageError::InvalidSettingValue {
            key: "import_export",
            value: preview.errors.join("; "),
        });
    }

    let summary = settings_to_update_summary(&document.settings);
    settings::update_settings(connection, settings_to_update(document.settings)?)?;

    Ok(ImportApplyResult {
        summary,
        warnings: preview.warnings,
    })
}

fn parse_settings_document(contents: &str) -> StorageResult<SettingsExportDocument> {
    serde_yaml::from_str::<SettingsExportDocument>(contents).map_err(StorageError::from)
}

/// ADR 0111 (#463): video transcription is retired — a bundle exported by a
/// pre-#463 install still carries the three `youtubeTranscription*` settings
/// keys. `ExportSettings` no longer declares those fields, so serde_yaml
/// already ignores them silently; this surfaces that as an explicit warning
/// instead of a silent drop, without ever erroring on the old bundle.
const RETIRED_SETTING_KEYS: &[(&str, &str)] = &[
    (
        "youtubeTranscriptionProvider",
        "youtube_transcription_provider",
    ),
    ("youtubeTranscriptionModel", "youtube_transcription_model"),
    (
        "youtubeTranscriptionTimeoutSeconds",
        "youtube_transcription_timeout_seconds",
    ),
];

fn retired_setting_warnings(contents: &str) -> Vec<String> {
    let Ok(raw) = serde_yaml::from_str::<serde_yaml::Value>(contents) else {
        return Vec::new();
    };
    let Some(settings) = raw.get("settings").and_then(|value| value.as_mapping()) else {
        return Vec::new();
    };
    RETIRED_SETTING_KEYS
        .iter()
        .filter(|(wire_key, _)| settings.contains_key(serde_yaml::Value::from(*wire_key)))
        .map(|(_, snake_key)| format!("Ignored retired setting {snake_key}"))
        .collect()
}

fn plan_settings_import(document: &SettingsExportDocument) -> ImportPreview {
    let mut errors = Vec::new();
    let warnings = Vec::new();

    if document.schema_version != SETTINGS_SCHEMA_VERSION {
        errors.push(format!(
            "Unsupported settings schema version: {}",
            document.schema_version
        ));
    }

    if let Err(error) = settings_to_update(document.settings.clone()) {
        errors.push(error.to_string());
    }

    ImportPreview {
        valid: errors.is_empty(),
        summary: settings_to_update_summary(&document.settings),
        warnings,
        errors,
    }
}

fn settings_to_update(settings: ExportSettings) -> StorageResult<SettingsUpdate> {
    if let Some(theme) = settings.theme.as_deref() {
        validate_allowed_import_setting("theme", theme, &["dark", "light", "system"])?;
    }
    if let Some(accent_palette) = settings.accent_palette.as_deref() {
        validate_allowed_import_setting(
            "accent_palette",
            accent_palette,
            &["night-neon", "midnight-horizon"],
        )?;
    }
    if let Some(locale) = settings.locale.as_deref() {
        validate_allowed_import_setting("locale", locale, &["en", "pl"])?;
    }
    if let Some(value) = settings.poll_interval_seconds {
        validate_allowed_import_setting_i64(
            "poll_interval_seconds",
            value,
            &[300, 900, 1800, 3600],
        )?;
    }
    if let Some(level) = settings.log_level.as_deref() {
        validate_allowed_import_setting(
            "log_level",
            level,
            &["off", "error", "warn", "info", "debug", "trace"],
        )?;
    }
    if let Some(value) = settings.log_max_files {
        validate_import_i64_range("log_max_files", value, 1, 20)?;
    }
    if let Some(value) = settings.log_max_file_bytes {
        validate_import_i64_range("log_max_file_bytes", value, 1_048_576, 104_857_600)?;
    }

    Ok(SettingsUpdate {
        theme: settings.theme,
        accent_palette: settings.accent_palette,
        locale: settings.locale,
        poll_interval_seconds: settings.poll_interval_seconds,
        log_level: settings.log_level,
        log_max_files: settings.log_max_files,
        log_max_file_bytes: settings.log_max_file_bytes,
        shortcut_bindings: settings.shortcut_bindings,
        // Connection-pool tuning is not part of import/export (local-only, ADR 0032).
        ..SettingsUpdate::default()
    })
}

fn settings_to_update_summary(settings: &ExportSettings) -> ImportApplySummary {
    let mut updated = 0usize;
    if settings.theme.is_some() {
        updated += 1;
    }
    if settings.accent_palette.is_some() {
        updated += 1;
    }
    if settings.locale.is_some() {
        updated += 1;
    }
    if settings.poll_interval_seconds.is_some() {
        updated += 1;
    }
    if settings.log_level.is_some() {
        updated += 1;
    }
    if settings.log_max_files.is_some() {
        updated += 1;
    }
    if settings.log_max_file_bytes.is_some() {
        updated += 1;
    }
    if settings.shortcut_bindings.is_some() {
        updated += 1;
    }
    ImportApplySummary {
        settings_updated: updated,
        ..ImportApplySummary::default()
    }
}

fn validate_allowed_import_setting(
    key: &'static str,
    value: &str,
    allowed: &[&str],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn validate_allowed_import_setting_i64(
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

fn validate_import_i64_range(
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
