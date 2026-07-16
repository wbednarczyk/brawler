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
    // Ownership extraction (ADR 0072 T3): a newly ingested periodic report may
    // carry an as-yet-unparsed shareholders table. Enqueue extraction for every
    // fetched periodic document still lacking ownership coverage — deterministic,
    // idempotent, and independent of the autopilot mode (its writes are final,
    // not AI proposals).
    crate::jobs::ownership_extraction::enqueue_ownership_extraction_catch_up(state, None);
}

pub fn record_scheduler_skip(state: &app_state::AppState, reason: &str) {
    state.increment_runtime_counter(
        "brawler_scheduler_skips_total",
        &[("module", "sources"), ("status", reason)],
    );
}

/// The parameters a refresh runs against — the trigger name plus an optional
/// specific date (only calendars use the date; other adapters ignore it). Carried
/// as one context so the [`Fetcher`] signature covers every arm kind (ADR 0069
/// amendment 2026-07-15) without per-arm fn-pointer shapes.
pub(crate) struct RefreshContext<'a> {
    pub trigger: &'a str,
    pub date: Option<&'a str>,
}

/// What a [`Fetcher::refresh`] produced: either a unified feed/calendar ingestion
/// result, or a company-directory refresh result. The dispatch half maps a
/// `Directory` outcome onto the ingestion shape (see [`directory_ingestion_result`]).
pub(crate) enum RefreshOutcome {
    Ingestion(storage::SourceIngestionResult),
    Directory(storage::CompanyRegistryRefreshResult),
}

/// The refresh-level behavior of a source adapter (ADR 0069, amended 2026-07-15):
/// the `SourceAdapter` port gains a behavioral contract. Every adapter implements
/// this once beside its item types instead of being wired imperatively into a
/// per-kind fn-pointer arm here. The strangler migration is complete (plan v0.55
/// T1/T2): feed, calendar, and directory adapters all live behind this trait; only
/// the disabled sources keep a non-trait arm (they have no fetch behavior yet).
pub(crate) trait Fetcher: Sync {
    /// Perform this adapter's refresh for the given context, returning the outcome
    /// the dispatch half unifies into a [`storage::SourceIngestionResult`].
    fn refresh(
        &self,
        state: &app_state::AppState,
        ctx: &RefreshContext,
    ) -> Result<RefreshOutcome, String>;

    /// Whether this source participates in the "refresh all runtime sources" sweep.
    /// Feed/calendar sources join (default); company-directory sources refresh on
    /// their own cadence and override to `false`. This models the exact
    /// pre-migration membership (Feed + Calendar joined; Directory did not).
    fn joins_full_refresh(&self) -> bool {
        true
    }
}

/// How a registered source adapter is refreshed at runtime — the dispatch half
/// of the `SourceAdapter` port (Architecture v2 / ADR 0050). Adding a runtime
/// source means adding a [`RuntimeAdapter`] entry to [`runtime_adapters`], not
/// editing a hardcoded dispatch match or sweep list.
enum RefreshBehavior {
    /// A source that implements the ADR 0069 [`Fetcher`] trait and dispatches
    /// polymorphically — the sole active arm now the strangler migration is done.
    Fetcher(&'static dyn Fetcher),
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
        match self.behavior {
            RefreshBehavior::Fetcher(fetcher) => fetcher.joins_full_refresh(),
            RefreshBehavior::Disabled(_) => false,
        }
    }

    fn refresh(
        &self,
        state: &app_state::AppState,
        trigger: &str,
        date: Option<&str>,
    ) -> Result<storage::SourceIngestionResult, String> {
        match self.behavior {
            RefreshBehavior::Fetcher(fetcher) => {
                match fetcher.refresh(state, &RefreshContext { trigger, date })? {
                    RefreshOutcome::Ingestion(result) => Ok(result),
                    RefreshOutcome::Directory(result) => Ok(directory_ingestion_result(result)),
                }
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
    // Strangler migration complete (plan v0.55 T1/T2, ADR 0069): every fetching
    // adapter dispatches polymorphically through the `Fetcher` trait-object arm;
    // each impl lives beside its item types in its own module. Only the disabled
    // sources keep the non-trait `Disabled` arm — they have no fetch behavior yet.
    // `gpw_espi_ebi` runs as a reconciliation WITNESS (plan v0.55 T3): it fetches
    // the official listing but reconciles against Bankier instead of ingesting.
    vec![
        RuntimeAdapter {
            id: sa::bankier_company::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&sa::bankier_company::BankierCompanyRefresh),
        },
        RuntimeAdapter {
            id: sa::bankier_calendar::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&sa::bankier_calendar::BankierCalendarRefresh),
        },
        RuntimeAdapter {
            id: sa::gpw_market_events::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&sa::gpw_market_events::GpwMarketEventsRefresh),
        },
        RuntimeAdapter {
            id: sa::bankier_rss::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&sa::bankier_rss::BankierRssRefresh),
        },
        RuntimeAdapter {
            id: sa::knf_short_selling::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&sa::knf_short_selling::KnfShortSellingRefresh),
        },
        RuntimeAdapter {
            id: sa::biznesradar_ownership::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&sa::biznesradar_ownership::BiznesRadarOwnershipAdapter),
        },
        RuntimeAdapter {
            id: sa::gpw_company_registry::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&sa::gpw_company_registry::GpwCompanyRegistryRefresh),
        },
        RuntimeAdapter {
            id: sa::newconnect_company_directory::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(
                &sa::newconnect_company_directory::NewConnectCompanyDirectoryRefresh,
            ),
        },
        RuntimeAdapter {
            id: crate::jobs::quote_daily_pull::YAHOO_ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&crate::jobs::quote_daily_pull::YahooEodRefresh),
        },
        RuntimeAdapter {
            id: sa::gpw_espi_ebi::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(&sa::gpw_espi_ebi::GpwEspiEbiWitness),
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

pub(crate) fn empty_source_result(adapter_id: &str) -> storage::SourceIngestionResult {
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

pub fn refresh_company_directories_for_trigger(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::CompanyRegistryRefreshResult, String> {
    let gpw_result =
        source_adapters::gpw_company_registry::refresh_gpw_company_registry_for_trigger(
            state, trigger,
        )?;
    let newconnect_result =
        source_adapters::newconnect_company_directory::refresh_newconnect_company_directory_for_trigger(
            state, trigger,
        )?;

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
        refresh_source_for_trigger, should_bootstrap_company_directories,
        SOURCE_COMPANY_REFRESH_KIND,
    };
    use crate::source_adapters::bankier_company::refresh_bankier_company_for_trigger;
    use crate::storage::{open_in_memory_database, AppState, CompanyLookupInput, NewCompany};

    #[test]
    fn fetcher_trait_impl_is_dispatched_for_its_adapter() {
        // ADR 0069 (amended 2026-07-15): the refresh-level `Fetcher` trait must be
        // dispatched polymorphically for adapters registered on the trait-object arm.
        // Register a scripted `Fetcher` and assert `RuntimeAdapter::refresh` invokes it.
        use super::{
            empty_source_result, Fetcher, RefreshBehavior, RefreshContext, RefreshOutcome,
            RuntimeAdapter,
        };
        use crate::app_state::AppState;
        use crate::storage::open_in_memory_database;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct ScriptedFetcher {
            calls: AtomicUsize,
        }

        impl Fetcher for ScriptedFetcher {
            fn refresh(
                &self,
                _state: &AppState,
                _ctx: &RefreshContext,
            ) -> Result<RefreshOutcome, String> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let mut result = empty_source_result("scripted");
                result.items_fetched = 4242;
                Ok(RefreshOutcome::Ingestion(result))
            }
        }

        let fetcher: &'static ScriptedFetcher = Box::leak(Box::new(ScriptedFetcher {
            calls: AtomicUsize::new(0),
        }));
        let adapter = RuntimeAdapter {
            id: "scripted",
            behavior: RefreshBehavior::Fetcher(fetcher),
        };
        let state = AppState::new(open_in_memory_database().expect("db"));

        let result = adapter
            .refresh(&state, "manual", None)
            .expect("trait dispatch should succeed");

        assert_eq!(
            fetcher.calls.load(Ordering::SeqCst),
            1,
            "the registered Fetcher impl must be invoked exactly once"
        );
        assert_eq!(
            result.items_fetched, 4242,
            "the trait impl's result must flow back through the dispatch path"
        );
        assert!(
            adapter.in_full_refresh(),
            "a Fetcher (feed-style) adapter joins the full-refresh sweep by default"
        );
    }

    #[test]
    fn directory_outcome_fetcher_maps_through_dispatch_and_skips_sweep() {
        // ADR 0069 T2: a directory-style adapter returns `RefreshOutcome::Directory`,
        // which the dispatch half maps onto the unified `SourceIngestionResult` shape
        // (entries_fetched -> items_fetched, etc.) exactly as the retired `Directory`
        // arm did — and it stays OUT of the full-refresh sweep (`joins_full_refresh`
        // = false), matching the pre-migration membership.
        use super::{Fetcher, RefreshBehavior, RefreshContext, RefreshOutcome, RuntimeAdapter};
        use crate::app_state::AppState;
        use crate::storage::{open_in_memory_database, CompanyRegistryRefreshResult};

        struct ScriptedDirectory;

        impl Fetcher for ScriptedDirectory {
            fn refresh(
                &self,
                _state: &AppState,
                _ctx: &RefreshContext,
            ) -> Result<RefreshOutcome, String> {
                Ok(RefreshOutcome::Directory(CompanyRegistryRefreshResult {
                    adapter_id: "scripted-directory".to_owned(),
                    entries_fetched: 7,
                    entries_upserted: 5,
                    entries_deactivated: 2,
                    fetched_at: "2026-07-15T00:00:00Z".to_owned(),
                }))
            }

            fn joins_full_refresh(&self) -> bool {
                false
            }
        }

        let adapter = RuntimeAdapter {
            id: "scripted-directory",
            behavior: RefreshBehavior::Fetcher(&ScriptedDirectory),
        };
        let state = AppState::new(open_in_memory_database().expect("db"));

        let result = adapter
            .refresh(&state, "manual", None)
            .expect("directory dispatch should succeed");

        assert_eq!(result.items_fetched, 7, "entries_fetched -> items_fetched");
        assert_eq!(result.items_created, 5, "entries_upserted -> items_created");
        assert_eq!(
            result.items_unmatched, 2,
            "entries_deactivated -> items_unmatched"
        );
        assert!(result.fetched_at.is_some());
        assert!(
            !adapter.in_full_refresh(),
            "a directory-style adapter is excluded from the full-refresh sweep"
        );
    }

    #[test]
    fn calendar_style_fetcher_receives_ctx_date() {
        // ADR 0069 T2: the calendar adapter needs the optional date; the trait carries
        // it through `RefreshContext`, replacing the retired `Calendar` fn-pointer arm.
        use super::{
            empty_source_result, Fetcher, RefreshBehavior, RefreshContext, RefreshOutcome,
            RuntimeAdapter,
        };
        use crate::app_state::AppState;
        use crate::storage::open_in_memory_database;
        use std::sync::Mutex;

        struct DateRecordingFetcher {
            seen: Mutex<Option<String>>,
        }

        impl Fetcher for DateRecordingFetcher {
            fn refresh(
                &self,
                _state: &AppState,
                ctx: &RefreshContext,
            ) -> Result<RefreshOutcome, String> {
                *self.seen.lock().expect("lock") = ctx.date.map(|d| d.to_owned());
                Ok(RefreshOutcome::Ingestion(empty_source_result("calendar")))
            }
        }

        let fetcher: &'static DateRecordingFetcher = Box::leak(Box::new(DateRecordingFetcher {
            seen: Mutex::new(None),
        }));
        let adapter = RuntimeAdapter {
            id: "calendar",
            behavior: RefreshBehavior::Fetcher(fetcher),
        };
        let state = AppState::new(open_in_memory_database().expect("db"));

        adapter
            .refresh(&state, "manual", Some("2026-06-01"))
            .expect("calendar dispatch should succeed");

        assert_eq!(
            fetcher.seen.lock().expect("lock").as_deref(),
            Some("2026-06-01"),
            "the trait impl must receive the dispatch date via RefreshContext"
        );
    }

    #[test]
    fn full_refresh_sweep_membership_is_pinned() {
        // ADR 0069 T2 tripwire: the strangler migration must not change which sources
        // join the "refresh all" sweep (feed/calendar-style join; directory and
        // disabled do not). Deliberate post-migration additions extend the pin on
        // purpose — update this list only with a reviewed reason. Additions:
        //   T4: knf-short-selling.
        //   T3: gpw-espi-ebi — the reconciliation witness now runs a Fetcher and
        //       joins the sweep (it reconciles against Bankier; it does NOT ingest).
        //   v0.56 T4: biznesradar-akcjonariat — the ownership breadth source joins
        //       the sweep (writes aggregator stakes; never ingests into the feed).
        use super::runtime_adapters;

        let members: Vec<&str> = runtime_adapters()
            .iter()
            .filter(|a| a.in_full_refresh())
            .map(|a| a.id)
            .collect();

        assert_eq!(
            members,
            vec![
                "bankier-company-komunikaty",
                "bankier-kalendarium-html",
                "gpw-market-events-rss",
                "bankier-market-rss",
                "knf-short-selling",
                "biznesradar-akcjonariat",
                "yahoo-eod",
                "gpw-espi-ebi",
            ],
            "the full-refresh sweep membership must match the pinned set exactly"
        );
    }

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
