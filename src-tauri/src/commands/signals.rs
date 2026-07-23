use serde::Deserialize;

use crate::commands::error::{CommandError, CommandErrorCode};
use crate::storage::ClassifyFilingOutcome;
use crate::{app_state, storage};

#[tauri::command]
pub fn list_company_signals(
    input: storage::CompanySignalListInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::CompanySignal>, String> {
    state
        .list_company_signals(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn confirm_company_signal(
    input: storage::CompanySignalActionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CompanySignal, String> {
    state
        .confirm_company_signal(&input.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reject_company_signal(
    input: storage::CompanySignalActionInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .reject_company_signal(&input.id)
        .map_err(|error| error.to_string())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmDerivedEventInput {
    event_id: String,
    /// `"confirm"` places the proposed event on the calendar; `"reject"` discards it.
    action: String,
}

/// Confirm or reject a `proposed` derived calendar event (dividend / general-meeting date).
/// A guessed-date event is never auto-confirmed (ADR 0036).
#[tauri::command]
pub fn confirm_derived_event(
    input: ConfirmDerivedEventInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    let confirm = match input.action.as_str() {
        "confirm" => true,
        "reject" => false,
        other => return Err(format!("unknown derived-event action: {other}")),
    };
    state
        .confirm_derived_event(&input.event_id, confirm)
        .map_err(|error| error.to_string())
}

// ---- Unclassified-filings triage (v0.60 M4, ADR 0088 dec. 4) ----------------
//
// Headless / MCP-first surface (like `set_qualitative_verdicts`): no new UI entry
// point this slice — surfacing the bucket in Today/Inbox is future scope. Both
// commands are exposed over the MCP registry (`list_unclassified_filings` read,
// `classify_filing` act).

/// Default page size for the unclassified-filings triage read (max 200 is
/// enforced by the storage layer).
const DEFAULT_UNCLASSIFIED_LIMIT: i64 = 50;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListUnclassifiedFilingsInput {
    /// Optional internal company id to scope the bucket to one company.
    #[serde(default)]
    pub company_id: Option<String>,
    /// Maximum filings to return (default 50, max 200).
    #[serde(default)]
    pub limit: Option<i64>,
}

/// List official filings the deterministic ESPI rule classifier could not place
/// (no `company_signals` row) — the explicit unclassified bucket (ADR 0088
/// dec. 4). Async + `spawn_blocking` (DoD §C).
#[tauri::command]
pub async fn list_unclassified_filings(
    input: ListUnclassifiedFilingsInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::UnclassifiedFiling>, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let limit = input.limit.unwrap_or(DEFAULT_UNCLASSIFIED_LIMIT);
        state
            .list_unclassified_filings(input.company_id.as_deref(), limit)
            .map_err(CommandError::from)
    })
    .await
    .map_err(|error| {
        CommandError::new(CommandErrorCode::Internal, format!("task failed: {error}"))
    })?
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClassifyFilingInput {
    /// The unclassified official filing to classify (`feed_items.id`) — this is
    /// also the created signal's evidence anchor.
    pub feed_item_id: String,
    /// A signal category key from the seeded taxonomy (e.g. `dividend`,
    /// `significant_contract`). An unknown key is rejected.
    pub category: String,
}

/// Shared logic for the typed command and the MCP act handler (single source of
/// truth). Validates the category against the real seeded taxonomy, that the
/// filing is an official report matched to a company, and that it is not already
/// classified; then creates a `confirmed` signal anchored to the feed item.
pub fn classify_filing_inner(
    state: &app_state::AppState,
    input: ClassifyFilingInput,
) -> Result<storage::CompanySignal, CommandError> {
    match state
        .classify_filing_outcome(&input.feed_item_id, &input.category)
        .map_err(CommandError::from)?
    {
        ClassifyFilingOutcome::Created(signal) => Ok(*signal),
        ClassifyFilingOutcome::UnknownCategory => Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            format!(
                "unknown signal category '{}' — use a key from the seeded taxonomy",
                input.category
            ),
        )),
        ClassifyFilingOutcome::FeedItemNotFound => Err(CommandError::new(
            CommandErrorCode::NotFound,
            format!("no feed item with id '{}'", input.feed_item_id),
        )),
        ClassifyFilingOutcome::NotAnOfficialFiling => Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            "feed item is not an official filing — only official reports can be classified",
        )),
        ClassifyFilingOutcome::NotMatchedToCompany => Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            "official filing is not matched to any company — cannot anchor a signal",
        )),
        ClassifyFilingOutcome::AlreadyClassified => Err(CommandError::new(
            CommandErrorCode::Conflict,
            "filing already has a signal — it is no longer unclassified",
        )),
    }
}

/// Classify one unclassified official filing into a `confirmed` signal (ADR 0088
/// dec. 4). The mandatory `feedItemId` is the evidence anchor. Async +
/// `spawn_blocking` (DoD §C).
#[tauri::command]
pub async fn classify_filing(
    input: ClassifyFilingInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CompanySignal, CommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || classify_filing_inner(&state, input))
        .await
        .map_err(|error| {
            CommandError::new(CommandErrorCode::Internal, format!("task failed: {error}"))
        })?
}
