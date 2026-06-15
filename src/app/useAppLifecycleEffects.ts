import { type Dispatch, type MutableRefObject, type SetStateAction, useEffect } from "react";
import type { Company, FeedItem, NotebookEntry, SourceAdapter, UserSettings } from "../api/types";
import type { CompanyEventMode, CompanyEventViewMode } from "../shared/types/events";
import type { Section } from "./navigation";
import { emptyNotebookForm, notebookFormFromEntry } from "./notebookForms";
import {
  feedPruneInitialDelayMs,
  feedPruneIntervalMs,
  schedulerStartJitterMs,
} from "./sourceScheduler";

type AppLifecycleEffectsInput = {
  activeSection: Section;
  accentPalette: string;
  companies: Company[];
  companyEventCompanyFilter: string;
  companyEventDateFrom: string;
  companyEventDateTo: string;
  companyEventMode: CompanyEventMode;
  companyEventStatusFilter: string;
  companyEventTypeFilter: string;
  companyEventViewMode: CompanyEventViewMode;
  companyEventWatchlistFilter: string;
  companyEventWeekRange: { start: string; end: string };
  effectiveTheme: string;
  eventWeekFetchAttemptedRef: MutableRefObject<Set<string>>;
  filteredFeedItems: FeedItem[];
  licenseCanUseApp: boolean;
  pruneOldFeedItems: () => void;
  refreshBankierCalendarWeek: (date: string, trigger?: "manual") => void;
  refreshCompanies: () => void;
  refreshCompanyEvents: (mode?: CompanyEventMode) => void;
  refreshCompanyRegistryEntries: () => void;
  refreshCompanyRegistryIfStale: (staleAfterSeconds: number) => void;
  refreshDatabaseStatus: () => void;
  refreshFeedItems: () => void;
  refreshSignals: () => void;
  refreshGeminiCredentialStatus: () => void;
  refreshHealth: () => void;
  refreshLicenseStatus: () => void;
  refreshNotebookEntries: (companyId: string) => void;
  refreshScheduledSource: (adapterId: string, intervalMs: number) => void;
  refreshSettings: () => void;
  refreshSourceAdapters: () => void;
  refreshTranscriptJobs: () => void;
  refreshWatchlistMemberships: () => void;
  refreshWatchlists: () => void;
  registryAdapter: SourceAdapter | null;
  scheduledSourceAdapterKey: string;
  scheduledSourceAdapters: SourceAdapter[];
  selectedClaimEntry: NotebookEntry | null;
  selectedCompanyId: string | null;
  selectedFeedItemId: string | null;
  selectedNotebookCompanyId: string | null;
  selectedNotebookEntry: NotebookEntry | null;
  selectedNotebookScreenEntry: NotebookEntry | null;
  settings: UserSettings | null;
  setClaimStatusDraft: Dispatch<SetStateAction<string>>;
  setNextRegistryRefreshAt: Dispatch<SetStateAction<number | null>>;
  setNextSourceRefreshAtByAdapterId: Dispatch<SetStateAction<Record<string, number>>>;
  setNotebookEditForm: Dispatch<SetStateAction<ReturnType<typeof emptyNotebookForm>>>;
  setNotebookEditMode: Dispatch<SetStateAction<boolean>>;
  setNotebookScreenEditForm: Dispatch<SetStateAction<ReturnType<typeof emptyNotebookForm>>>;
  setNotebookScreenEditMode: Dispatch<SetStateAction<boolean>>;
  setSelectedClaimEntryId: Dispatch<SetStateAction<string | null>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookCompanyId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookEntryId: Dispatch<SetStateAction<string | null>>;
  sourceAdapters: SourceAdapter[];
  sourceAdaptersRef: MutableRefObject<SourceAdapter[]>;
  sourceRefreshFailureCount: number;
};

export function useAppLifecycleEffects({
  activeSection,
  accentPalette,
  companies,
  companyEventCompanyFilter,
  companyEventDateFrom,
  companyEventDateTo,
  companyEventMode,
  companyEventStatusFilter,
  companyEventTypeFilter,
  companyEventViewMode,
  companyEventWatchlistFilter,
  companyEventWeekRange,
  effectiveTheme,
  eventWeekFetchAttemptedRef,
  filteredFeedItems,
  licenseCanUseApp,
  pruneOldFeedItems,
  refreshBankierCalendarWeek,
  refreshCompanies,
  refreshCompanyEvents,
  refreshCompanyRegistryEntries,
  refreshCompanyRegistryIfStale,
  refreshDatabaseStatus,
  refreshFeedItems,
  refreshSignals,
  refreshGeminiCredentialStatus,
  refreshHealth,
  refreshLicenseStatus,
  refreshNotebookEntries,
  refreshScheduledSource,
  refreshSettings,
  refreshSourceAdapters,
  refreshTranscriptJobs,
  refreshWatchlistMemberships,
  refreshWatchlists,
  registryAdapter,
  scheduledSourceAdapterKey,
  scheduledSourceAdapters,
  selectedClaimEntry,
  selectedCompanyId,
  selectedFeedItemId,
  selectedNotebookCompanyId,
  selectedNotebookEntry,
  selectedNotebookScreenEntry,
  settings,
  setClaimStatusDraft,
  setNextRegistryRefreshAt,
  setNextSourceRefreshAtByAdapterId,
  setNotebookEditForm,
  setNotebookEditMode,
  setNotebookScreenEditForm,
  setNotebookScreenEditMode,
  setSelectedClaimEntryId,
  setSelectedFeedItemId,
  setSelectedNotebookCompanyId,
  setSelectedNotebookEntryId,
  sourceAdapters,
  sourceAdaptersRef,
  sourceRefreshFailureCount,
}: AppLifecycleEffectsInput) {
  useEffect(() => {
    document.documentElement.dataset.theme = effectiveTheme;
    document.documentElement.dataset.palette = accentPalette;
  }, [accentPalette, effectiveTheme]);

  useEffect(() => {
    if (selectedFeedItemId && filteredFeedItems.some((item) => item.id === selectedFeedItemId)) {
      return;
    }

    const nextSelectedFeedItemId = filteredFeedItems[0]?.id ?? null;

    if (selectedFeedItemId !== nextSelectedFeedItemId) {
      setSelectedFeedItemId(nextSelectedFeedItemId);
    }
  }, [filteredFeedItems, selectedFeedItemId]);

  useEffect(() => {
    sourceAdaptersRef.current = sourceAdapters;
  }, [sourceAdapters]);

  useEffect(() => {
    refreshHealth();
    refreshDatabaseStatus();
    refreshSettings();
    refreshLicenseStatus();

    if (!licenseCanUseApp) {
      return;
    }

    refreshCompanies();
    refreshWatchlists();
    refreshWatchlistMemberships();
    refreshFeedItems();
    refreshSignals();
    refreshCompanyEvents();
    refreshTranscriptJobs();
    refreshSourceAdapters();
    refreshGeminiCredentialStatus();
  }, [licenseCanUseApp]);

  useEffect(() => {
    if (!licenseCanUseApp) {
      return;
    }

    void refreshCompanyEvents(companyEventMode);
  }, [
    licenseCanUseApp,
    companyEventCompanyFilter,
    companyEventDateFrom,
    companyEventDateTo,
    companyEventMode,
    companyEventStatusFilter,
    companyEventTypeFilter,
    companyEventViewMode,
    companyEventWeekRange.end,
    companyEventWeekRange.start,
    companyEventWatchlistFilter,
  ]);

  useEffect(() => {
    if (!licenseCanUseApp || activeSection !== "Events" || companyEventViewMode !== "week") {
      return;
    }

    const weekStart = companyEventWeekRange.start;
    if (eventWeekFetchAttemptedRef.current.has(weekStart)) {
      return;
    }

    eventWeekFetchAttemptedRef.current.add(weekStart);
    void refreshBankierCalendarWeek(weekStart, "manual");
  }, [licenseCanUseApp, activeSection, companyEventViewMode, companyEventWeekRange.start]);

  useEffect(() => {
    if (!licenseCanUseApp) {
      return undefined;
    }

    const firstRunDelayMs = feedPruneInitialDelayMs + schedulerStartJitterMs(feedPruneIntervalMs);
    let intervalId: number | null = null;
    const timeoutId = window.setTimeout(() => {
      void pruneOldFeedItems();
      intervalId = window.setInterval(() => {
        void pruneOldFeedItems();
      }, feedPruneIntervalMs);
    }, firstRunDelayMs);

    return () => {
      window.clearTimeout(timeoutId);
      if (intervalId !== null) {
        window.clearInterval(intervalId);
      }
    };
  }, [licenseCanUseApp]);

  useEffect(() => {
    if (
      !licenseCanUseApp ||
      !settings ||
      settings.pollIntervalSeconds <= 0 ||
      scheduledSourceAdapters.length === 0
    ) {
      setNextSourceRefreshAtByAdapterId({});
      return undefined;
    }

    const intervalSeconds =
      sourceRefreshFailureCount >= 2
        ? Math.min(settings.pollIntervalSeconds * 2, 3600)
        : settings.pollIntervalSeconds;
    const intervalMs = intervalSeconds * 1000;
    const timers = scheduledSourceAdapters.map((adapter) => {
      const firstRunDelayMs = intervalMs + schedulerStartJitterMs(intervalMs);
      setNextSourceRefreshAtByAdapterId((current) => ({
        ...current,
        [adapter.id]: Date.now() + firstRunDelayMs,
      }));

      let intervalId: number | null = null;
      const timeoutId = window.setTimeout(() => {
        setNextSourceRefreshAtByAdapterId((current) => ({
          ...current,
          [adapter.id]: Date.now() + intervalMs,
        }));
        void refreshScheduledSource(adapter.id, intervalMs);
        intervalId = window.setInterval(() => {
          setNextSourceRefreshAtByAdapterId((current) => ({
            ...current,
            [adapter.id]: Date.now() + intervalMs,
          }));
          void refreshScheduledSource(adapter.id, intervalMs);
        }, intervalMs);
      }, firstRunDelayMs);

      return { intervalId: () => intervalId, timeoutId };
    });

    return () => {
      for (const timer of timers) {
        window.clearTimeout(timer.timeoutId);
        const intervalId = timer.intervalId();
        if (intervalId !== null) {
          window.clearInterval(intervalId);
        }
      }
      setNextSourceRefreshAtByAdapterId({});
    };
  }, [licenseCanUseApp, settings?.pollIntervalSeconds, sourceRefreshFailureCount, scheduledSourceAdapterKey]);

  useEffect(() => {
    if (!licenseCanUseApp || !registryAdapter?.enabled || registryAdapter.defaultPollIntervalSeconds <= 0) {
      setNextRegistryRefreshAt(null);
      return undefined;
    }

    const intervalMs = registryAdapter.defaultPollIntervalSeconds * 1000;
    const firstRunDelayMs = intervalMs + schedulerStartJitterMs(intervalMs);
    setNextRegistryRefreshAt(Date.now() + firstRunDelayMs);

    let intervalId: number | null = null;
    const timeoutId = window.setTimeout(() => {
      setNextRegistryRefreshAt(Date.now() + intervalMs);
      void refreshCompanyRegistryIfStale(registryAdapter.defaultPollIntervalSeconds);
      intervalId = window.setInterval(() => {
        setNextRegistryRefreshAt(Date.now() + intervalMs);
        void refreshCompanyRegistryIfStale(registryAdapter.defaultPollIntervalSeconds);
      }, intervalMs);
    }, firstRunDelayMs);

    return () => {
      window.clearTimeout(timeoutId);
      if (intervalId !== null) {
        window.clearInterval(intervalId);
      }
      setNextRegistryRefreshAt(null);
    };
  }, [licenseCanUseApp, registryAdapter?.enabled, registryAdapter?.defaultPollIntervalSeconds]);

  useEffect(() => {
    if (!licenseCanUseApp) {
      return;
    }

    if (!selectedCompanyId) {
      setSelectedNotebookEntryId(null);
      setSelectedClaimEntryId(null);
      setNotebookEditMode(false);
      return;
    }

    refreshNotebookEntries(selectedCompanyId);
  }, [licenseCanUseApp, selectedCompanyId]);

  useEffect(() => {
    if (!licenseCanUseApp || activeSection !== "Notebooks" || companies.length === 0) {
      return;
    }

    const nextCompanyId =
      selectedNotebookCompanyId &&
      companies.some((company) => company.id === selectedNotebookCompanyId)
        ? selectedNotebookCompanyId
        : companies[0].id;

    if (selectedNotebookCompanyId !== nextCompanyId) {
      setSelectedNotebookCompanyId(nextCompanyId);
    }

    refreshNotebookEntries(nextCompanyId);
  }, [licenseCanUseApp, activeSection, companies, selectedNotebookCompanyId]);

  useEffect(() => {
    if (licenseCanUseApp && activeSection === "Companies") {
      refreshCompanyRegistryEntries();
    }
  }, [licenseCanUseApp, activeSection, companies.length]);

  useEffect(() => {
    if (!selectedNotebookEntry) {
      setNotebookEditMode(false);
      setNotebookEditForm(emptyNotebookForm());
      return;
    }

    setNotebookEditForm(notebookFormFromEntry(selectedNotebookEntry));
    setNotebookEditMode(false);
  }, [selectedNotebookEntry]);

  useEffect(() => {
    if (!selectedNotebookScreenEntry) {
      setNotebookScreenEditMode(false);
      setNotebookScreenEditForm(emptyNotebookForm());
      return;
    }

    setNotebookScreenEditForm(notebookFormFromEntry(selectedNotebookScreenEntry));
    setNotebookScreenEditMode(false);
  }, [selectedNotebookScreenEntry]);

  useEffect(() => {
    if (!selectedClaimEntry) {
      setClaimStatusDraft("");
      return;
    }

    setClaimStatusDraft(selectedClaimEntry.claimStatus ?? "open");
  }, [selectedClaimEntry]);
}
