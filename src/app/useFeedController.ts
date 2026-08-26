import { useState, type Dispatch, type KeyboardEvent, type SetStateAction } from "react";
import * as feedApi from "../api/feed";
import type { Company, FeedItem } from "../api/types";
import type { InboxStatusFilter } from "../screens/Inbox/inboxTypes";
import type { Section } from "./navigation";
import type { SpolkaTransition } from "./useSpolkaScreenWiring";

type FeedControllerInput = {
  companies: Company[];
  filteredFeedItems: FeedItem[];
  selectedFeedItem: FeedItem | null;
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setFeedError: Dispatch<SetStateAction<string | null>>;
  setFeedState: Dispatch<SetStateAction<FeedItem[]>>;
  setInboxCompanyFilter: Dispatch<SetStateAction<string>>;
  setInboxSignalFilter: Dispatch<SetStateAction<string>>;
  setInboxSourceFilter: Dispatch<SetStateAction<string>>;
  setInboxStatusFilter: Dispatch<SetStateAction<InboxStatusFilter>>;
  setInboxTypeFilter: Dispatch<SetStateAction<string>>;
  setInboxWatchlistFilter: Dispatch<SetStateAction<string>>;
  setSearchQuery: Dispatch<SetStateAction<string>>;
  setSelectedCompanyFeedItemId: Dispatch<SetStateAction<string | null>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
  /** ONE guarded company transition (F3a, sol R2 finding 3): never set
   * company state ahead of the Spółka dirty guard. */
  navigate: (transition: SpolkaTransition) => void;
};

export function useFeedController({
  companies,
  filteredFeedItems,
  selectedFeedItem,
  setActiveSection,
  setFeedError,
  setFeedState,
  setInboxCompanyFilter,
  setInboxSignalFilter,
  setInboxSourceFilter,
  setInboxStatusFilter,
  setInboxTypeFilter,
  setInboxWatchlistFilter,
  setSearchQuery,
  setSelectedCompanyFeedItemId,
  setSelectedFeedItemId,
  navigate,
}: FeedControllerInput) {
  // Today's "Otwórz komunikat" seam (F2 S3, plan decision 6): bumped on every
  // `openInboxItem` call so InboxScreen can raise its S-overlay
  // (`inboxDetailOpen`) from OUTSIDE without the screen reaching into that
  // local state — it just reacts to this prop changing.
  const [inboxDetailActivationToken, setInboxDetailActivationToken] = useState(0);

  function updateFeedItemState(item: FeedItem, update: (item: FeedItem) => FeedItem) {
    const nextItem = update(item);

    feedApi.updateFeedItemState({
      id: nextItem.id,
      read: !nextItem.unread,
      saved: nextItem.saved,
    })
      .then((response) => {
        setFeedState((current) =>
          current.map((item) => (item.id === response.id ? response : item)),
        );
        setFeedError(null);
      })
      .catch((error) => {
        setFeedError(String(error));
      });
  }

  function updateSelectedFeedItem(update: (item: FeedItem) => FeedItem) {
    if (!selectedFeedItem) {
      return;
    }

    updateFeedItemState(selectedFeedItem, update);
  }

  function toggleFeedItemReadState(item: FeedItem) {
    updateFeedItemState(item, (current) => ({
      ...current,
      unread: !current.unread,
    }));
  }

  function markVisibleInboxAsRead() {
    const unreadItems = filteredFeedItems.filter((item) => item.unread);
    if (unreadItems.length === 0) {
      return;
    }

    Promise.all(
      unreadItems.map((item) =>
        feedApi.updateFeedItemState({
          id: item.id,
          read: true,
          saved: item.saved,
        }),
      ),
    )
      .then((responses) => {
        const updatedById = new Map(responses.map((item) => [item.id, item]));
        setFeedState((current) =>
          current.map((item) => updatedById.get(item.id) ?? item),
        );
        setFeedError(null);
      })
      .catch((error) => {
        setFeedError(String(error));
      });
  }

  function selectFeedItemFromKeyboard(event: KeyboardEvent<HTMLElement>, item: FeedItem) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      setSelectedFeedItemId(item.id);
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();

      const rows = Array.from(
        event.currentTarget.parentElement?.querySelectorAll<HTMLElement>("[data-feed-row='true']") ??
          [],
      );
      const currentIndex = rows.indexOf(event.currentTarget);
      const direction = event.key === "ArrowDown" ? 1 : -1;
      const nextIndex = Math.min(Math.max(currentIndex + direction, 0), rows.length - 1);
      const nextRow = rows[nextIndex];
      const nextFeedItemId = nextRow?.dataset.feedItemId;

      if (nextRow && nextFeedItemId) {
        nextRow.focus();
        setSelectedFeedItemId(nextFeedItemId);
      }
    }
  }

  function clearInboxFilters() {
    setSearchQuery("");
    setInboxWatchlistFilter("all");
    setInboxCompanyFilter("all");
    setInboxTypeFilter("all");
    setInboxSignalFilter("all");
    setInboxSourceFilter("all");
    setInboxStatusFilter("all");
  }

  // Single reset-then-scope entry for every cross-screen navigation into the
  // Inbox. Hand-rolled partial resets at the call sites left stale filters
  // behind (each site forgot a different one), silently hiding the whole feed
  // — Inbox showed 0 items on "All" while feed_items had rows (bug c80dabe).
  // Never set individual Inbox filters from a navigation flow; go through here.
  function scopeInboxToCompany(company: string) {
    clearInboxFilters();
    setInboxCompanyFilter(company);
  }

  function openCompanyWorkspaceFromFeedItem(item: FeedItem) {
    const company = companies.find((candidate) => candidate.qualifiedTicker === item.company);

    if (!company) {
      return;
    }

    // Opening a company from a feed item lands the Spółka screen with that
    // item raised in the workshop (ADR 0107) — one guarded transition.
    setSelectedCompanyFeedItemId(item.id);
    navigate({ companyId: company.id, section: "Spolka", tool: { t: "feedItem", feedItemId: item.id } });
  }

  function inspectCompanyFeedItem(item: FeedItem) {
    setSelectedFeedItemId(item.id);
    scopeInboxToCompany(item.company);
    setActiveSection("Inbox");
  }

  // Today's `openInboxItem(feedItemId, companyId)` nav intent (F2 S3, plan
  // decision 6): scope Inbox filters to the company, select EXACTLY this feed
  // item, and bump the activation token so the S-overlay opens on it — never
  // the silent first-row fallback (`useAppViewModel.ts` `selectedFeedItem`).
  function openInboxItem(feedItemId: string, companyId: string) {
    const company = companies.find((candidate) => candidate.id === companyId);
    scopeInboxToCompany(company?.qualifiedTicker ?? "all");
    setSelectedFeedItemId(feedItemId);
    setActiveSection("Inbox");
    setInboxDetailActivationToken((token) => token + 1);
  }

  return {
    clearInboxFilters,
    inboxDetailActivationToken,
    inspectCompanyFeedItem,
    markVisibleInboxAsRead,
    openCompanyWorkspaceFromFeedItem,
    openInboxItem,
    scopeInboxToCompany,
    selectFeedItemFromKeyboard,
    toggleFeedItemReadState,
    updateFeedItemState,
    updateSelectedFeedItem,
  };
}
