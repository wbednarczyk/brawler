use serde::Serialize;

pub mod storage;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

mod commands {
    use super::HealthResponse;

    #[tauri::command]
    pub fn health() -> HealthResponse {
        HealthResponse {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::health])
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
