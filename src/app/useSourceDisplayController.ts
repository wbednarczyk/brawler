import type { Dispatch, KeyboardEvent, SetStateAction } from "react";
import type { SourceAdapter, UserSettings } from "../api/types";
import { formatPollInterval } from "../shared/formatting/date";
import { gpwRegistryAdapterId } from "./sourceScheduler";
import type { Section } from "./navigation";

type SourceDisplayControllerInput = {
  nextRegistryRefreshAt: number | null;
  nextSourceRefreshAtByAdapterId: Record<string, number>;
  refreshCompanyRegistryEntries: () => Promise<void>;
  refreshUnmatchedSourceItems: (adapterId: string) => Promise<void>;
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setCompanyRegistryListExpanded: Dispatch<SetStateAction<boolean>>;
  setExpandedUnmatchedAdapters: Dispatch<SetStateAction<Record<string, boolean>>>;
  setSelectedSourceAdapterId: Dispatch<SetStateAction<string | null>>;
  settings: UserSettings | null;
  sourceAdapters: SourceAdapter[];
  sourceRefreshFailureCount: number;
};

export function useSourceDisplayController({
  nextRegistryRefreshAt,
  nextSourceRefreshAtByAdapterId,
  refreshCompanyRegistryEntries,
  refreshUnmatchedSourceItems,
  setActiveSection,
  setCompanyRegistryListExpanded,
  setExpandedUnmatchedAdapters,
  setSelectedSourceAdapterId,
  settings,
  sourceAdapters,
  sourceRefreshFailureCount,
}: SourceDisplayControllerInput) {
  function toggleSourceAdapter(adapterId: string) {
    setSelectedSourceAdapterId((current) => (current === adapterId ? null : adapterId));
    if (adapterId === gpwRegistryAdapterId) {
      refreshCompanyRegistryEntries();
    } else {
      refreshUnmatchedSourceItems(adapterId);
    }
  }

  function toggleUnmatchedSourceItems(adapterId: string) {
    setExpandedUnmatchedAdapters((current) => ({
      ...current,
      [adapterId]: !current[adapterId],
    }));
    refreshUnmatchedSourceItems(adapterId);
  }

  function toggleCompanyRegistryList() {
    setCompanyRegistryListExpanded((current) => !current);
    refreshCompanyRegistryEntries();
  }

  function openSourceStatus() {
    const relevantAdapter =
      sourceAdapters.find((adapter) => adapter.lastError) ??
      sourceAdapters.find((adapter) => adapter.enabled) ??
      sourceAdapters[0] ??
      null;

    setSelectedSourceAdapterId(relevantAdapter?.id ?? null);
    if (relevantAdapter) {
      if (relevantAdapter.id === gpwRegistryAdapterId) {
        refreshCompanyRegistryEntries();
      } else {
        refreshUnmatchedSourceItems(relevantAdapter.id);
      }
    }
    setActiveSection("Sources");
  }

  function toggleSourceAdapterFromKeyboard(
    event: KeyboardEvent<HTMLElement>,
    adapterId: string,
  ) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggleSourceAdapter(adapterId);
    }
  }

  function formatSourceScheduler(adapter: SourceAdapter) {
    if (!adapter.enabled) {
      return "Off";
    }

    if (adapter.id === gpwRegistryAdapterId) {
      return adapter.defaultPollIntervalSeconds > 0
        ? `In-app · ${formatPollInterval(adapter.defaultPollIntervalSeconds)}`
        : "Off";
    }

    if (!settings || settings.pollIntervalSeconds <= 0) {
      return "Off";
    }

    if (sourceRefreshFailureCount >= 2) {
      return `In-app · ${formatPollInterval(settings.pollIntervalSeconds)} · backoff ${formatPollInterval(
        Math.min(settings.pollIntervalSeconds * 2, 3600),
      )}`;
    }

    return `In-app · ${formatPollInterval(settings.pollIntervalSeconds)}`;
  }

  function formatSourceTrigger(adapter: SourceAdapter) {
    if (adapter.lastTrigger === "scheduler") {
      return "Scheduler";
    }

    if (adapter.lastTrigger === "manual") {
      return "Manual";
    }

    return "None";
  }

  function formatNextRefresh(adapter: SourceAdapter) {
    if (!adapter.enabled) {
      return "Off";
    }

    const nextRefreshAt =
      adapter.id === gpwRegistryAdapterId ? nextRegistryRefreshAt : nextSourceRefreshAtByAdapterId[adapter.id];

    if (!nextRefreshAt) {
      return "Off";
    }

    const seconds = Math.max(0, Math.ceil((nextRefreshAt - Date.now()) / 1000));
    return `In ${formatPollInterval(seconds)}`;
  }

  return {
    formatNextRefresh,
    formatSourceScheduler,
    formatSourceTrigger,
    openSourceStatus,
    toggleCompanyRegistryList,
    toggleSourceAdapter,
    toggleSourceAdapterFromKeyboard,
    toggleUnmatchedSourceItems,
  };
}
