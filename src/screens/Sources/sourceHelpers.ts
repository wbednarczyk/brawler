import type { SourceAdapter } from "../../api/types";
import { formatEnumLabel } from "../../shared/formatting/labels";

type SourceGroupId =
  | "officialReports"
  | "calendarEvents"
  | "publicMedia"
  | "companyDirectory";

export type SourceAdapterGroup = {
  id: SourceGroupId;
  label: string;
  description: string;
  adapters: SourceAdapter[];
};

const sourceGroupDefinitions: Array<Omit<SourceAdapterGroup, "adapters">> = [
  {
    id: "officialReports",
    label: "Official reports",
    description: "Regulatory reports and official disclosure feeds.",
  },
  {
    id: "calendarEvents",
    label: "Calendar and events",
    description: "Company dates, report calendars, and market event feeds.",
  },
  {
    id: "publicMedia",
    label: "Public media and news",
    description: "Public market news and company-specific media sources.",
  },
  {
    id: "companyDirectory",
    label: "Company directory",
    description: "Company lookup and matching support.",
  },
];

export function groupSourceAdapters(adapters: SourceAdapter[]): SourceAdapterGroup[] {
  const groups = new Map<SourceGroupId, SourceAdapter[]>(
    sourceGroupDefinitions.map((group) => [group.id, []]),
  );

  for (const adapter of adapters) {
    groups.get(sourceGroupIdForAdapter(adapter))?.push(adapter);
  }

  return sourceGroupDefinitions
    .map((definition) => ({
      ...definition,
      adapters: (groups.get(definition.id) ?? []).sort(compareSourceAdapters),
    }))
    .filter((group) => group.adapters.length > 0);
}

function sourceGroupIdForAdapter(adapter: SourceAdapter): SourceGroupId {
  if (adapter.sourceType === "company_registry") {
    return "companyDirectory";
  }

  if (adapter.sourceType === "official_report" || adapter.sourceType === "official_report_secondary") {
    return "officialReports";
  }

  if (adapter.sourceType === "official_calendar" || adapter.sourceType === "public_calendar") {
    return "calendarEvents";
  }

  return "publicMedia";
}

function compareSourceAdapters(left: SourceAdapter, right: SourceAdapter) {
  if (left.lastError && !right.lastError) {
    return -1;
  }

  if (!left.lastError && right.lastError) {
    return 1;
  }

  return left.displayName.localeCompare(right.displayName);
}

// The plain-string sentence formerly built here now renders as
// `SourceLastResultText` (`SourceAdapterRow.tsx`, F4b S4 contract § Sources
// telemetry pass) — one `Figure kind="count"` per number instead of an
// interpolated string `text()` can never round-trip.

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
    public_json: "Public data",
    api: "API",
    manual: "Manual",
    authenticated: "Authenticated",
    paywalled: "Subscription",
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
  manual: "Manual",
    authenticated: "Authenticated",
    paywalled: "Paywalled",
  };

  return labels[adapter.fetchMode] ?? formatFetchMode(adapter.fetchMode);
}

export function formatSourceSubtitle(adapter: SourceAdapter) {
  if (adapter.id === "gpw-company-registry") {
    return "Company directory · GPW company list";
  }

  if (adapter.id === "newconnect-company-directory") {
    return "Company directory · NewConnect company list";
  }

  return formatSourceType(adapter.sourceType);
}

export function formatSourceHealth(adapter: SourceAdapter) {
  const labels: Record<SourceAdapter["healthStatus"], string> = {
    healthy: "Healthy",
    attention: "Needs attention",
    notRefreshed: "Not refreshed yet",
    off: "Off",
  };

  return labels[adapter.healthStatus] ?? "Unknown";
}

export function sourceLastResultLabel(adapter: SourceAdapter) {
  return adapter.sourceType === "company_registry" ? "Directory result" : "Last result";
}

export function sourcePolicyLabel(adapter: SourceAdapter) {
  return adapter.sourceType === "company_registry" ? "Refresh policy" : "Rate limit";
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
