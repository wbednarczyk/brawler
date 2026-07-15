//! Alert-rule CRUD + attention-event list/seen/dismiss commands (ADR 0068,
//! plan v0.54-attention-routing §T3). Thin `#[tauri::command]` wrappers over
//! `AttentionStore` (via `AppState::attention()`); all meaningful behavior +
//! validation lives in `storage::attention`. Evaluation hooks are inline in the
//! evidence-producing jobs (T2), not commands.

use serde::Deserialize;

use crate::{app_state, storage};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuleActionInput {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAlertRuleEnabledInput {
    id: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionEventActionInput {
    id: String,
}

/// Create a user-owned alert rule. Trigger-specific invariants (signal category
/// present, `price_min ≤ price_max`, valid scope) are enforced by the store.
#[tauri::command]
pub fn create_alert_rule(
    input: storage::NewAlertRule,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::AlertRule, String> {
    state
        .attention()
        .create_alert_rule(input)
        .map_err(|error| error.to_string())
}

/// All alert rules, oldest first (stable `created_at, id` order).
#[tauri::command]
pub fn list_alert_rules(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::AlertRule>, String> {
    state
        .attention()
        .list_alert_rules()
        .map_err(|error| error.to_string())
}

/// Update mutable fields of a rule; `null` fields are left unchanged. Re-checks
/// the trigger-type invariants against the merged rule.
#[tauri::command]
pub fn update_alert_rule(
    input: storage::AlertRuleUpdate,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::AlertRule, String> {
    state
        .attention()
        .update_alert_rule(input)
        .map_err(|error| error.to_string())
}

/// Enable / disable a rule (disabled rules never fire). Returns the updated row.
#[tauri::command]
pub fn set_alert_rule_enabled(
    input: SetAlertRuleEnabledInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::AlertRule, String> {
    state
        .attention()
        .set_alert_rule_enabled(&input.id, input.enabled)
        .map_err(|error| error.to_string())
}

/// Delete a rule (its attention events CASCADE with it). Idempotent.
#[tauri::command]
pub fn delete_alert_rule(
    input: AlertRuleActionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .attention()
        .delete_alert_rule(&input.id)
        .map_err(|error| error.to_string())
}

/// Fired attention events (newest first), optionally filtered by company and
/// whether to include dismissed events (default: only non-dismissed).
#[tauri::command]
pub fn list_attention_events(
    input: Option<storage::AttentionEventListInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::AttentionEvent>, String> {
    state
        .attention()
        .list_attention_events(input.unwrap_or_default())
        .map_err(|error| error.to_string())
}

/// Mark an event seen (read but not dismissed). Idempotent.
#[tauri::command]
pub fn mark_attention_event_seen(
    input: AttentionEventActionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .attention()
        .mark_attention_event_seen(&input.id)
        .map_err(|error| error.to_string())
}

/// Dismiss an event (also marks it seen). Dismissed events drop out of the
/// default list. Idempotent.
#[tauri::command]
pub fn dismiss_attention_event(
    input: AttentionEventActionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .attention()
        .dismiss_attention_event(&input.id)
        .map_err(|error| error.to_string())
}
