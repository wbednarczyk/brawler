use super::*;
use serde_json::json;

fn sample_event() -> NewDiagnosticEvent {
    NewDiagnosticEvent {
        occurred_at: None,
        module: "ai_analysis".to_owned(),
        scope: Some(DiagnosticScope {
            scope_type: "ai_analysis_job".to_owned(),
            id: Some("analysis_job_01".to_owned()),
        }),
        stage: "request_sent".to_owned(),
        severity: "info".to_owned(),
        message: "Gemini analysis request sent.".to_owned(),
        metadata: Some(json!({
            "providerId": "provider_gemini",
            "model": "gemini-2.5-flash",
            "promptVersion": "m13.source_grounded.v1",
            "apiKey": "secret-key",
            "prompt": "full prompt",
            "nested": {
                "rawResponse": "raw response"
            }
        })),
    }
}

#[test]
fn skips_diagnostic_recording_when_developer_mode_is_disabled() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let event = state
        .record_diagnostic_event(sample_event())
        .expect("diagnostic event should be accepted");

    assert!(event.is_none());
    assert!(state
        .list_diagnostic_events(10)
        .expect("diagnostic events should list")
        .is_empty());
}

#[test]
fn records_redacted_diagnostic_event_when_developer_mode_is_enabled() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode should enable");

    let event = state
        .record_diagnostic_event(sample_event())
        .expect("diagnostic event should record")
        .expect("event should be stored");

    assert!(event.id.starts_with("diagnostic_event_"));
    assert_eq!(event.module, "ai_analysis");
    assert_eq!(event.stage, "request_sent");
    assert_eq!(event.severity, "info");
    assert_eq!(event.message, "Gemini analysis request sent.");
    assert_eq!(
        event.scope.as_ref().map(|scope| scope.scope_type.as_str()),
        Some("ai_analysis_job")
    );
    assert_eq!(
        event.scope.as_ref().and_then(|scope| scope.id.as_deref()),
        Some("analysis_job_01")
    );
    assert_eq!(event.metadata["providerId"], "provider_gemini");
    assert_eq!(event.metadata["promptVersion"], "m13.source_grounded.v1");
    assert_eq!(event.metadata["apiKey"], "[redacted]");
    assert_eq!(event.metadata["prompt"], "[redacted]");
    assert_eq!(event.metadata["nested"]["rawResponse"], "[redacted]");
}

#[test]
fn clears_diagnostic_events_without_disabling_developer_mode() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode should enable");
    state
        .record_diagnostic_event(sample_event())
        .expect("diagnostic event should record");

    let deleted = state
        .clear_diagnostic_events()
        .expect("diagnostic events should clear");

    assert_eq!(deleted, 1);
    assert!(state
        .list_diagnostic_events(10)
        .expect("diagnostic events should list")
        .is_empty());
    assert!(
        state
            .get_settings()
            .expect("settings should load")
            .developer_mode
    );
}

#[test]
fn trims_diagnostic_events_to_retention_limit() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode should enable");

    for index in 0..1_005 {
        let mut event = sample_event();
        event.message = format!("Diagnostic event {index}");
        state
            .record_diagnostic_event(event)
            .expect("diagnostic event should record");
    }

    let events = state
        .list_diagnostic_events(1_500)
        .expect("diagnostic events should list");

    assert_eq!(events.len(), 1_000);
}

#[test]
fn trims_diagnostic_events_older_than_retention_window() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode should enable");

    let mut old_event = sample_event();
    old_event.occurred_at = Some("2000-01-01T00:00:00.000Z".to_owned());
    old_event.message = "Old diagnostic event".to_owned();
    state
        .record_diagnostic_event(old_event)
        .expect("old diagnostic event should record before retention trim");

    state
        .record_diagnostic_event(sample_event())
        .expect("current diagnostic event should record");

    let events = state
        .list_diagnostic_events(10)
        .expect("diagnostic events should list");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].message, "Gemini analysis request sent.");
}

#[test]
fn rejects_invalid_diagnostic_values() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode should enable");
    let mut event = sample_event();
    event.severity = "critical".to_owned();

    let error = state
        .record_diagnostic_event(event)
        .expect_err("invalid diagnostic value should fail");

    assert!(matches!(
        error,
        StorageError::InvalidDiagnosticValue {
            key: "severity",
            ..
        }
    ));
}

/// Integrity events bypass the developer-mode gate (sol review round 3/4):
/// they carry data-loss evidence (a swallowed Layer 1 capture failure), and a
/// settings flag must never decide whether the owner learns about lost data.
/// The ordinary diagnostic path above stays gated — this is the ONE ungated
/// door, for integrity only.
#[test]
fn integrity_events_persist_with_developer_mode_off() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    // Default profile — developer mode NOT enabled.
    let event = state
        .diagnostics()
        .record_integrity_event(sample_event())
        .expect("integrity event should record")
        .expect("event should be stored despite developer mode being off");
    assert!(event.id.starts_with("diagnostic_event_"));

    let listed = state
        .list_diagnostic_events(10)
        .expect("diagnostic events should list");
    assert_eq!(listed.len(), 1, "the integrity event is durably visible");
}
