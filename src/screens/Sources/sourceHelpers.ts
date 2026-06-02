import type { SourceAdapter } from "../../api/types";
import { formatEnumLabel } from "../../shared/formatting/labels";

export function formatSourceLastResult(adapter: SourceAdapter) {
  if (adapter.lastItemsFetched === null) {
    return "None";
  }

  if (adapter.id === "gpw-company-registry") {
    return `${adapter.lastItemsFetched} cached entries · ${
      adapter.lastItemsCreated ?? 0
    } refreshed or updated`;
  }

  const listingResult = `${adapter.lastItemsFetched} fetched · ${adapter.lastItemsCreated ?? 0} created · ${
    adapter.lastItemsMatched ?? 0
  } matched · ${adapter.lastItemsUnmatched ?? 0} unmatched`;

  if (adapter.lastDetailItemsAttempted === null) {
    return listingResult;
  }

  return `${listingResult} · details ${adapter.lastDetailItemsStored ?? 0}/${
    adapter.lastDetailItemsAttempted
  } stored · ${adapter.lastDetailItemsFailed ?? 0} failed`;
}

export function formatSourceType(value: string) {
  const labels: Record<string, string> = {
    official_report: "Official Reports",
    official_report_secondary: "Secondary Official Reports",
    official_calendar: "Official Calendar",
    public_calendar: "Public Calendar",
    public_media: "Public Media",
    analysis: "Analysis",
    authenticated_research: "Authenticated Research",
    company_registry: "Company Registry",
  };

  return labels[value] ?? formatEnumLabel(value);
}

export function formatFetchMode(value: string) {
  const labels: Record<string, string> = {
    public_page: "Public Page",
    rss: "RSS",
    public_json: "Public JSON",
    api: "API",
    manual: "Manual",
    authenticated: "Authenticated",
    paywalled: "Paywalled",
  };

  return labels[value] ?? formatEnumLabel(value);
}

export function formatSourceAccess(adapter: SourceAdapter) {
  if (!adapter.enabled) {
    return "Disabled";
  }

  const labels: Record<string, string> = {
    public_page: "Public Web Page",
    rss: "Public RSS",
    public_json: "Public JSON",
    api: "Public API",
    manual: "Manual/Local",
    authenticated: "Authenticated",
    paywalled: "Paywalled",
  };

  return labels[adapter.fetchMode] ?? formatFetchMode(adapter.fetchMode);
}

export function formatSourceSubtitle(adapter: SourceAdapter) {
  if (adapter.id === "gpw-company-registry") {
    return "Company registry · Public GPW company list";
  }

  return `${formatSourceType(adapter.sourceType)} · ${formatFetchMode(adapter.fetchMode)}`;
}

export function sourceLastResultLabel(adapter: SourceAdapter) {
  return adapter.id === "gpw-company-registry" ? "Cache result" : "Last result";
}

export function sourcePolicyLabel(adapter: SourceAdapter) {
  return adapter.id === "gpw-company-registry" ? "Refresh policy" : "Rate limit";
}

export function formatSourceTrigger(adapter: SourceAdapter) {
  if (adapter.lastTrigger === "scheduler") {
    return "Scheduler";
  }

  if (adapter.lastTrigger === "manual") {
    return "Manual";
  }

  return "None";
}
