import {
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
  useEffect,
  useRef,
} from "react";
import type { Company, FeedItem, SourceAdapter } from "../api/types";
import { getSchedulerStatus } from "../api/sources";
import type { CompanyEventMode, CompanyEventViewMode } from "../shared/types/events";
import type { Section } from "./navigation";
import { handleCloseRequested } from "./handleCloseRequested";
import type { SpolkaToolHostApi } from "../screens/Spolka/ToolHost";

// Tauri v2 stamps this global on the window object; absent in the browser
// dev/test harness, where the close interceptor below is a no-op (repoctx:
// no existing runtime-detection helper to reuse).
function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

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
  /** Advance the Dziś refresh-completion signal (F2): the SCHEDULED ingest
   * mirror below is a refresh completion too — without this bump the Today
   * query key never moves while the app sits open (sol re-verify finding 1). */
  onRefreshCompletion: () => void;
  /**
   * Refetch the app-level attention state (ADR 0097 dec. 6) on EVERY scheduler
   * poll tick: background work (autopilot completions, terminal job failures)
   * raises attention events with no frontend-visible trigger of its own — a
   * source-due transition is NOT a proxy for "an event landed" (the queue can
   * finish long after the refresh that enqueued it). The controller skips the
   * state update when data is unchanged, so the steady state is render-free.
   * Startup load is the controller's own license-gated effect.
   */
  refreshAttention: () => void;
  refreshGeminiCredentialStatus: () => void;
  refreshHealth: () => void;
  refreshLicenseStatus: () => void;
  refreshSettings: () => void;
  refreshSourceAdapters: () => void;
  refreshTranscriptJobs: () => void;
  refreshWatchlistMemberships: () => void;
  refreshWatchlists: () => void;
  selectedFeedItemId: string | null;
  setNextRegistryRefreshAt: Dispatch<SetStateAction<number | null>>;
  setNextSourceRefreshAtByAdapterId: Dispatch<SetStateAction<Record<string, number>>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
  sourceAdapters: SourceAdapter[];
  sourceAdaptersRef: MutableRefObject<SourceAdapter[]>;
  /** F3a S2 (ADR 0107): gates a native window-close request the same way as
   * every other cross-screen navigation — a dirty Spółka tool prevents the
   * close and opens the stay/discard dialog; Discard closes the window. */
  spolkaTool: SpolkaToolHostApi;
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
  onRefreshCompletion,
  refreshAttention,
  refreshGeminiCredentialStatus,
  refreshHealth,
  refreshLicenseStatus,
  refreshSettings,
  refreshSourceAdapters,
  refreshTranscriptJobs,
  refreshWatchlistMemberships,
  refreshWatchlists,
  selectedFeedItemId,
  setNextRegistryRefreshAt,
  setNextSourceRefreshAtByAdapterId,
  setSelectedFeedItemId,
  sourceAdapters,
  sourceAdaptersRef,
  spolkaTool,
}: AppLifecycleEffectsInput) {
  const previousSourceDueRef = useRef<Record<string, number>>({});

  // `useSpolkaToolHost()` returns a FRESH object every render — `spolkaTool`
  // itself is not stable, only its `isDirty`/`guardNavigation` closures'
  // BEHAVIOR is (they always read the latest tool state). The close-request
  // effect below installs its listener ONCE (`[]` deps, so it doesn't leak a
  // subscription on every render); without this ref it would keep calling
  // the render-1 `spolkaTool` forever — bound to "no tool open yet" — so a
  // tool opened afterward could never prevent the close (sol R1 finding 2).
  const spolkaToolRef = useRef(spolkaTool);
  useEffect(() => {
    spolkaToolRef.current = spolkaTool;
  });

  useEffect(() => {
    if (!isTauriRuntime()) return undefined;

    let unlisten: (() => void) | undefined;
    let cancelled = false;
    // The browser dev/test harness (`mockIPC` from `@tauri-apps/api/mocks`)
    // stamps `__TAURI_INTERNALS__` too, but doesn't populate the window
    // metadata `getCurrentWindow()` needs — `isTauriRuntime()` alone can't
    // tell the two apart, so a throw here (real or mocked) is swallowed: a
    // real Tauri window simply gets no close interceptor that run, same as
    // the browser harness's intended no-op.
    void import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => {
        if (cancelled) return;
        const win = getCurrentWindow();
        return win
          .onCloseRequested((event) => {
            handleCloseRequested(event, {
              isDirty: () => spolkaToolRef.current.isDirty(),
              ask: () => spolkaToolRef.current.guardNavigation(() => void win.destroy()),
            });
          })
          .then((stop) => {
            if (cancelled) {
              stop();
            } else {
              unlisten = stop;
            }
          });
      })
      .catch(() => {
        // Not a real Tauri window (or the harness's partial shim) — no-op.
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
    // Installs ONCE ([] deps, no re-listen leak) — the handler reads
    // spolkaToolRef.current, always the latest render's host, so it needs no
    // reactive dependency here.
  }, []);
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
      activeSection !== "Events" ||
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
            onRefreshCompletion();
          }
          // Every tick, not only on a source-due transition: queued background
          // work (autopilot, job failures) raises attention events on its own
          // schedule — see the `refreshAttention` input doc.
          refreshAttention();
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
    if (licenseCanUseApp && activeSection === "Companies") {
      refreshCompanyRegistryEntries();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refresh registry entries when Companies opens; the non-memoized refreshCompanyRegistryEntries identity is intentionally excluded
  }, [licenseCanUseApp, activeSection, companies.length]);
}
