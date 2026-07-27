use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::app_state::AppState;
use crate::storage::SchedulerStatus;

pub async fn run_blocking_task<T>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| format!("refresh task failed: {error}"))?
}

/// Source type of the company-registry adapter (scheduled separately from feeds).
const REGISTRY_SOURCE_TYPE: &str = "company_registry";
/// How often the scheduler wakes to re-evaluate due times (responsive to settings
/// and enable/disable changes without restarting). Cadence itself comes from the
/// poll interval, not this tick.
const BASE_TICK: Duration = Duration::from_secs(10);
/// Spread first runs so adapters do not all fire at once (mirrors the frontend
/// start jitter); derived deterministically per adapter id.
const MAX_START_JITTER_SECONDS: i64 = 30;

/// Daily cadence for the app-open auto-triggers: the morning briefing (ADR 0068
/// §T5) and the BiznesRadar-primary fundamentals pull (ADR 0086 dec. 2). While
/// the app is open, each enqueues at most one job per day. Both handlers are
/// idempotent per day (the pull's per-page cadence cache makes an extra run a
/// no-op), so this only governs enqueue churn, not correctness.
const DAILY_INTERVAL_MS: i64 = 86_400_000;

/// Job kind: one scheduled source-adapter refresh. Payload `{adapterId}`.
pub const SOURCE_REFRESH_KIND: &str = "scheduled_source_refresh";
/// Job kind: a scheduled company-registry refresh-if-stale check.
pub const REGISTRY_REFRESH_KIND: &str = "scheduled_registry_refresh";

/// Spawn the Rust-side source scheduler (ADR 0055 / AV5). It **owns the refresh
/// cadence** — replacing the frontend `setInterval`, which the webview throttles
/// when the window is hidden/minimized and the OS can suspend. Each tick re-arms a
/// `source_refresh`/`registry_refresh` job on the durable queue for every adapter
/// whose interval has elapsed (the worker executes it; detection rides refresh
/// completion). Runs only while the app is open. The next-due snapshot is published
/// to `AppState` for the UI to render. A poisoned settings/license read just skips
/// the tick.
pub fn spawn(state: AppState) {
    // Run on a dedicated blocking thread (like the queue worker). A tick is light
    // DB work — it only re-arms queue rows and publishes the next-due snapshot; the
    // actual network refresh runs on the worker — so a blocking loop is simplest.
    tauri::async_runtime::spawn_blocking(move || {
        let mut source_due: HashMap<String, i64> = HashMap::new();
        let mut registry_due: Option<i64> = None;
        let mut briefing_due: Option<i64> = None;
        let mut aggregator_pull_due: Option<i64> = None;
        let mut fx_pull_due: Option<i64> = None;
        loop {
            std::thread::sleep(BASE_TICK);
            let (next_source, next_registry) = run_tick(&state, source_due, registry_due);
            source_due = next_source;
            registry_due = next_registry;

            // Daily morning-briefing auto-trigger (ADR 0068 §T5). App-open only,
            // license-gated like the refresh cadence; the handler stays idempotent
            // per day so an app restart (which resets `briefing_due`) re-composes at
            // most once per day.
            if crate::commands::licensing::current_license_can_use_app(&state) {
                // Each app-open daily job goes through the ONE helper — the due
                // state is reassigned inside it, so a new daily job cannot forget
                // the reassignment and fire on every 60s tick (review 2026-07-22).
                fire_daily(&mut briefing_due, || {
                    crate::jobs::morning_briefing::enqueue_daily_briefing(&state)
                });
                // Daily BiznesRadar-primary fundamentals pull (ADR 0086 dec. 2).
                // The per-(company, page_kind) cadence cache inside the pull makes
                // an app-restart re-fire a cheap no-op — no page is fetched twice
                // inside its 24h window.
                fire_daily(&mut aggregator_pull_due, || {
                    crate::jobs::aggregator_fundamentals_pull::enqueue_daily_pull(&state)
                });
                // Daily NBP FX pull (ADR 0089 dec. 2). One recurring row keeps the
                // needed currencies' mids current; the job self-heals a missing
                // currency into a full-history backfill. App-open only, idempotent
                // per day under its singleton job id.
                fire_daily(&mut fx_pull_due, || {
                    crate::jobs::fx_daily_pull::enqueue_fx_daily_pull(&state)
                });
            }
        }
    });
}

/// Minimal view of a source adapter the schedule math needs. Decouples the pure
/// due-time computation from the full `SourceAdapter` row (and from the keychain /
/// `AppState` reads in [`run_tick`]) so the cadence logic is unit-testable offline.
struct TickAdapter {
    id: String,
    enabled: bool,
    is_registry: bool,
    default_poll_interval_seconds: i64,
}

/// What one evaluation decided: which jobs to re-arm now, and the next schedule.
/// Side-effect-free — [`run_tick`] applies the re-arms and publishes the snapshot.
#[derive(Debug, Default, PartialEq)]
struct ScheduleDecision {
    /// Source adapter ids whose interval elapsed this tick → re-arm a refresh job.
    source_to_refresh: Vec<String>,
    /// Next due time (epoch ms) per source adapter id — the published snapshot.
    next_source: HashMap<String, i64>,
    /// `Some(staleAfterSeconds)` when the registry refresh is due this tick.
    registry_to_refresh: Option<i64>,
    /// Next registry due time (epoch ms), or `None` when no registry is scheduled.
    next_registry: Option<i64>,
}

/// Pure scheduler math: given `now`, the global poll interval, the enabled
/// adapters, and the prior schedule, decide what is due and the next schedule.
/// No keychain, no `AppState`, no side effects — this is the seam the tick tests
/// drive. Gating that needs the OS keychain (license) stays in [`run_tick`].
fn compute_schedule(
    now: i64,
    poll_seconds: i64,
    adapters: &[TickAdapter],
    mut prior_source_due: HashMap<String, i64>,
    prior_registry_due: Option<i64>,
) -> ScheduleDecision {
    let mut decision = ScheduleDecision::default();

    // Feed/source adapters share the global poll interval (matching the
    // frontend) — unless an adapter declares a LONGER default interval (e.g.
    // the daily market_data pulls, ADR 0082): the effective interval is
    // max(global, adapter default) so a 900s global poll never hammers a
    // once-a-day source.
    if poll_seconds > 0 {
        for adapter in adapters.iter().filter(|a| a.enabled && !a.is_registry) {
            let interval_ms = poll_seconds.max(adapter.default_poll_interval_seconds) * 1000;
            let due = prior_source_due
                .remove(&adapter.id)
                .unwrap_or_else(|| now + interval_ms + start_jitter_ms(&adapter.id));
            if now >= due {
                decision.source_to_refresh.push(adapter.id.clone());
                decision
                    .next_source
                    .insert(adapter.id.clone(), now + interval_ms);
            } else {
                decision.next_source.insert(adapter.id.clone(), due);
            }
        }
    }

    // Company registry uses its own adapter interval.
    if let Some(registry) = adapters.iter().find(|a| a.enabled && a.is_registry) {
        if registry.default_poll_interval_seconds > 0 {
            let interval_ms = registry.default_poll_interval_seconds * 1000;
            let due = prior_registry_due
                .unwrap_or_else(|| now + interval_ms + start_jitter_ms(&registry.id));
            if now >= due {
                // The stale window is the registry interval, matching the UI call.
                decision.registry_to_refresh = Some(registry.default_poll_interval_seconds);
                decision.next_registry = Some(now + interval_ms);
            } else {
                decision.next_registry = Some(due);
            }
        }
    }

    decision
}

/// One scheduler evaluation. The keychain-gated shell: it applies the license gate
/// (which reads the OS keychain, so this entry point is not offline-testable), then
/// delegates everything else to [`apply_tick`]. Gating mirrors the UI (license
/// `canUseApp` + poll interval + enabled adapters), so moving the timer to Rust
/// changes *where* the cadence lives, not *whether* it runs.
fn run_tick(
    state: &AppState,
    source_due: HashMap<String, i64>,
    registry_due: Option<i64>,
) -> (HashMap<String, i64>, Option<i64>) {
    if !crate::commands::licensing::current_license_can_use_app(state) {
        state.set_scheduler_status(SchedulerStatus::default());
        return (HashMap::new(), None);
    }
    apply_tick(state, source_due, registry_due)
}

/// The post-license-gate body of one tick — `AppState`-bound but keychain-free, so
/// it is exercisable offline. Loads settings and adapters, delegates the cadence
/// math to the pure [`compute_schedule`] seam, then applies the decision: re-arms
/// the due refresh jobs on the durable queue and publishes the next-due snapshot.
fn apply_tick(
    state: &AppState,
    source_due: HashMap<String, i64>,
    registry_due: Option<i64>,
) -> (HashMap<String, i64>, Option<i64>) {
    let settings = match state.get_settings() {
        Ok(settings) => settings,
        Err(_) => return (source_due, registry_due),
    };
    let adapters = match state.list_source_adapters() {
        Ok(adapters) => adapters,
        Err(_) => return (source_due, registry_due),
    };

    let tick_adapters: Vec<TickAdapter> = adapters
        .iter()
        .map(|adapter| TickAdapter {
            id: adapter.id.clone(),
            enabled: adapter.enabled,
            is_registry: adapter.source_type == REGISTRY_SOURCE_TYPE,
            default_poll_interval_seconds: adapter.default_poll_interval_seconds,
        })
        .collect();

    let decision = compute_schedule(
        now_ms(),
        settings.poll_interval_seconds,
        &tick_adapters,
        source_due,
        registry_due,
    );

    for adapter_id in &decision.source_to_refresh {
        let payload = format!("{{\"adapterId\":{adapter_id:?}}}");
        let _ = state.jobs().reschedule(
            &format!("{SOURCE_REFRESH_KIND}:{adapter_id}"),
            SOURCE_REFRESH_KIND,
            &payload,
            2,
        );
    }
    if let Some(stale_after_seconds) = decision.registry_to_refresh {
        let payload = format!("{{\"staleAfterSeconds\":{stale_after_seconds}}}");
        let _ = state
            .jobs()
            .reschedule(REGISTRY_REFRESH_KIND, REGISTRY_REFRESH_KIND, &payload, 2);
    }

    state.set_scheduler_status(SchedulerStatus {
        source_next_due_ms: decision.next_source.clone(),
        registry_next_due_ms: decision.next_registry,
    });
    (decision.next_source, decision.next_registry)
}

/// Pure daily-cadence decision for the morning briefing: fire when there is no
/// prior due time (first tick after app open) or the prior due time has elapsed;
/// the next due time is always `now + DAILY_INTERVAL_MS`. Testable offline.
fn compute_daily_due(now: i64, prior_due: Option<i64>) -> (bool, i64) {
    match prior_due {
        Some(due) if now < due => (false, due),
        _ => (true, now + DAILY_INTERVAL_MS),
    }
}

/// Run one app-open daily job's tick: fire `enqueue` when due and ALWAYS
/// reassign the due state — the triad lives here once so per-job copies cannot
/// drop the stateful line.
fn fire_daily(due: &mut Option<i64>, enqueue: impl FnOnce()) {
    let (fire, next) = compute_daily_due(now_ms(), *due);
    if fire {
        enqueue();
    }
    *due = Some(next);
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

/// Deterministic per-adapter first-run jitter in `[0, MAX_START_JITTER_SECONDS)`.
/// Deterministic (not random) so it is test-stable and resume-safe.
fn start_jitter_ms(adapter_id: &str) -> i64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    adapter_id.hash(&mut hasher);
    let bucket = (hasher.finish() % MAX_START_JITTER_SECONDS as u64) as i64;
    bucket * 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_daily_due_fires_on_first_tick_then_daily() {
        // First tick after app open (no prior due) fires immediately and schedules
        // the next one a day out; a tick before the next due is a no-op; a tick at
        // or after it fires again. Idempotency of same-day composes is the handler's
        // job (this only governs enqueue cadence).
        let (fire, next) = compute_daily_due(NOW, None);
        assert!(fire, "first tick fires");
        assert_eq!(next, NOW + DAILY_INTERVAL_MS);

        let (fire, next_again) = compute_daily_due(NOW + 1_000, Some(next));
        assert!(!fire, "not due yet");
        assert_eq!(next_again, next, "keeps the pending due time");

        let (fire, next_day) = compute_daily_due(next, Some(next));
        assert!(fire, "fires again once the day elapsed");
        assert_eq!(next_day, next + DAILY_INTERVAL_MS);
    }

    #[test]
    fn start_jitter_is_deterministic_and_bounded() {
        // Deterministic per adapter id (test-stable, resume-safe) and within the
        // [0, MAX_START_JITTER_SECONDS) window so first runs spread out.
        let a = start_jitter_ms("gpw-espi");
        let b = start_jitter_ms("gpw-espi");
        let c = start_jitter_ms("bankier-rss");
        assert_eq!(a, b, "same adapter id -> same jitter");
        assert!((0..MAX_START_JITTER_SECONDS * 1000).contains(&a));
        assert!((0..MAX_START_JITTER_SECONDS * 1000).contains(&c));
    }

    fn source(id: &str, enabled: bool) -> TickAdapter {
        TickAdapter {
            id: id.to_owned(),
            enabled,
            is_registry: false,
            default_poll_interval_seconds: 0,
        }
    }

    fn registry(id: &str, enabled: bool, poll: i64) -> TickAdapter {
        TickAdapter {
            id: id.to_owned(),
            enabled,
            is_registry: true,
            default_poll_interval_seconds: poll,
        }
    }

    const NOW: i64 = 1_700_000_000_000;

    #[test]
    fn compute_schedule_arms_due_sources_and_preserves_pending() {
        let adapters = [
            source("due", true),
            source("pending", true),
            source("off", false),
        ];
        let prior = HashMap::from([
            ("due".to_owned(), NOW - 1),
            ("pending".to_owned(), NOW + 5_000),
        ]);

        let decision = compute_schedule(NOW, 60, &adapters, prior, None);

        // Due adapter re-armed; pending preserved; disabled never scheduled.
        assert_eq!(decision.source_to_refresh, vec!["due".to_owned()]);
        assert_eq!(decision.next_source.get("due"), Some(&(NOW + 60_000)));
        assert_eq!(decision.next_source.get("pending"), Some(&(NOW + 5_000)));
        assert!(!decision.next_source.contains_key("off"));
        assert_eq!(decision.registry_to_refresh, None);
        assert_eq!(decision.next_registry, None);
    }

    #[test]
    fn compute_schedule_honors_a_longer_per_adapter_interval() {
        // A market_data adapter declares a daily cadence; the global feed poll
        // (e.g. 900s) must not hammer it — the effective interval is
        // max(global, adapter default) (ADR 0082 rate-limit policy).
        let daily = TickAdapter {
            id: "yahoo-eod".to_owned(),
            enabled: true,
            is_registry: false,
            default_poll_interval_seconds: 86_400,
        };
        let prior = HashMap::from([("yahoo-eod".to_owned(), NOW - 1)]);

        let decision = compute_schedule(NOW, 900, &[daily], prior, None);

        assert_eq!(decision.source_to_refresh, vec!["yahoo-eod".to_owned()]);
        assert_eq!(
            decision.next_source.get("yahoo-eod"),
            Some(&(NOW + 86_400_000)),
            "re-armed on its own daily interval, not the 900s global poll"
        );
    }

    #[test]
    fn compute_schedule_first_run_defers_behind_jitter() {
        // With no prior schedule, an adapter's first due time is now + interval +
        // jitter, so it is never re-armed on the very first tick.
        let adapters = [source("fresh", true)];

        let decision = compute_schedule(NOW, 60, &adapters, HashMap::new(), None);

        assert!(decision.source_to_refresh.is_empty());
        assert_eq!(
            decision.next_source.get("fresh"),
            Some(&(NOW + 60_000 + start_jitter_ms("fresh")))
        );
    }

    #[test]
    fn compute_schedule_skips_sources_when_poll_disabled() {
        // poll_seconds == 0 disables source cadence entirely; the registry still runs.
        let adapters = [source("a", true), registry("reg", true, 600)];

        let decision = compute_schedule(NOW, 0, &adapters, HashMap::new(), Some(NOW - 1));

        assert!(decision.source_to_refresh.is_empty());
        assert!(decision.next_source.is_empty());
        assert_eq!(decision.registry_to_refresh, Some(600));
        assert_eq!(decision.next_registry, Some(NOW + 600_000));
    }

    #[test]
    fn compute_schedule_arms_registry_when_due_and_preserves_when_pending() {
        let adapters = [registry("reg", true, 600)];

        let due = compute_schedule(NOW, 60, &adapters, HashMap::new(), Some(NOW - 1));
        assert_eq!(due.registry_to_refresh, Some(600));
        assert_eq!(due.next_registry, Some(NOW + 600_000));

        let pending = compute_schedule(NOW, 60, &adapters, HashMap::new(), Some(NOW + 10_000));
        assert_eq!(pending.registry_to_refresh, None);
        assert_eq!(pending.next_registry, Some(NOW + 10_000));
    }

    /// Closes the integration gap above the pure seam: `apply_tick` is the
    /// keychain-free body of the tick, so this drives it against a real in-memory
    /// `AppState` (seeded adapters, settings, durable queue) and asserts the wiring
    /// — adapter mapping → `compute_schedule` → re-arm on the queue → published
    /// snapshot — not just the math. The license-gated `run_tick` shell stays
    /// uncoverable offline by design (it reads the OS keychain).
    #[test]
    fn apply_tick_arms_due_source_adapters_and_enqueues_refresh_jobs() {
        use crate::storage::{open_in_memory_database, SettingsUpdate};

        let state = AppState::new(open_in_memory_database().expect("db"));
        state
            .update_settings(SettingsUpdate {
                poll_interval_seconds: Some(900),
                ..Default::default()
            })
            .expect("settings");

        let enabled_sources: Vec<String> = state
            .list_source_adapters()
            .expect("adapters")
            .into_iter()
            .filter(|adapter| adapter.enabled && adapter.source_type != REGISTRY_SOURCE_TYPE)
            .map(|adapter| adapter.id)
            .collect();
        assert!(
            !enabled_sources.is_empty(),
            "the seed must include enabled source adapters for this to exercise the path"
        );

        // Force every enabled source adapter due now (prior due in the distant past).
        let forced_due: HashMap<String, i64> =
            enabled_sources.iter().map(|id| (id.clone(), 0)).collect();
        let (next_source, _next_registry) = apply_tick(&state, forced_due, None);

        // Each due adapter is re-armed with a future next-due time (now + interval),
        // and that snapshot is what gets published to `AppState`.
        for id in &enabled_sources {
            let due = next_source.get(id).copied().unwrap_or(0);
            assert!(
                due > now_ms(),
                "adapter {id} should be re-armed into the future"
            );
        }

        // Side effect: a real `scheduled_source_refresh` job landed on the queue,
        // payload-tagged with a real adapter id.
        let job = state
            .jobs()
            .claim_next()
            .expect("claim")
            .expect("a source-refresh job should be enqueued");
        assert_eq!(job.kind, SOURCE_REFRESH_KIND);
        assert!(
            enabled_sources.iter().any(|id| job.payload.contains(id)),
            "refresh payload should name a real adapter id, got {}",
            job.payload
        );
    }
}
