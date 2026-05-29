use serde::Serialize;
use tauri::Manager;

pub mod storage;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

mod commands {
    use super::{storage, HealthResponse};

    #[tauri::command]
    pub fn health() -> HealthResponse {
        HealthResponse {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[tauri::command]
    pub fn database_status(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::DatabaseStatus, String> {
        state.database_status().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn list_companies(
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<Vec<storage::Company>, String> {
        state.list_companies().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn create_company(
        input: storage::NewCompany,
        state: tauri::State<'_, storage::AppState>,
    ) -> Result<storage::Company, String> {
        state
            .create_company(input)
            .map_err(|error| error.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let database_path = app_data_dir.join("brawler.sqlite3");
            let connection = storage::open_database(database_path)?;

            app.manage(storage::AppState::new(connection));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health,
            commands::database_status,
            commands::list_companies,
            commands::create_company
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Brawler application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn health_reports_ok() {
        let response = super::commands::health();

        assert_eq!(response.status, "ok");
        assert_eq!(response.version, "0.1.0");
    }
}
