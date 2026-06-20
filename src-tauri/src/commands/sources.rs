use serde::Deserialize;

use crate::{app_state, jobs, storage};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSourcesInput {
    trigger: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSourceInput {
    adapter_id: String,
    #[cfg_attr(feature = "ts-export", ts(type = "\"manual\" | \"scheduler\""))]
    trigger: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(optional, type = "string | null"))]
    date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "RefreshCompanyRegistryIfStaleInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRegistryIfStaleInput {
    #[cfg_attr(feature = "ts-export", ts(type = "\"scheduler\""))]
    trigger: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(type = "number"))]
    stale_after_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ListSourceAdaptersInput {
    include_developer_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct SetSourceEnabledInput {
    adapter_id: String,
    enabled: bool,
}

#[tauri::command]
pub fn list_source_adapters(
    input: Option<ListSourceAdaptersInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::SourceAdapter>, String> {
    state
        .list_source_adapters_with_developer(
            input
                .and_then(|input| input.include_developer_only)
                .unwrap_or(false),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_source_adapter_enabled(
    input: SetSourceEnabledInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::SourceAdapter, String> {
    state
        .set_source_adapter_enabled(&input.adapter_id, input.enabled)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_company_registry_entries(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::CompanyRegistryEntry>, String> {
    state
        .list_company_registry_entries()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_unmatched_source_items(
    adapter_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::UnmatchedSourceItem>, String> {
    state
        .list_unmatched_source_items(&adapter_id)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyBackfillInput {
    company_id: String,
}

/// Run an on-track history backfill for one tracked company (ADR 0036). Long-running and
/// throttled; progress is readable via `get_backfill_progress` while it runs.
#[tauri::command]
pub async fn backfill_company_history(
    input: CompanyBackfillInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::BackfillProgress, String> {
    let state = state.inner().clone();
    jobs::scheduler::run_blocking_task(move || {
        Ok(jobs::backfill::backfill_company_history(
            &state,
            &input.company_id,
        ))
    })
    .await
}

/// Read the latest backfill progress/diagnostics for a company, if a run has been recorded.
#[tauri::command]
pub fn get_backfill_progress(
    input: CompanyBackfillInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<storage::BackfillProgress>, String> {
    Ok(state.get_backfill_progress(&input.company_id))
}

#[tauri::command]
pub async fn refresh_sources(
    input: Option<RefreshSourcesInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::SourceIngestionResult, String> {
    let state = state.inner().clone();
    jobs::scheduler::run_blocking_task(move || {
        let trigger = refresh_trigger(input.and_then(|input| input.trigger));
        jobs::source_refresh::refresh_sources_for_trigger(&state, &trigger)
    })
    .await
}

#[tauri::command]
pub async fn refresh_source(
    input: RefreshSourceInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::SourceIngestionResult, String> {
    let state = state.inner().clone();
    let RefreshSourceInput {
        adapter_id,
        trigger,
        date,
    } = input;

    jobs::scheduler::run_blocking_task(move || {
        let trigger = refresh_trigger(trigger);
        jobs::source_refresh::refresh_source_for_trigger(
            &state,
            &adapter_id,
            &trigger,
            date.as_deref(),
        )
    })
    .await
}

#[tauri::command]
pub async fn refresh_gpw_company_registry(
    input: Option<RefreshSourcesInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::CompanyRegistryRefreshResult, String> {
    let state = state.inner().clone();

    jobs::scheduler::run_blocking_task(move || {
        let trigger = input
            .and_then(|input| input.trigger)
            .filter(|trigger| trigger == "manual" || trigger == "scheduler" || trigger == "lookup")
            .unwrap_or_else(|| "manual".to_owned());

        jobs::source_refresh::refresh_company_directories_for_trigger(&state, &trigger)
    })
    .await
}

#[tauri::command]
pub async fn refresh_gpw_company_registry_if_stale(
    input: Option<RefreshRegistryIfStaleInput>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<storage::CompanyRegistryRefreshResult>, String> {
    let state = state.inner().clone();

    jobs::scheduler::run_blocking_task(move || {
        let trigger = input
            .as_ref()
            .and_then(|input| input.trigger.clone())
            .filter(|trigger| trigger == "scheduler")
            .unwrap_or_else(|| "scheduler".to_owned());
        let stale_after_seconds = input
            .and_then(|input| input.stale_after_seconds)
            .unwrap_or(86_400);

        if !state
            .company_directories_are_stale(stale_after_seconds)
            .map_err(|error| error.to_string())?
        {
            jobs::source_refresh::record_scheduler_skip(&state, "registry_fresh");
            return Ok(None);
        }

        jobs::source_refresh::refresh_company_directories_for_trigger(&state, &trigger).map(Some)
    })
    .await
}

fn refresh_trigger(trigger: Option<String>) -> String {
    trigger
        .filter(|trigger| trigger == "manual" || trigger == "scheduler")
        .unwrap_or_else(|| "manual".to_owned())
}
