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
        loop {
            std::thread::sleep(BASE_TICK);
            let (next_source, next_registry) = run_tick(&state, source_due, registry_due);
            source_due = next_source;
            registry_due = next_registry;
        }
    });
}

/// One scheduler evaluation. Pure-ish: takes the prior schedule, re-arms due jobs,
/// returns the next schedule. Gating mirrors the UI (license `canUseApp` + poll
/// interval + enabled adapters), so moving the timer to Rust changes *where* the
/// cadence lives, not *whether* it runs.
fn run_tick(
    state: &AppState,
    mut source_due: HashMap<String, i64>,
    mut registry_due: Option<i64>,
) -> (HashMap<String, i64>, Option<i64>) {
    if !crate::commands::licensing::current_license_can_use_app(state) {
        state.set_scheduler_status(SchedulerStatus::default());
        return (HashMap::new(), None);
    }

    let settings = match state.get_settings() {
        Ok(settings) => settings,
        Err(_) => return (source_due, registry_due),
    };
    let adapters = match state.list_source_adapters() {
        Ok(adapters) => adapters,
        Err(_) => return (source_due, registry_due),
    };
    let now = now_ms();

    // Feed/source adapters share the global poll interval (matching the frontend).
    let poll_seconds = settings.poll_interval_seconds;
    let mut next_source: HashMap<String, i64> = HashMap::new();
    if poll_seconds > 0 {
        let interval_ms = poll_seconds * 1000;
        for adapter in adapters
            .iter()
            .filter(|adapter| adapter.enabled && adapter.source_type != REGISTRY_SOURCE_TYPE)
        {
            let due = source_due
                .remove(&adapter.id)
                .unwrap_or_else(|| now + interval_ms + start_jitter_ms(&adapter.id));
            if now >= due {
                let payload = format!("{{\"adapterId\":{:?}}}", adapter.id);
                let _ = state.jobs().reschedule(
                    &format!("{SOURCE_REFRESH_KIND}:{}", adapter.id),
                    SOURCE_REFRESH_KIND,
                    &payload,
                    2,
                );
                next_source.insert(adapter.id.clone(), now + interval_ms);
            } else {
                next_source.insert(adapter.id.clone(), due);
            }
        }
    }

    // Company registry uses its own adapter interval.
    let mut next_registry: Option<i64> = None;
    if let Some(registry) = adapters
        .iter()
        .find(|adapter| adapter.enabled && adapter.source_type == REGISTRY_SOURCE_TYPE)
    {
        if registry.default_poll_interval_seconds > 0 {
            let interval_ms = registry.default_poll_interval_seconds * 1000;
            let due =
                registry_due.unwrap_or_else(|| now + interval_ms + start_jitter_ms(&registry.id));
            if now >= due {
                // The stale window is the registry interval, matching the UI call.
                let payload = format!(
                    "{{\"staleAfterSeconds\":{}}}",
                    registry.default_poll_interval_seconds
                );
                let _ = state.jobs().reschedule(
                    REGISTRY_REFRESH_KIND,
                    REGISTRY_REFRESH_KIND,
                    &payload,
                    2,
                );
                next_registry = Some(now + interval_ms);
            } else {
                next_registry = Some(due);
            }
        }
    }
    registry_due = next_registry;

    state.set_scheduler_status(SchedulerStatus {
        source_next_due_ms: next_source.clone(),
        registry_next_due_ms: registry_due,
    });
    (next_source, registry_due)
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
}
