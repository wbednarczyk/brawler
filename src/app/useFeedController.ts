import type { Dispatch, KeyboardEvent, SetStateAction } from "react";
import * as feedApi from "../api/feed";
import type { Company, FeedItem } from "../api/types";
import type { InboxStatusFilter } from "../screens/Inbox/inboxTypes";
import type { Section } from "./navigation";

type FeedControllerInput = {
  companies: Company[];
  filteredFeedItems: FeedItem[];
  selectedFeedItem: FeedItem | null;
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setCockpitInitialCompanyId: Dispatch<SetStateAction<string | null>>;
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
  setSelectedCompanyId: Dispatch<SetStateAction<string | null>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
};

export function useFeedController({
  companies,
  filteredFeedItems,
  selectedFeedItem,
  setActiveSection,
  setCockpitInitialCompanyId,
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
  setSelectedCompanyId,
  setSelectedFeedItemId,
}: FeedControllerInput) {
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

  function openCompanyWorkspaceFromFeedItem(item: FeedItem) {
    const company = companies.find((candidate) => candidate.qualifiedTicker === item.company);

    if (!company) {
      return;
    }

    // Opening a company from a feed item lands the curated cockpit dashboard
    // scoped to it (ADR 0057), replacing the retired tabbed workspace.
    setSelectedCompanyId(company.id);
    setSelectedCompanyFeedItemId(item.id);
    setCockpitInitialCompanyId(company.id);
    setActiveSection("Cockpit");
  }

  function inspectCompanyFeedItem(item: FeedItem) {
    setSelectedFeedItemId(item.id);
    setInboxCompanyFilter(item.company);
    setActiveSection("Inbox");
  }

  return {
    clearInboxFilters,
    inspectCompanyFeedItem,
    markVisibleInboxAsRead,
    openCompanyWorkspaceFromFeedItem,
    selectFeedItemFromKeyboard,
    toggleFeedItemReadState,
    updateFeedItemState,
    updateSelectedFeedItem,
  };
}
