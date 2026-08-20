import type { Company, FeedItem } from "../../api/types";
import { formatDetailTimestamp as formatTimestamp } from "../../shared/format/datetime";
import { feedItemSummary } from "../../app/useNotebookController";
import { CompanyFeedSection } from "../Companies/CompanyFeedSection";
import { useCockpitCompanyFeed } from "./useCockpitCompanyFeed";

// Company-scoped feed panel for the curated dashboard (ADR 0057). Reuses the real
// `CompanyFeedSection` with cockpit-owned selection state (`useCockpitCompanyFeed`);
// the cross-screen actions (Open in Inbox / Note) are intentionally omitted — the
// dashboard panel is self-contained and those stay reachable from the Inbox.
export function CockpitCompanyFeedPanel({ company, feedItems }: { company: Company; feedItems: FeedItem[] }) {
  const feed = useCockpitCompanyFeed(company, feedItems);
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
    />
  );
}
