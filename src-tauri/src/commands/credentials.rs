use serde::Deserialize;
use serde_json::json;

use crate::{app_state, providers::credentials, storage};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialInput {
    provider_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetProviderApiKeyInput {
    provider_id: String,
    api_key: String,
}

#[tauri::command]
pub fn get_provider_credential_status(
    input: ProviderCredentialInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<credentials::CredentialStatus, String> {
    let status = credentials::provider_credential_status(&input.provider_id)
        .ok_or_else(|| format!("Unknown AI provider: {}", input.provider_id))?;
    log::info!(
        "module=credentials stage=credential_checked providerId={} secretKind={} configured={} storage={} devFallbackAvailable={}",
        status.provider_id,
        status.secret_kind,
        status.configured,
        status.storage,
        status.dev_fallback_available
    );
    record_credential_diagnostic(
        &state,
        "credential_checked",
        "info",
        "Credential status checked.",
        &status,
    );
    record_credential_metric(&state, "checked", &status);

    Ok(status)
}

#[tauri::command]
pub fn set_provider_api_key(
    input: SetProviderApiKeyInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<credentials::CredentialStatus, String> {
    match credentials::set_provider_api_key(&input.provider_id, &input.api_key) {
        Ok(status) => {
            log::info!(
                "module=credentials stage=stored providerId={} secretKind={} configured={} storage={}",
                status.provider_id,
                status.secret_kind,
                status.configured,
                status.storage
            );
            record_credential_diagnostic(&state, "stored", "info", "Credential saved.", &status);
            record_credential_metric(&state, "stored", &status);
            Ok(status)
        }
        Err(error) => {
            log::error!(
                "module=credentials stage=failed providerId={} secretKind=api_key errorClass=credential_save_error error={}",
                input.provider_id,
                error
            );
            record_credential_error_diagnostic(
                &state,
                &input.provider_id,
                "failed",
                "Credential save failed.",
                "credential_save_error",
            );
            state.increment_runtime_counter(
                "brawler_credential_operations_total",
                &[
                    ("provider_id", input.provider_id.as_str()),
                    ("status", "failed"),
                ],
            );
            Err(error.to_string())
        }
    }
}

#[tauri::command]
pub fn clear_provider_api_key(
    input: ProviderCredentialInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<credentials::CredentialStatus, String> {
    match credentials::clear_provider_api_key(&input.provider_id) {
        Ok(status) => {
            log::info!(
                "module=credentials stage=cleared providerId={} secretKind={} configured={} storage={}",
                status.provider_id,
                status.secret_kind,
                status.configured,
                status.storage
            );
            record_credential_diagnostic(&state, "cleared", "info", "Credential cleared.", &status);
            record_credential_metric(&state, "cleared", &status);
            Ok(status)
        }
        Err(error) => {
            log::error!(
                "module=credentials stage=failed providerId={} secretKind=api_key errorClass=credential_clear_error error={}",
                input.provider_id,
                error
            );
            record_credential_error_diagnostic(
                &state,
                &input.provider_id,
                "failed",
                "Credential clear failed.",
                "credential_clear_error",
            );
            state.increment_runtime_counter(
                "brawler_credential_operations_total",
                &[
                    ("provider_id", input.provider_id.as_str()),
                    ("status", "failed"),
                ],
            );
            Err(error.to_string())
        }
    }
}

fn record_credential_metric(
    state: &app_state::AppState,
    stage: &str,
    status: &credentials::CredentialStatus,
) {
    let outcome = if status.error.is_some() {
        "error"
    } else if status.configured {
        "configured"
    } else {
        "not_configured"
    };
    state.increment_runtime_counter(
        "brawler_credential_checks_total",
        &[("provider_id", status.provider_id), ("status", outcome)],
    );
    state.increment_runtime_counter(
        "brawler_credential_operations_total",
        &[("provider_id", status.provider_id), ("status", stage)],
    );
}

fn record_credential_diagnostic(
    state: &app_state::AppState,
    stage: &str,
    severity: &str,
    message: &str,
    status: &credentials::CredentialStatus,
) {
    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "credentials".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "credential".to_owned(),
            id: Some(format!("{}:{}", status.provider_id, status.secret_kind)),
        }),
        stage: stage.to_owned(),
        severity: severity.to_owned(),
        message: message.to_owned(),
        metadata: Some(json!({
            "providerId": status.provider_id,
            "secretKind": status.secret_kind,
            "configured": status.configured,
            "storage": status.storage,
            "devFallbackAvailable": status.dev_fallback_available,
            "hasBackendError": status.error.is_some()
        })),
    });
}

fn record_credential_error_diagnostic(
    state: &app_state::AppState,
    provider_id: &str,
    stage: &str,
    message: &str,
    error_class: &str,
) {
    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "credentials".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "credential".to_owned(),
            id: Some(format!("{provider_id}:api_key")),
        }),
        stage: stage.to_owned(),
        severity: "error".to_owned(),
        message: message.to_owned(),
        metadata: Some(json!({
            "providerId": provider_id,
            "secretKind": "api_key",
            "errorClass": error_class
        })),
    });
}
