use crate::{app_state, source_adapters, storage};
use serde_json::json;

pub fn refresh_sources_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    log::info!("module=sources stage=running adapterId=all trigger={trigger}");
    record_source_diagnostic(
        state,
        "all",
        "running",
        "info",
        "Source refresh started.",
        json!({ "trigger": trigger }),
    );
    let result = refresh_sources_for_trigger_inner(state, trigger);

    match result {
        Ok(result) => {
            log::info!(
                "module=sources stage=succeeded adapterId={} trigger={} itemsFetched={} itemsCreated={} itemsMatched={} itemsUnmatched={}",
                result.adapter_id,
                trigger,
                result.items_fetched,
                result.items_created,
                result.items_matched,
                result.items_unmatched
            );
            record_source_diagnostic(
                state,
                "all",
                "succeeded",
                "info",
                "Source refresh completed.",
                source_result_metadata(trigger, &result),
            );
            Ok(result)
        }
        Err(error) => {
            log::error!(
                "module=sources stage=failed adapterId=all trigger={} errorClass=source_refresh_error error={}",
                trigger,
                error
            );
            record_source_diagnostic(
                state,
                "all",
                "failed",
                "error",
                "Source refresh failed.",
                json!({
                    "trigger": trigger,
                    "errorClass": "source_refresh_error"
                }),
            );
            Err(error)
        }
    }
}

pub fn refresh_source_for_trigger(
    state: &app_state::AppState,
    adapter_id: &str,
    trigger: &str,
    date: Option<&str>,
) -> Result<storage::SourceIngestionResult, String> {
    log::info!(
        "module=sources stage=running adapterId={} trigger={} hasDate={}",
        adapter_id,
        trigger,
        date.is_some()
    );
    record_source_diagnostic(
        state,
        adapter_id,
        "running",
        "info",
        "Source adapter refresh started.",
        json!({
            "adapterId": adapter_id,
            "trigger": trigger,
            "hasDate": date.is_some()
        }),
    );
    let result = refresh_source_for_trigger_inner(state, adapter_id, trigger, date);

    match result {
        Ok(result) => {
            log::info!(
                "module=sources stage=succeeded adapterId={} trigger={} itemsFetched={} itemsCreated={} itemsMatched={} itemsUnmatched={}",
                result.adapter_id,
                trigger,
                result.items_fetched,
                result.items_created,
                result.items_matched,
                result.items_unmatched
            );
            record_source_diagnostic(
                state,
                adapter_id,
                "succeeded",
                "info",
                "Source adapter refresh completed.",
                source_result_metadata(trigger, &result),
            );
            Ok(result)
        }
        Err(error) => {
            log::error!(
                "module=sources stage=failed adapterId={} trigger={} errorClass=source_refresh_error error={}",
                adapter_id,
                trigger,
                error
            );
            record_source_diagnostic(
                state,
                adapter_id,
                "failed",
                "error",
                "Source adapter refresh failed.",
                json!({
                    "adapterId": adapter_id,
                    "trigger": trigger,
                    "errorClass": "source_refresh_error"
                }),
            );
            Err(error)
        }
    }
}

fn refresh_sources_for_trigger_inner(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let bankier_result = refresh_bankier_rss_for_trigger(state, trigger)?;
    let bankier_company_result = refresh_bankier_company_for_trigger(state, trigger)?;
    let gpw_market_events_result = refresh_gpw_market_events_for_trigger(state, trigger)?;
    let bankier_calendar_result = refresh_bankier_calendar_for_trigger(state, trigger)?;

    Ok(storage::SourceIngestionResult {
        adapter_id: bankier_company_result.adapter_id,
        items_fetched: bankier_result.items_fetched
            + bankier_company_result.items_fetched
            + gpw_market_events_result.items_fetched
            + bankier_calendar_result.items_fetched,
        items_created: bankier_result.items_created
            + bankier_company_result.items_created
            + gpw_market_events_result.items_created
            + bankier_calendar_result.items_created,
        items_matched: bankier_result.items_matched
            + bankier_company_result.items_matched
            + gpw_market_events_result.items_matched
            + bankier_calendar_result.items_matched,
        items_unmatched: bankier_result.items_unmatched
            + bankier_company_result.items_unmatched
            + gpw_market_events_result.items_unmatched
            + bankier_calendar_result.items_unmatched,
        detail_items_attempted: bankier_company_result.detail_items_attempted,
        detail_items_stored: bankier_company_result.detail_items_stored,
        detail_items_failed: bankier_company_result.detail_items_failed,
        fetched_at: bankier_company_result
            .fetched_at
            .or(bankier_calendar_result.fetched_at)
            .or(gpw_market_events_result.fetched_at)
            .or(bankier_result.fetched_at),
    })
}

fn refresh_source_for_trigger_inner(
    state: &app_state::AppState,
    adapter_id: &str,
    trigger: &str,
    date: Option<&str>,
) -> Result<storage::SourceIngestionResult, String> {
    match adapter_id {
        source_adapters::gpw_espi_ebi::ADAPTER_ID => Err(
            "GPW ESPI/EBI is disabled while Bankier Company Komunikaty is the active official-report source"
                .to_owned(),
        ),
        source_adapters::bankier_rss::ADAPTER_ID => refresh_bankier_rss_for_trigger(state, trigger),
        source_adapters::bankier_company::ADAPTER_ID => {
            refresh_bankier_company_for_trigger(state, trigger)
        }
        source_adapters::gpw_market_events::ADAPTER_ID => {
            refresh_gpw_market_events_for_trigger(state, trigger)
        }
        source_adapters::bankier_calendar::ADAPTER_ID => {
            refresh_bankier_calendar_for_trigger_and_date(state, trigger, date)
        }
        source_adapters::gpw_company_registry::ADAPTER_ID => {
            let registry_result = refresh_gpw_company_registry_for_trigger(state, trigger)?;
            Ok(storage::SourceIngestionResult {
                adapter_id: registry_result.adapter_id,
                items_fetched: registry_result.entries_fetched,
                items_created: registry_result.entries_upserted,
                items_matched: 0,
                items_unmatched: registry_result.entries_deactivated,
                detail_items_attempted: 0,
                detail_items_stored: 0,
                detail_items_failed: 0,
                fetched_at: Some(registry_result.fetched_at),
            })
        }
        "portal-analiz" => {
            Err("Portal Analiz is disabled until its authenticated adapter is implemented".to_owned())
        }
        "bankier-firma-rss" => {
            Err("Bankier Firma RSS is disabled until matching quality is proven".to_owned())
        }
        "bankier-wiadomosci-rss" => Err(
            "Bankier Wiadomosci RSS is disabled because broad news matching is not accepted for runtime ingestion"
                .to_owned(),
        ),
        _ => Err(format!("Unknown source adapter: {adapter_id}")),
    }
}

fn source_result_metadata(
    trigger: &str,
    result: &storage::SourceIngestionResult,
) -> serde_json::Value {
    json!({
        "adapterId": result.adapter_id,
        "trigger": trigger,
        "itemsFetched": result.items_fetched,
        "itemsCreated": result.items_created,
        "itemsMatched": result.items_matched,
        "itemsUnmatched": result.items_unmatched,
        "detailItemsAttempted": result.detail_items_attempted,
        "detailItemsStored": result.detail_items_stored,
        "detailItemsFailed": result.detail_items_failed,
        "hasFetchedAt": result.fetched_at.is_some()
    })
}

fn record_source_diagnostic(
    state: &app_state::AppState,
    adapter_id: &str,
    stage: &str,
    severity: &str,
    message: &str,
    metadata: serde_json::Value,
) {
    let _ = state.record_diagnostic_event(storage::NewDiagnosticEvent {
        occurred_at: None,
        module: "sources".to_owned(),
        scope: Some(storage::DiagnosticScope {
            scope_type: "source_adapter".to_owned(),
            id: Some(adapter_id.to_owned()),
        }),
        stage: stage.to_owned(),
        severity: severity.to_owned(),
        message: message.to_owned(),
        metadata: Some(metadata),
    });
}

#[allow(dead_code)]
fn refresh_gpw_espi_ebi_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let _ = state.record_source_adapter_attempt(source_adapters::gpw_espi_ebi::ADAPTER_ID, trigger);

    let fetcher = source_adapters::gpw_espi_ebi::HttpGpwPageFetcher;
    let mut listings = match source_adapters::gpw_espi_ebi::fetch_report_listings(&fetcher) {
        Ok(listings) => listings,
        Err(error) => {
            let message = error.to_string();
            let _ = state
                .record_source_adapter_error(source_adapters::gpw_espi_ebi::ADAPTER_ID, &message);

            return Err(message);
        }
    };
    let detail_policy = source_adapters::gpw_espi_ebi::detail_fetch_policy();
    let mut details_fetched = 0usize;
    let mut details_stored = 0usize;
    let mut details_failed = 0usize;
    let mut last_detail_warning: Option<String> = None;

    if detail_policy.enabled_by_default {
        for listing in &mut listings {
            if details_fetched >= detail_policy.max_details_per_refresh {
                break;
            }

            if detail_policy.matched_items_only
                && !state
                    .tracks_gpw_listing_company(&listing.company_ticker, &listing.isin)
                    .map_err(|error| error.to_string())?
            {
                continue;
            }

            if details_fetched > 0 {
                std::thread::sleep(std::time::Duration::from_secs(
                    detail_policy.min_delay_between_requests_seconds,
                ));
            }

            match source_adapters::gpw_espi_ebi::fetch_report_detail(&fetcher, &listing.detail_url)
            {
                Ok(detail) => {
                    let evaluation = source_adapters::gpw_espi_ebi::evaluate_report_detail(&detail);
                    if evaluation.usable_for_ingestion {
                        listing.body_text = Some(detail.body_text);
                        listing.attachments = detail.attachments;
                        details_stored += 1;
                    } else {
                        last_detail_warning = Some(format!(
                            "{}: {}",
                            listing.title,
                            evaluation.warnings.join(", ")
                        ));
                        details_failed += 1;
                    }
                }
                Err(error) => {
                    last_detail_warning = Some(format!("{}: {}", listing.title, error));
                    let _ = state.record_source_adapter_error(
                        source_adapters::gpw_espi_ebi::ADAPTER_ID,
                        &error.to_string(),
                    );
                    details_failed += 1;
                }
            }

            details_fetched += 1;
        }
    }

    let _ = state.record_source_adapter_state(
        source_adapters::gpw_espi_ebi::ADAPTER_ID,
        "last_detail_items_attempted",
        &details_fetched.to_string(),
    );
    let _ = state.record_source_adapter_state(
        source_adapters::gpw_espi_ebi::ADAPTER_ID,
        "last_detail_items_stored",
        &details_stored.to_string(),
    );
    let _ = state.record_source_adapter_state(
        source_adapters::gpw_espi_ebi::ADAPTER_ID,
        "last_detail_items_failed",
        &details_failed.to_string(),
    );
    let _ = state.record_source_adapter_state(
        source_adapters::gpw_espi_ebi::ADAPTER_ID,
        "last_detail_warning",
        last_detail_warning.as_deref().unwrap_or(""),
    );

    let mut result = state
        .ingest_gpw_report_listings(&listings)
        .map_err(|error| error.to_string())?;
    result.detail_items_attempted = details_fetched;
    result.detail_items_stored = details_stored;
    result.detail_items_failed = details_failed;

    Ok(result)
}

fn refresh_bankier_rss_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let _ = state.record_source_adapter_attempt(source_adapters::bankier_rss::ADAPTER_ID, trigger);
    let bankier_fetcher = source_adapters::bankier_rss::HttpBankierRssFetcher;
    let bankier_items = match source_adapters::bankier_rss::fetch_rss_items(&bankier_fetcher) {
        Ok(items) => items,
        Err(error) => {
            let message = error.to_string();
            let _ = state
                .record_source_adapter_error(source_adapters::bankier_rss::ADAPTER_ID, &message);

            return Err(message);
        }
    };

    state
        .ingest_bankier_rss_items(&bankier_items)
        .map_err(|error| error.to_string())
}

fn refresh_bankier_company_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let _ =
        state.record_source_adapter_attempt(source_adapters::bankier_company::ADAPTER_ID, trigger);
    let bankier_company_fetcher = source_adapters::bankier_company::HttpBankierCompanyFetcher;
    let bankier_company_targets = state
        .list_bankier_company_targets()
        .map_err(|error| error.to_string())?;
    let cached_detail_urls = state
        .list_bankier_company_detail_cached_urls()
        .map_err(|error| error.to_string())?
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let mut bankier_company_items = Vec::new();
    let mut bankier_company_last_error: Option<String> = None;

    for (index, target) in bankier_company_targets.iter().enumerate() {
        if index > 0 {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        match source_adapters::bankier_company::fetch_company_items_with_detail_filter(
            &bankier_company_fetcher,
            target,
            |item| !cached_detail_urls.contains(&item.link),
        ) {
            Ok((identifiers, mut items)) => {
                if let Some(identifiers) = identifiers {
                    let _ =
                        state.upsert_bankier_company_identifiers(&target.company_id, &identifiers);
                }
                bankier_company_items.append(&mut items);
            }
            Err(error) => {
                let message = format!("{}: {}", target.qualified_ticker, error);
                bankier_company_last_error = Some(message.clone());
                let _ = state.record_source_adapter_error(
                    source_adapters::bankier_company::ADAPTER_ID,
                    &message,
                );
            }
        }
    }

    let result = state
        .ingest_bankier_company_items(&bankier_company_items)
        .map_err(|error| error.to_string())?;
    if let Some(message) = bankier_company_last_error {
        let _ = state
            .record_source_adapter_error(source_adapters::bankier_company::ADAPTER_ID, &message);
    }

    Ok(result)
}

fn refresh_gpw_market_events_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let _ = state
        .record_source_adapter_attempt(source_adapters::gpw_market_events::ADAPTER_ID, trigger);
    let fetcher = source_adapters::gpw_market_events::HttpGpwMarketEventsFetcher;
    let event_items = match source_adapters::gpw_market_events::fetch_market_events(&fetcher) {
        Ok(items) => items,
        Err(error) => {
            let message = error.to_string();
            let _ = state.record_source_adapter_error(
                source_adapters::gpw_market_events::ADAPTER_ID,
                &message,
            );

            return Err(message);
        }
    };

    state
        .ingest_gpw_market_event_items(&event_items)
        .map_err(|error| error.to_string())
}

fn refresh_bankier_calendar_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    refresh_bankier_calendar_for_trigger_and_date(state, trigger, None)
}

fn refresh_bankier_calendar_for_trigger_and_date(
    state: &app_state::AppState,
    trigger: &str,
    date: Option<&str>,
) -> Result<storage::SourceIngestionResult, String> {
    let _ =
        state.record_source_adapter_attempt(source_adapters::bankier_calendar::ADAPTER_ID, trigger);
    let fetcher = source_adapters::bankier_calendar::HttpBankierCalendarFetcher;
    let event_items =
        match source_adapters::bankier_calendar::fetch_calendar_events_for_date(&fetcher, date) {
            Ok(items) => items,
            Err(error) => {
                let message = error.to_string();
                let _ = state.record_source_adapter_error(
                    source_adapters::bankier_calendar::ADAPTER_ID,
                    &message,
                );

                return Err(message);
            }
        };

    state
        .ingest_bankier_calendar_event_items(&event_items)
        .map_err(|error| error.to_string())
}

pub fn should_bootstrap_gpw_registry(
    input: &storage::CompanyLookupInput,
    state: &app_state::AppState,
) -> Result<bool, String> {
    let has_lookup_value = input
        .ticker
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || input
            .isin
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || input
            .display_name
            .as_deref()
            .is_some_and(|value| value.trim().chars().count() >= 3);

    if input.exchange.trim().to_uppercase() != "GPW" || !has_lookup_value {
        return Ok(false);
    }

    state
        .gpw_company_registry_needs_bootstrap_refresh()
        .map_err(|error| error.to_string())
}

pub fn refresh_gpw_company_registry_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::CompanyRegistryRefreshResult, String> {
    let _ = state
        .record_source_adapter_attempt(source_adapters::gpw_company_registry::ADAPTER_ID, trigger);

    let fetcher = source_adapters::gpw_company_registry::HttpGpwCompanyRegistryFetcher;
    let (entries, fetched_at) =
        match source_adapters::gpw_company_registry::fetch_company_registry_entries(&fetcher) {
            Ok(result) => result,
            Err(error) => {
                let message = error.to_string();
                let _ = state.record_source_adapter_error(
                    source_adapters::gpw_company_registry::ADAPTER_ID,
                    &message,
                );

                return Err(message);
            }
        };

    state
        .refresh_gpw_company_registry(&entries, &fetched_at)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::refresh_source_for_trigger;
    use crate::storage::{open_in_memory_database, AppState};

    #[test]
    fn failed_source_refresh_records_lightweight_diagnostics_when_enabled() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .set_developer_mode_enabled(true)
            .expect("developer mode should enable");

        let result = refresh_source_for_trigger(&state, "unknown-adapter", "manual", None);

        assert!(result.is_err());
        let events = state
            .list_diagnostic_events(10)
            .expect("diagnostic events should list");
        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|event| event.stage == "running"));
        assert!(events.iter().any(|event| event.stage == "failed"));
        assert!(events.iter().all(|event| event.module == "sources"));
        assert!(events.iter().all(|event| !event
            .metadata
            .to_string()
            .contains("Unknown source adapter")));
    }
}
