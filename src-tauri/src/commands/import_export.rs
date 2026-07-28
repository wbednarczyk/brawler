use crate::{app_state, storage};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportContentsInput {
    pub contents: String,
}

#[tauri::command]
pub fn export_research_data(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ExportPayload, String> {
    state
        .export_research_data()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_research_import(
    input: ImportContentsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ImportPreview, String> {
    state
        .preview_research_import(&input.contents)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn apply_research_import(
    input: ImportContentsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ImportApplyResult, String> {
    state
        .apply_research_import(&input.contents)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn export_settings_data(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ExportPayload, String> {
    state
        .export_settings_data()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn preview_settings_import(
    input: ImportContentsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ImportPreview, String> {
    state
        .preview_settings_import(&input.contents)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn apply_settings_import(
    input: ImportContentsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::ImportApplyResult, String> {
    state
        .apply_settings_import(&input.contents)
        .map_err(|error| error.to_string())
}

/// Input for `write_export_file` — the save path chosen in the OS save dialog
/// plus the export payload. The backend enforces the extension whitelist and
/// performs the write, so the webview carries **no filesystem permission at
/// all** (issue #106; strict-permissions posture: typed commands only — this
/// replaced the unscoped `fs:allow-write-text-file` capability).
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct WriteExportFileInput {
    pub path: String,
    pub contents: String,
    /// Lower-case extension whitelist without dots, e.g. `["json"]` or
    /// `["yaml", "yml"]`. A path missing an allowed extension gets
    /// `default_extension` appended (the dialog cannot guarantee one — the
    /// user may type a bare name).
    pub allowed_extensions: Vec<String>,
    pub default_extension: String,
}

/// Pure path policy for the export write: absolute path required (the OS save
/// dialog always returns one — anything else is not dialog-mediated), and the
/// final path always carries an allowed extension.
pub(crate) fn resolve_export_path(
    input: &WriteExportFileInput,
) -> Result<std::path::PathBuf, String> {
    let raw = input.path.trim();
    if raw.is_empty() {
        return Err("invalid_export_path: empty path".to_owned());
    }
    if !std::path::Path::new(raw).is_absolute() {
        return Err(format!("invalid_export_path: not an absolute path: {raw}"));
    }
    if input.allowed_extensions.is_empty() {
        return Err("invalid_export_path: empty extension whitelist".to_owned());
    }
    let lower = raw.to_lowercase();
    let has_allowed = input.allowed_extensions.iter().any(|extension| {
        lower.ends_with(&format!(
            ".{}",
            extension.trim_start_matches('.').to_lowercase()
        ))
    });
    if has_allowed {
        Ok(std::path::PathBuf::from(raw))
    } else {
        let default = input.default_extension.trim_start_matches('.');
        Ok(std::path::PathBuf::from(format!("{raw}.{default}")))
    }
}

/// Write an export payload to the dialog-selected path. Offloaded — file IO
/// never runs on the UI thread. Returns the final path (extension-enforced).
#[tauri::command]
pub async fn write_export_file(input: WriteExportFileInput) -> Result<String, String> {
    let target = resolve_export_path(&input)?;
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::write(&target, input.contents.as_bytes())
            .map_err(|error| format!("export_write_failed: {error}"))?;
        Ok(target.to_string_lossy().into_owned())
    })
    .await
    .map_err(|error| format!("export_write_failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(path: &str, allowed: &[&str], default: &str) -> WriteExportFileInput {
        WriteExportFileInput {
            path: path.to_owned(),
            contents: "{}".to_owned(),
            allowed_extensions: allowed.iter().map(|s| (*s).to_owned()).collect(),
            default_extension: default.to_owned(),
        }
    }

    #[test]
    fn keeps_an_allowed_extension_case_insensitively() {
        let resolved = resolve_export_path(&input("/tmp/export.JSON", &["json"], "json")).unwrap();
        assert_eq!(resolved, std::path::PathBuf::from("/tmp/export.JSON"));
    }

    #[test]
    fn appends_the_default_extension_when_missing() {
        let resolved =
            resolve_export_path(&input("/tmp/export", &["yaml", "yml"], "yaml")).unwrap();
        assert_eq!(resolved, std::path::PathBuf::from("/tmp/export.yaml"));
    }

    #[test]
    fn rejects_a_relative_or_empty_path() {
        assert!(
            resolve_export_path(&input("export.json", &["json"], "json"))
                .unwrap_err()
                .starts_with("invalid_export_path")
        );
        assert!(resolve_export_path(&input("  ", &["json"], "json"))
            .unwrap_err()
            .starts_with("invalid_export_path"));
    }

    #[test]
    fn writes_the_contents_to_disk() {
        let dir = std::env::temp_dir().join(format!("brawler-export-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("payload");
        let mut request = input(path.to_str().unwrap(), &["json"], "json");
        request.contents = "{\"ok\":true}".to_owned();
        let resolved = resolve_export_path(&request).unwrap();
        std::fs::write(&resolved, request.contents.as_bytes()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("payload.json")).unwrap(),
            "{\"ok\":true}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
