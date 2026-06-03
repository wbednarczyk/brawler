use serde::{Deserialize, Serialize};

use crate::{app_state, storage};

const DEFAULT_DIAGNOSTIC_EVENT_LIMIT: i64 = 200;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDiagnosticEventsInput {
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearDiagnosticEventsResult {
    pub events_deleted: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSummary {
    pub summary: String,
    pub event_count: usize,
}

#[tauri::command]
pub fn list_diagnostic_events(
    input: Option<ListDiagnosticEventsInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::DiagnosticEvent>, String> {
    ensure_developer_mode_enabled(&state)?;

    let limit = input
        .and_then(|input| input.limit)
        .unwrap_or(DEFAULT_DIAGNOSTIC_EVENT_LIMIT);

    state
        .list_diagnostic_events(limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_diagnostic_events(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<ClearDiagnosticEventsResult, String> {
    ensure_developer_mode_enabled(&state)?;

    state
        .clear_diagnostic_events()
        .map(|events_deleted| ClearDiagnosticEventsResult { events_deleted })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_diagnostic_summary(
    input: Option<ListDiagnosticEventsInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<DiagnosticSummary, String> {
    ensure_developer_mode_enabled(&state)?;

    let limit = input
        .and_then(|input| input.limit)
        .unwrap_or(DEFAULT_DIAGNOSTIC_EVENT_LIMIT);
    let events = state
        .list_diagnostic_events(limit)
        .map_err(|error| error.to_string())?;

    Ok(DiagnosticSummary {
        summary: format_diagnostic_summary(&events),
        event_count: events.len(),
    })
}

fn ensure_developer_mode_enabled(
    state: &tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    let settings = state.get_settings().map_err(|error| error.to_string())?;
    if settings.developer_mode {
        Ok(())
    } else {
        Err("Developer mode is not active.".to_owned())
    }
}

fn format_diagnostic_summary(events: &[storage::DiagnosticEvent]) -> String {
    if events.is_empty() {
        return "Diagnostic summary\nNo diagnostic events recorded.".to_owned();
    }

    let mut lines = vec![
        "Diagnostic summary".to_owned(),
        format!("Events included: {}", events.len()),
    ];

    for event in events {
        let scope = event
            .scope
            .as_ref()
            .map(|scope| {
                scope
                    .id
                    .as_ref()
                    .map(|id| format!("{}:{id}", scope.scope_type))
                    .unwrap_or_else(|| scope.scope_type.clone())
            })
            .unwrap_or_else(|| "global".to_owned());
        lines.push(format!(
            "{} | {} | {} | {} | {} | {}",
            event.occurred_at, event.severity, event.module, scope, event.stage, event.message
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn formats_redacted_diagnostic_summary_without_raw_json() {
        let events = vec![storage::DiagnosticEvent {
            id: "diagnostic_event_01".to_owned(),
            occurred_at: "2026-06-03T10:15:00.000Z".to_owned(),
            module: "ai_analysis".to_owned(),
            scope: Some(storage::DiagnosticScope {
                scope_type: "ai_analysis_job".to_owned(),
                id: Some("analysis_job_01".to_owned()),
            }),
            stage: "request_sent".to_owned(),
            severity: "info".to_owned(),
            message: "Gemini analysis request sent.".to_owned(),
            metadata: json!({
                "apiKey": "[redacted]",
                "model": "gemini-2.5-flash"
            }),
            created_at: "2026-06-03T10:15:00.000Z".to_owned(),
        }];

        let summary = format_diagnostic_summary(&events);

        assert!(summary.contains("Diagnostic summary"));
        assert!(summary.contains("Events included: 1"));
        assert!(summary.contains("ai_analysis_job:analysis_job_01"));
        assert!(summary.contains("request_sent"));
        assert!(!summary.contains("apiKey"));
        assert!(!summary.contains("gemini-2.5-flash"));
        assert!(!summary.contains('{'));
    }
}
