use serde::Deserialize;
use serde_json::json;

use crate::{
    app_state,
    licensing::{
        self, LicenseStatus, LicenseStatusKind, LicenseTokenStore, OsKeychainLicenseTokenStore,
    },
    storage,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitLicenseKeyInput {
    license_key: String,
}

#[tauri::command]
pub fn get_license_status(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<LicenseStatus, String> {
    let store = OsKeychainLicenseTokenStore;
    let status = read_license_status(&store, &state);
    persist_status_if_useful(&state, &status)?;
    record_license_status(&state, "checked", &status);

    Ok(status)
}

#[tauri::command]
pub fn submit_license_key(
    input: SubmitLicenseKeyInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<LicenseStatus, String> {
    let token = input.license_key.trim();
    let status = licensing::evaluate_local_license_token(token);
    if !status.can_use_app {
        record_license_status(&state, "rejected", &status);
        return Ok(status);
    }

    let store = OsKeychainLicenseTokenStore;
    store.save_token(token).map_err(|error| error.to_string())?;
    persist_status_if_useful(&state, &status)?;
    record_license_status(&state, "stored", &status);

    Ok(status)
}

#[tauri::command]
pub fn clear_license_key(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<LicenseStatus, String> {
    let store = OsKeychainLicenseTokenStore;
    store.clear_token().map_err(|error| error.to_string())?;
    state
        .clear_license_metadata()
        .map_err(|error| error.to_string())?;

    let status = LicenseStatus::missing();
    record_license_status(&state, "cleared", &status);

    Ok(status)
}

/// Whether the current local license permits using the app. Used by the Rust-side
/// scheduler (ADR 0055) to gate background refresh exactly as the UI gates it
/// (`licenseStatus.canUseApp`), so moving the timer to Rust does not bypass the
/// license check.
pub(crate) fn current_license_can_use_app(state: &app_state::AppState) -> bool {
    let store = OsKeychainLicenseTokenStore;
    read_license_status(&store, state).can_use_app
}

fn read_license_status(
    store: &dyn LicenseTokenStore,
    state: &app_state::AppState,
) -> LicenseStatus {
    match store.read_token() {
        Ok(Some(token)) => licensing::evaluate_local_license_token(&token),
        Ok(None) => LicenseStatus::missing(),
        Err(error) => {
            let status = LicenseStatus::storage_error(error.to_string());
            record_license_status(state, "failed", &status);
            status
        }
    }
}

fn persist_status_if_useful(
    state: &app_state::AppState,
    status: &LicenseStatus,
) -> Result<(), String> {
    if matches!(
        status.status,
        LicenseStatusKind::Missing | LicenseStatusKind::StorageError
    ) {
        return Ok(());
    }

    state
        .upsert_license_metadata(metadata_update_from_status(status))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn metadata_update_from_status(status: &LicenseStatus) -> storage::LicenseMetadataUpdate {
    let license = status.license.as_ref();

    storage::LicenseMetadataUpdate {
        status: status.status.as_str().to_owned(),
        reason: status.reason.clone(),
        license_id: license.map(|license| license.license_id.clone()),
        holder: license.map(|license| license.holder.clone()),
        channel: license.map(|license| license.channel.clone()),
        edition: license.map(|license| license.edition.clone()),
        features: license
            .map(|license| license.features.clone())
            .unwrap_or_default(),
        issued_at: license.map(|license| license.issued_at.clone()),
        expires_at: license.map(|license| license.expires_at.clone()),
        app_version_range: license.map(|license| license.app_version_range.clone()),
        key_id: license.map(|license| license.key_id.clone()),
    }
}

fn record_license_status(state: &app_state::AppState, stage: &str, status: &LicenseStatus) {
    let status_label = status.status.as_str();
    let license_id = status
        .license
        .as_ref()
        .map(|license| license.license_id.as_str())
        .unwrap_or("none");

    if status.can_use_app {
        log::info!(
            "module=licensing stage={} status={} canUseApp=true licenseId={} channel={} edition={}",
            stage,
            status_label,
            license_id,
            status
                .license
                .as_ref()
                .map(|license| license.channel.as_str())
                .unwrap_or("unknown"),
            status
                .license
                .as_ref()
                .map(|license| license.edition.as_str())
                .unwrap_or("unknown")
        );
    } else {
        log::warn!(
            "module=licensing stage={} status={} canUseApp=false licenseId={}",
            stage,
            status_label,
            license_id
        );
    }

    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "licensing".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "license".to_owned(),
            id: status
                .license
                .as_ref()
                .map(|license| license.license_id.clone()),
        }),
        stage: stage.to_owned(),
        severity: if status.can_use_app {
            "info"
        } else {
            "warning"
        }
        .to_owned(),
        message: "License status evaluated.".to_owned(),
        metadata: Some(json!({
            "status": status_label,
            "canUseApp": status.can_use_app,
            "licenseId": status.license.as_ref().map(|license| license.license_id.as_str()),
            "channel": status.license.as_ref().map(|license| license.channel.as_str()),
            "edition": status.license.as_ref().map(|license| license.edition.as_str()),
            "hasReason": status.reason.is_some()
        })),
    });

    state.increment_runtime_counter(
        "brawler_license_operations_total",
        &[("module", "licensing"), ("status", status_label)],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_update_does_not_include_raw_token() {
        let status = LicenseStatus {
            status: LicenseStatusKind::Valid,
            can_use_app: true,
            reason: None,
            license: Some(licensing::LicenseDisplayMetadata {
                license_id: "lic_test".to_owned(),
                holder: "Friend".to_owned(),
                channel: "friend_test".to_owned(),
                edition: "friend".to_owned(),
                features: vec!["core".to_owned()],
                issued_at: "2026-01-01T00:00:00Z".to_owned(),
                expires_at: "2027-01-01T00:00:00Z".to_owned(),
                app_version_range: "*".to_owned(),
                key_id: "test".to_owned(),
            }),
            checked_at: "2026-06-04T12:00:00Z".to_owned(),
        };

        let metadata = metadata_update_from_status(&status);

        assert_eq!(metadata.status, "valid");
        assert_eq!(metadata.license_id.as_deref(), Some("lic_test"));
        assert_eq!(metadata.features, vec!["core"]);
    }
}
