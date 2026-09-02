import type { Company, FeedItem } from "../../../api/types";
import { formatDetailTimestamp as formatTimestamp } from "../../../shared/format/datetime";
import { buildFeedItemNoteDraft, feedItemSummary } from "../../../app/useNotebookController";
import { CompanyFeedSection } from "../../Companies/CompanyFeedSection";
import type { Tool } from "../route";
import { useCompanyFeedPanel } from "./useCompanyFeedPanel";

// Company-scoped feed panel for the Spółka `feed`/`feedItem` workshop tools
// (ADR 0057, ADR 0107). Reuses the real `CompanyFeedSection` with its own
// selection state (`useCompanyFeedPanel`); "Open in Inbox" is intentionally
// omitted — that jump stays reachable from the Inbox itself. "Note" is wired
// (sol fix1 item 3): the panel already knows its `company`, so a feed item's
// draft opens the SAME company's `notatnik` tool through `onOpenTool` — no
// cross-company lookup/navigation needed (unlike the Inbox's
// `openFeedItemNoteDraft`, which must first find the item's company).
export function CompanyFeedPanel({
  company,
  feedItems,
  initialSelectedFeedItemId,
  leadWithDetail,
  onOpenTool,
}: {
  company: Company;
  feedItems: FeedItem[];
  initialSelectedFeedItemId?: string | null;
  /** Renders the pre-selected item's detail FIRST, above the list (Spółka
   * `feedItem` workshop tool, owner dogfooding v0.74 item 7) instead of the
   * default inline-under-the-row placement. */
  leadWithDetail?: boolean;
  /** Opens another tool on THIS company (ToolRenderContext.onOpenTool) — used
   * only to land a feed item's note draft in the `notatnik` tool. Optional so
   * a caller with no tool host still renders a self-contained panel. */
  onOpenTool?: (tool: Tool) => void;
}) {
  const feed = useCompanyFeedPanel(company, feedItems, initialSelectedFeedItemId);
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
      openFeedItemNoteDraft={
        onOpenTool ? (item) => onOpenTool({ t: "notatnik", draft: buildFeedItemNoteDraft(item) }) : undefined
      }
    />
  );
}
