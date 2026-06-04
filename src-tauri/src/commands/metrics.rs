use tauri::Manager;

use crate::{app_state, storage};

#[tauri::command]
pub fn get_local_metrics_snapshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::LocalMetricsSnapshot, String> {
    ensure_developer_mode_enabled(&state)?;

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;

    state
        .local_metrics_snapshot(&app_data_dir)
        .map_err(|error| error.to_string())
}

fn ensure_developer_mode_enabled(state: &app_state::AppState) -> Result<(), String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    if settings.developer_mode {
        Ok(())
    } else {
        Err("Developer mode is not active.".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_commands_require_developer_mode() {
        let connection = storage::open_in_memory_database().expect("database should initialize");
        let state = app_state::AppState::new(connection);

        assert!(ensure_developer_mode_enabled(&state).is_err());

        state
            .set_developer_mode_enabled(true)
            .expect("developer mode should enable");
        assert!(ensure_developer_mode_enabled(&state).is_ok());
    }
}
