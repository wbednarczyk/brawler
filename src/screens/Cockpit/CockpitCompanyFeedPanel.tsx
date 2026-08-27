import type { Company, FeedItem } from "../../api/types";
import { formatDetailTimestamp as formatTimestamp } from "../../shared/format/datetime";
import { feedItemSummary } from "../../app/useNotebookController";
import { CompanyFeedSection } from "../Companies/CompanyFeedSection";
import { useCockpitCompanyFeed } from "./useCockpitCompanyFeed";

// Company-scoped feed panel for the curated dashboard (ADR 0057). Reuses the real
// `CompanyFeedSection` with cockpit-owned selection state (`useCockpitCompanyFeed`);
// the cross-screen actions (Open in Inbox / Note) are intentionally omitted — the
// dashboard panel is self-contained and those stay reachable from the Inbox.
export function CockpitCompanyFeedPanel({
  company,
  feedItems,
  initialSelectedFeedItemId,
  leadWithDetail,
}: {
  company: Company;
  feedItems: FeedItem[];
  initialSelectedFeedItemId?: string | null;
  /** Renders the pre-selected item's detail FIRST, above the list (Spółka
   * `feedItem` workshop tool, owner dogfooding v0.74 item 7) instead of the
   * default inline-under-the-row placement. */
  leadWithDetail?: boolean;
}) {
  const feed = useCockpitCompanyFeed(company, feedItems, initialSelectedFeedItemId);
  return (
    <CompanyFeedSection
      company={company}
      feedItems={feed.items}
      selectedFeedItem={feed.selectedFeedItem}
      toggleFeedItem={feed.toggleFeedItem}
      selectFeedItemFromKeyboard={(event, item) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          feed.toggleFeedItem(item);
        }
      }}
      updateFeedItemState={feed.updateFeedItemState}
      formatTimestamp={formatTimestamp}
      feedItemSummary={feedItemSummary}
      leadWithDetail={leadWithDetail}
    />
  );
}
