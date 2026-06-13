import { mockIPC } from "@tauri-apps/api/mocks";
import type {
  Company,
  CompanyEvent,
  CompanyRegistryEntry,
  CredentialStatus,
  FeedItem,
  LicenseStatus,
  LocalMetricsSnapshot,
  NotebookEntry,
  SourceAdapter,
  TranscriptJob,
  UserSettings,
  Watchlist,
  WatchlistMembership,
} from "../api/types";

type InvokeArgs = Record<string, unknown> | undefined;

const companies: Company[] = [
  company("company_gpw_cdr", "GPW", "CDR", "CD PROJEKT S.A.", "PLOPTTC00011"),
  company("company_gpw_pkn", "GPW", "PKN", "ORLEN S.A.", "PLPKN0000018"),
  company("company_gpw_kgh", "GPW", "KGH", "KGHM POLSKA MIEDZ S.A.", "PLKGHM000017"),
  company("company_gpw_pzu", "GPW", "PZU", "PZU S.A.", "PLPZU0000011"),
  company("company_gpw_dnp", "GPW", "DNP", "DINO POLSKA S.A.", "PLDINPL00011"),
  company("company_gpw_acp", "GPW", "ACP", "ASSECO POLAND S.A.", "PLSOFTB00016"),
  company("company_gpw_bft", "GPW", "BFT", "BENEFIT SYSTEMS S.A.", "PLBNFTS00107"),
  company("company_gpw_cbf", "GPW", "CBF", "CYFROWY POLSAT S.A.", "PLCFRPT00013"),
  company("company_gpw_cmp", "GPW", "CMP", "COMPREMUM S.A.", "PLCOMP000001"),
  company("company_gpw_nc4", "NC", "4MB", "4MOBILITY SPOLKA AKCYJNA", "PLESLTN00010"),
  ...Array.from({ length: 18 }, (_, index) => {
    const number = index + 1;
    const ticker = `T${String(number).padStart(2, "0")}`;
    return company(
      `company_gpw_${ticker.toLowerCase()}`,
      "GPW",
      ticker,
      `BROWSER SMOKE COMPANY ${number} S.A.`,
      `PLSMOKE${String(number).padStart(5, "0")}`,
    );
  }),
];

const watchlists: Watchlist[] = [
  { id: "watchlist_main_gpw", name: "Main GPW", description: null, companyCount: 6 },
  { id: "watchlist_followups", name: "Follow-up", description: null, companyCount: 3 },
];

const watchlistMemberships: WatchlistMembership[] = [
  ...companies.slice(0, 16).map((entry) => membership("watchlist_main_gpw", "Main GPW", entry.id)),
  membership("watchlist_followups", "Follow-up", "company_gpw_cdr"),
  membership("watchlist_followups", "Follow-up", "company_gpw_acp"),
  membership("watchlist_followups", "Follow-up", "company_gpw_bft"),
];

const feedItems: FeedItem[] = companies.slice(0, 7).map((entry, index) => ({
  id: `feed_${entry.id}`,
  company: entry.qualifiedTicker,
  type: index % 2 === 0 ? "Official report" : "News",
  source: index % 2 === 0 ? "Bankier Company Komunikaty" : "Bankier Gielda RSS",
  time: index === 0 ? "Today 09:12" : "Yesterday",
  title: `${entry.displayName} source item for browser layout smoke`,
  unread: index < 3,
  saved: index === 1,
  sourceUrl: "https://example.test/source",
  language: "en",
  publishedAt: "2026-06-05T09:12:00Z",
  fetchedAt: "2026-06-05T09:15:00Z",
  attribution: "Sample",
  summary: "Deterministic source item used by browser UI regression smoke tests.",
  bodyText: "Longer body text remains available for detail-pane layout checks.",
  attachments: [{ id: `attachment_${entry.id}`, label: "report.pdf", url: "https://example.test/report.pdf" }],
}));

const sourceAdapters: SourceAdapter[] = [
  sourceAdapter("gpw-company-registry", "GPW Company Directory", "company_registry", "required", true, ["GPW"], 470),
  sourceAdapter("newconnect-company-directory", "NewConnect Company Directory", "company_registry", "required", true, ["NEWCONNECT"], 350),
  sourceAdapter("bankier-company-komunikaty", "Bankier Company Komunikaty", "official_report", "optional", true, ["GPW"], 7),
  sourceAdapter("bankier-market-rss", "Bankier Gielda RSS", "public_media", "optional", false, ["GPW"], 3),
  sourceAdapter("gpw-market-events-rss", "GPW Market Events RSS", "official_calendar", "optional", true, ["GPW"], 5),
  sourceAdapter("bankier-kalendarium-html", "Bankier Kalendarium", "public_calendar", "optional", true, ["GPW"], 4),
  sourceAdapter("portal-analiz", "Portal Analiz", "authenticated_research", "developer", false, ["GPW"], null),
];

const registryEntries: CompanyRegistryEntry[] = companies.map((entry) => ({
  sourceAdapterId: entry.exchange === "NC" ? "newconnect-company-directory" : "gpw-company-registry",
  exchange: entry.exchange,
  ticker: entry.ticker,
  qualifiedTicker: entry.qualifiedTicker,
  displayName: entry.displayName,
  isin: entry.isin,
  sourceUrl: "https://example.test/company",
  fetchedAt: "2026-06-05T09:00:00Z",
  tracked: true,
}));

const notebookEntries: NotebookEntry[] = companies.flatMap((entry, index) =>
  Array.from({ length: 18 }, (_, noteIndex) =>
    notebookEntry(
      `note_${entry.id}_${noteIndex + 1}`,
      entry.id,
      `${entry.ticker} research note ${noteIndex + 1}`,
      (index + noteIndex) % 3 === 0 ? "claim" : "note",
    ),
  ),
);

const companyEvents: CompanyEvent[] = companies.slice(0, 4).map((entry, index) => ({
  id: `event_${entry.id}`,
  companyId: entry.id,
  company: entry.qualifiedTicker,
  companyName: entry.displayName,
  eventType: "periodic_report",
  title: `${entry.ticker} periodic report`,
  eventDate: `2026-06-${String(10 + index).padStart(2, "0")}`,
  eventTime: null,
  status: "scheduled",
  sourceType: "official_calendar",
  sourceAdapterId: "gpw-market-events-rss",
  sourceEventKey: `event:${entry.id}`,
  sourceUrl: "https://example.test/event",
  attribution: "Sample",
  fetchedAt: "2026-06-05T09:00:00Z",
  manual: false,
  createdAt: "2026-06-05T09:00:00Z",
  updatedAt: "2026-06-05T09:00:00Z",
}));

const settings: UserSettings = {
  theme: "dark",
  locale: "en",
  accentPalette: "night-neon",
  developerMode: false,
  pollIntervalSeconds: 900,
  settingsSource: "browser-smoke",
  settingsImportExportFormat: "yaml",
  yamlImportExportStatus: "accepted_deferred",
  aiProviders: {
    youtubeTranscriptionProvider: "provider_gemini",
    youtubeTranscriptionModel: "gemini-2.5-flash",
    youtubeTranscriptionTimeoutSeconds: 300,
    generalAnalysisProvider: null,
    generalAnalysisModel: "gemini-2.5-flash",
    generalAnalysisTimeoutSeconds: 90,
  },
  aiAnalysisMode: "source_grounded",
  logs: { level: "info", maxFiles: 5, maxFileBytes: 5_242_880 },
  shortcutBindings: {},
};

const licenseStatus: LicenseStatus = {
  status: "valid",
  canUseApp: true,
  reason: null,
  license: {
    licenseId: "browser_smoke",
    holder: "Browser Smoke",
    channel: "author",
    edition: "author",
    features: ["core"],
    issuedAt: "2026-06-01T00:00:00Z",
    expiresAt: "2027-01-01T00:00:00Z",
    appVersionRange: "*",
    keyId: "browser_smoke",
  },
  checkedAt: "2026-06-05T09:00:00Z",
};

const credentialStatus: CredentialStatus = {
  providerId: "provider_gemini",
  secretKind: "api_key",
  configured: false,
  storage: "not_configured",
  label: "Gemini API key",
  devFallbackAvailable: false,
  error: null,
};

const localMetricsSnapshot: LocalMetricsSnapshot = {
  collectedAt: "2026-06-05T09:00:00Z",
  samples: [],
};

export function installBrowserSmokeRuntime() {
  mockIPC((command, args) => handleCommand(command, args as InvokeArgs), { shouldMockEvents: true });
}

function handleCommand(command: string, args: InvokeArgs) {
  switch (command) {
    case "health":
      return { status: "ok", version: "0.22.0" };
    case "database_status":
      return { appliedMigrations: 29, companies: companies.length, sourceAdapters: sourceAdapters.length, settings: 12 };
    case "get_license_status":
      return licenseStatus;
    case "get_settings":
      return settings;
    case "list_companies":
      return companies;
    case "list_watchlists":
      return watchlists;
    case "list_watchlist_memberships":
      return watchlistMemberships;
    case "list_feed_items":
      return feedItems;
    case "list_source_adapters":
      return listSourceAdapters(args);
    case "list_unmatched_source_items":
      return [];
    case "list_company_registry_entries":
      return registryEntries;
    case "list_company_events":
      return companyEvents;
    case "list_video_transcript_jobs":
      return [] satisfies TranscriptJob[];
    case "list_notebook_entries":
      return notebookEntries.filter((entry) => entry.companyId === (args as { companyId?: string })?.companyId);
    case "get_provider_credential_status":
      return credentialStatus;
    case "list_ai_analysis_jobs":
      return [];
    case "get_local_metrics_snapshot":
      return localMetricsSnapshot;
    case "refresh_gpw_company_registry_if_stale":
      return null;
    case "delete_unsaved_feed_items":
      return { itemsDeleted: 0, deletedAt: "2026-06-05T09:00:00Z" };
    case "prune_old_feed_items":
      return { retentionDays: 30, itemsDeleted: 0, prunedAt: "2026-06-05T09:00:00Z" };
    case "plugin:path|resolve_directory":
      return "/tmp";
    case "plugin:path|resolve":
      return "/tmp/brawler-export.json";
    case "plugin:dialog|save":
      return "/tmp/brawler-export.json";
    case "plugin:fs|write_text_file":
    case "plugin:opener|open_url":
      return null;
    default:
      throw new Error(`Unhandled browser smoke command: ${command}`);
  }
}

function company(id: string, exchange: string, ticker: string, displayName: string, isin: string): Company {
  return {
    id,
    exchange,
    ticker,
    qualifiedTicker: `${exchange}:${ticker}`,
    displayName,
    isin,
    cik: null,
    lei: null,
  };
}

function membership(watchlistId: string, watchlistName: string, companyId: string): WatchlistMembership {
  return { watchlistId, watchlistName, companyId };
}

function sourceAdapter(
  id: string,
  displayName: string,
  sourceType: string,
  visibility: SourceAdapter["visibility"],
  enabled: boolean,
  markets: string[],
  fetched: number | null,
): SourceAdapter {
  return {
    id,
    displayName,
    sourceType,
    fetchMode: "public_page",
    visibility,
    userConfigurable: visibility === "optional",
    healthStatus: enabled ? "healthy" : "off",
    enabled,
    defaultPollIntervalSeconds: enabled ? 900 : 0,
    sourceUrl: "https://example.test/source",
    rateLimitPolicy: "Browser smoke test data",
    policyNote: "Browser smoke test data",
    lastAttemptAt: "2026-06-05T09:00:00Z",
    lastTrigger: "manual",
    lastSuccessAt: enabled ? "2026-06-05T09:00:00Z" : null,
    lastErrorAt: null,
    lastError: null,
    lastItemsFetched: fetched,
    lastItemsCreated: fetched,
    lastItemsMatched: fetched,
    lastItemsUnmatched: 0,
    lastDetailItemsAttempted: null,
    lastDetailItemsStored: null,
    lastDetailItemsFailed: null,
    lastDetailWarning: null,
    markets,
  };
}

function notebookEntry(id: string, companyId: string, title: string, kind: string): NotebookEntry {
  return {
    id,
    companyId,
    title,
    body: `# ${title}\n\nBrowser smoke note body with enough content to verify panel scrolling and rendering.`,
    bodyFormat: "markdown",
    tags: ["browser-smoke"],
    kind,
    claimStatus: kind === "claim" ? "open" : null,
    eventDate: null,
    followUpAfter: kind === "claim" ? "2026-Q3" : null,
    followUpDate: null,
    createdAt: "2026-06-05T09:00:00Z",
    updatedAt: "2026-06-05T09:00:00Z",
    origins: [],
  };
}

function listSourceAdapters(args: InvokeArgs) {
  const includeDeveloperOnly = Boolean(
    (args as { input?: { includeDeveloperOnly?: boolean } } | undefined)?.input?.includeDeveloperOnly,
  );

  return includeDeveloperOnly
    ? sourceAdapters
    : sourceAdapters.filter((adapter) => adapter.visibility !== "developer");
}
