import {
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
  useEffect,
  useRef,
} from "react";
import type { Company, FeedItem, NotebookEntry, SourceAdapter } from "../api/types";
import { getSchedulerStatus } from "../api/sources";
import type { CompanyEventMode, CompanyEventViewMode } from "../shared/types/events";
import type { Section } from "./navigation";
import { emptyNotebookForm, notebookFormFromEntry } from "./notebookForms";

// How often the UI mirrors the Rust scheduler's next-due snapshot and reloads
// views when a background refresh has fired. The Rust scheduler owns the actual
// refresh cadence (ADR 0055); this is a lightweight view-sync, not a scheduler.
const SCHEDULER_SYNC_INTERVAL_MS = 15_000;

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
  refreshBankierCalendarWeek: (date: string, trigger?: "manual") => void;
  refreshCompanies: () => void;
  refreshCompanyEvents: (mode?: CompanyEventMode) => void;
  refreshCompanyRegistryEntries: () => void;
  refreshDatabaseStatus: () => void;
  refreshFeedItems: () => void;
  refreshSignals: () => void;
  refreshGeminiCredentialStatus: () => void;
  refreshHealth: () => void;
  refreshLicenseStatus: () => void;
  refreshNotebookEntries: (companyId: string) => void;
  refreshSettings: () => void;
  refreshSourceAdapters: () => void;
  refreshTranscriptJobs: () => void;
  refreshWatchlistMemberships: () => void;
  refreshWatchlists: () => void;
  selectedCompanyId: string | null;
  selectedFeedItemId: string | null;
  selectedNotebookCompanyId: string | null;
  selectedNotebookEntry: NotebookEntry | null;
  selectedNotebookScreenEntry: NotebookEntry | null;
  setNextRegistryRefreshAt: Dispatch<SetStateAction<number | null>>;
  setNextSourceRefreshAtByAdapterId: Dispatch<SetStateAction<Record<string, number>>>;
  setNotebookEditForm: Dispatch<SetStateAction<ReturnType<typeof emptyNotebookForm>>>;
  setNotebookEditMode: Dispatch<SetStateAction<boolean>>;
  setNotebookScreenEditForm: Dispatch<SetStateAction<ReturnType<typeof emptyNotebookForm>>>;
  setNotebookScreenEditMode: Dispatch<SetStateAction<boolean>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookCompanyId: Dispatch<SetStateAction<string | null>>;
  setSelectedNotebookEntryId: Dispatch<SetStateAction<string | null>>;
  sourceAdapters: SourceAdapter[];
  sourceAdaptersRef: MutableRefObject<SourceAdapter[]>;
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
  refreshBankierCalendarWeek,
  refreshCompanies,
  refreshCompanyEvents,
  refreshCompanyRegistryEntries,
  refreshDatabaseStatus,
  refreshFeedItems,
  refreshSignals,
  refreshGeminiCredentialStatus,
  refreshHealth,
  refreshLicenseStatus,
  refreshNotebookEntries,
  refreshSettings,
  refreshSourceAdapters,
  refreshTranscriptJobs,
  refreshWatchlistMemberships,
  refreshWatchlists,
  selectedCompanyId,
  selectedFeedItemId,
  selectedNotebookCompanyId,
  selectedNotebookEntry,
  selectedNotebookScreenEntry,
  setNextRegistryRefreshAt,
  setNextSourceRefreshAtByAdapterId,
  setNotebookEditForm,
  setNotebookEditMode,
  setNotebookScreenEditForm,
  setNotebookScreenEditMode,
  setSelectedFeedItemId,
  setSelectedNotebookCompanyId,
  setSelectedNotebookEntryId,
  sourceAdapters,
  sourceAdaptersRef,
}: AppLifecycleEffectsInput) {
  const previousSourceDueRef = useRef<Record<string, number>>({});
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
  }, [filteredFeedItems, selectedFeedItemId, setSelectedFeedItemId]);

  useEffect(() => {
    sourceAdaptersRef.current = sourceAdapters;
  }, [sourceAdapters, sourceAdaptersRef]);

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
    // eslint-disable-next-line react-hooks/exhaustive-deps -- initial data load: runs when the license gate flips; the non-memoized refresh callbacks from AppStateRoot are intentionally excluded so startup does not re-fetch every render
  }, [licenseCanUseApp]);

  useEffect(() => {
    if (!licenseCanUseApp) {
      return;
    }

    void refreshCompanyEvents(companyEventMode);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- re-fetch when the event filters change; the non-memoized refreshCompanyEvents identity is intentionally excluded
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
    if (
      !licenseCanUseApp ||
      (activeSection !== "Events" && activeSection !== "Cockpit") ||
      companyEventViewMode !== "week"
    ) {
      return;
    }

    const weekStart = companyEventWeekRange.start;
    if (eventWeekFetchAttemptedRef.current.has(weekStart)) {
      return;
    }

    eventWeekFetchAttemptedRef.current.add(weekStart);
    void refreshBankierCalendarWeek(weekStart, "manual");
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fetch each week once (guarded by eventWeekFetchAttemptedRef); the non-memoized refreshBankierCalendarWeek identity is intentionally excluded
  }, [licenseCanUseApp, activeSection, companyEventViewMode, companyEventWeekRange.start]);

  // Feed cleanup is manual-only (owner decision 2026-07-19). The app must never
  // delete feed items on a timer — an automatic 30-day prune silently removed
  // ~3,900 items (incl. researched periodic reports) before it was disabled. The
  // capability lives on as a user-triggered action in Settings; the future
  // retention mechanism is tracked as a backlog card.

  // Source/registry refresh cadence is owned by the Rust-side scheduler (ADR 0055
  // / AV5) — a webview timer is throttled when the window is hidden/suspended, so
  // the frontend must not decide *when* to refresh. This effect only **mirrors**
  // the scheduler's next-due snapshot for the "next refresh at …" display and
  // reloads views when a background refresh has fired (detected by an adapter's
  // next-due jumping forward), preserving the post-refresh view update without a
  // frontend-owned schedule.
  useEffect(() => {
    if (!licenseCanUseApp) {
      setNextSourceRefreshAtByAdapterId({});
      setNextRegistryRefreshAt(null);
      previousSourceDueRef.current = {};
      return undefined;
    }

    let cancelled = false;
    const syncScheduler = () => {
      void getSchedulerStatus()
        .then((status) => {
          if (cancelled) {
            return;
          }
          const nextDue = status.sourceNextDueMs ?? {};
          // A refresh fired when an adapter's next-due moved forward since last poll.
          const previous = previousSourceDueRef.current;
          const fired = Object.entries(nextDue).some(
            ([adapterId, due]) => (previous[adapterId] ?? 0) > 0 && due > (previous[adapterId] ?? 0),
          );
          previousSourceDueRef.current = nextDue;
          setNextSourceRefreshAtByAdapterId(nextDue);
          setNextRegistryRefreshAt(status.registryNextDueMs ?? null);
          if (fired) {
            refreshFeedItems();
            refreshSignals();
            refreshSourceAdapters();
            refreshDatabaseStatus();
            refreshCompanyRegistryEntries();
          }
        })
        .catch(() => {
          // A transient status read failure just leaves the last display in place.
        });
    };

    syncScheduler();
    const intervalId = window.setInterval(syncScheduler, SCHEDULER_SYNC_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- view-sync poll keyed on the license gate; the Rust scheduler owns refresh cadence (ADR 0055), this only mirrors its next-due snapshot; the non-memoized refresh callbacks are intentionally excluded
  }, [licenseCanUseApp]);

  useEffect(() => {
    if (!licenseCanUseApp) {
      return;
    }

    if (!selectedCompanyId) {
      setSelectedNotebookEntryId(null);
      setNotebookEditMode(false);
      return;
    }

    refreshNotebookEntries(selectedCompanyId);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- load notebook entries on company change; the non-memoized refreshNotebookEntries and stable setters are intentionally excluded
  }, [licenseCanUseApp, selectedCompanyId]);

  useEffect(() => {
    if (
      !licenseCanUseApp ||
      (activeSection !== "Notebooks" && activeSection !== "Cockpit") ||
      companies.length === 0
    ) {
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
    // eslint-disable-next-line react-hooks/exhaustive-deps -- select + load the notebook company when Notebooks opens; the non-memoized refreshNotebookEntries and stable setter are intentionally excluded
  }, [licenseCanUseApp, activeSection, companies, selectedNotebookCompanyId]);

  useEffect(() => {
    if (licenseCanUseApp && activeSection === "Companies") {
      refreshCompanyRegistryEntries();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refresh registry entries when Companies opens; the non-memoized refreshCompanyRegistryEntries identity is intentionally excluded
  }, [licenseCanUseApp, activeSection, companies.length]);

  useEffect(() => {
    if (!selectedNotebookEntry) {
      setNotebookEditMode(false);
      setNotebookEditForm(emptyNotebookForm());
      return;
    }

    setNotebookEditForm(notebookFormFromEntry(selectedNotebookEntry));
    setNotebookEditMode(false);
  }, [selectedNotebookEntry, setNotebookEditForm, setNotebookEditMode]);

  useEffect(() => {
    if (!selectedNotebookScreenEntry) {
      setNotebookScreenEditMode(false);
      setNotebookScreenEditForm(emptyNotebookForm());
      return;
    }

    setNotebookScreenEditForm(notebookFormFromEntry(selectedNotebookScreenEntry));
    setNotebookScreenEditMode(false);
  }, [selectedNotebookScreenEntry, setNotebookScreenEditForm, setNotebookScreenEditMode]);
}
