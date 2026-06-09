import { render } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { downloadDir, join } from "@tauri-apps/api/path";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { openUrl } from "@tauri-apps/plugin-opener";
import { beforeEach, vi } from "vitest";
import { App } from "../App";
import type {
  AiAnalysisJob,
  AccentPalette,
  AppLocale,
  LicenseStatus,
  LocalMetricsSnapshot,
  ShortcutBindingSetting,
  Theme,
  UserSettings,
  Watchlist,
  WatchlistMembership,
} from "../api/types";

export { screen, waitFor, within } from "@testing-library/react";
export { default as userEvent } from "@testing-library/user-event";
export { invoke } from "@tauri-apps/api/core";
export { downloadDir, join } from "@tauri-apps/api/path";
export { save } from "@tauri-apps/plugin-dialog";
export { writeTextFile } from "@tauri-apps/plugin-fs";
export { openUrl } from "@tauri-apps/plugin-opener";
export { expect } from "vitest";
export { vi };

export function renderApp() {
  return render(<App initialLicenseStatus={appTestState.licenseStatusResponse} />);
}

const noFeedAttachments: Array<{ id: string; label: string; url: string }> = [];

function formatTestDate(date: Date) {
const year = date.getFullYear();
const month = String(date.getMonth() + 1).padStart(2, "0");
const day = String(date.getDate()).padStart(2, "0");

return `${year}-${month}-${day}`;
}

function currentWeekTestDate(dayOffset: number) {
const today = new Date();
const weekday = today.getDay();
const mondayOffset = weekday === 0 ? -6 : 1 - weekday;
const date = new Date(today);
date.setDate(today.getDate() + mondayOffset + dayOffset);

return formatTestDate(date);
}

const initialFeedItems = [
{
  id: "feed_sample_cdr_report",
  company: "GPW:CDR",
  type: "Official report",
  source: "GPW ESPI/EBI",
  time: "Today 09:12",
  title: "Current report placeholder for watchlist company",
  unread: true,
  saved: false,
  sourceUrl: "https://www.gpw.pl/komunikaty",
  language: "pl",
  publishedAt: "Today 09:12",
  fetchedAt: "Today 09:15",
  attribution: "GPW",
  summary: "Sample official report used to validate feed filtering and detail rendering.",
  bodyText: "",
  attachments: noFeedAttachments,
},
{
  id: "feed_sample_pkn_news",
  company: "GPW:PKN",
  type: "News",
  source: "Sample feed",
  time: "Yesterday",
  title: "Sample item proving the inbox layout can scan dense rows",
  unread: false,
  saved: true,
  sourceUrl: "https://example.test/sample/pkn",
  language: "en",
  publishedAt: "Yesterday",
  fetchedAt: "Yesterday",
  attribution: "Sample",
  summary: "Saved sample item used to validate the saved filter before real ingestion exists.",
  bodyText: "",
  attachments: noFeedAttachments,
},
{
  id: "feed_sample_kgh_transcript",
  company: "GPW:KGH",
  type: "Transcript",
  source: "Sample transcript",
  time: "Mon",
  title: "Transcript-derived note candidate waits for future provider work",
  unread: false,
  saved: false,
  sourceUrl: "https://example.test/sample/kgh-transcript",
  language: "en",
  publishedAt: "Mon",
  fetchedAt: "Mon",
  attribution: "Sample",
  summary: "Transcript placeholder for future video and notebook workflows.",
  bodyText: "",
  attachments: noFeedAttachments,
},
{
  id: "feed_sample_pzu_report",
  company: "GPW:PZU",
  type: "Official report",
  source: "GPW ESPI/EBI",
  time: "Fri",
  title: "PZU governance report placeholder",
  unread: false,
  saved: false,
  sourceUrl: "https://www.gpw.pl/komunikaty",
  language: "pl",
  publishedAt: "Fri",
  fetchedAt: "Fri",
  attribution: "GPW",
  summary: "Fourth sample item keeps the sample feed aligned with tracked GPW lookup companies.",
  bodyText: "",
  attachments: noFeedAttachments,
},
];

const rawSourceAdapters = [
{
  id: "gpw-espi-ebi",
  displayName: "GPW ESPI/EBI",
  sourceType: "official_report",
  fetchMode: "public_page",
  enabled: false,
  defaultPollIntervalSeconds: 0,
  sourceUrl: "https://www.gpw.pl/komunikaty",
  rateLimitPolicy: "Disabled while Bankier Company Komunikaty is the active official-report source",
  policyNote:
    "Registered for later revisit, but disabled because the global GPW listing slice missed tracked-company reports found by Bankier per-company komunikaty pages.",
  lastAttemptAt: null,
  lastTrigger: "manual",
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: 2,
  lastItemsCreated: 1,
  lastItemsMatched: 1,
  lastItemsUnmatched: 1,
  lastDetailItemsAttempted: 1,
  lastDetailItemsStored: 1,
  lastDetailItemsFailed: 0,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "gpw-company-registry",
  displayName: "GPW Company Directory",
  sourceType: "company_registry",
  fetchMode: "public_page",
  enabled: true,
  defaultPollIntervalSeconds: 86400,
  sourceUrl: "https://www.gpw.pl/spolki?offset=0&limit=500",
  rateLimitPolicy: "Manual refresh plus daily stale-cache scheduled refresh",
  policyNote: "Fetches the complete public GPW company list and caches ticker and ISIN metadata locally for lookup, autocomplete, and ticker-first matching.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: 400,
  lastItemsCreated: 400,
  lastItemsMatched: null,
  lastItemsUnmatched: null,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "newconnect-company-directory",
  displayName: "NewConnect Company Directory",
  sourceType: "company_registry",
  fetchMode: "public_page",
  enabled: true,
  defaultPollIntervalSeconds: 86400,
  sourceUrl: "https://newconnect.pl/spolki?offset=0&limit=500",
  rateLimitPolicy: "Manual refresh plus daily stale-cache scheduled refresh",
  policyNote:
    "Fetches the complete public NewConnect company list and caches ticker and ISIN metadata for lookup, autocomplete, and ticker-first matching.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: 350,
  lastItemsCreated: 350,
  lastItemsMatched: null,
  lastItemsUnmatched: null,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["NEWCONNECT"],
},
{
  id: "bankier-market-rss",
  displayName: "Bankier Giełda RSS",
  sourceType: "public_media",
  fetchMode: "rss",
  enabled: true,
  defaultPollIntervalSeconds: 900,
  sourceUrl: "https://www.bankier.pl/rss/gielda.xml",
  rateLimitPolicy: "Manual refresh plus normal in-app source scheduler; RSS feed only, no article crawling",
  policyNote: "Fetches Bankier.pl public Giełda RSS headlines as public media items; linked article pages are not crawled in this slice.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: 2,
  lastItemsCreated: 1,
  lastItemsMatched: 1,
  lastItemsUnmatched: 1,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "bankier-company-komunikaty",
  displayName: "Bankier Company Komunikaty",
  sourceType: "official_report",
  fetchMode: "public_json",
  enabled: true,
  defaultPollIntervalSeconds: 900,
  sourceUrl: "https://www.bankier.pl/gielda/notowania/akcje/{TICKER}/komunikaty",
  rateLimitPolicy:
    "Manual refresh plus normal in-app source scheduler; tracked GPW companies only; cached Bankier tag ids; one listing page plus matched article pages per company",
  policyNote:
    "Fetches Bankier.pl per-company public komunikaty JSON and article pages for tracked GPW companies only. Bankier is the active v1 official-report source while GPW ESPI/EBI is disabled.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: 2,
  lastItemsCreated: 1,
  lastItemsMatched: 1,
  lastItemsUnmatched: 0,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "bankier-firma-rss",
  displayName: "Bankier Firma RSS",
  sourceType: "public_media",
  fetchMode: "rss",
  enabled: false,
  defaultPollIntervalSeconds: 0,
  sourceUrl: "https://www.bankier.pl/rss/firma.xml",
  rateLimitPolicy:
    "Reviewed public RSS candidate; disabled until matching quality is proven against tracked GPW companies",
  policyNote:
    "Reviewed M8 follow-up candidate. Public and RSS-native, but broader business coverage needs matching-quality tests before runtime enablement.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: null,
  lastItemsCreated: null,
  lastItemsMatched: null,
  lastItemsUnmatched: null,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "bankier-wiadomosci-rss",
  displayName: "Bankier Wiadomosci RSS",
  sourceType: "public_media",
  fetchMode: "rss",
  enabled: false,
  defaultPollIntervalSeconds: 0,
  sourceUrl: "https://www.bankier.pl/rss/wiadomosci.xml",
  rateLimitPolicy:
    "Reviewed public RSS candidate; disabled because expected listed-company signal is broad and noisy",
  policyNote:
    "Reviewed M8 follow-up candidate. Public and RSS-native, but broad news coverage and stale backfill risk make it unsuitable for default v1 ingestion.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: null,
  lastItemsCreated: null,
  lastItemsMatched: null,
  lastItemsUnmatched: null,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "portal-analiz",
  displayName: "Portal Analiz",
  sourceType: "authenticated_research",
  fetchMode: "authenticated",
  enabled: false,
  defaultPollIntervalSeconds: 0,
  sourceUrl: "https://portalanaliz.pl/",
  rateLimitPolicy:
    "Late-v1 disabled placeholder; no automated access until the authenticated-source implementation is explicitly built",
  policyNote:
    "Late-v1 planned authenticated private research adapter governed by ADR 0014. Credentials must use the OS keychain and no generic login or scraping subsystem is approved.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: null,
  lastItemsCreated: null,
  lastItemsMatched: null,
  lastItemsUnmatched: null,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "gpw-market-events-rss",
  displayName: "GPW Market Events RSS",
  sourceType: "official_calendar",
  fetchMode: "rss",
  enabled: true,
  defaultPollIntervalSeconds: 900,
  sourceUrl: "https://www.gpw.pl/rss-calendar-of-market-events",
  rateLimitPolicy: "Manual refresh plus normal in-app source scheduler",
  policyNote: "Fetches GPW official market-events RSS for tracked companies matched by exact ticker.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: 2,
  lastItemsCreated: 2,
  lastItemsMatched: 2,
  lastItemsUnmatched: 0,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "bankier-kalendarium-html",
  displayName: "Bankier Kalendarium",
  sourceType: "public_calendar",
  fetchMode: "public_page",
  enabled: true,
  defaultPollIntervalSeconds: 900,
  sourceUrl: "https://www.bankier.pl/gielda/kalendarium",
  rateLimitPolicy: "Manual refresh plus normal in-app source scheduler",
  policyNote: "Fetches Bankier public calendar pages for tracked companies matched by exact ticker.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: 2,
  lastItemsCreated: 2,
  lastItemsMatched: 2,
  lastItemsUnmatched: 0,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "strefa-report-calendar",
  displayName: "Strefa Report Calendar",
  sourceType: "public_calendar",
  fetchMode: "public_page",
  enabled: false,
  defaultPollIntervalSeconds: 0,
  sourceUrl: "https://strefainwestorow.pl/",
  rateLimitPolicy: "Developer-only candidate pending source review",
  policyNote: "Fallback candidate for periodic-report publication dates.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: null,
  lastItemsCreated: null,
  lastItemsMatched: null,
  lastItemsUnmatched: null,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
{
  id: "money-calendar",
  displayName: "Money Calendar",
  sourceType: "public_calendar",
  fetchMode: "public_page",
  enabled: false,
  defaultPollIntervalSeconds: 0,
  sourceUrl: "https://www.money.pl/",
  rateLimitPolicy: "Developer-only candidate pending source review",
  policyNote: "Fallback candidate for calendar and report-date coverage.",
  lastAttemptAt: null,
  lastTrigger: null,
  lastSuccessAt: null,
  lastErrorAt: null,
  lastError: null,
  lastItemsFetched: null,
  lastItemsCreated: null,
  lastItemsMatched: null,
  lastItemsUnmatched: null,
  lastDetailItemsAttempted: null,
  lastDetailItemsStored: null,
  lastDetailItemsFailed: null,
  lastDetailWarning: null,
  markets: ["GPW"],
},
];

const requiredSourceIds = new Set(["gpw-company-registry", "newconnect-company-directory"]);
const optionalSourceIds = new Set([
  "bankier-company-komunikaty",
  "bankier-market-rss",
  "gpw-market-events-rss",
  "bankier-kalendarium-html",
]);

const sourceAdapters = rawSourceAdapters.map((adapter) => {
  const visibility = requiredSourceIds.has(adapter.id)
    ? "required"
    : optionalSourceIds.has(adapter.id)
      ? "optional"
      : "developer";

  return {
    ...adapter,
    visibility,
    userConfigurable: visibility === "optional",
    healthStatus: adapter.enabled ? "notRefreshed" : "off",
  };
});

function cloneSourceAdapters() {
  return sourceAdapters.map((adapter) => ({ ...adapter, markets: [...adapter.markets] }));
}

const initialUnmatchedSourceItems = [
{
  id: "feed_gpw_espi_ebi_unmatched_lbw",
  adapterId: "gpw-espi-ebi",
  companyName: "LUBAWA S.A.",
  title: "Unmatched GPW report from sample source",
  sourceUrl: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=999999",
  publishedAt: "2026-05-30T17:13:31+02:00",
  fetchedAt: "2026-05-30T17:30:00Z",
},
];

const initialCompanyRegistryEntries = [
{
  sourceAdapterId: "gpw-company-registry",
  exchange: "GPW",
  ticker: "CDR",
  qualifiedTicker: "GPW:CDR",
  displayName: "CD PROJEKT S.A.",
  isin: "PLOPTTC00011",
  sourceUrl: "https://www.gpw.pl/spolka?isin=PLOPTTC00011",
  fetchedAt: "2026-05-31T12:00:00Z",
  tracked: true,
},
{
  sourceAdapterId: "gpw-company-registry",
  exchange: "GPW",
  ticker: "DNP",
  qualifiedTicker: "GPW:DNP",
  displayName: "DINO POLSKA S.A.",
  isin: "PLDINPL00011",
  sourceUrl: "https://www.gpw.pl/spolka?isin=PLDINPL00011",
  fetchedAt: "2026-05-31T12:00:00Z",
  tracked: false,
},
{
  sourceAdapterId: "newconnect-company-directory",
  exchange: "NC",
  ticker: "4MB",
  qualifiedTicker: "NC:4MB",
  displayName: "4MOBILITY SPÓŁKA AKCYJNA",
  isin: "PLESLTN00010",
  sourceUrl: "https://newconnect.pl/spolka?isin=PLESLTN00010",
  fetchedAt: "2026-05-31T12:00:00Z",
  tracked: false,
},
{
  sourceAdapterId: "future-company-directory",
  exchange: "XETRA",
  ticker: "SAP",
  qualifiedTicker: "XETRA:SAP",
  displayName: "SAP SE",
  isin: "DE0007164600",
  sourceUrl: "https://example.test/xetra/sap",
  fetchedAt: "2026-05-31T12:00:00Z",
  tracked: false,
},
];

type CreateCompanyArgs = {
input: {
  exchange: string;
  ticker: string;
  displayName: string;
  isin: string | null;
};
};

type TestCompany = {
id: string;
exchange: string;
ticker: string;
qualifiedTicker: string;
displayName: string;
isin: string | null;
cik: string | null;
lei: string | null;
};

const initialSettings: UserSettings = {
theme: "dark",
locale: "en",
accentPalette: "night-neon",
developerMode: false,
pollIntervalSeconds: 900,
settingsSource: "sqlite",
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
logs: {
  level: "info",
  maxFiles: 5,
  maxFileBytes: 5_242_880,
},
shortcutBindings: {},
};

const initialGeminiCredentialStatus = {
providerId: "provider_gemini",
purpose: "youtube_transcription",
secretKind: "api_key",
configured: false,
storage: "not_configured",
label: "Gemini YouTube transcription API key",
devFallbackAvailable: false,
error: null as string | null,
};

const initialLocalMetricsSnapshot: LocalMetricsSnapshot = {
  collectedAt: "2026-06-04T10:00:00.000Z",
  samples: [
    {
      name: "brawler_source_refresh_total",
      description: "Process-lifetime source refresh attempts by adapter and status.",
      kind: "counter",
      unit: "count",
      value: 2,
      labels: [
        { key: "adapter_id", value: "bankier-company-komunikaty" },
        { key: "status", value: "succeeded" },
      ],
      collectedAt: "2026-06-04T10:00:00.000Z",
    },
    {
      name: "brawler_sqlite_database_bytes",
      description: "Current SQLite database size.",
      kind: "gauge",
      unit: "bytes",
      value: 524288,
      labels: [{ key: "collector", value: "sqlite" }],
      collectedAt: "2026-06-04T10:00:00.000Z",
    },
  ],
};

const initialLicenseStatus: LicenseStatus = {
  status: "valid",
  canUseApp: true,
  reason: null,
  license: {
    licenseId: "lic_friend_test",
    holder: "Friend Tester",
    channel: "friend_test",
    edition: "friend",
    features: ["core"],
    issuedAt: "2026-06-01T00:00:00Z",
    expiresAt: "2027-01-01T00:00:00Z",
    appVersionRange: "*",
    keyId: "owner_friend_test_2026_06",
  },
  checkedAt: "2026-06-04T10:00:00Z",
};

export const missingLicenseStatus: LicenseStatus = {
  status: "missing",
  canUseApp: false,
  reason: "A valid local license is required.",
  license: null,
  checkedAt: "2026-06-04T10:00:00Z",
};

export const invalidLicenseStatus: LicenseStatus = {
  status: "invalid",
  canUseApp: false,
  reason: "This license key could not be verified.",
  license: null,
  checkedAt: "2026-06-04T10:00:00Z",
};

const initialCompanies: TestCompany[] = [
{
  id: "company_gpw_cdr",
  exchange: "GPW",
  ticker: "CDR",
  qualifiedTicker: "GPW:CDR",
  displayName: "CD PROJEKT S.A.",
  isin: "PLOPTTC00011",
  cik: null,
  lei: null,
},
{
  id: "company_gpw_pkn",
  exchange: "GPW",
  ticker: "PKN",
  qualifiedTicker: "GPW:PKN",
  displayName: "ORLEN S.A.",
  isin: "PLPKN0000018",
  cik: null,
  lei: null,
},
{
  id: "company_gpw_kgh",
  exchange: "GPW",
  ticker: "KGH",
  qualifiedTicker: "GPW:KGH",
  displayName: "KGHM POLSKA MIEDZ S.A.",
  isin: "PLKGHM000017",
  cik: null,
  lei: null,
},
{
  id: "company_gpw_pzu",
  exchange: "GPW",
  ticker: "PZU",
  qualifiedTicker: "GPW:PZU",
  displayName: "PZU S.A.",
  isin: "PLPZU0000011",
  cik: null,
  lei: null,
},
];

type TestCompanyEvent = {
id: string;
companyId: string;
company: string;
companyName: string;
eventType: string;
title: string;
eventDate: string;
eventTime: string | null;
status: string;
sourceType: string;
sourceAdapterId: string | null;
sourceEventKey: string | null;
sourceUrl: string | null;
attribution: string | null;
fetchedAt: string | null;
manual: boolean;
createdAt: string;
updatedAt: string;
};

const initialCompanyEvents: TestCompanyEvent[] = [
{
  id: "event_gpw_market_events_cdr",
  companyId: "company_gpw_cdr",
  company: "GPW:CDR",
  companyName: "CD PROJEKT S.A.",
  eventType: "corporate_action",
  title: "Main Market - Corporate actions - Equity - CDR",
  eventDate: formatTestDate(new Date()),
  eventTime: null,
  status: "scheduled",
  sourceType: "official_calendar",
  sourceAdapterId: "gpw-market-events-rss",
  sourceEventKey: "gpw-market-events-rss:2099-06-01:corporate-actions:equity:cdr",
  sourceUrl: "https://www.gpw.pl/market-events-calendar?date=2099-06-01",
  attribution: "GPW",
  fetchedAt: "2026-06-01T08:00:00Z",
  manual: false,
  createdAt: "2026-06-01T08:00:00Z",
  updatedAt: "2026-06-01T08:00:00Z",
},
{
  id: "event_gpw_market_events_pzu",
  companyId: "company_gpw_pzu",
  company: "GPW:PZU",
  companyName: "PZU S.A.",
  eventType: "market_making",
  title: "Main Market - End of market making activities - Equity - PZU",
  eventDate: currentWeekTestDate(2),
  eventTime: null,
  status: "confirmed",
  sourceType: "official_calendar",
  sourceAdapterId: "gpw-market-events-rss",
  sourceEventKey: "gpw-market-events-rss:2099-06-03:end-of-market-making:equity:pzu",
  sourceUrl: "https://www.gpw.pl/market-events-calendar?date=2099-06-03",
  attribution: "GPW",
  fetchedAt: "2026-06-01T08:00:00Z",
  manual: false,
  createdAt: "2026-06-01T08:00:00Z",
  updatedAt: "2026-06-01T08:00:00Z",
},
];

type TestNotebookEntry = {
id: string;
companyId: string;
title: string;
body: string;
bodyFormat: string;
tags: string[];
kind: string;
claimStatus: string | null;
eventDate: string | null;
followUpAfter: string | null;
followUpDate: string | null;
createdAt: string;
updatedAt: string;
origins: Array<{
  id: string;
  sourceType: string;
  sourceId: string | null;
  sourceUrl: string | null;
  label: string | null;
  createdAt: string;
}>;
};

const initialNotebookEntry: TestNotebookEntry = {
id: "note_company_gpw_cdr_release_schedule",
companyId: "company_gpw_cdr",
title: "Release schedule promise",
body: "Management promised a release milestone in the next two quarters.",
bodyFormat: "markdown",
tags: ["management-guidance", "product"],
kind: "claim",
claimStatus: "open",
eventDate: "2026-05-29",
followUpAfter: "2026-Q4",
followUpDate: "2026-11-30",
createdAt: "2026-05-29T10:00:00Z",
updatedAt: "2026-05-29T10:00:00Z",
origins: [
  {
    id: "note_origin_release_schedule_manual_1",
    sourceType: "manual",
    sourceId: null,
    sourceUrl: null,
    label: "Manual note",
    createdAt: "2026-05-29T10:00:00Z",
  },
],
};

type TestTranscriptJob = {
id: string;
companyId: string | null;
company: string | null;
companyName: string | null;
providerId: string;
sourceType: string;
sourceUrl: string;
sourceLabel: string | null;
companyResolutionStatus: string;
recognizedCompanyCandidates: unknown[];
status: string;
errorCode: string | null;
createdAt: string;
startedAt: string | null;
finishedAt: string | null;
error: string | null;
};

type TestTranscriptSegment = {
id: string;
transcriptJobId: string;
companyId: string | null;
startSeconds: number | null;
endSeconds: number | null;
speaker: string | null;
text: string;
language: string | null;
createdAt: string;
};

const initialTranscriptJobs: TestTranscriptJob[] = [
{
  id: "transcript_job_unresolved_conference",
  companyId: null,
  company: null,
  companyName: null,
  providerId: "provider_gemini",
  sourceType: "youtube_url",
  sourceUrl: "https://www.youtube.com/watch?v=conference",
  sourceLabel: "Q2 conference",
  companyResolutionStatus: "unresolved",
  recognizedCompanyCandidates: [],
  status: "queued",
  errorCode: null,
  createdAt: "2026-06-01T10:00:00Z",
  startedAt: null,
  finishedAt: null,
  error: null,
},
];

const initialTranscriptSegmentsByJobId: Record<string, TestTranscriptSegment[]> = {
transcript_job_unresolved_conference: [
  {
    id: "transcript_segment_opening",
    transcriptJobId: "transcript_job_unresolved_conference",
    companyId: null,
    startSeconds: 0,
    endSeconds: 42,
    speaker: "CEO",
    text: "We expect the second half to be stronger after the release window stabilizes.",
    language: "en",
    createdAt: "2026-06-01T10:07:00Z",
  },
  {
    id: "transcript_segment_margin",
    transcriptJobId: "transcript_job_unresolved_conference",
    companyId: null,
    startSeconds: 43,
    endSeconds: 96,
    speaker: "CFO",
    text: "Gross margin should normalize over the next two quarters.",
    language: "en",
    createdAt: "2026-06-01T10:07:00Z",
  },
],
};

vi.mock("@tauri-apps/api/core", () => ({
invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
downloadDir: vi.fn(() => Promise.resolve("/home/test/Downloads")),
join: vi.fn((...paths: string[]) => Promise.resolve(paths.join("/"))),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
openUrl: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
save: vi.fn(() => Promise.resolve("/tmp/brawler-export.json")),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
writeTextFile: vi.fn(() => Promise.resolve()),
}));

export const appTestState = {
  companiesResponse: initialCompanies,
  feedItemsResponse: initialFeedItems,
  companyEventsResponse: initialCompanyEvents,
  transcriptJobsResponse: initialTranscriptJobs,
  companyRegistryEntriesResponse: initialCompanyRegistryEntries,
  watchlistsResponse: [
    {
      id: "watchlist_main_gpw",
      name: "Main GPW",
      description: null,
      companyCount: 1,
    },
  ] as Watchlist[],
  watchlistMembershipsResponse: [
    {
      watchlistId: "watchlist_main_gpw",
      watchlistName: "Main GPW",
      companyId: "company_gpw_cdr",
    },
  ] as WatchlistMembership[],
  notebookEntriesResponse: [] as TestNotebookEntry[],
  aiAnalysisJobsResponse: {} as Record<string, AiAnalysisJob[]>,
  settingsResponse: initialSettings,
  refreshSourcesError: null as string | null,
  geminiCredentialStatusResponse: initialGeminiCredentialStatus,
  localMetricsSnapshotResponse: initialLocalMetricsSnapshot,
  licenseStatusResponse: initialLicenseStatus,
  sourceAdaptersResponse: cloneSourceAdapters(),
};

beforeEach(() => {
  appTestState.companiesResponse = initialCompanies;
  appTestState.feedItemsResponse = initialFeedItems;
  appTestState.companyEventsResponse = initialCompanyEvents;
  appTestState.transcriptJobsResponse = initialTranscriptJobs;
  appTestState.companyRegistryEntriesResponse = initialCompanyRegistryEntries;
  appTestState.watchlistsResponse = [
    {
      id: "watchlist_main_gpw",
      name: "Main GPW",
      description: null,
      companyCount: 1,
    },
  ];
  appTestState.watchlistMembershipsResponse = [
    {
      watchlistId: "watchlist_main_gpw",
      watchlistName: "Main GPW",
      companyId: "company_gpw_cdr",
    },
  ];
  appTestState.notebookEntriesResponse = [];
  appTestState.aiAnalysisJobsResponse = {};
  appTestState.settingsResponse = initialSettings;
  appTestState.refreshSourcesError = null;
  appTestState.geminiCredentialStatusResponse = initialGeminiCredentialStatus;
  appTestState.localMetricsSnapshotResponse = initialLocalMetricsSnapshot;
  appTestState.licenseStatusResponse = initialLicenseStatus;
  appTestState.sourceAdaptersResponse = cloneSourceAdapters();
  vi.mocked(invoke).mockClear();
  vi.mocked(downloadDir).mockClear();
  vi.mocked(join).mockClear();
  vi.mocked(openUrl).mockClear();
  vi.mocked(save).mockClear();
  vi.mocked(writeTextFile).mockClear();
  vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
    if (command === "health") {
      return Promise.resolve({ status: "ok", version: "0.3.0" });
    }

    if (command === "database_status") {
      return Promise.resolve({
        appliedMigrations: 29,
        companies: 0,
        sourceAdapters: 12,
        settings: 11,
      });
    }

    if (command === "list_companies") {
      return Promise.resolve(appTestState.companiesResponse);
    }

    if (command === "lookup_company") {
      const input = (args as {
        input: {
          exchange: string;
          ticker: string | null;
          displayName: string | null;
          isin: string | null;
        };
      }).input;
      const exchange = input.exchange.trim().toUpperCase();
      const ticker = input.ticker?.trim().toUpperCase();
      const isin = input.isin?.trim().toUpperCase();
      const displayName = input.displayName?.trim().toUpperCase();
      const match = appTestState.companyRegistryEntriesResponse
        .filter((entry) =>
          (ticker && entry.ticker.toUpperCase() === ticker) ||
          (isin && entry.isin?.toUpperCase() === isin) ||
          (displayName && displayName.length >= 3 && entry.displayName.toUpperCase().includes(displayName)),
        )
        .sort((left, right) => {
          const leftPreferred = left.exchange.toUpperCase() === exchange ? 0 : 1;
          const rightPreferred = right.exchange.toUpperCase() === exchange ? 0 : 1;
          return leftPreferred - rightPreferred || left.qualifiedTicker.localeCompare(right.qualifiedTicker);
        })[0];

      return Promise.resolve(
        match
          ? {
              exchange: match.exchange,
              ticker: match.ticker,
              qualifiedTicker: match.qualifiedTicker,
              displayName: match.displayName,
              isin: match.isin ?? "",
              source: "company_directory",
            }
          : null,
      );
    }

    if (command === "create_company") {
      const { input } = args as CreateCompanyArgs;
      const created = {
        id: `company_${input.exchange.toLowerCase()}_${input.ticker.toLowerCase()}`,
        exchange: input.exchange,
        ticker: input.ticker,
        qualifiedTicker: `${input.exchange}:${input.ticker}`,
        displayName: input.displayName,
        isin: input.isin,
        cik: null,
        lei: null,
      };
      appTestState.companiesResponse = [...appTestState.companiesResponse, created];
      appTestState.companyRegistryEntriesResponse = appTestState.companyRegistryEntriesResponse.map((entry) =>
        entry.exchange === input.exchange && entry.ticker === input.ticker
          ? { ...entry, tracked: true }
          : entry,
      );

      return Promise.resolve(created);
    }

    if (command === "delete_company") {
      return Promise.resolve();
    }

    if (command === "export_research_data") {
      return Promise.resolve({
        fileName: "brawler-research-data-2026-06-05.json",
        mediaType: "application/json",
        contents: "{\"schemaVersion\":1}",
        summary: {
          companies: appTestState.companiesResponse.length,
          watchlists: appTestState.watchlistsResponse.length,
          memberships: appTestState.watchlistMembershipsResponse.length,
          notebookEntries: appTestState.notebookEntriesResponse.length,
          settings: 0,
        },
      });
    }

    if (command === "preview_research_import") {
      return Promise.resolve({
        valid: true,
        summary: {
          companiesCreated: 1,
          companiesMerged: 1,
          watchlistsCreated: 1,
          watchlistsMerged: 0,
          membershipsCreated: 1,
          notebookEntriesCreated: 1,
          notebookEntriesSkipped: 0,
          settingsUpdated: 0,
        },
        warnings: [],
        errors: [],
      });
    }

    if (command === "apply_research_import") {
      return Promise.resolve({
        summary: {
          companiesCreated: 1,
          companiesMerged: 1,
          watchlistsCreated: 1,
          watchlistsMerged: 0,
          membershipsCreated: 1,
          notebookEntriesCreated: 1,
          notebookEntriesSkipped: 0,
          settingsUpdated: 0,
        },
        warnings: [],
      });
    }

    if (command === "export_settings_data") {
      return Promise.resolve({
        fileName: "brawler-settings-2026-06-05.yaml",
        mediaType: "application/x-yaml",
        contents: "schemaVersion: 1\nsettings:\n  theme: dark\n",
        summary: {
          companies: 0,
          watchlists: 0,
          memberships: 0,
          notebookEntries: 0,
          settings: 15,
        },
      });
    }

    if (command === "preview_settings_import") {
      return Promise.resolve({
        valid: true,
        summary: {
          companiesCreated: 0,
          companiesMerged: 0,
          watchlistsCreated: 0,
          watchlistsMerged: 0,
          membershipsCreated: 0,
          notebookEntriesCreated: 0,
          notebookEntriesSkipped: 0,
          settingsUpdated: 2,
        },
        warnings: [],
        errors: [],
      });
    }

    if (command === "apply_settings_import") {
      appTestState.settingsResponse = {
        ...appTestState.settingsResponse,
        theme: "light",
      };

      return Promise.resolve({
        summary: {
          companiesCreated: 0,
          companiesMerged: 0,
          watchlistsCreated: 0,
          watchlistsMerged: 0,
          membershipsCreated: 0,
          notebookEntriesCreated: 0,
          notebookEntriesSkipped: 0,
          settingsUpdated: 2,
        },
        warnings: [],
      });
    }

    if (command === "list_watchlists") {
      return Promise.resolve(appTestState.watchlistsResponse);
    }

    if (command === "list_watchlist_memberships") {
      return Promise.resolve(appTestState.watchlistMembershipsResponse);
    }

    if (command === "create_watchlist") {
      const { input } = args as { input: { name: string; description: string | null } };
      const created = {
        id: `watchlist_${input.name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "")}`,
        name: input.name,
        description: input.description,
        companyCount: 0,
      };
      appTestState.watchlistsResponse = [...appTestState.watchlistsResponse, created];

      return Promise.resolve(created);
    }

    if (command === "rename_watchlist") {
      const { input } = args as { input: { id: string; name: string; description: string | null } };
      let renamed: Watchlist | null = null;
      appTestState.watchlistsResponse = appTestState.watchlistsResponse.map((watchlist) => {
        if (watchlist.id !== input.id) {
          return watchlist;
        }

        renamed = {
          ...watchlist,
          name: input.name,
          description: input.description,
        };
        return renamed;
      });
      appTestState.watchlistMembershipsResponse = appTestState.watchlistMembershipsResponse.map((membership) =>
        membership.watchlistId === input.id
          ? { ...membership, watchlistName: input.name }
          : membership,
      );

      return Promise.resolve(renamed);
    }

    if (command === "delete_watchlist") {
      const { watchlistId } = args as { watchlistId: string };
      appTestState.watchlistsResponse = appTestState.watchlistsResponse.filter(
        (watchlist) => watchlist.id !== watchlistId,
      );
      appTestState.watchlistMembershipsResponse = appTestState.watchlistMembershipsResponse.filter(
        (membership) => membership.watchlistId !== watchlistId,
      );
      return Promise.resolve();
    }

    if (command === "add_company_to_watchlist") {
      const { input } = args as { input: { watchlistId: string; companyId: string } };
      const watchlist = appTestState.watchlistsResponse.find((entry) => entry.id === input.watchlistId);
      if (
        watchlist &&
        !appTestState.watchlistMembershipsResponse.some(
          (membership) =>
            membership.watchlistId === input.watchlistId && membership.companyId === input.companyId,
        )
      ) {
        appTestState.watchlistMembershipsResponse = [
          ...appTestState.watchlistMembershipsResponse,
          {
            watchlistId: input.watchlistId,
            watchlistName: watchlist.name,
            companyId: input.companyId,
          },
        ];
        appTestState.watchlistsResponse = appTestState.watchlistsResponse.map((entry) =>
          entry.id === input.watchlistId
            ? { ...entry, companyCount: entry.companyCount + 1 }
            : entry,
        );
      }
      return Promise.resolve();
    }

    if (command === "remove_company_from_watchlist") {
      const { input } = args as { input: { watchlistId: string; companyId: string } };
      const hadMembership = appTestState.watchlistMembershipsResponse.some(
        (membership) =>
          membership.watchlistId === input.watchlistId && membership.companyId === input.companyId,
      );
      appTestState.watchlistMembershipsResponse = appTestState.watchlistMembershipsResponse.filter(
        (membership) =>
          membership.watchlistId !== input.watchlistId || membership.companyId !== input.companyId,
      );
      if (hadMembership) {
        appTestState.watchlistsResponse = appTestState.watchlistsResponse.map((entry) =>
          entry.id === input.watchlistId
            ? { ...entry, companyCount: Math.max(0, entry.companyCount - 1) }
            : entry,
        );
      }
      return Promise.resolve();
    }

    if (command === "list_feed_items") {
      return Promise.resolve(appTestState.feedItemsResponse);
    }

    if (command === "list_company_events") {
      const input = (args as {
        input: {
          mode: string;
          companyId: string | null;
          watchlistId: string | null;
          eventType: string | null;
          status: string | null;
          dateFrom: string | null;
          dateTo: string | null;
        };
      }).input;

      return Promise.resolve(
        appTestState.companyEventsResponse.filter((event) => {
          const companyMatches = !input.companyId || event.companyId === input.companyId;
          const typeMatches = !input.eventType || event.eventType === input.eventType;
          const statusMatches = !input.status || event.status === input.status;
          const dateFromMatches = !input.dateFrom || event.eventDate >= input.dateFrom;
          const dateToMatches = !input.dateTo || event.eventDate <= input.dateTo;
          const watchlistMatches =
            !input.watchlistId ||
            (input.watchlistId === "watchlist_main_gpw" && event.companyId === "company_gpw_cdr");

          return (
            companyMatches &&
            typeMatches &&
            statusMatches &&
            dateFromMatches &&
            dateToMatches &&
            watchlistMatches
          );
        }),
      );
    }

    if (command === "create_company_event") {
      const input = (args as {
        input: {
          companyId: string;
          eventType: string;
          title: string;
          eventDate: string;
          eventTime: string | null;
          status: string;
          sourceType: string;
          sourceAdapterId: string | null;
          sourceEventKey: string | null;
          sourceUrl: string | null;
          attribution: string | null;
          fetchedAt: string | null;
        };
      }).input;
      const company = appTestState.companiesResponse.find((entry) => entry.id === input.companyId);
      const existing = appTestState.transcriptJobsResponse.find(
        (job) => job.sourceUrl === input.sourceUrl,
      );

      if (existing) {
        return Promise.resolve(existing);
      }

      const created = {
        id: "manual_event_created",
        companyId: input.companyId,
        company: company?.qualifiedTicker ?? "GPW:UNK",
        companyName: company?.displayName ?? "Unknown company",
        eventType: input.eventType,
        title: input.title,
        eventDate: input.eventDate,
        eventTime: input.eventTime,
        status: input.status,
        sourceType: input.sourceType,
        sourceAdapterId: input.sourceAdapterId,
        sourceEventKey: input.sourceEventKey,
        sourceUrl: input.sourceUrl,
        attribution: input.attribution,
        fetchedAt: input.fetchedAt,
        manual: true,
        createdAt: "2026-06-01T08:00:00Z",
        updatedAt: "2026-06-01T08:00:00Z",
      };

      appTestState.companyEventsResponse = [...appTestState.companyEventsResponse, created];

      return Promise.resolve(created);
    }

    if (command === "list_video_transcript_jobs") {
      const input = (args as { input: { companyId: string | null } }).input;

      return Promise.resolve(
        appTestState.transcriptJobsResponse.filter((job) => !input.companyId || job.companyId === input.companyId),
      );
    }

    if (command === "create_video_transcript_job") {
      const input = (args as {
        input: {
          sourceUrl: string;
          companyId: string | null;
          providerId: string | null;
          sourceLabel: string | null;
          recognizedCompanyCandidates: unknown[] | null;
        };
      }).input;
      const existing = appTestState.transcriptJobsResponse.find(
        (job) =>
          job.sourceUrl === input.sourceUrl &&
          (job.companyId ?? null) === (input.companyId ?? null),
      );

      if (existing) {
        return Promise.resolve(existing);
      }

      const company = appTestState.companiesResponse.find((entry) => entry.id === input.companyId);
      const created = {
        id: "transcript_job_created",
        companyId: input.companyId,
        company: company?.qualifiedTicker ?? null,
        companyName: company?.displayName ?? null,
        providerId: input.providerId ?? "provider_gemini",
        sourceType: "youtube_url",
        sourceUrl: input.sourceUrl,
        sourceLabel: input.sourceLabel,
        companyResolutionStatus: input.companyId ? "provided" : "unresolved",
        recognizedCompanyCandidates: input.recognizedCompanyCandidates ?? [],
        status: "queued",
        errorCode: null,
        createdAt: "2026-06-01T10:05:00Z",
        startedAt: null,
        finishedAt: null,
        error: null,
      };

      appTestState.transcriptJobsResponse = [created, ...appTestState.transcriptJobsResponse];

      return Promise.resolve(created);
    }

    if (command === "list_transcript_segments") {
      const { transcriptJobId } = args as { transcriptJobId: string };

      return Promise.resolve(initialTranscriptSegmentsByJobId[transcriptJobId] ?? []);
    }

    if (command === "delete_video_transcript_job") {
      const { jobId } = args as { jobId: string };
      appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.filter((job) => job.id !== jobId);

      return Promise.resolve();
    }

    if (command === "update_video_transcript_job") {
      const input = (args as {
        input: {
          jobId: string;
          sourceLabel: string | null;
        };
      }).input;
      const existing = appTestState.transcriptJobsResponse.find((job) => job.id === input.jobId);

      if (!existing) {
        return Promise.reject(new Error("job not found"));
      }

      const updated = {
        ...existing,
        sourceLabel: input.sourceLabel,
      };
      appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.map((job) =>
        job.id === input.jobId ? updated : job,
      );

      return Promise.resolve(updated);
    }

    if (command === "run_video_transcript_job") {
      const input = (args as { input: { jobId: string; providerMode: string } }).input;
      const existing = appTestState.transcriptJobsResponse.find((job) => job.id === input.jobId);

      if (!existing) {
        return Promise.reject(new Error("job not found"));
      }

      if (input.providerMode === "provider_gemini" && !appTestState.geminiCredentialStatusResponse.configured) {
        const failed = {
          ...existing,
          status: "failed",
          startedAt: "2026-06-01T10:06:00Z",
          finishedAt: "2026-06-01T10:07:00Z",
          errorCode: "provider_not_configured",
          error: "Gemini transcription provider is not configured.",
        };
        appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.map((job) =>
          job.id === input.jobId ? failed : job,
        );

        return Promise.resolve(failed);
      }

      const updated = {
        ...existing,
        status: "completed",
        startedAt: "2026-06-01T10:06:00Z",
        finishedAt: "2026-06-01T10:07:00Z",
        errorCode: null,
        error: null,
      };
      appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.map((job) =>
        job.id === input.jobId ? updated : job,
      );

      return Promise.resolve(updated);
    }

    if (command === "resolve_transcript_job_company") {
      const input = (args as { input: { jobId: string; companyId: string } }).input;
      const company = appTestState.companiesResponse.find((entry) => entry.id === input.companyId);
      const resolved = appTestState.transcriptJobsResponse.find((job) => job.id === input.jobId);

      if (!resolved) {
        return Promise.reject(new Error("job not found"));
      }

      const updated = {
        ...resolved,
        companyId: input.companyId,
        company: company?.qualifiedTicker ?? null,
        companyName: company?.displayName ?? null,
        companyResolutionStatus: "provided",
      };
      appTestState.transcriptJobsResponse = appTestState.transcriptJobsResponse.map((job) =>
        job.id === input.jobId ? updated : job,
      );

      return Promise.resolve(updated);
    }

    if (command === "delete_unsaved_feed_items") {
      const deletedCount = appTestState.feedItemsResponse.filter((feedItem) => !feedItem.saved).length;
      appTestState.feedItemsResponse = appTestState.feedItemsResponse.filter((feedItem) => feedItem.saved);

      return Promise.resolve({
        itemsDeleted: deletedCount,
        deletedAt: "2026-05-31T12:00:00Z",
      });
    }

    if (command === "prune_old_feed_items") {
      return Promise.resolve({
        retentionDays: 30,
        itemsDeleted: 0,
        prunedAt: "2026-05-31T12:00:00Z",
      });
    }

    if (command === "list_notebook_entries") {
      const companyId = (args as { companyId: string }).companyId;

      return Promise.resolve(
        appTestState.notebookEntriesResponse.filter((entry) => entry.companyId === companyId),
      );
    }

    if (command === "create_notebook_entry") {
      const input = (args as {
        input: {
          companyId: string;
          title: string;
          body: string;
          bodyFormat: string;
          tags: string[];
          kind: string;
          claimStatus: string | null;
          eventDate: string | null;
          followUpAfter: string | null;
          followUpDate: string | null;
          origins: Array<{
            sourceType: string;
            sourceId: string | null;
            sourceUrl: string | null;
            label: string | null;
          }>;
        };
      }).input;
      const created = {
        id: `note_${input.companyId}_${input.title.toLowerCase().replace(/\s+/g, "_")}`,
        companyId: input.companyId,
        title: input.title,
        body: input.body,
        bodyFormat: input.bodyFormat,
        tags: input.tags.map((tag) => tag.toLowerCase()).sort(),
        kind: input.kind,
        claimStatus: input.claimStatus,
        eventDate: input.eventDate,
        followUpAfter: input.followUpAfter,
        followUpDate: input.followUpDate,
        createdAt: "2026-05-29T10:00:00Z",
        updatedAt: "2026-05-29T10:00:00Z",
        origins: input.origins.map((item, index) => ({
          id: `note_origin_${index}`,
          sourceType: item.sourceType,
          sourceId: item.sourceId,
          sourceUrl: item.sourceUrl,
          label: item.label,
          createdAt: "2026-05-29T10:00:00Z",
        })),
      };

      appTestState.notebookEntriesResponse = [created, ...appTestState.notebookEntriesResponse];

      return Promise.resolve(created);
    }

    if (command === "create_note_from_transcript_selection") {
      const input = (args as {
        input: {
          transcriptJobId: string;
          transcriptSegmentIds: string[];
          noteDraft: {
            title: string;
            body: string;
            tags: string[];
            kind: string;
            claimStatus: string | null;
            eventDate: string | null;
            followUpAfter: string | null;
            followUpDate: string | null;
          };
        };
      }).input;
      const job = appTestState.transcriptJobsResponse.find((entry) => entry.id === input.transcriptJobId);
      const created = {
        id: "note_from_transcript_selection",
        companyId: job?.companyId ?? "company_gpw_cdr",
        title: input.noteDraft.title,
        body: input.noteDraft.body,
        bodyFormat: "markdown",
        tags: input.noteDraft.tags.map((tag) => tag.toLowerCase()).sort(),
        kind: input.noteDraft.kind,
        claimStatus: input.noteDraft.claimStatus,
        eventDate: input.noteDraft.eventDate,
        followUpAfter: input.noteDraft.followUpAfter,
        followUpDate: input.noteDraft.followUpDate,
        createdAt: "2026-06-01T10:08:00Z",
        updatedAt: "2026-06-01T10:08:00Z",
        origins: input.transcriptSegmentIds.map((segmentId, index) => ({
          id: `note_origin_transcript_${index}`,
          sourceType: "transcript_segment",
          sourceId: segmentId,
          sourceUrl: job?.sourceUrl ?? null,
          label: `Transcript ${input.transcriptJobId} ${segmentId}`,
          createdAt: "2026-06-01T10:08:00Z",
        })),
      };

      appTestState.notebookEntriesResponse = [created, ...appTestState.notebookEntriesResponse];

      return Promise.resolve(created);
    }

    if (command === "update_notebook_entry") {
      const input = (args as {
        input: {
          id: string;
          title: string;
          body: string;
          tags: string[];
          kind: string;
          claimStatus: string | null;
          eventDate: string | null;
          followUpAfter: string | null;
          followUpDate: string | null;
        };
      }).input;
      const existing = appTestState.notebookEntriesResponse.find((entry) => entry.id === input.id);
      const updated = {
        ...(existing ?? initialNotebookEntry),
        id: input.id,
        title: input.title,
        body: input.body,
        tags: input.tags.map((tag) => tag.toLowerCase()).sort(),
        kind: input.kind,
        claimStatus: input.claimStatus,
        eventDate: input.eventDate,
        followUpAfter: input.followUpAfter,
        followUpDate: input.followUpDate,
        updatedAt: "2026-05-29T10:05:00Z",
      };

      appTestState.notebookEntriesResponse = appTestState.notebookEntriesResponse.map((entry) =>
        entry.id === updated.id ? updated : entry,
      );

      return Promise.resolve(updated);
    }

    if (command === "list_source_adapters") {
      const input = (args as { input?: { includeDeveloperOnly?: boolean } } | undefined)?.input;
      const includeDeveloperOnly = Boolean(input?.includeDeveloperOnly);
      return Promise.resolve(
        includeDeveloperOnly
          ? appTestState.sourceAdaptersResponse
          : appTestState.sourceAdaptersResponse.filter((adapter) => adapter.visibility !== "developer"),
      );
    }

    if (command === "set_source_adapter_enabled") {
      const input = (args as { input: { adapterId: string; enabled: boolean } }).input;
      const adapter = appTestState.sourceAdaptersResponse.find((sourceAdapter) => sourceAdapter.id === input.adapterId);
      if (!adapter || !adapter.userConfigurable) {
        return Promise.reject(new Error("source is not user configurable"));
      }

      adapter.enabled = input.enabled;
      adapter.healthStatus = input.enabled ? "notRefreshed" : "off";
      return Promise.resolve(adapter);
    }

    if (command === "list_unmatched_source_items") {
      return Promise.resolve(initialUnmatchedSourceItems);
    }

    if (command === "list_company_registry_entries") {
      return Promise.resolve(appTestState.companyRegistryEntriesResponse);
    }

    if (command === "refresh_sources") {
      if (appTestState.refreshSourcesError) {
        return Promise.reject(new Error(appTestState.refreshSourcesError));
      }

      appTestState.feedItemsResponse = [
        {
          ...initialFeedItems[0],
          id: "feed_gpw_espi_ebi_refreshed_ntc",
          company: "GPW:CDR",
          title: "Refreshed GPW report from sample source",
          summary: "",
          time: "2026-05-30T17:13:31+02:00",
          publishedAt: "2026-05-30T17:13:31+02:00",
          fetchedAt: "2026-05-30T17:30:00Z",
          unread: true,
          saved: false,
          bodyText: "Official GPW body text fetched from the detail page.",
          attachments: [
            {
              id: "feed_attachment_sample_report_pdf",
              label: "report.pdf",
              url: "https://www.gpw.pl/pub/GPW/ESPI/2026/report.pdf",
            },
          ],
        },
        ...appTestState.feedItemsResponse,
      ];

      return Promise.resolve({
        adapterId: "gpw-espi-ebi",
        itemsFetched: 2,
        itemsCreated: 2,
        itemsMatched: 1,
        itemsUnmatched: 1,
        detailItemsAttempted: 1,
        detailItemsStored: 1,
        detailItemsFailed: 0,
        fetchedAt: "2026-05-30T17:30:00Z",
      });
    }

    if (command === "refresh_source") {
      const input = (args as { input: { adapterId: string } }).input;

      return Promise.resolve({
        adapterId: input.adapterId,
        itemsFetched: 2,
        itemsCreated: 1,
        itemsMatched: 1,
        itemsUnmatched: 0,
        detailItemsAttempted: 0,
        detailItemsStored: 0,
        detailItemsFailed: 0,
        fetchedAt: "2026-05-30T17:30:00Z",
      });
    }

    if (command === "refresh_gpw_company_registry") {
      return Promise.resolve({
        adapterId: "company-directories",
        entriesFetched: 750,
        entriesUpserted: 750,
        entriesDeactivated: 0,
        fetchedAt: "2026-05-31T12:00:00Z",
      });
    }

    if (command === "get_settings") {
      return Promise.resolve(appTestState.settingsResponse);
    }

    if (command === "get_license_status") {
      return Promise.resolve(appTestState.licenseStatusResponse);
    }

    if (command === "submit_license_key") {
      const input = (args as { input: { licenseKey: string } }).input;

      if (input.licenseKey.includes("valid-friend-license")) {
        appTestState.licenseStatusResponse = initialLicenseStatus;
      } else {
        appTestState.licenseStatusResponse = invalidLicenseStatus;
      }

      return Promise.resolve(appTestState.licenseStatusResponse);
    }

    if (command === "clear_license_key") {
      appTestState.licenseStatusResponse = missingLicenseStatus;

      return Promise.resolve(appTestState.licenseStatusResponse);
    }

    if (command === "get_local_metrics_snapshot") {
      return Promise.resolve(appTestState.localMetricsSnapshotResponse);
    }

    if (command === "get_gemini_transcription_credential_status") {
      return Promise.resolve(appTestState.geminiCredentialStatusResponse);
    }

    if (command === "set_gemini_transcription_api_key") {
      const input = (args as { input: { apiKey: string } }).input;

      if (!input.apiKey.trim()) {
        return Promise.reject(new Error("credential value is required"));
      }

      appTestState.geminiCredentialStatusResponse = {
        ...initialGeminiCredentialStatus,
        configured: true,
        storage: "os_keychain",
      };

      return Promise.resolve(appTestState.geminiCredentialStatusResponse);
    }

    if (command === "clear_gemini_transcription_api_key") {
      appTestState.geminiCredentialStatusResponse = initialGeminiCredentialStatus;

      return Promise.resolve(appTestState.geminiCredentialStatusResponse);
    }

    if (command === "update_settings") {
      const input = (args as {
        input: {
          theme?: Theme;
          accentPalette?: AccentPalette;
          locale?: AppLocale;
          pollIntervalSeconds?: number;
          youtubeTranscriptionModel?: string;
          youtubeTranscriptionTimeoutSeconds?: number;
          generalAnalysisProvider?: string;
          generalAnalysisModel?: string;
          generalAnalysisTimeoutSeconds?: number;
          shortcutBindings?: Record<string, ShortcutBindingSetting>;
        };
      }).input;

    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      theme: input.theme ?? appTestState.settingsResponse.theme,
      accentPalette: input.accentPalette ?? appTestState.settingsResponse.accentPalette,
      locale: input.locale ?? appTestState.settingsResponse.locale,
        pollIntervalSeconds: input.pollIntervalSeconds ?? appTestState.settingsResponse.pollIntervalSeconds,
        shortcutBindings: input.shortcutBindings ?? appTestState.settingsResponse.shortcutBindings,
        aiProviders: {
          ...appTestState.settingsResponse.aiProviders,
          youtubeTranscriptionModel:
            input.youtubeTranscriptionModel ??
            appTestState.settingsResponse.aiProviders.youtubeTranscriptionModel,
          youtubeTranscriptionTimeoutSeconds:
            input.youtubeTranscriptionTimeoutSeconds ??
            appTestState.settingsResponse.aiProviders.youtubeTranscriptionTimeoutSeconds,
          generalAnalysisProvider:
            input.generalAnalysisProvider ??
            appTestState.settingsResponse.aiProviders.generalAnalysisProvider,
          generalAnalysisModel:
            input.generalAnalysisModel ??
            appTestState.settingsResponse.aiProviders.generalAnalysisModel,
          generalAnalysisTimeoutSeconds:
            input.generalAnalysisTimeoutSeconds ??
            appTestState.settingsResponse.aiProviders.generalAnalysisTimeoutSeconds,
        },
      };

      return Promise.resolve(appTestState.settingsResponse);
    }

    if (command === "list_ai_analysis") {
      const input = (args as { input: { feedItemId: string } }).input;

      return Promise.resolve(appTestState.aiAnalysisJobsResponse[input.feedItemId] ?? []);
    }

    if (command === "start_ai_analysis") {
      const input = (args as {
        input: { feedItemId: string; promptPresetId?: string; customQuestion?: string };
      }).input;
      const item =
        appTestState.feedItemsResponse.find((feedItem) => feedItem.id === input.feedItemId) ??
        initialFeedItems.find((feedItem) => feedItem.id === input.feedItemId) ??
        initialFeedItems[0];
      const createdAt = new Date("2026-01-01T10:00:00.000Z").toISOString();
      const job: AiAnalysisJob = {
        id: `ai_job_${input.feedItemId}_${appTestState.aiAnalysisJobsResponse[input.feedItemId]?.length ?? 0}`,
        feedItemId: input.feedItemId,
        promptPresetId: input.promptPresetId ?? "default_summary",
        customQuestion: input.customQuestion ?? null,
        providerId: appTestState.settingsResponse.aiProviders.generalAnalysisProvider ?? "provider_gemini",
        model: appTestState.settingsResponse.aiProviders.generalAnalysisModel,
        promptVersion: "analysis_v1",
        status: "succeeded",
        errorCode: null,
        error: null,
        createdAt,
        startedAt: createdAt,
        finishedAt: createdAt,
        result: {
          id: `ai_result_${input.feedItemId}`,
          aiAnalysisJobId: null,
          feedItemId: input.feedItemId,
          providerId: appTestState.settingsResponse.aiProviders.generalAnalysisProvider ?? "provider_gemini",
          model: appTestState.settingsResponse.aiProviders.generalAnalysisModel,
          promptVersion: "analysis_v1",
          summary: `AI summary for ${item.title}`,
          significance: "medium",
          reasoning: "Grounded in the selected feed item summary and source metadata.",
          language: item.language,
          tags: ["analysis", "feed"],
          sourceReferences: [
            {
              id: `ai_source_${input.feedItemId}`,
              sourceUrl: item.sourceUrl,
              label: item.source,
              createdAt,
            },
          ],
          createdAt,
        },
      };

      appTestState.aiAnalysisJobsResponse[input.feedItemId] = [
        job,
        ...(appTestState.aiAnalysisJobsResponse[input.feedItemId] ?? []),
      ];

      return Promise.resolve(job);
    }

    if (command === "retry_ai_analysis") {
      const { jobId } = args as { jobId: string };
      const feedItemId =
        Object.entries(appTestState.aiAnalysisJobsResponse).find(([, jobs]) =>
          jobs.some((job) => job.id === jobId),
        )?.[0] ?? initialFeedItems[0].id;
      const job = appTestState.aiAnalysisJobsResponse[feedItemId]?.find((candidate) => candidate.id === jobId);

      if (!job) {
        return Promise.reject(new Error("AI analysis job not found"));
      }

      const retriedJob: AiAnalysisJob = {
        ...job,
        status: "succeeded",
        errorCode: null,
        error: null,
      };
      appTestState.aiAnalysisJobsResponse[feedItemId] = [retriedJob];

      return Promise.resolve(retriedJob);
    }

    if (command === "update_feed_item_state") {
      const input = (args as { input: { id: string; read?: boolean; saved?: boolean } }).input;
      const item =
        appTestState.feedItemsResponse.find((feedItem) => feedItem.id === input.id) ??
        initialFeedItems.find((feedItem) => feedItem.id === input.id) ??
        initialFeedItems[0];

      return Promise.resolve({
        ...item,
        unread: input.read === undefined ? item.unread : !input.read,
        saved: input.saved ?? item.saved,
      });
    }

    return Promise.reject(new Error(`Unexpected command: ${command}`));
  });
});
export {
  currentWeekTestDate,
  initialCompanies,
  initialFeedItems,
  initialGeminiCredentialStatus,
  initialNotebookEntry,
  initialTranscriptJobs,
};
