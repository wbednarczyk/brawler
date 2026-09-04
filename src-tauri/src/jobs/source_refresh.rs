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
    // Management-holdings extraction (ADR 0083 T5): the same newly ingested periodic
    // reports may carry an as-yet-unparsed management-holdings section. Deterministic
    // and idempotent — its writes are final, not AI proposals.
    crate::jobs::management_holdings_extraction::enqueue_management_extraction_catch_up(
        state, None,
    );
    // Report-history backfill catch-up (v0.57, ADR 0077 amendment): an automated
    // company with NO fetched periodic report gets one automatic backfill enqueued
    // — the trigger parity that makes backfill happen without the user clicking.
    // Idempotent (coverage predicate + stable per-company job id), off-thread on
    // the durable queue, `off`-mode companies skipped with an explicit reason.
    crate::jobs::backfill::enqueue_company_backfill_catch_up(state, None);
    // Red-flag reconciliation (ADR 0083 D8, T7): an expected periodic report whose
    // calendar date passed the grace with no official report ingested raises a
    // `report_delay`. Best-effort — a detection failure never fails the refresh.
    if let Err(error) = state.red_flags().detect_report_delays() {
        log::warn!("module=sources stage=red_flags report_delay detection failed: {error}");
    }
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
            id: sa::biznesradar_recommendations::ADAPTER_ID,
            behavior: RefreshBehavior::Fetcher(
                &sa::biznesradar_recommendations::BiznesRadarRecommendationsAdapter,
            ),
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
    let adapters = runtime_adapters();
    sweep_adapters(state, &adapters, trigger)
}

/// The full-refresh sweep over the given adapters. Extracted so tests can drive
/// it with scripted adapters (the runtime registry is static).
fn sweep_adapters(
    state: &app_state::AppState,
    adapters: &[RuntimeAdapter],
    trigger: &str,
) -> Result<storage::SourceIngestionResult, String> {
    let mut total = empty_source_result("all");
    // Owner rule (2026-07-21, live KNF UNIQUE-constraint abort): one source must
    // NEVER block the refresh of the others. A failure is recorded on the failing
    // source's own row (the Sources screen renders per-row errors) and the sweep
    // continues. The sweep as a whole errors only when EVERY attempted source
    // failed — "nothing refreshed" is not a success, but a partial sweep is.
    let mut attempted = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for adapter in adapters.iter().filter(|a| a.in_full_refresh()) {
        attempted += 1;
        // Direct-activity registry (ADR 0109 dec. 3): one `source-refresh:<adapter>`
        // occurrence per adapter for the manual/MCP "refresh all sources" sweep —
        // the scheduled per-adapter queue kind writes its OWN occurrence via the
        // queue's dispatch seam (`jobs::queue`), so this path never double-counts
        // it (this sweep is reached only by the manual/MCP "refresh all" entry
        // points, never by the queue).
        let identity = state.checkout().ok().and_then(|connection| {
            crate::jobs::activity_identity::identity_for_job(
                crate::jobs::scheduler::SOURCE_REFRESH_KIND,
                &format!("direct:{}", adapter.id),
                &json!({ "adapterId": adapter.id }).to_string(),
                &connection,
            )
        });
        let guard =
            identity.and_then(|identity| crate::storage::activity_registry::start(state, identity));
        let outcome = refresh_optional_source(state, adapter, trigger);
        if let Some(guard) = guard {
            guard.settle(outcome.as_ref().map(|_| ()).map_err(|e| e.as_str()));
        }
        match outcome {
            Ok(result) => {
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
            Err(error) => {
                log::warn!(
                    "module=source_refresh stage=sweep adapter={} status=failed error={error}",
                    adapter.id
                );
                let _ = state.record_source_adapter_error(adapter.id, &error);
                failures.push(format!("{}: {error}", adapter.id));
            }
        }
    }

    if attempted > 0 && failures.len() == attempted {
        return Err(failures.join(" | "));
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

    // Insider attachment-PDF tier (ADR 0083 D6, plan v0.57 T4b): fetch + parse the
    // MAR art. 19 notification documents for newly cover-note-parsed insider filings
    // and fill the NULL volume/price/tx_date figures. Runs here (source/refresh lane,
    // network off the ingestion write path), attempt-once per filing. Best-effort — a
    // failure never fails the refresh.
    match crate::jobs::insider_attachment::fetch_and_parse_insider_attachments(
        state,
        &document_fetcher,
    ) {
        Ok(summary) if summary.filings_attempted > 0 => log::info!(
            "module=insider_attachment stage=sweep attempted={} parsed={} filled={} \
             appended={} conflicts={} no_attachment={} no_text_layer={} not_found={} retry={}",
            summary.filings_attempted,
            summary.parsed,
            summary.filled,
            summary.appended,
            summary.conflicts,
            summary.no_attachment,
            summary.no_text_layer,
            summary.not_found,
            summary.fetch_retry,
        ),
        Ok(_) => {}
        Err(error) => {
            let _ = state.record_source_adapter_error(
                adapter_id,
                &format!("insider attachment tier failed: {error}"),
            );
        }
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

/// Direct-activity wrapper for a single-adapter refresh (ADR 0109 dec. 3): the
/// scheduled queue handler (`ScheduledSourceRefreshHandler`) calls
/// [`refresh_source_for_trigger`] directly and writes its OWN occurrence via
/// the queue's dispatch seam, so this wrapper is for the awaited command paths
/// only (the manual Tauri command and its MCP twin) — nothing double-counts.
pub fn refresh_source_direct(
    state: &app_state::AppState,
    adapter_id: &str,
    trigger: &str,
    date: Option<&str>,
) -> Result<storage::SourceIngestionResult, String> {
    let identity = state.checkout().ok().and_then(|connection| {
        crate::jobs::activity_identity::identity_for_job(
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            &format!("direct:{adapter_id}"),
            &json!({ "adapterId": adapter_id }).to_string(),
            &connection,
        )
    });
    let guard =
        identity.and_then(|identity| crate::storage::activity_registry::start(state, identity));
    let outcome = refresh_source_for_trigger(state, adapter_id, trigger, date);
    if let Some(guard) = guard {
        guard.settle(outcome.as_ref().map(|_| ()).map_err(|e| e.as_str()));
    }
    outcome
}

/// Direct-activity wrapper for the company-registry refresh (ADR 0109 dec. 3):
/// the scheduled queue handler (`ScheduledRegistryRefreshHandler`) calls
/// [`refresh_company_directories_for_trigger`] directly and writes its own
/// occurrence via the queue's dispatch seam, so this wrapper is for the
/// awaited command paths only (`refresh_gpw_company_registry` and its
/// stale-checked twin) — nothing double-counts.
pub fn refresh_company_directories_direct(
    state: &app_state::AppState,
    trigger: &str,
) -> Result<storage::CompanyRegistryRefreshResult, String> {
    let identity = state.checkout().ok().and_then(|connection| {
        crate::jobs::activity_identity::identity_for_job(
            crate::jobs::scheduler::REGISTRY_REFRESH_KIND,
            &format!("direct:{}", crate::jobs::scheduler::REGISTRY_REFRESH_KIND),
            "{}",
            &connection,
        )
    });
    let guard =
        identity.and_then(|identity| crate::storage::activity_registry::start(state, identity));
    let outcome = refresh_company_directories_for_trigger(state, trigger);
    if let Some(guard) = guard {
        guard.settle(outcome.as_ref().map(|_| ()).map_err(|e| e.as_str()));
    }
    outcome
}

#[cfg(test)]
#[path = "source_refresh_tests.rs"]
mod tests;
