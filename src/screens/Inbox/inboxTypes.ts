import type { KeyboardEvent, PointerEvent } from "react";
import type {
  Company,
  CompanySignal,
  FeedItem,
  Watchlist,
} from "../../api/types";

export type FeedSignalCategoryOption = {
  category: string;
  displayName: string;
};

export type InboxStatusFilter = "all" | "unread" | "saved";

export type InboxEmptyState = "no-companies" | "no-feed" | "no-matches" | null;

export type InboxReviewStats = {
  visible: number;
  unread: number;
  saved: number;
};

export type InboxScreenProps = {
  watchlists: Watchlist[];
  companies: Company[];
  feedTypes: string[];
  feedSources: string[];
  feedSignalCategories: FeedSignalCategoryOption[];
  filteredFeedItems: FeedItem[];
  signalsByFeedItemId: Record<string, CompanySignal[]>;
  signalsError: string | null;
  selectedFeedItem: FeedItem | null;
  selectedFeedCompany: Company | null;
  inboxStatusFilter: InboxStatusFilter;
  searchQuery: string;
  inboxWatchlistFilter: string;
  inboxCompanyFilter: string;
  inboxTypeFilter: string;
  inboxSignalFilter: string;
  inboxSourceFilter: string;
  inboxReviewStats: InboxReviewStats;
  inboxEmptyState: InboxEmptyState;
  hasActiveInboxFilters: boolean;
  deleteUnsavedFeedState: string;
  sourceRefreshState: string;
  detailPaneFraction: number;
  detailPaneMinFraction: number;
  detailPaneMaxFraction: number;
  feedError: string | null;
  deleteUnsavedFeedError: string | null;
  sourceRefreshError: string | null;
  healthError: string | null;
  databaseError: string | null;
  setInboxStatusFilter: (filter: InboxStatusFilter) => void;
  setSearchQuery: (query: string) => void;
  setInboxWatchlistFilter: (filter: string) => void;
  setInboxCompanyFilter: (filter: string) => void;
  setInboxTypeFilter: (filter: string) => void;
  setInboxSignalFilter: (filter: string) => void;
  setInboxSourceFilter: (filter: string) => void;
  confirmCompanySignal: (signalId: string) => Promise<void> | void;
  rejectCompanySignal: (signalId: string) => Promise<void> | void;
  setSelectedFeedItemId: (itemId: string) => void;
  setActiveSection: (section: "Companies") => void;
  markVisibleInboxAsRead: () => void;
  deleteUnsavedFeedItems: () => void;
  clearInboxFilters: () => void;
  refreshSources: (trigger: "manual") => Promise<void> | Promise<unknown>;
  openSourceStatus: () => void;
  toggleFeedItemReadState: (item: FeedItem) => void;
  selectFeedItemFromKeyboard: (event: KeyboardEvent<HTMLElement>, item: FeedItem) => void;
  updateSelectedFeedItem: (update: (item: FeedItem) => FeedItem) => void;
  openCompanyWorkspaceFromFeedItem: (item: FeedItem) => void;
  openFeedItemNoteDraft: (item: FeedItem) => void;
  resizeDetailPaneWithKeyboard: (event: KeyboardEvent<HTMLDivElement>) => void;
  startDetailPaneResize: (event: PointerEvent<HTMLDivElement>) => void;
  resizeDetailPane: (event: PointerEvent<HTMLDivElement>) => void;
  stopDetailPaneResize: (event: PointerEvent<HTMLDivElement>) => void;
  feedItemSummary: (item: FeedItem) => string;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
};
