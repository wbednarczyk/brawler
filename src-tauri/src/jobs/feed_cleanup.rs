use crate::{app_state, storage};

pub fn prune_old_feed_items(
    state: &app_state::AppState,
    retention_days: i64,
) -> Result<storage::FeedPruneResult, String> {
    state
        .prune_old_feed_items(retention_days)
        .map_err(|error| error.to_string())
}

pub fn delete_unsaved_feed_items(
    state: &app_state::AppState,
) -> Result<storage::FeedDeleteResult, String> {
    state
        .delete_unsaved_feed_items()
        .map_err(|error| error.to_string())
}
