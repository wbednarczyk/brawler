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
} from "../../api/types";
import type {
  EvidenceLink,
  ResearchBriefJob,
  ResearchDigestJob,
  ResearchEvidenceInput,
  ResearchEvidenceItem,
  ResearchQuestion,
  ResearchReminder,
  ResearchReviewCheckpoint,
} from "../../api/researchTypes";

export const noFeedAttachments: Array<{ id: string; label: string; url: string }> = [];

function formatTestDate(date: Date) {
const year = date.getFullYear();
const month = String(date.getMonth() + 1).padStart(2, "0");
const day = String(date.getDate()).padStart(2, "0");

return `${year}-${month}-${day}`;
}

export function currentWeekTestDate(dayOffset: number) {
const today = new Date();
const weekday = today.getDay();
const mondayOffset = weekday === 0 ? -6 : 1 - weekday;
const date = new Date(today);
date.setDate(today.getDate() + mondayOffset + dayOffset);

return formatTestDate(date);
}

export const initialFeedItems = [
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

export const initialResearchEvidenceItems: ResearchEvidenceItem[] = [
  {
    id: "research_feed_cdr_report",
    evidenceType: "feed_item",
    sourceDomain: "feed",
    sourceId: "feed_sample_cdr_report",
    companyId: "company_gpw_cdr",
    occurredAt: "2026-06-05T09:12:00Z",
    title: "Current report placeholder for watchlist company",
    summary: "Sample official report used to validate research timeline rendering.",
    sourceUrl: "https://www.gpw.pl/komunikaty",
    attribution: "GPW",
    trustCategory: "official_report",
    reviewState: {
      changedSinceCompanyReview: true,
      changedSinceWatchlistReview: true,
    },
  },
  {
    id: "research_note_cdr_claim",
    evidenceType: "notebook_entry",
    sourceDomain: "notebooks",
    sourceId: "note_company_gpw_cdr_test",
    companyId: "company_gpw_cdr",
    occurredAt: "2026-06-03T00:00:00Z",
    title: "CDR follow-up note",
    summary: "Notebook entry linked into the company research timeline.",
    sourceUrl: null,
    attribution: "Manual note",
    trustCategory: "user_note",
    reviewState: {
      changedSinceCompanyReview: false,
      changedSinceWatchlistReview: false,
    },
  },
  {
    id: "research_event_cdr_meeting",
    evidenceType: "company_event",
    sourceDomain: "events",
    sourceId: "event_cdr_shareholder_meeting",
    companyId: "company_gpw_cdr",
    occurredAt: "2026-06-02T00:00:00Z",
    title: "CDR annual shareholder meeting",
    summary: "shareholder_meeting",
    sourceUrl: "https://example.test/events/cdr-meeting",
    attribution: "GPW calendar",
    trustCategory: "market_calendar",
    reviewState: {
      changedSinceCompanyReview: false,
      changedSinceWatchlistReview: false,
    },
  },
  {
    id: "research_ai_cdr_summary",
    evidenceType: "ai_analysis",
    sourceDomain: "ai_analysis",
    sourceId: "ai_result_cdr_summary",
    companyId: "company_gpw_cdr",
    occurredAt: "2026-06-01T12:00:00Z",
    title: "AI analysis",
    summary: "AI-generated source-grounded summary.",
    sourceUrl: "https://example.test/sample/cdr-report",
    attribution: "provider_gemini",
    trustCategory: "ai_generated",
    reviewState: {
      changedSinceCompanyReview: false,
      changedSinceWatchlistReview: false,
    },
  },
];

export const initialResearchQuestions: ResearchQuestion[] = [
  {
    id: "research_question_company_gpw_cdr_margin",
    scopeType: "company",
    scopeId: "company_gpw_cdr",
    title: "Will margins recover?",
    body: "Track management comments and follow-up reports.",
    status: "open",
    closedAt: null,
    createdAt: "2026-06-01T10:00:00Z",
    updatedAt: "2026-06-01T10:00:00Z",
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

export function cloneSourceAdapters() {
  return sourceAdapters.map((adapter) => ({ ...adapter, markets: [...adapter.markets] }));
}

export const initialUnmatchedSourceItems = [
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

export const initialCompanyRegistryEntries = [
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

export type CreateCompanyArgs = {
input: {
  exchange: string;
  ticker: string;
  displayName: string;
  isin: string | null;
};
};

export type TestCompany = {
id: string;
exchange: string;
ticker: string;
qualifiedTicker: string;
displayName: string;
isin: string | null;
cik: string | null;
lei: string | null;
};

export const initialSettings: UserSettings = {
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

export const initialGeminiCredentialStatus = {
providerId: "provider_gemini",
secretKind: "api_key",
configured: false,
storage: "not_configured",
label: "Gemini API key",
devFallbackAvailable: false,
error: null as string | null,
};

export const initialLocalMetricsSnapshot: LocalMetricsSnapshot = {
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

export const initialLicenseStatus: LicenseStatus = {
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
  canUseApp: true,
  reason: "Core features are available without a license. Add a license only for gated entitlements.",
  license: null,
  checkedAt: "2026-06-04T10:00:00Z",
};

export const invalidLicenseStatus: LicenseStatus = {
  status: "invalid",
  canUseApp: true,
  reason: "This license key could not be verified.",
  license: null,
  checkedAt: "2026-06-04T10:00:00Z",
};

export const initialCompanies: TestCompany[] = [
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

export type TestCompanyEvent = {
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

export const initialCompanyEvents: TestCompanyEvent[] = [
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

export type TestNotebookEntry = {
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

export const initialNotebookEntry: TestNotebookEntry = {
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

export type TestTranscriptJob = {
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

export type TestTranscriptSegment = {
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

export const initialTranscriptJobs: TestTranscriptJob[] = [
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

export const initialTranscriptSegmentsByJobId: Record<string, TestTranscriptSegment[]> = {
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
