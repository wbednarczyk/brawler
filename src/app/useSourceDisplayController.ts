import type { Dispatch, KeyboardEvent, SetStateAction } from "react";
import type { SourceAdapter, UserSettings } from "../api/types";
import { formatPollInterval } from "../shared/format/datetime";
import { useLocale } from "../shared/locale";
import type { Section } from "./navigation";

type SourceDisplayControllerInput = {
  nextRegistryRefreshAt: number | null;
  nextSourceRefreshAtByAdapterId: Record<string, number>;
  refreshCompanyRegistryEntries: () => Promise<void>;
  setActiveSection: Dispatch<SetStateAction<Section>>;
  setCompanyRegistryListExpanded: Dispatch<SetStateAction<boolean>>;
  setSelectedSourceAdapterId: Dispatch<SetStateAction<string | null>>;
  settings: UserSettings | null;
  sourceAdapters: SourceAdapter[];
  sourceRefreshFailureCount: number;
};

export function useSourceDisplayController({
  nextRegistryRefreshAt,
  nextSourceRefreshAtByAdapterId,
  refreshCompanyRegistryEntries,
  setActiveSection,
  setCompanyRegistryListExpanded,
  setSelectedSourceAdapterId,
  settings,
  sourceAdapters,
  sourceRefreshFailureCount,
}: SourceDisplayControllerInput) {
  const { text } = useLocale();

  function isCompanyDirectorySource(adapter: SourceAdapter) {
    return adapter.sourceType === "company_registry";
  }

  function toggleSourceAdapter(adapterId: string) {
    setSelectedSourceAdapterId((current) => (current === adapterId ? null : adapterId));
    const adapter = sourceAdapters.find((sourceAdapter) => sourceAdapter.id === adapterId);
    if (adapter && isCompanyDirectorySource(adapter)) {
      refreshCompanyRegistryEntries();
    }
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
    if (relevantAdapter && isCompanyDirectorySource(relevantAdapter)) {
      refreshCompanyRegistryEntries();
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

  // F4b S4 (contract § Sources telemetry pass): the cadence line interpolates
  // a formatted interval, so the composed sentence can never round-trip
  // through the flat `text()` string map at the call site — only the static
  // words are translated here, the dynamic pieces are concatenated after.
  // "backoff" retires in favor of naming the retry (dev-vocabulary guardrail).
  function formatSourceScheduler(adapter: SourceAdapter) {
    if (!adapter.enabled) {
      return text("Off");
    }

    if (isCompanyDirectorySource(adapter)) {
      return adapter.defaultPollIntervalSeconds > 0
        ? `${text("Automatically every")} ${formatPollInterval(adapter.defaultPollIntervalSeconds)}`
        : text("Off");
    }

    if (!settings || settings.pollIntervalSeconds <= 0) {
      return text("Off");
    }

    if (sourceRefreshFailureCount >= 2) {
      return `${text("Automatically every")} ${formatPollInterval(settings.pollIntervalSeconds)} · ${text(
        "retry in",
      )} ${formatPollInterval(Math.min(settings.pollIntervalSeconds * 2, 3600))}`;
    }

    return `${text("Automatically every")} ${formatPollInterval(settings.pollIntervalSeconds)}`;
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
      return text("Off");
    }

    const nextRefreshAt =
      isCompanyDirectorySource(adapter) ? nextRegistryRefreshAt : nextSourceRefreshAtByAdapterId[adapter.id];

    if (!nextRefreshAt) {
      return text("Off");
    }

    const seconds = Math.max(0, Math.ceil((nextRefreshAt - Date.now()) / 1000));
    return `${text("next in")} ${formatPollInterval(seconds)}`;
  }

  return {
    formatNextRefresh,
    formatSourceScheduler,
    formatSourceTrigger,
    openSourceStatus,
    toggleCompanyRegistryList,
    toggleSourceAdapter,
    toggleSourceAdapterFromKeyboard,
  };
}
