import { callCommand } from "./tauri";
import type { FeedItem } from "./types";
import type { UpdateFeedItemStateInput } from "./generated/UpdateFeedItemStateInput";

// Input types GENERATED via ts-rs (ADR 0048).
export type { UpdateFeedItemStateInput } from "./generated/UpdateFeedItemStateInput";

export function listFeedItems() {
  return callCommand<FeedItem[]>("list_feed_items");
}

export function updateFeedItemState(input: UpdateFeedItemStateInput) {
  return callCommand<FeedItem>("update_feed_item_state", { input });
}
