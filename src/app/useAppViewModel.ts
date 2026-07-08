import { useCallback, useMemo } from "react";
import type { SourceStatusSummary } from "./AppShell";
import type { InboxEmptyState, InboxStatusFilter } from "../screens/Inbox/inboxTypes";
import type { TranscriptJobForm } from "../screens/Transcripts/transcriptTypes";
import type { CompanyEventViewMode } from "../shared/types/events";
import type { NotebookForm } from "../shared/types/notebook";
import {
  addLocalDays,
  formatLocalDate,
  parseLocalDate,
  startOfIsoWeek,
} from "../shared/format/datetime";
import type {
  Company,
  CompanyEvent,
  CompanyForm,
  CompanyRegistryEntry,
  CompanySignal,
  FeedItem,
  NotebookEntry,
  SourceAdapter,
  Theme,
  UserSettings,
  WatchlistMembership,
} from "../api/types";
import { resolveTheme } from "./theme";
import { notebookFormFromEntry } from "./notebookForms";

type AppViewModelInput = {
  companies: Company[];
  companyEventViewMode: CompanyEventViewMode;
  companyEventWeekAnchorDate: string;
  companyEvents: CompanyEvent[];
  companyForm: CompanyForm;
  companyListSearch: string;
  companyWatchlistFilter: string;
  companyRegistryEntries: CompanyRegistryEntry[];
  companyRegistrySearch: string;
  feedState: FeedItem[];
  inboxCompanyFilter: string;
  inboxSignalFilter: string;
  inboxSourceFilter: string;
  inboxStatusFilter: InboxStatusFilter;
  inboxTypeFilter: string;
  inboxWatchlistFilter: string;
  notebookEditForm: NotebookForm;
  signals: CompanySignal[];
  notebookEntries: NotebookEntry[];
  notebookScreenWatchlistFilter: string;
  notebookScreenClaimStatusFilter: string;
  notebookScreenEditForm: NotebookForm;
  notebookScreenFollowUpFilter: string;
  notebookScreenKindFilter: string;
  notebookScreenTagFilter: string;
  searchQuery: string;
  selectedCompanyFeedItemId: string | null;
  selectedCompanyId: string | null;
  selectedCompanyRegistryTicker: string | null;
  selectedCompanyEventId: string | null;
  selectedFeedItemId: string | null;
  selectedNotebookCompanyId: string | null;
  selectedNotebookEntryId: string | null;
  selectedNotebookScreenEntryId: string | null;
  settings: UserSettings | null;
  sourceAdapters: SourceAdapter[];
  sourceAdaptersError: string | null;
  theme: Theme;
  transcriptJobForm: TranscriptJobForm;
  watchlistMemberships: WatchlistMembership[];
};

export function useAppViewModel({
  companies,
  companyEventWeekAnchorDate,
  companyEvents,
  companyForm,
  companyListSearch,
  companyWatchlistFilter,
  companyRegistryEntries,
  companyRegistrySearch,
  feedState,
  inboxCompanyFilter,
  inboxSignalFilter,
  inboxSourceFilter,
  inboxStatusFilter,
  inboxTypeFilter,
  inboxWatchlistFilter,
  signals,
  notebookEditForm,
  notebookEntries,
  notebookScreenWatchlistFilter,
  notebookScreenClaimStatusFilter,
  notebookScreenEditForm,
  notebookScreenFollowUpFilter,
  notebookScreenKindFilter,
  notebookScreenTagFilter,
  searchQuery,
  selectedCompanyFeedItemId,
  selectedCompanyId,
  selectedCompanyRegistryTicker,
  selectedCompanyEventId,
  selectedFeedItemId,
  selectedNotebookCompanyId,
  selectedNotebookEntryId,
  selectedNotebookScreenEntryId,
  sourceAdapters,
  sourceAdaptersError,
  theme,
  transcriptJobForm,
  watchlistMemberships,
}: AppViewModelInput) {
  const effectiveTheme = useMemo(() => resolveTheme(theme), [theme]);
  const membershipsByCompany = useMemo(() => {
    return watchlistMemberships.reduce<Record<string, WatchlistMembership[]>>(
      (grouped, membership) => {
        grouped[membership.companyId] = grouped[membership.companyId] ?? [];
        grouped[membership.companyId].push(membership);
        return grouped;
      },
      {},
    );
  }, [watchlistMemberships]);
  const companiesById = useMemo(() => {
    return companies.reduce<Record<string, Company>>((indexed, company) => {
      indexed[company.id] = company;
      return indexed;
    }, {});
  }, [companies]);
  const companyIdsByWatchlist = useMemo(() => {
    return watchlistMemberships.reduce<Record<string, Set<string>>>((grouped, membership) => {
      grouped[membership.watchlistId] = grouped[membership.watchlistId] ?? new Set<string>();
      grouped[membership.watchlistId].add(membership.companyId);
      return grouped;
    }, {});
  }, [watchlistMemberships]);
  const companyMatchesWatchlist = useCallback(
    (company: Company, watchlistId: string) =>
      watchlistId === "all" || Boolean(companyIdsByWatchlist[watchlistId]?.has(company.id)),
    [companyIdsByWatchlist],
  );
  const transcriptCompanySuggestions = useMemo(() => {
    const query = transcriptJobForm.companyQuery.trim().toLowerCase();

    if (!query) {
      return companies.slice(0, 5);
    }

    return companies
      .filter((company) => {
        const values = [
          company.ticker,
          company.qualifiedTicker,
          company.displayName,
          company.isin ?? "",
        ].map((value) => value.toLowerCase());

        return values.some((value) => value.includes(query));
      })
      .slice(0, 5);
  }, [companies, transcriptJobForm.companyQuery]);
  const totalUnreadFeedItems = useMemo(
    () => feedState.filter((item) => item.unread).length,
    [feedState],
  );
  const feedTypes = useMemo(() => {
    return Array.from(new Set(feedState.map((item) => item.type))).sort();
  }, [feedState]);
  const feedSources = useMemo(() => {
    return Array.from(new Set(feedState.map((item) => item.source))).sort();
  }, [feedState]);
  const signalsByFeedItemId = useMemo(() => {
    return signals.reduce<Record<string, CompanySignal[]>>((grouped, signal) => {
      grouped[signal.feedItemId] = grouped[signal.feedItemId] ?? [];
      grouped[signal.feedItemId].push(signal);

      return grouped;
    }, {});
  }, [signals]);
  // Distinct categories present in the feed, for the signal-type filter. Each
  // appears once with its localized-ready display name from the registry.
  const feedSignalCategories = useMemo(() => {
    const byCategory = new Map<string, string>();
    for (const signal of signals) {
      if (!byCategory.has(signal.category)) {
        byCategory.set(signal.category, signal.categoryDisplayName);
      }
    }

    return Array.from(byCategory.entries())
      .map(([category, displayName]) => ({ category, displayName }))
      .sort((a, b) => a.displayName.localeCompare(b.displayName));
  }, [signals]);
  const selectedCompanyEvent =
    companyEvents.find((event) => event.id === selectedCompanyEventId) ?? null;
  const companyEventTypes = useMemo(() => {
    return Array.from(new Set(companyEvents.map((event) => event.eventType))).sort();
  }, [companyEvents]);
  const companyEventStatuses = useMemo(() => {
    return Array.from(new Set(companyEvents.map((event) => event.status))).sort();
  }, [companyEvents]);
  const companyEventWeekRange = useMemo(() => {
    const weekStart = startOfIsoWeek(parseLocalDate(companyEventWeekAnchorDate));
    const weekEnd = addLocalDays(weekStart, 6);

    return {
      start: formatLocalDate(weekStart),
      end: formatLocalDate(weekEnd),
    };
  }, [companyEventWeekAnchorDate]);
  const companyEventWeekDays = useMemo(() => {
    const weekStart = parseLocalDate(companyEventWeekRange.start);

    return Array.from({ length: 7 }, (_, dayOffset) => {
      const date = addLocalDays(weekStart, dayOffset);

      return {
        date: formatLocalDate(date),
        label: date.toLocaleDateString(undefined, { weekday: "long" }),
      };
    });
  }, [companyEventWeekRange.start]);
  const companyEventsByDate = useMemo(() => {
    return companyEvents.reduce<Record<string, CompanyEvent[]>>((grouped, event) => {
      grouped[event.eventDate] = grouped[event.eventDate] ?? [];
      grouped[event.eventDate].push(event);

      return grouped;
    }, {});
  }, [companyEvents]);
  const companyEventWorkingWeekDays = useMemo(
    () => companyEventWeekDays.slice(0, 5),
    [companyEventWeekDays],
  );
  const companyEventWeekendDays = useMemo(
    () => companyEventWeekDays.slice(5),
    [companyEventWeekDays],
  );
  const companyEventWeekendEvents = useMemo(() => {
    return companyEventWeekendDays.flatMap((day) => companyEventsByDate[day.date] ?? []);
  }, [companyEventWeekendDays, companyEventsByDate]);
  const filteredFeedItems = useMemo(() => {
    const normalizedSearch = searchQuery.trim().toLowerCase();
    const allowedTickers =
      inboxWatchlistFilter === "all"
        ? null
        : new Set(
            watchlistMemberships
              .filter((membership) => membership.watchlistId === inboxWatchlistFilter)
              .map((membership) => companiesById[membership.companyId]?.qualifiedTicker)
              .filter((qualifiedTicker): qualifiedTicker is string => Boolean(qualifiedTicker)),
          );

    return feedState.filter((item) => {
      const watchlistMatches = allowedTickers ? allowedTickers.has(item.company) : true;
      const companyMatches = inboxCompanyFilter === "all" || item.company === inboxCompanyFilter;
      const typeMatches = inboxTypeFilter === "all" || item.type === inboxTypeFilter;
      const signalMatches =
        inboxSignalFilter === "all" ||
        (signalsByFeedItemId[item.id]?.some((signal) => signal.category === inboxSignalFilter) ??
          false);
      const sourceMatches = inboxSourceFilter === "all" || item.source === inboxSourceFilter;
      const statusMatches =
        inboxStatusFilter === "all" ||
        (inboxStatusFilter === "unread" && item.unread) ||
        (inboxStatusFilter === "saved" && item.saved);
      const searchMatches =
        normalizedSearch.length === 0 ||
        [item.company, item.title, item.source, item.type, item.summary]
          .join(" ")
          .toLowerCase()
          .includes(normalizedSearch);

      return (
        watchlistMatches &&
        companyMatches &&
        typeMatches &&
        signalMatches &&
        sourceMatches &&
        statusMatches &&
        searchMatches
      );
    });
  }, [
    companiesById,
    feedState,
    inboxCompanyFilter,
    inboxSignalFilter,
    inboxSourceFilter,
    inboxStatusFilter,
    inboxTypeFilter,
    inboxWatchlistFilter,
    searchQuery,
    signalsByFeedItemId,
    watchlistMemberships,
  ]);
  const selectedFeedItem =
    filteredFeedItems.find((item) => item.id === selectedFeedItemId) ?? filteredFeedItems[0] ?? null;
  const inboxReviewStats = useMemo(
    () => ({
      visible: filteredFeedItems.length,
      unread: filteredFeedItems.filter((item) => item.unread).length,
      saved: filteredFeedItems.filter((item) => item.saved).length,
    }),
    [filteredFeedItems],
  );
  const selectedFeedCompany =
    selectedFeedItem
      ? companies.find((company) => company.qualifiedTicker === selectedFeedItem.company) ?? null
      : null;
  const selectedCompany = companies.find((company) => company.id === selectedCompanyId) ?? null;
  const selectedCompanyNotebookEntries = useMemo(() => {
    if (!selectedCompany) {
      return [];
    }

    return notebookEntries.filter((entry) => entry.companyId === selectedCompany.id);
  }, [notebookEntries, selectedCompany]);
  const selectedNotebookEntry =
    selectedCompanyNotebookEntries.find((entry) => entry.id === selectedNotebookEntryId) ??
    selectedCompanyNotebookEntries[0] ??
    null;
  const isNotebookEditDirty = selectedNotebookEntry
    ? JSON.stringify(notebookEditForm) !== JSON.stringify(notebookFormFromEntry(selectedNotebookEntry))
    : false;
  const filteredNotebookScreenCompanies = useMemo(() => {
    return companies.filter((company) =>
      companyMatchesWatchlist(company, notebookScreenWatchlistFilter),
    );
  }, [companies, companyMatchesWatchlist, notebookScreenWatchlistFilter]);
  const selectedNotebookScreenCompany =
    filteredNotebookScreenCompanies.find((company) => company.id === selectedNotebookCompanyId) ??
    filteredNotebookScreenCompanies[0] ??
    null;
  const selectedNotebookScreenEntries = useMemo(() => {
    if (!selectedNotebookScreenCompany) {
      return [];
    }

    const normalizedSearch = searchQuery.trim().toLowerCase();
    const normalizedTagFilter = notebookScreenTagFilter.trim().toLowerCase();
    const entries = notebookEntries.filter(
      (entry) => entry.companyId === selectedNotebookScreenCompany.id,
    );

    return entries.filter((entry) => {
      const kindMatches =
        notebookScreenKindFilter === "all" || entry.kind === notebookScreenKindFilter;
      const statusMatches =
        notebookScreenClaimStatusFilter === "all" ||
        entry.claimStatus === notebookScreenClaimStatusFilter;
      const hasFollowUp = Boolean(entry.followUpAfter || entry.followUpDate);
      const followUpMatches =
        notebookScreenFollowUpFilter === "all" ||
        (notebookScreenFollowUpFilter === "has_follow_up" && hasFollowUp) ||
        (notebookScreenFollowUpFilter === "no_follow_up" && !hasFollowUp);
      const tagMatches =
        normalizedTagFilter.length === 0 ||
        entry.tags.some((tag) => tag.toLowerCase().includes(normalizedTagFilter));
      const searchMatches =
        normalizedSearch.length === 0 ||
        [
          entry.title,
          entry.body,
          entry.kind,
          entry.claimStatus ?? "",
          entry.followUpAfter ?? "",
          entry.followUpDate ?? "",
          entry.tags.join(" "),
        ]
          .join(" ")
          .toLowerCase()
          .includes(normalizedSearch);

      return kindMatches && statusMatches && followUpMatches && tagMatches && searchMatches;
    });
  }, [
    notebookEntries,
    notebookScreenClaimStatusFilter,
    notebookScreenKindFilter,
    notebookScreenFollowUpFilter,
    notebookScreenTagFilter,
    searchQuery,
    selectedNotebookScreenCompany,
  ]);
  const selectedNotebookScreenEntry =
    selectedNotebookScreenEntries.find((entry) => entry.id === selectedNotebookScreenEntryId) ?? null;
  const isNotebookScreenEditDirty = selectedNotebookScreenEntry
    ? JSON.stringify(notebookScreenEditForm) !==
      JSON.stringify(notebookFormFromEntry(selectedNotebookScreenEntry))
    : false;
  const selectedCompanyFeedItems = useMemo(() => {
    if (!selectedCompany) {
      return [];
    }

    return feedState.filter((item) => item.company === selectedCompany.qualifiedTicker);
  }, [feedState, selectedCompany]);
  const selectedCompanyFeedItem =
    selectedCompanyFeedItems.find((item) => item.id === selectedCompanyFeedItemId) ?? null;
  const registryAdapter = useMemo(
    () => sourceAdapters.find((adapter) => adapter.sourceType === "company_registry") ?? null,
    [sourceAdapters],
  );
  const scheduledSourceAdapters = useMemo(
    () => sourceAdapters.filter((adapter) => adapter.enabled && adapter.sourceType !== "company_registry"),
    [sourceAdapters],
  );
  const scheduledSourceAdapterKey = useMemo(
    () => scheduledSourceAdapters.map((adapter) => adapter.id).join("|"),
    [scheduledSourceAdapters],
  );
  const filteredCompanyRegistryEntries = useMemo(() => {
    const normalizedSearch = companyRegistrySearch.trim().toLowerCase();

    if (normalizedSearch.length === 0) {
      return companyRegistryEntries;
    }

    return companyRegistryEntries.filter((entry) =>
      [
        entry.exchange,
        entry.ticker,
        entry.qualifiedTicker,
        entry.displayName,
        entry.isin ?? "",
      ]
        .join(" ")
        .toLowerCase()
        .includes(normalizedSearch),
    );
  }, [companyRegistryEntries, companyRegistrySearch]);
  const companyFormRegistryMatches = useMemo(() => {
    if (selectedCompanyRegistryTicker) {
      return [];
    }

    const searchTerms = [companyForm.ticker, companyForm.displayName, companyForm.isin]
      .map((value) => value.trim().toLowerCase())
      .filter((value) => value.length >= 2);

    if (searchTerms.length === 0) {
      return [];
    }

    return companyRegistryEntries
      .filter((entry) => {
        return searchTerms.some((term) =>
          [
            entry.exchange,
            entry.ticker,
            entry.qualifiedTicker,
            entry.displayName,
            entry.isin ?? "",
          ]
            .join(" ")
            .toLowerCase()
            .includes(term),
        );
      })
      .slice(0, 5);
  }, [
    companyForm.displayName,
    companyForm.isin,
    companyForm.ticker,
    companyRegistryEntries,
    selectedCompanyRegistryTicker,
  ]);
  const filteredCompanies = useMemo(() => {
    const normalizedSearch = companyListSearch.trim().toLowerCase();

    return companies.filter((company) => {
      const watchlistMatches = companyMatchesWatchlist(company, companyWatchlistFilter);
      const searchMatches =
        normalizedSearch.length === 0 ||
        [
        company.exchange,
        company.ticker,
        company.qualifiedTicker,
        company.displayName,
        company.isin ?? "",
      ]
        .join(" ")
        .toLowerCase()
          .includes(normalizedSearch);

      return watchlistMatches && searchMatches;
    });
  }, [companies, companyMatchesWatchlist, companyListSearch, companyWatchlistFilter]);
  const selectedCompanyFeedStats = useMemo(
    () => ({
      total: selectedCompanyFeedItems.length,
      unread: selectedCompanyFeedItems.filter((item) => item.unread).length,
      saved: selectedCompanyFeedItems.filter((item) => item.saved).length,
    }),
    [selectedCompanyFeedItems],
  );
  const sourceStatusSummary = useMemo<SourceStatusSummary>(() => {
    if (sourceAdaptersError) {
      return {
        label: "error",
        title: `Source refresh failed: ${sourceAdaptersError}`,
        tone: "danger",
      };
    }

    if (sourceAdapters.length === 0) {
      return {
        label: "0 sources",
        title: "No sources configured",
        tone: "warn",
      };
    }

    const enabledAdapters = sourceAdapters.filter((adapter) => adapter.enabled);
    const adaptersWithErrors = sourceAdapters.filter((adapter) => adapter.lastError);

    if (adaptersWithErrors.length > 0) {
      return {
        label: `${adaptersWithErrors.length} issue${adaptersWithErrors.length === 1 ? "" : "s"}`,
        title: `${adaptersWithErrors.length} source issue${adaptersWithErrors.length === 1 ? "" : "s"}`,
        tone: "danger",
      };
    }

    return {
      label: `${enabledAdapters.length}/${sourceAdapters.length}`,
      title: `${enabledAdapters.length} enabled source${enabledAdapters.length === 1 ? "" : "s"} ready`,
      tone: "ok",
    };
  }, [sourceAdapters, sourceAdaptersError]);
  const hasActiveInboxFilters =
    searchQuery.trim().length > 0 ||
    inboxWatchlistFilter !== "all" ||
    inboxCompanyFilter !== "all" ||
    inboxTypeFilter !== "all" ||
    inboxSignalFilter !== "all" ||
    inboxSourceFilter !== "all" ||
    inboxStatusFilter !== "all";
  const inboxEmptyState: InboxEmptyState =
    companies.length === 0
      ? "no-companies"
      : feedState.length === 0
        ? "no-feed"
        : filteredFeedItems.length === 0
          ? "no-matches"
          : null;

  return {
    companiesById,
    companyEventStatuses,
    companyEventTypes,
    companyEventWeekRange,
    companyEventWeekendDays,
    companyEventWeekendEvents,
    companyEventWorkingWeekDays,
    companyEventsByDate,
    companyFormRegistryMatches,
    effectiveTheme,
    feedSignalCategories,
    feedSources,
    feedTypes,
    filteredCompanies,
    filteredCompanyRegistryEntries,
    filteredFeedItems,
    filteredNotebookScreenCompanies,
    hasActiveInboxFilters,
    inboxEmptyState,
    inboxReviewStats,
    isNotebookEditDirty,
    isNotebookScreenEditDirty,
    membershipsByCompany,
    registryAdapter,
    scheduledSourceAdapterKey,
    scheduledSourceAdapters,
    selectedCompany,
    selectedCompanyEvent,
    selectedCompanyFeedItem,
    selectedCompanyFeedItems,
    selectedCompanyFeedStats,
    selectedCompanyNotebookEntries,
    selectedFeedCompany,
    selectedFeedItem,
    selectedNotebookEntry,
    selectedNotebookScreenCompany,
    selectedNotebookScreenEntries,
    selectedNotebookScreenEntry,
    signalsByFeedItemId,
    sourceStatusSummary,
    totalUnreadFeedItems,
    transcriptCompanySuggestions,
  };
}
