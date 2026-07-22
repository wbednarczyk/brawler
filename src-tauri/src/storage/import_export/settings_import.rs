use super::*;

pub(super) fn preview_settings_import(contents: &str) -> StorageResult<ImportPreview> {
    let document = parse_settings_document(contents)?;
    Ok(plan_settings_import(&document))
}

pub(super) fn apply_settings_import(
    connection: &Connection,
    contents: &str,
) -> StorageResult<ImportApplyResult> {
    let document = parse_settings_document(contents)?;
    let preview = plan_settings_import(&document);
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
    if let Some(provider) = settings.youtube_transcription_provider.as_deref() {
        validate_allowed_import_setting(
            "youtube_transcription_provider",
            provider,
            &["provider_gemini"],
        )?;
    }
    if let Some(model) = settings.youtube_transcription_model.as_deref() {
        validate_allowed_import_setting(
            "youtube_transcription_model",
            model,
            &[
                "gemini-2.5-flash-lite",
                "gemini-2.5-flash",
                "gemini-3.1-flash-lite",
                "gemini-3.5-flash",
            ],
        )?;
    }
    if let Some(value) = settings.youtube_transcription_timeout_seconds {
        validate_allowed_import_setting_i64(
            "youtube_transcription_timeout_seconds",
            value,
            &[45, 90, 180, 300, 600],
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
        youtube_transcription_provider: settings.youtube_transcription_provider,
        youtube_transcription_model: settings.youtube_transcription_model,
        youtube_transcription_timeout_seconds: settings.youtube_transcription_timeout_seconds,
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
    if settings.youtube_transcription_provider.is_some() {
        updated += 1;
    }
    if settings.youtube_transcription_model.is_some() {
        updated += 1;
    }
    if settings.youtube_transcription_timeout_seconds.is_some() {
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
