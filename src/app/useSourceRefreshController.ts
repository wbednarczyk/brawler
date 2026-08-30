import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import * as sourcesApi from "../api/sources";
import type {
  CompanyRegistryRefreshResult,
  SourceAdapter,
  SourceIngestionResult,
  SourceRefreshTrigger,
  UserSettings,
} from "../api/types";
import type { SourceRefreshState } from "./appTypes";
import { eventSourceAdapterIds } from "./sourceScheduler";

type RefreshCallbacks = {
  refreshCompanyEvents: () => Promise<void>;
  refreshCompanyRegistryEntries: () => Promise<void>;
  refreshDatabaseStatus: () => Promise<void>;
  refreshFeedItems: () => Promise<void>;
  refreshSignals: () => Promise<void>;
  refreshSourceAdapters: () => Promise<void>;
  /**
   * Refetch the app-level attention state (ADR 0097 dec. 6) — a source refresh
   * can raise attention events (reconciliation, fired alerts), so the shared
   * post-refresh view update reloads it alongside feed/signals.
   */
  refreshAttention: () => void;
};

/** Fires once a source refresh (any of manual/single/event/scheduled) COMPLETES its
 * shared post-refresh view reload — Today's `refreshCompletionCount` key (fix wave B
 * finding 1a) increments from this, decoupled from whether attention's own data
 * happened to change on that pass. */
type CompletionSignal = { onRefreshCompletion?: () => void };

type SourceRefreshControllerInput = RefreshCallbacks & CompletionSignal & {
  /**
   * Fired once a MANUAL all-sources refresh (`refreshSources("manual")`, from
   * the topbar control or the Sources panel) completes successfully — the
   * caller raises a transient success toast (ADR 0068 T6 adoption sweep).
   * Scheduler-triggered refreshes and per-adapter/events refreshes stay silent.
   */
  onManualRefreshSuccess?: () => void;
  scheduledSourceAdapters: SourceAdapter[];
  settings: UserSettings | null;
  sourceAdapterRefreshInFlight: string | null;
  sourceAdaptersRef: MutableRefObject<SourceAdapter[]>;
  sourceRefreshInFlightRef: MutableRefObject<boolean>;
  setRegistryRefreshError: Dispatch<SetStateAction<string | null>>;
  setRegistryRefreshResult: Dispatch<SetStateAction<CompanyRegistryRefreshResult | null>>;
  setRegistryRefreshState: Dispatch<SetStateAction<SourceRefreshState>>;
  setSelectedSourceAdapterId: Dispatch<SetStateAction<string | null>>;
  setSourceAdapterRefreshInFlight: Dispatch<SetStateAction<string | null>>;
  setSourceRefreshError: Dispatch<SetStateAction<string | null>>;
  setSourceRefreshFailureCount: Dispatch<SetStateAction<number>>;
  setSourceRefreshResult: Dispatch<SetStateAction<SourceIngestionResult | null>>;
  setSourceRefreshState: Dispatch<SetStateAction<SourceRefreshState>>;
};

function sourceAdapterRefreshDue(adapter: SourceAdapter, intervalMs: number) {
  const now = Date.now();

  if (!adapter.lastSuccessAt) {
    return true;
  }

  const lastSuccessAt = Date.parse(adapter.lastSuccessAt);
  if (Number.isNaN(lastSuccessAt)) {
    return true;
  }

  return now - lastSuccessAt >= intervalMs;
}

function isCompanyDirectorySource(adapter: SourceAdapter) {
  return adapter.sourceType === "company_registry";
}

function combineSourceResults(results: SourceIngestionResult[], adapterId: string): SourceIngestionResult {
  return results.reduce<SourceIngestionResult>(
    (combined, result) => ({
      adapterId,
      itemsFetched: combined.itemsFetched + result.itemsFetched,
      itemsCreated: combined.itemsCreated + result.itemsCreated,
      itemsMatched: combined.itemsMatched + result.itemsMatched,
      itemsUnmatched: combined.itemsUnmatched + result.itemsUnmatched,
      detailItemsAttempted: combined.detailItemsAttempted + result.detailItemsAttempted,
      detailItemsStored: combined.detailItemsStored + result.detailItemsStored,
      detailItemsFailed: combined.detailItemsFailed + result.detailItemsFailed,
      fetchedAt: result.fetchedAt ?? combined.fetchedAt,
    }),
    {
      adapterId,
      itemsFetched: 0,
      itemsCreated: 0,
      itemsMatched: 0,
      itemsUnmatched: 0,
      detailItemsAttempted: 0,
      detailItemsStored: 0,
      detailItemsFailed: 0,
      fetchedAt: null,
    },
  );
}

export function useSourceRefreshController({
  refreshCompanyEvents,
  refreshCompanyRegistryEntries,
  refreshDatabaseStatus,
  refreshFeedItems,
  refreshSignals,
  refreshSourceAdapters,
  refreshAttention,
  onRefreshCompletion,
  onManualRefreshSuccess,
  scheduledSourceAdapters,
  settings,
  sourceAdapterRefreshInFlight,
  sourceAdaptersRef,
  sourceRefreshInFlightRef,
  setRegistryRefreshError,
  setRegistryRefreshResult,
  setRegistryRefreshState,
  setSelectedSourceAdapterId,
  setSourceAdapterRefreshInFlight,
  setSourceRefreshError,
  setSourceRefreshFailureCount,
  setSourceRefreshResult,
  setSourceRefreshState,
}: SourceRefreshControllerInput) {
  function finishSourceRefresh() {
    setSourceRefreshState("done");
    window.setTimeout(() => {
      setSourceRefreshState("idle");
    }, 900);
  }

  function failSourceRefresh(error: unknown) {
    setSourceRefreshError(String(error));
    setSourceRefreshFailureCount((current) => current + 1);
    setSourceRefreshState("idle");
    refreshSourceAdapters();
  }

  function refreshViewsAfterSourceRefresh(includeFeed: boolean) {
    return Promise.all([
      // Official-report ingestion classifies filings into signals, so reload
      // signals whenever the feed is reloaded — otherwise badges only appear
      // after an app restart (the mount-time signal load).
      ...(includeFeed ? [refreshFeedItems(), refreshSignals()] : []),
      refreshCompanyEvents(),
      refreshSourceAdapters(),
      refreshDatabaseStatus(),
      // Attention events ride every ingestion (reconciliation, fired alerts) —
      // reload the app-level state so the Today badge/stream stay current.
      Promise.resolve(refreshAttention()),
    ]).then((result) => {
      // Fires on EVERY completed refresh, independent of whether attention's
      // own data happened to change on this pass (fix wave B finding 1a) — the
      // old `["todayView", attention.events]` query key relied on attention's
      // array identity changing, which `sameCollection` skips when a refresh
      // ingests new filings/media but raises no NEW attention event, silently
      // missing a Today refetch. A dedicated completion count can't miss one.
      onRefreshCompletion?.();
      return result;
    });
  }

  function anySourceAdapterRefreshDue(intervalMs: number) {
    return scheduledSourceAdapters.some((adapter) => sourceAdapterRefreshDue(adapter, intervalMs));
  }

  function refreshSources(trigger: SourceRefreshTrigger = "manual") {
    if (sourceRefreshInFlightRef.current) {
      return Promise.resolve();
    }

    if (trigger === "scheduler") {
      if (!settings || settings.pollIntervalSeconds <= 0) {
        return Promise.resolve();
      }

      if (!anySourceAdapterRefreshDue(settings.pollIntervalSeconds * 1000)) {
        return Promise.resolve();
      }
    }

    sourceRefreshInFlightRef.current = true;
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return sourcesApi.refreshSources(trigger)
      .then((response) => {
        setSourceRefreshResult(response);
        setSourceRefreshFailureCount(0);
        setSelectedSourceAdapterId(response.adapterId);
        return refreshViewsAfterSourceRefresh(true);
      })
      .then(() => {
        finishSourceRefresh();
        if (trigger === "manual") {
          onManualRefreshSuccess?.();
        }
      })
      .catch(failSourceRefresh)
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
      });
  }

  function refreshCompanyRegistry(trigger: SourceRefreshTrigger = "manual") {
    setRegistryRefreshState("refreshing");
    setRegistryRefreshError(null);

    return sourcesApi.refreshGpwCompanyRegistry(trigger)
      .then((response) => {
        setRegistryRefreshResult(response);
        if (response.adapterId !== "company-directories") {
          setSelectedSourceAdapterId(response.adapterId);
        }
        return Promise.all([refreshSourceAdapters(), refreshDatabaseStatus(), refreshCompanyRegistryEntries()]);
      })
      .then(() => {
        setRegistryRefreshState("done");
        window.setTimeout(() => {
          setRegistryRefreshState("idle");
        }, 900);
      })
      .catch((error) => {
        setRegistryRefreshError(String(error));
        setRegistryRefreshState("idle");
        refreshSourceAdapters();
      });
  }

  function refreshSingleSource(adapter: SourceAdapter, trigger: SourceRefreshTrigger = "manual") {
    if (sourceRefreshInFlightRef.current || sourceAdapterRefreshInFlight) {
      return Promise.resolve();
    }

    if (!adapter.enabled) {
      return Promise.resolve();
    }

    if (isCompanyDirectorySource(adapter)) {
      return refreshCompanyRegistry(trigger);
    }

    sourceRefreshInFlightRef.current = true;
    setSourceAdapterRefreshInFlight(adapter.id);
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return sourcesApi.refreshSource({
      adapterId: adapter.id,
      trigger,
    })
      .then((response) => {
        setSourceRefreshResult(response);
        setSourceRefreshFailureCount(0);
        setSelectedSourceAdapterId(response.adapterId);
        return refreshViewsAfterSourceRefresh(true);
      })
      .then(finishSourceRefresh)
      .catch(failSourceRefresh)
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
        setSourceAdapterRefreshInFlight(null);
      });
  }

  function refreshEventSources(trigger: SourceRefreshTrigger = "manual", date: string | null = null) {
    if (sourceRefreshInFlightRef.current || sourceAdapterRefreshInFlight) {
      return Promise.resolve();
    }

    sourceRefreshInFlightRef.current = true;
    setSourceAdapterRefreshInFlight("events");
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return eventSourceAdapterIds
      .reduce<Promise<SourceIngestionResult[]>>(
        (chain, adapterId) =>
          chain.then((results) =>
            sourcesApi.refreshSource({
              adapterId,
              trigger,
              date: adapterId === "bankier-kalendarium-html" ? date : null,
            }).then((result) => [...results, result]),
          ),
        Promise.resolve([]),
      )
      .then((results) => {
        const summary = combineSourceResults(results, "bankier-kalendarium-html");
        setSourceRefreshResult(summary);
        setSourceRefreshFailureCount(0);
        setSelectedSourceAdapterId("bankier-kalendarium-html");
        return refreshViewsAfterSourceRefresh(false);
      })
      .then(finishSourceRefresh)
      .catch(failSourceRefresh)
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
        setSourceAdapterRefreshInFlight(null);
      });
  }

  function refreshBankierCalendarWeek(date: string, trigger: SourceRefreshTrigger = "manual") {
    if (sourceRefreshInFlightRef.current || sourceAdapterRefreshInFlight) {
      return Promise.resolve();
    }

    sourceRefreshInFlightRef.current = true;
    setSourceAdapterRefreshInFlight("bankier-kalendarium-html");
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return sourcesApi.refreshSource({
      adapterId: "bankier-kalendarium-html",
      trigger,
      date,
    })
      .then((response) => {
        setSourceRefreshResult(response);
        setSourceRefreshFailureCount(0);
        setSelectedSourceAdapterId(response.adapterId);
        return refreshViewsAfterSourceRefresh(false);
      })
      .then(finishSourceRefresh)
      .catch(failSourceRefresh)
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
        setSourceAdapterRefreshInFlight(null);
      });
  }

  function refreshScheduledSource(adapterId: string, intervalMs: number) {
    const adapter = sourceAdaptersRef.current.find((sourceAdapter) => sourceAdapter.id === adapterId);

    if (!adapter || !adapter.enabled) {
      return Promise.resolve();
    }

    if (sourceRefreshInFlightRef.current || !sourceAdapterRefreshDue(adapter, intervalMs)) {
      return Promise.resolve();
    }

    sourceRefreshInFlightRef.current = true;
    setSourceRefreshState("refreshing");
    setSourceRefreshError(null);

    return sourcesApi.refreshSource({
      adapterId: adapter.id,
      trigger: "scheduler",
    })
      .then((response) => {
        setSourceRefreshResult(response);
        setSourceRefreshFailureCount(0);
        return refreshViewsAfterSourceRefresh(true);
      })
      .then(finishSourceRefresh)
      .catch(failSourceRefresh)
      .finally(() => {
        sourceRefreshInFlightRef.current = false;
      });
  }

  function refreshCompanyRegistryIfStale(staleAfterSeconds: number) {
    return sourcesApi.refreshGpwCompanyRegistryIfStale({
      trigger: "scheduler",
      staleAfterSeconds,
    })
      .then((response) => {
        if (!response) {
          return Promise.resolve();
        }

        setRegistryRefreshResult(response);
        return Promise.all([refreshSourceAdapters(), refreshDatabaseStatus(), refreshCompanyRegistryEntries()]).then(
          () => undefined,
        );
      })
      .catch((error) => {
        setRegistryRefreshError(String(error));
        refreshSourceAdapters();
      });
  }

  return {
    refreshBankierCalendarWeek,
    refreshCompanyRegistry,
    refreshCompanyRegistryIfStale,
    refreshEventSources,
    refreshScheduledSource,
    refreshSingleSource,
    refreshSources,
  };
}
