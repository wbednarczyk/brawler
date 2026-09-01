import type { Dispatch, KeyboardEvent, SetStateAction } from "react";
import type { SourceAdapter, UserSettings } from "../api/types";
import { useLocale, type LocaleCode } from "../shared/locale";
import { pluralNoun, type PluralForms } from "../shared/locale/plural";
import type { Section } from "./navigation";

const DAY_FORMS: PluralForms = { en: ["day", "days"], pl: ["dzień", "dni", "dni"] };

// Locale-aware duration for the scheduler/next-refresh sentences (sol R1
// finding: `formatPollInterval`, shared/format/datetime.ts, is English-only
// and spells out "day"/"days" — embedding it in an otherwise-Polish sentence
// via string concatenation produced mixed-language text, e.g. "Automatycznie
// co 1 day"). `formatPollInterval` itself stays untouched — Settings screens
// use it too, unrelated to this bug. min/h/s stay bare abbreviations
// (already locale-neutral in this app's convention, not flagged); only the
// spelled-out day word needs a plural form per locale.
function formatDurationLocalized(seconds: number, locale: LocaleCode): string {
  if (seconds >= 86400 && seconds % 86400 === 0) {
    const days = seconds / 86400;
    return `${days} ${pluralNoun(locale, days, DAY_FORMS)}`;
  }
  if (seconds >= 86400) {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    if (hours === 0) return `${days} ${pluralNoun(locale, days, DAY_FORMS)}`;
    return `${days}d ${hours}h`;
  }
  if (seconds >= 3600) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
  }
  if (seconds < 60) return `${seconds}s`;
  if (seconds % 60 === 0) return `${seconds / 60} min`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes} min ${seconds % 60}s`;
}

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
  const { locale, text } = useLocale();

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

  // F4b S4 (contract § Sources telemetry pass; sol R1: one text() sentence
  // template per shape, no fragment concatenation): each shape is ONE
  // translated template with placeholders, filled in after translation — a
  // Polish sentence can never end up with a raw English interval spliced in.
  // "backoff" retires in favor of naming the retry (dev-vocabulary guardrail).
  function formatSourceScheduler(adapter: SourceAdapter) {
    if (!adapter.enabled) {
      return text("Off");
    }

    if (isCompanyDirectorySource(adapter)) {
      return adapter.defaultPollIntervalSeconds > 0
        ? text("Automatically every {interval}").replace(
            "{interval}",
            formatDurationLocalized(adapter.defaultPollIntervalSeconds, locale),
          )
        : text("Off");
    }

    if (!settings || settings.pollIntervalSeconds <= 0) {
      return text("Off");
    }

    if (sourceRefreshFailureCount >= 2) {
      return text("Automatically every {interval} · retry in {retry}")
        .replace("{interval}", formatDurationLocalized(settings.pollIntervalSeconds, locale))
        .replace(
          "{retry}",
          formatDurationLocalized(Math.min(settings.pollIntervalSeconds * 2, 3600), locale),
        );
    }

    return text("Automatically every {interval}").replace(
      "{interval}",
      formatDurationLocalized(settings.pollIntervalSeconds, locale),
    );
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
    return text("next in {time}").replace("{time}", formatDurationLocalized(seconds, locale));
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
