import type { FeedItem } from "../../../api/types";

export type FeedDetailMediaProps = {
  item: FeedItem;
  feedItemSummary: (item: FeedItem) => string;
};

// Media kind (RSS/news, no ESPI markers): title + excerpt is the whole must-see
// tier (ADR 0104 dec. 5 — detail shrinks to its content; InboxMedia.dc.html).
export function FeedDetailMedia({ item, feedItemSummary }: FeedDetailMediaProps) {
  const excerpt = feedItemSummary(item);
  return excerpt ? <p className="feed-detail-excerpt">{excerpt}</p> : null;
}
