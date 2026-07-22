use crate::{app_state, storage};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperModeUnlockInput {
    passphrase: String,
}

#[tauri::command]
pub fn get_settings(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::UserSettings, String> {
    state.get_settings().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_settings(
    input: storage::SettingsUpdate,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::UserSettings, String> {
    state
        .update_settings(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn disable_developer_mode(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::UserSettings, String> {
    state
        .set_developer_mode_enabled(false)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn unlock_developer_mode(
    input: DeveloperModeUnlockInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::UserSettings, String> {
    if !developer_unlock_code_matches(&input.passphrase) {
        return Err("Developer mode unlock failed.".to_owned());
    }

    state
        .set_developer_mode_enabled(true)
        .map_err(|error| error.to_string())
}

fn developer_unlock_code_matches(passphrase: &str) -> bool {
    developer_unlock_code_matches_value(
        std::env::var("BRAWLER_DEVELOPER_UNLOCK_CODE")
            .ok()
            .as_deref(),
        passphrase,
    )
}

fn developer_unlock_code_matches_value(expected: Option<&str>, passphrase: &str) -> bool {
    expected
        .map(|expected| {
            let expected = expected.trim();
            !expected.is_empty() && expected == passphrase.trim()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn validates_developer_unlock_code_values() {
        assert!(super::developer_unlock_code_matches_value(
            Some("author-passphrase"),
            "author-passphrase"
        ));
        assert!(super::developer_unlock_code_matches_value(
            Some(" author-passphrase "),
            "author-passphrase"
        ));
        assert!(!super::developer_unlock_code_matches_value(
            Some("author-passphrase"),
            "wrong"
        ));
        assert!(!super::developer_unlock_code_matches_value(Some(""), ""));
        assert!(!super::developer_unlock_code_matches_value(
            None, "anything"
        ));
    }
}
