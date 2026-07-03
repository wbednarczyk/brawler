use crate::{app_state, source_adapters, storage};
use serde_json::json;

/// Job kind: refresh **one** company for a company-scoped source adapter (ADR 0059).
/// The company-scoped scheduled refresh (bankier-company) is a planner that enqueues
/// one of these per tracked company instead of looping all companies in a single job,
/// so the per-source lock serializes them politely, other lanes run alongside, and
/// unfinished per-company work resumes across restarts. Payload `{adapterId, companyId}`.
pub const SOURCE_COMPANY_REFRESH_KIND: &str = "source_company_refresh";

pub fn refresh_sources_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let started_at = std::time::Instant::now();
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
            record_source_refresh_metrics(state, &result.adapter_id, "succeeded", started_at);
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
            after_successful_refresh(state);
            Ok(result)
        }
        Err(error) => {
            record_source_refresh_metrics(state, "all", "failed", started_at);
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
    let started_at = std::time::Instant::now();
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
            record_source_refresh_metrics(state, &result.adapter_id, "succeeded", started_at);
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
            after_successful_refresh(state);
            Ok(result)
        }
        Err(error) => {
            record_source_refresh_metrics(state, adapter_id, "failed", started_at);
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

/// Cross-cutting work that must run after **every** successful source refresh,
/// from any entry point. Centralized here (not inlined per call site) so a new
/// refresh path cannot silently skip it — call this from the success arm.
///
/// Guardrail (ADR 0045): autopilot detection is event-driven off refresh
/// completion (ADR 0055). The bug it prevents: a refresh path that ingests a new
/// periodic report but never starts an autopilot run. Both `refresh_*_for_trigger`
/// route through here; any future refresh entry point must too. Best-effort and
/// idempotent — it never fails the refresh.
fn after_successful_refresh(state: &app_state::AppState) {
    crate::jobs::autopilot::run_detection_sweep(state);
}

pub fn record_scheduler_skip(state: &app_state::AppState, reason: &str) {
    state.increment_runtime_counter(
        "brawler_scheduler_skips_total",
        &[("module", "sources"), ("status", reason)],
    );
}

/// How a registered source adapter is refreshed at runtime — the dispatch half
/// of the `SourceAdapter` port (Architecture v2 / ADR 0050). Adding a runtime
/// source means adding a [`RuntimeAdapter`] entry to [`runtime_adapters`], not
/// editing a hardcoded dispatch match or sweep list.
enum RefreshBehavior {
    /// A feed/media/calendar source that joins the "refresh all" sweep and ingests items.
    Feed(fn(&app_state::AppState, &str) -> Result<storage::SourceIngestionResult, String>),
    /// A calendar source refreshable for an optional specific date; also joins the sweep.
    Calendar(
        fn(
            &app_state::AppState,
            &str,
            Option<&str>,
        ) -> Result<storage::SourceIngestionResult, String>,
    ),
    /// A company-directory source, refreshed on its own cadence (not in the sweep).
    Directory(
        fn(&app_state::AppState, &str) -> Result<storage::CompanyRegistryRefreshResult, String>,
    ),
    /// A registered-but-disabled source; refreshing it returns this reason.
    Disabled(&'static str),
}

/// One registered runtime adapter: its id plus how it refreshes.
struct RuntimeAdapter {
    id: &'static str,
    behavior: RefreshBehavior,
}

impl RuntimeAdapter {
    /// Whether this source participates in the "refresh all runtime sources" sweep.
    fn in_full_refresh(&self) -> bool {
        matches!(
            self.behavior,
            RefreshBehavior::Feed(_) | RefreshBehavior::Calendar(_)
        )
    }

    fn refresh(
        &self,
        state: &app_state::AppState,
        trigger: &str,
        date: Option<&str>,
    ) -> Result<storage::SourceIngestionResult, String> {
        match self.behavior {
            RefreshBehavior::Feed(refresh) => refresh(state, trigger),
            RefreshBehavior::Calendar(refresh) => refresh(state, trigger, date),
            RefreshBehavior::Directory(refresh) => {
                Ok(directory_ingestion_result(refresh(state, trigger)?))
            }
            RefreshBehavior::Disabled(reason) => Err(reason.to_owned()),
        }
    }
}

/// Map a company-directory refresh result onto the unified ingestion-result shape.
fn directory_ingestion_result(
    result: storage::CompanyRegistryRefreshResult,
) -> storage::SourceIngestionResult {
    storage::SourceIngestionResult {
        adapter_id: result.adapter_id,
        items_fetched: result.entries_fetched,
        items_created: result.entries_upserted,
        items_matched: 0,
        items_unmatched: result.entries_deactivated,
        detail_items_attempted: 0,
        detail_items_stored: 0,
        detail_items_failed: 0,
        fetched_at: Some(result.fetched_at),
    }
}

/// The registry the refresh path iterates (ADR 0050). Declaration order sets the
/// full-refresh `fetched_at` precedence (first `Some` wins): company > calendar >
/// market-events > rss. Sources not listed here are unknown to the refresh path.
fn runtime_adapters() -> Vec<RuntimeAdapter> {
    use source_adapters as sa;
    vec![
        RuntimeAdapter {
            id: sa::bankier_company::ADAPTER_ID,
            behavior: RefreshBehavior::Feed(refresh_bankier_company_for_trigger),
        },
        RuntimeAdapter {
            id: sa::bankier_calendar::ADAPTER_ID,
            behavior: RefreshBehavior::Calendar(refresh_bankier_calendar_for_trigger_and_date),
        },
        RuntimeAdapter {
            id: sa::gpw_market_events::ADAPTER_ID,
            behavior: RefreshBehavior::Feed(refresh_gpw_market_events_for_trigger),
        },
        RuntimeAdapter {
            id: sa::bankier_rss::ADAPTER_ID,
            behavior: RefreshBehavior::Feed(refresh_bankier_rss_for_trigger),
        },
        RuntimeAdapter {
            id: sa::gpw_company_registry::ADAPTER_ID,
            behavior: RefreshBehavior::Directory(refresh_gpw_company_registry_for_trigger),
        },
        RuntimeAdapter {
            id: sa::newconnect_company_directory::ADAPTER_ID,
            behavior: RefreshBehavior::Directory(refresh_newconnect_company_directory_for_trigger),
        },
        RuntimeAdapter {
            id: sa::gpw_espi_ebi::ADAPTER_ID,
            behavior: RefreshBehavior::Disabled(
                "GPW ESPI/EBI is disabled while Bankier Company Komunikaty is the active official-report source",
            ),
        },
        RuntimeAdapter {
            id: "portal-analiz",
            behavior: RefreshBehavior::Disabled(
                "Portal Analiz is disabled until its authenticated adapter is implemented",
            ),
        },
        RuntimeAdapter {
            id: "bankier-firma-rss",
            behavior: RefreshBehavior::Disabled(
                "Bankier Firma RSS is disabled until matching quality is proven",
            ),
        },
        RuntimeAdapter {
            id: "bankier-wiadomosci-rss",
            behavior: RefreshBehavior::Disabled(
                "Bankier Wiadomosci RSS is disabled because broad news matching is not accepted for runtime ingestion",
            ),
        },
    ]
}

fn refresh_sources_for_trigger_inner(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let mut total = empty_source_result("all");

    for adapter in runtime_adapters().iter().filter(|a| a.in_full_refresh()) {
        let result = refresh_optional_source(state, adapter, trigger)?;
        total.items_fetched += result.items_fetched;
        total.items_created += result.items_created;
        total.items_matched += result.items_matched;
        total.items_unmatched += result.items_unmatched;
        total.detail_items_attempted += result.detail_items_attempted;
        total.detail_items_stored += result.detail_items_stored;
        total.detail_items_failed += result.detail_items_failed;
        if total.fetched_at.is_none() {
            total.fetched_at = result.fetched_at;
        }
    }

    Ok(total)
}

fn refresh_source_for_trigger_inner(
    state: &app_state::AppState,
    adapter_id: &str,
    trigger: &str,
    date: Option<&str>,
) -> Result<storage::SourceIngestionResult, String> {
    if !state
        .source_adapter_enabled(adapter_id)
        .map_err(|error| error.to_string())?
    {
        return Err("Source is turned off".to_owned());
    }

    match runtime_adapters().into_iter().find(|a| a.id == adapter_id) {
        Some(adapter) => adapter.refresh(state, trigger, date),
        None => Err(format!("Unknown source adapter: {adapter_id}")),
    }
}

/// Refresh one sweep source, skipping (empty result) when it is turned off.
fn refresh_optional_source(
    state: &app_state::AppState,
    adapter: &RuntimeAdapter,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    if !state
        .source_adapter_enabled(adapter.id)
        .map_err(|error| error.to_string())?
    {
        return Ok(empty_source_result(adapter.id));
    }

    adapter.refresh(state, trigger, None)
}

fn empty_source_result(adapter_id: &str) -> storage::SourceIngestionResult {
    storage::SourceIngestionResult {
        adapter_id: adapter_id.to_owned(),
        items_fetched: 0,
        items_created: 0,
        items_matched: 0,
        items_unmatched: 0,
        detail_items_attempted: 0,
        detail_items_stored: 0,
        detail_items_failed: 0,
        fetched_at: None,
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

fn record_source_refresh_metrics(
    state: &app_state::AppState,
    adapter_id: &str,
    status: &str,
    started_at: std::time::Instant,
) {
    state.increment_runtime_counter(
        "brawler_source_refresh_total",
        &[("adapter_id", adapter_id), ("status", status)],
    );
    state.observe_runtime_duration_seconds(
        "brawler_source_refresh_duration_seconds",
        &[("adapter_id", adapter_id), ("status", status)],
        started_at.elapsed().as_secs_f64(),
    );
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

/// Plan a bankier-company refresh: enqueue one idempotent `source_company_refresh`
/// job per tracked company instead of looping every company in a single monolith
/// job (ADR 0059). The former monolith (a ~100-company loop with a 1 s sleep each)
/// monopolized the worker for minutes and starved autopilot; the per-company jobs
/// are serialized by the per-source lock (politeness preserved), run alongside other
/// lanes, and resume across restarts. Returns quickly with a summary — the per-company
/// jobs do the actual fetch/ingest and each rides detection on its own completion.
fn refresh_bankier_company_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let adapter_id = source_adapters::bankier_company::ADAPTER_ID;
    let _ = state.record_source_adapter_attempt(adapter_id, trigger);
    let targets = state
        .list_bankier_company_targets()
        .map_err(|error| error.to_string())?;

    let mut planned = 0usize;
    for target in &targets {
        let job_id = format!(
            "{SOURCE_COMPANY_REFRESH_KIND}:{adapter_id}:{}",
            target.company_id
        );
        let payload =
            json!({ "adapterId": adapter_id, "companyId": target.company_id }).to_string();
        // `reschedule` re-arms a stable per-company id: pending/terminal rows reset,
        // an in-flight row is left alone — so a re-plan never disturbs a running job
        // and never accumulates duplicate rows.
        match state
            .jobs()
            .reschedule(&job_id, SOURCE_COMPANY_REFRESH_KIND, &payload, 3)
        {
            Ok(_) => planned += 1,
            Err(error) => log::warn!(
                "module=sources stage=plan_failed adapterId={adapter_id} companyId={} error={error}",
                target.company_id
            ),
        }
    }
    log::info!(
        "module=sources stage=planned adapterId={adapter_id} trigger={trigger} companiesPlanned={planned}"
    );
    Ok(empty_source_result(adapter_id))
}

/// Refresh **one** company for the bankier-company source (ADR 0059) — the extracted
/// body of the former monolith loop. Fetches that company's komunikaty (with the
/// detail-URL cache filter), upserts its identifiers, ingests its items, and fetches
/// any newly-registered periodic-report attachments (best-effort, ADR 0036).
pub fn refresh_one_bankier_company(
    state: &app_state::AppState,
    target: &source_adapters::bankier_company::BankierCompanyTarget,
) -> Result<storage::SourceIngestionResult, String> {
    let adapter_id = source_adapters::bankier_company::ADAPTER_ID;
    let fetcher = source_adapters::bankier_company::HttpBankierCompanyFetcher;
    let cached_detail_urls = state
        .list_bankier_company_detail_cached_urls()
        .map_err(|error| error.to_string())?
        .into_iter()
        .collect::<std::collections::HashSet<_>>();

    let items = match source_adapters::bankier_company::fetch_company_items_with_detail_filter(
        &fetcher,
        target,
        |item| !cached_detail_urls.contains(&item.link),
    ) {
        Ok((identifiers, items)) => {
            if let Some(identifiers) = identifiers {
                let _ = state.upsert_bankier_company_identifiers(&target.company_id, &identifiers);
            }
            items
        }
        Err(error) => {
            let message = format!("{}: {}", target.qualified_ticker, error);
            let _ = state.record_source_adapter_error(adapter_id, &message);
            return Err(message);
        }
    };

    let result = state
        .ingest_bankier_company_items(&items)
        .map_err(|error| error.to_string())?;

    // Fetch files for periodic-report attachments registered during ingestion (ADR 0036).
    // Best-effort: a fetch failure is recorded on the document and never fails the refresh.
    let document_fetcher = crate::document_fetcher::HttpDocumentFetcher::new();
    if let Err(error) =
        crate::report_documents_capture::fetch_pending_attachments(state, &document_fetcher)
    {
        let _ = state
            .record_source_adapter_error(adapter_id, &format!("attachment fetch failed: {error}"));
    }

    Ok(result)
}

/// Queue entry point for a `source_company_refresh` job (ADR 0059): resolve the
/// company target from the payload, refresh that one company, and — on success —
/// run detection so autopilot rides the actual ingest (the guardrail in
/// [`after_successful_refresh`]). The per-source lock (held by the worker across this
/// call) guarantees at most one company of this source refreshes at a time.
pub fn run_source_company_refresh(
    state: &app_state::AppState,
    payload: &str,
) -> Result<(), String> {
    let parsed: serde_json::Value =
        serde_json::from_str(payload).map_err(|error| error.to_string())?;
    let adapter_id = parsed
        .get("adapterId")
        .and_then(|value| value.as_str())
        .ok_or("source company refresh missing adapterId")?;
    let company_id = parsed
        .get("companyId")
        .and_then(|value| value.as_str())
        .ok_or("source company refresh missing companyId")?;

    if adapter_id != source_adapters::bankier_company::ADAPTER_ID {
        return Err(format!(
            "unsupported company-scoped source adapter: {adapter_id}"
        ));
    }

    let target = state
        .list_bankier_company_targets()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|target| target.company_id == company_id)
        .ok_or_else(|| format!("no bankier company target for {company_id}"))?;

    refresh_one_bankier_company(state, &target)?;
    after_successful_refresh(state);
    Ok(())
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

pub fn should_bootstrap_company_directories(
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

    if !has_lookup_value {
        return Ok(false);
    }

    state
        .company_directories_need_bootstrap_refresh()
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

pub fn refresh_newconnect_company_directory_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::CompanyRegistryRefreshResult, String> {
    let _ = state.record_source_adapter_attempt(
        source_adapters::newconnect_company_directory::ADAPTER_ID,
        trigger,
    );

    let fetcher =
        source_adapters::newconnect_company_directory::HttpNewConnectCompanyDirectoryFetcher;
    let (entries, fetched_at) =
        match source_adapters::newconnect_company_directory::fetch_company_directory_entries(
            &fetcher,
        ) {
            Ok(result) => result,
            Err(error) => {
                let message = error.to_string();
                let _ = state.record_source_adapter_error(
                    source_adapters::newconnect_company_directory::ADAPTER_ID,
                    &message,
                );

                return Err(message);
            }
        };

    state
        .refresh_newconnect_company_directory(&entries, &fetched_at)
        .map_err(|error| error.to_string())
}

pub fn refresh_company_directories_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::CompanyRegistryRefreshResult, String> {
    let gpw_result = refresh_gpw_company_registry_for_trigger(state, trigger)?;
    let newconnect_result = refresh_newconnect_company_directory_for_trigger(state, trigger)?;

    Ok(storage::CompanyRegistryRefreshResult {
        adapter_id: "company-directories".to_owned(),
        entries_fetched: gpw_result.entries_fetched + newconnect_result.entries_fetched,
        entries_upserted: gpw_result.entries_upserted + newconnect_result.entries_upserted,
        entries_deactivated: gpw_result.entries_deactivated + newconnect_result.entries_deactivated,
        fetched_at: gpw_result.fetched_at.max(newconnect_result.fetched_at),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        refresh_bankier_company_for_trigger, refresh_source_for_trigger,
        should_bootstrap_company_directories, SOURCE_COMPANY_REFRESH_KIND,
    };
    use crate::storage::{open_in_memory_database, AppState, CompanyLookupInput, NewCompany};

    #[test]
    fn bankier_company_refresh_plans_one_job_per_tracked_company() {
        // Chunked refresh (ADR 0059): the company-scoped refresh is a planner that
        // enqueues one idempotent `source_company_refresh` job per tracked company,
        // not a single monolith loop. The former monolith monopolized the worker for
        // minutes and starved autopilot.
        let state = AppState::new(open_in_memory_database().expect("db"));
        for ticker in ["CDR", "CBF", "PKN"] {
            state
                .create_company(NewCompany {
                    exchange: "GPW".to_owned(),
                    ticker: ticker.to_owned(),
                    display_name: format!("{ticker} S.A."),
                    isin: None,
                    cik: None,
                    lei: None,
                })
                .expect("company");
        }

        let result =
            refresh_bankier_company_for_trigger(&state, "manual").expect("planner should succeed");
        assert_eq!(
            result.items_fetched, 0,
            "the planner enqueues work; it does not ingest itself"
        );
        assert_eq!(
            state.jobs().counts().expect("counts").pending,
            3,
            "one per-company job enqueued per tracked GPW company"
        );

        // Each enqueued job is a per-company refresh carrying its company id.
        let claimed = state.jobs().claim_next().expect("claim").expect("a job");
        assert_eq!(claimed.kind, SOURCE_COMPANY_REFRESH_KIND);
        assert!(
            claimed.payload.contains("companyId"),
            "payload targets one company, got: {}",
            claimed.payload
        );

        // Re-planning is idempotent (stable per-company ids via reschedule): the two
        // still-pending rows reset, the running one is left alone — no duplicates.
        refresh_bankier_company_for_trigger(&state, "manual").expect("re-plan");
        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.pending + counts.running,
            3,
            "still one row per company"
        );
    }

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

    #[test]
    fn company_directory_bootstrap_is_not_limited_to_current_exchange_codes() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let should_bootstrap = should_bootstrap_company_directories(
            &CompanyLookupInput {
                exchange: "XETRA".to_owned(),
                ticker: Some("SAP".to_owned()),
                display_name: None,
                isin: None,
            },
            &state,
        )
        .expect("bootstrap decision should succeed");

        assert!(should_bootstrap);
    }

    #[test]
    fn company_directory_bootstrap_requires_a_lookup_value() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let should_bootstrap = should_bootstrap_company_directories(
            &CompanyLookupInput {
                exchange: "XETRA".to_owned(),
                ticker: None,
                display_name: Some("SA".to_owned()),
                isin: None,
            },
            &state,
        )
        .expect("bootstrap decision should succeed");

        assert!(!should_bootstrap);
    }
}
