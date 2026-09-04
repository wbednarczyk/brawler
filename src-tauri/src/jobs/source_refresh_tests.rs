use super::{
    refresh_source_direct, refresh_source_for_trigger, should_bootstrap_company_directories,
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
    // (entries_fetched -> items_fetched, etc.) and stays OUT of the full-refresh
    // sweep (`joins_full_refresh` = false).
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
    // it through `RefreshContext`.
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
fn a_failing_source_refresh_records_the_adapter_error_on_its_row() {
    // Per-surface VISIBILITY test for `FailureSurface::SourcesAdapterHealth`
    // (ADR 0091 dec. 3, epic #40 S3): the source-refresh family is classified as
    // owning its failure surface exclusively, which is only legitimate if a
    // failed refresh really does state itself on the Sources screen. A scripted
    // failing fetcher for a REAL registered adapter must leave `last_error` +
    // `last_error_at` on that adapter's row — the datum the Sources screen reads.
    use super::{sweep_adapters, Fetcher, RefreshBehavior, RefreshContext, RefreshOutcome};
    use crate::app_state::AppState;
    use crate::jobs::failure_surface::{failure_surface, FailureSurface};
    use crate::jobs::scheduler::SOURCE_REFRESH_KIND;
    use crate::storage::open_in_memory_database;

    struct FailingFetcher;

    impl Fetcher for FailingFetcher {
        fn refresh(
            &self,
            _state: &AppState,
            _ctx: &RefreshContext,
        ) -> Result<RefreshOutcome, String> {
            Err("HTTP 503 from the publisher".to_owned())
        }
    }

    assert_eq!(
        failure_surface(SOURCE_REFRESH_KIND),
        Some(FailureSurface::SourcesAdapterHealth),
        "the scheduled refresh is classified onto the Sources surface"
    );

    let adapter = super::RuntimeAdapter {
        id: crate::source_adapters::bankier_rss::ADAPTER_ID,
        behavior: RefreshBehavior::Fetcher(&FailingFetcher),
    };
    let state = AppState::new(open_in_memory_database().expect("db"));

    let swept = sweep_adapters(&state, std::slice::from_ref(&adapter), "scheduler");
    assert!(
        swept.is_err(),
        "every attempted source failed → the sweep errs"
    );

    let row = state
        .list_source_adapters()
        .expect("adapters")
        .into_iter()
        .find(|entry| entry.id == crate::source_adapters::bankier_rss::ADAPTER_ID)
        .expect("the adapter is registered");
    assert_eq!(
        row.last_error.as_deref(),
        Some("HTTP 503 from the publisher"),
        "the failure is stated on the source's own row (the Sources surface)"
    );
    assert!(
        row.last_error_at.is_some(),
        "and stamped, so the screen can show when it broke"
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
    //   v0.58 A2: biznesradar-rekomendacje — the analyst-recommendation source
    //       joins the sweep (feeds the append-only recommendation store).
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
            "biznesradar-rekomendacje",
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
/// Owner rule (2026-07-21, after the live KNF UNIQUE-constraint abort): one
/// source must NEVER block the refresh of the others. A failing adapter is
/// recorded on ITS OWN row and the sweep continues; the sweep as a whole
/// errors only when EVERY enabled source failed.
#[test]
fn a_failing_source_does_not_block_the_rest_of_the_sweep() {
    use super::{
        empty_source_result, sweep_adapters, Fetcher, RefreshBehavior, RefreshContext,
        RefreshOutcome, RuntimeAdapter,
    };
    use crate::app_state::AppState;
    use crate::storage::open_in_memory_database;

    struct FailingFetcher;
    impl Fetcher for FailingFetcher {
        fn refresh(
            &self,
            _state: &AppState,
            _ctx: &RefreshContext,
        ) -> Result<RefreshOutcome, String> {
            Err("sqlite error: UNIQUE constraint failed: short_positions.id".to_owned())
        }
    }
    struct HealthyFetcher;
    impl Fetcher for HealthyFetcher {
        fn refresh(
            &self,
            _state: &AppState,
            _ctx: &RefreshContext,
        ) -> Result<RefreshOutcome, String> {
            let mut result = empty_source_result("healthy");
            result.items_fetched = 7;
            Ok(RefreshOutcome::Ingestion(result))
        }
    }

    let failing: &'static FailingFetcher = Box::leak(Box::new(FailingFetcher));
    let healthy: &'static HealthyFetcher = Box::leak(Box::new(HealthyFetcher));
    // The failing adapter comes FIRST — the pre-fix sweep aborts here and the
    // healthy source never runs, which is exactly the live regression.
    let adapters = vec![
        RuntimeAdapter {
            id: "knf-short-selling",
            behavior: RefreshBehavior::Fetcher(failing),
        },
        RuntimeAdapter {
            id: "biznesradar-rekomendacje",
            behavior: RefreshBehavior::Fetcher(healthy),
        },
    ];
    let state = AppState::new(open_in_memory_database().expect("db"));

    let result = sweep_adapters(&state, &adapters, "manual")
        .expect("a single failing source must not fail the whole sweep");
    assert_eq!(
        result.items_fetched, 7,
        "the healthy source behind the failing one must still refresh"
    );

    let row = state
        .list_source_adapters()
        .expect("adapter list query")
        .into_iter()
        .find(|a| a.id == "knf-short-selling")
        .expect("knf row exists");
    assert!(
        row.last_error
            .as_deref()
            .is_some_and(|e| e.contains("UNIQUE constraint failed")),
        "the failure lands on the FAILING source's own row, got: {:?}",
        row.last_error
    );

    // All-failed is still a sweep-level error (nothing refreshed is not a success).
    let all_failing = vec![RuntimeAdapter {
        id: "knf-short-selling",
        behavior: RefreshBehavior::Fetcher(failing),
    }];
    assert!(
        sweep_adapters(&state, &all_failing, "manual").is_err(),
        "a sweep where EVERY enabled source failed must still report an error"
    );
}

/// C5 guard — the sweep is the single `last_error` writer for a failing
/// adapter, and it must preserve the adapter's CURATED context, never clobber
/// it with a rawer message. An adapter path that records a curated error on
/// its own row and then propagates that SAME curated context in its `Err`
/// (the shape every inner adapter path follows, incl. the bankier per-company
/// paths at ~567/583/610) must leave the curated text on the row after the
/// sweep. If a future refactor makes the sweep record a generic/raw message
/// instead of the adapter's `Err`, or an inner path stops carrying its curated
/// context in the `Err`, this reddens. The sweep write STAYS (the KNF
/// ingest-failure path relies on it); this pins "final == curated context".
#[test]
fn sweep_records_the_curated_adapter_error_not_a_rawer_message() {
    use super::{
        sweep_adapters, Fetcher, RefreshBehavior, RefreshContext, RefreshOutcome, RuntimeAdapter,
    };
    use crate::app_state::AppState;
    use crate::storage::open_in_memory_database;

    const CURATED: &str = "CDR: komunikaty fetch failed (HTTP 503)";

    struct CuratedFetcher;
    impl Fetcher for CuratedFetcher {
        fn refresh(
            &self,
            state: &AppState,
            _ctx: &RefreshContext,
        ) -> Result<RefreshOutcome, String> {
            // Mirror an inner adapter path: record the curated context on the
            // row, then propagate the SAME curated context in the Err so the
            // sweep (the single writer) records the curated message verbatim.
            let _ = state.record_source_adapter_error("knf-short-selling", CURATED);
            Err(CURATED.to_owned())
        }
    }

    let curated: &'static CuratedFetcher = Box::leak(Box::new(CuratedFetcher));
    let adapters = vec![RuntimeAdapter {
        id: "knf-short-selling",
        behavior: RefreshBehavior::Fetcher(curated),
    }];
    let state = AppState::new(open_in_memory_database().expect("db"));

    // Every enabled source failed, so the sweep itself errors — but the row
    // must still carry the curated context (that is the whole point).
    assert!(sweep_adapters(&state, &adapters, "manual").is_err());

    let row = state
        .list_source_adapters()
        .expect("adapter list query")
        .into_iter()
        .find(|a| a.id == "knf-short-selling")
        .expect("knf row exists");
    assert_eq!(
        row.last_error.as_deref(),
        Some(CURATED),
        "the sweep must leave the adapter's curated context on the row, not a rawer message"
    );
}

#[test]
fn manual_refresh_is_one_occurrence_per_adapter_via_the_core() {
    // ADR 0109 dec. 3: the Tauri command and its MCP twin both go through
    // `refresh_source_direct` — asserted here by calling that shared
    // wrapper directly and checking exactly one `job_runs` row lands under
    // `direct:<adapter>`, never a raw queue `run_key`.
    use crate::storage::open_in_memory_database;

    let state = AppState::new(open_in_memory_database().expect("db"));
    // An unregistered adapter id resolves fast (no network) but still
    // exercises the full wrapper: identity resolves, the guard opens, the
    // core runs and errors, the guard settles.
    let result = refresh_source_direct(&state, "not-a-real-adapter", "manual", None);
    assert!(result.is_err(), "an unknown adapter id is a clean error");

    let connection = state.checkout_for_tests().expect("checkout");
    let mut statement = connection
        .prepare("SELECT run_key, status FROM job_runs")
        .expect("prepare");
    let rows: Vec<(String, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query")
        .map(|row| row.expect("row"))
        .collect();
    assert_eq!(
        rows,
        vec![(
            "direct:source-refresh:not-a-real-adapter".to_owned(),
            "failed".to_owned()
        )],
        "exactly one occurrence, under the direct: run_key, terminal"
    );
}

#[test]
fn queue_handler_core_call_is_not_double_counted() {
    // The scheduled queue handler calls `refresh_source_for_trigger`
    // (the unwrapped core) directly — never `refresh_source_direct` — so a
    // scheduled run's occurrence carries the QUEUE job id as its run_key,
    // never a `direct:` one, and there is exactly one row per attempt.
    use crate::storage::open_in_memory_database;

    let state = AppState::new(open_in_memory_database().expect("db"));
    // Simulate the queue handler's own call shape directly (no worker
    // machinery needed to prove the core is unwrapped): the handler in
    // `jobs::handlers::ScheduledSourceRefreshHandler` calls exactly this.
    let _ = refresh_source_for_trigger(&state, "not-a-real-adapter", "scheduler", None);

    let connection = state.checkout_for_tests().expect("checkout");
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM job_runs", [], |row| row.get(0))
        .expect("count");
    assert_eq!(
        count, 0,
        "the unwrapped core writes no occurrence itself — only the queue's \
             dispatch seam (`begin_attempt`) and the direct wrapper do"
    );
}

#[test]
fn manual_refresh_panic_settles_the_occurrence_interrupted() {
    // sol diff R2 #8(d): a panic INSIDE the core, reached through the REAL
    // manual-refresh wrapper (`sweep_adapters`, backing "refresh all
    // sources") — never a hand-simulated `ActivityGuard` — must not strand
    // the occurrence `running` forever. `sweep_adapters` has no unwind
    // boundary of its own, so the panic propagates uncontained to the
    // caller; `ActivityGuard`'s Drop is what settles it `interrupted`
    // (nothing here ever reaches the ordinary `guard.settle(...)` call).
    use super::{
        sweep_adapters, Fetcher, RefreshBehavior, RefreshContext, RefreshOutcome, RuntimeAdapter,
    };
    use crate::app_state::AppState;
    use crate::storage::open_in_memory_database;

    struct PanickingFetcher;
    impl Fetcher for PanickingFetcher {
        fn refresh(
            &self,
            _state: &AppState,
            _ctx: &RefreshContext,
        ) -> Result<RefreshOutcome, String> {
            panic!("boom: manual refresh core panicked");
        }
    }

    let fetcher: &'static PanickingFetcher = Box::leak(Box::new(PanickingFetcher));
    let adapters = vec![RuntimeAdapter {
        id: "knf-short-selling",
        behavior: RefreshBehavior::Fetcher(fetcher),
    }];
    let state = AppState::new(open_in_memory_database().expect("db"));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sweep_adapters(&state, &adapters, "manual")
    }));
    assert!(
        result.is_err(),
        "the panic must actually unwind through sweep_adapters, uncontained"
    );

    let connection = state.checkout_for_tests().expect("checkout");
    let status: String = connection
        .query_row(
            "SELECT status FROM job_runs WHERE activity_key = 'source-refresh:knf-short-selling'",
            [],
            |row| row.get(0),
        )
        .expect("occurrence row");
    assert_eq!(
        status, "interrupted",
        "ActivityGuard's Drop must settle the occurrence interrupted, never strand it running"
    );
}
