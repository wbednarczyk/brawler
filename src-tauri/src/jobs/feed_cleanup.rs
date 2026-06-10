use crate::{app_state, storage};

pub fn prune_old_feed_items(
    state: &app_state::AppState,
    retention_days: i64,
) -> Result<storage::FeedPruneResult, String> {
    let started_at = std::time::Instant::now();
    let result = state
        .prune_old_feed_items(retention_days)
        .map_err(|error| {
            record_cleanup_metrics(state, "retention", "failed", 0, started_at);
            error.to_string()
        })?;
    record_cleanup_metrics(
        state,
        "retention",
        "succeeded",
        result.items_deleted,
        started_at,
    );

    Ok(result)
}

pub fn delete_unsaved_feed_items(
    state: &app_state::AppState,
) -> Result<storage::FeedDeleteResult, String> {
    let started_at = std::time::Instant::now();
    let result = state.delete_unsaved_feed_items().map_err(|error| {
        record_cleanup_metrics(state, "unsaved", "failed", 0, started_at);
        error.to_string()
    })?;
    record_cleanup_metrics(
        state,
        "unsaved",
        "succeeded",
        result.items_deleted,
        started_at,
    );

    Ok(result)
}

fn record_cleanup_metrics(
    state: &app_state::AppState,
    mode: &str,
    status: &str,
    items_deleted: usize,
    started_at: std::time::Instant,
) {
    state.increment_runtime_counter(
        "brawler_feed_cleanup_runs_total",
        &[("module", mode), ("status", status)],
    );
    state.observe_runtime_duration_seconds(
        "brawler_feed_cleanup_duration_seconds",
        &[("module", mode), ("status", status)],
        started_at.elapsed().as_secs_f64(),
    );
    if items_deleted > 0 {
        state.add_runtime_counter_value(
            "brawler_feed_cleanup_deleted_total",
            &[("module", mode), ("status", "deleted")],
            items_deleted as f64,
        );
    }
}
