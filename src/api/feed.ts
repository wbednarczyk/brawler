import { callCommand } from "./tauri";
import type { FeedDeleteResult, FeedItem, FeedPruneResult } from "./types";
import type { UpdateFeedItemStateInput } from "./generated/UpdateFeedItemStateInput";
import type { PruneOldFeedItemsInput } from "./generated/PruneOldFeedItemsInput";

// Input types GENERATED via ts-rs (ADR 0048).
export type { UpdateFeedItemStateInput } from "./generated/UpdateFeedItemStateInput";
export type { PruneOldFeedItemsInput } from "./generated/PruneOldFeedItemsInput";

export function listFeedItems() {
  return callCommand<FeedItem[]>("list_feed_items");
}

export function updateFeedItemState(input: UpdateFeedItemStateInput) {
  return callCommand<FeedItem>("update_feed_item_state", { input });
}

export function deleteUnsavedFeedItems() {
  return callCommand<FeedDeleteResult>("delete_unsaved_feed_items");
}

export function pruneOldFeedItems(input: PruneOldFeedItemsInput) {
  return callCommand<FeedPruneResult>("prune_old_feed_items", { input });
}
