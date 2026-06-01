import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

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
    sourceUrl: "https://example.local/sample/pkn",
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
    source: "Local sample",
    time: "Mon",
    title: "Transcript-derived note candidate waits for future provider work",
    unread: false,
    saved: false,
    sourceUrl: "https://example.local/sample/kgh-transcript",
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
    summary: "Fourth sample item keeps the sample feed aligned with local GPW lookup companies.",
    bodyText: "",
    attachments: noFeedAttachments,
  },
];

const sourceAdapters = [
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
    displayName: "GPW Company Registry",
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
];

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
    exchange: "GPW",
    ticker: "DNP",
    qualifiedTicker: "GPW:DNP",
    displayName: "DINO POLSKA S.A.",
    isin: "PLDINPL00011",
    sourceUrl: "https://www.gpw.pl/spolka?isin=PLDINPL00011",
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

const initialSettings = {
  theme: "dark",
  accentPalette: "night-neon",
  pollIntervalSeconds: 900,
  settingsSource: "sqlite",
  settingsImportExportFormat: "yaml",
  yamlImportExportStatus: "accepted_deferred",
  aiProviders: {
    youtubeTranscriptionProvider: "gemini",
    generalAnalysisProvider: null,
  },
  aiAnalysisMode: "source_grounded",
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
    eventDate: currentWeekTestDate(0),
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

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(() => Promise.resolve()),
}));

describe("App", () => {
  let companiesResponse = initialCompanies;
  let feedItemsResponse = initialFeedItems;
  let companyEventsResponse = initialCompanyEvents;
  let companyRegistryEntriesResponse = initialCompanyRegistryEntries;
  let notebookEntriesResponse: TestNotebookEntry[] = [];
  let refreshSourcesError: string | null = null;

  beforeEach(() => {
    companiesResponse = initialCompanies;
    feedItemsResponse = initialFeedItems;
    companyEventsResponse = initialCompanyEvents;
    companyRegistryEntriesResponse = initialCompanyRegistryEntries;
    notebookEntriesResponse = [];
    refreshSourcesError = null;
    vi.mocked(invoke).mockClear();
    vi.mocked(openUrl).mockClear();
    vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
      if (command === "health") {
        return Promise.resolve({ status: "ok", version: "0.3.0" });
      }

      if (command === "database_status") {
        return Promise.resolve({
          appliedMigrations: 3,
          companies: 0,
          sourceAdapters: 7,
          settings: 7,
        });
      }

      if (command === "list_companies") {
        return Promise.resolve(companiesResponse);
      }

      if (command === "lookup_company") {
        return Promise.resolve({
          exchange: "GPW",
          ticker: "CDR",
          qualifiedTicker: "GPW:CDR",
          displayName: "CD PROJEKT S.A.",
          isin: "PLOPTTC00011",
          source: "gpw_registry",
        });
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
        companiesResponse = [...companiesResponse, created];
        companyRegistryEntriesResponse = companyRegistryEntriesResponse.map((entry) =>
          entry.exchange === input.exchange && entry.ticker === input.ticker
            ? { ...entry, tracked: true }
            : entry,
        );

        return Promise.resolve(created);
      }

      if (command === "delete_company") {
        return Promise.resolve();
      }

      if (command === "list_watchlists") {
        return Promise.resolve([
          {
            id: "watchlist_main_gpw",
            name: "Main GPW",
            description: null,
            companyCount: 1,
          },
        ]);
      }

      if (command === "list_watchlist_memberships") {
        return Promise.resolve([
          {
            watchlistId: "watchlist_main_gpw",
            watchlistName: "Main GPW",
            companyId: "company_gpw_cdr",
          },
        ]);
      }

      if (command === "create_watchlist") {
        return Promise.resolve({
          id: "watchlist_main_gpw",
          name: "Main GPW",
          description: null,
          companyCount: 0,
        });
      }

      if (command === "add_company_to_watchlist") {
        return Promise.resolve();
      }

      if (command === "remove_company_from_watchlist") {
        return Promise.resolve();
      }

      if (command === "list_feed_items") {
        return Promise.resolve(feedItemsResponse);
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
          companyEventsResponse.filter((event) => {
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
        const company = companiesResponse.find((entry) => entry.id === input.companyId);
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

        companyEventsResponse = [...companyEventsResponse, created];

        return Promise.resolve(created);
      }

      if (command === "delete_unsaved_feed_items") {
        const deletedCount = feedItemsResponse.filter((feedItem) => !feedItem.saved).length;
        feedItemsResponse = feedItemsResponse.filter((feedItem) => feedItem.saved);

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
          notebookEntriesResponse.filter((entry) => entry.companyId === companyId),
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

        notebookEntriesResponse = [created, ...notebookEntriesResponse];

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
        const existing = notebookEntriesResponse.find((entry) => entry.id === input.id);
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

        notebookEntriesResponse = notebookEntriesResponse.map((entry) =>
          entry.id === updated.id ? updated : entry,
        );

        return Promise.resolve(updated);
      }

      if (command === "list_source_adapters") {
        return Promise.resolve(sourceAdapters);
      }

      if (command === "list_unmatched_source_items") {
        return Promise.resolve(initialUnmatchedSourceItems);
      }

      if (command === "list_company_registry_entries") {
        return Promise.resolve(companyRegistryEntriesResponse);
      }

      if (command === "refresh_sources") {
        if (refreshSourcesError) {
          return Promise.reject(new Error(refreshSourcesError));
        }

        feedItemsResponse = [
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
          ...feedItemsResponse,
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
          adapterId: "gpw-company-registry",
          entriesFetched: 400,
          entriesUpserted: 400,
          entriesDeactivated: 0,
          fetchedAt: "2026-05-31T12:00:00Z",
        });
      }

      if (command === "get_settings") {
        return Promise.resolve(initialSettings);
      }

      if (command === "update_settings") {
        const input = (args as { input: { theme?: string; pollIntervalSeconds?: number } }).input;

        return Promise.resolve({
          ...initialSettings,
          theme: input.theme ?? initialSettings.theme,
          pollIntervalSeconds: input.pollIntervalSeconds ?? initialSettings.pollIntervalSeconds,
        });
      }

      if (command === "update_feed_item_state") {
        const input = (args as { input: { id: string; read?: boolean; saved?: boolean } }).input;
        const item =
          feedItemsResponse.find((feedItem) => feedItem.id === input.id) ??
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

  it("renders the investor inbox shell", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(
      (await screen.findAllByText("Current report placeholder for watchlist company")).length,
    ).toBeGreaterThan(0);
    expect(within(screen.getByLabelText("Feed items")).getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getAllByText("Current report placeholder for watchlist company").length).toBeGreaterThan(0);
    expect(
      screen.getAllByText("Sample official report used to validate feed filtering and detail rendering.").length,
    ).toBeGreaterThan(0);
    expect(await screen.findByText("ok 0.3.0")).toBeInTheDocument();
    expect(screen.getByText("DB")).toBeInTheDocument();
    expect(screen.getByLabelText("Database connection active")).toBeInTheDocument();
  });

  it("shows unread feed count in the Inbox navigation item", async () => {
    render(<App />);

    const inboxNav = await screen.findByRole("button", { name: /Inbox/ });

    expect(within(inboxNav).getByText("1")).toHaveClass("nav-badge");
    expect(within(inboxNav).getByLabelText("1 unread feed item")).toBeInTheDocument();
  });

  it("shows upcoming company events from real source-backed event data", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Events" }));

    expect(screen.getByRole("heading", { name: "Events" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Week" })).toHaveClass("segment-active");
    expect(screen.getByText("Main Market - Corporate actions - Equity - CDR")).toBeInTheDocument();
    const cdrEventRow = screen.getByRole("button", {
      name: "Open event: Main Market - Corporate actions - Equity - CDR",
    });
    expect(within(cdrEventRow).getByText("Corporate Action")).toBeInTheDocument();
    expect(within(cdrEventRow).getByText("GPW:CDR")).toBeInTheDocument();
    expect(within(cdrEventRow).getByText("Today")).toBeInTheDocument();

    await user.click(cdrEventRow);

    const eventDetails = screen.getByLabelText("Event details");
    expect(eventDetails).toBeInTheDocument();
    expect(within(eventDetails).getByText("Official Calendar")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open source" })).toBeInTheDocument();
  });

  it("filters company events by company and type", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Events" }));
    await user.click(screen.getByRole("button", { name: "List" }));
    expect(screen.getByText("Main Market - Corporate actions - Equity - CDR")).toBeInTheDocument();
    expect(
      screen.getByText("Main Market - End of market making activities - Equity - PZU"),
    ).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Event company filter"), "company_gpw_pzu");

    await waitFor(() => {
      expect(screen.queryByText("Main Market - Corporate actions - Equity - CDR")).not.toBeInTheDocument();
    });
    expect(
      screen.getByText("Main Market - End of market making activities - Equity - PZU"),
    ).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("list_company_events", {
      input: expect.objectContaining({
        companyId: "company_gpw_pzu",
        eventType: null,
        mode: "upcoming",
      }),
    });

    await user.selectOptions(screen.getByLabelText("Event type filter"), "market_making");

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_company_events", {
        input: expect.objectContaining({
          companyId: "company_gpw_pzu",
          eventType: "market_making",
          mode: "upcoming",
        }),
      });
    });
  });

  it("creates a manual company event from the Events screen", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Events" }));
    await user.click(screen.getByRole("button", { name: "Add event" }));

    await user.selectOptions(screen.getByLabelText("Manual event company"), "company_gpw_pzu");
    await user.selectOptions(screen.getByLabelText("Manual event type"), "dividend");
    await user.clear(screen.getByLabelText("Manual event date"));
    await user.type(screen.getByLabelText("Manual event date"), currentWeekTestDate(3));
    await user.type(screen.getByLabelText("Manual event title"), "Dividend decision expected");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_company_event", {
        input: expect.objectContaining({
          companyId: "company_gpw_pzu",
          eventType: "dividend",
          title: "Dividend decision expected",
          sourceType: "manual",
        }),
      });
    });
    expect((await screen.findAllByText("Dividend decision expected")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("Manual").length).toBeGreaterThan(0);
  });

  it("shows selected feed item details", async () => {
    const user = userEvent.setup();

    render(<App />);

    const selectedRow = await screen.findByRole("button", {
      name: "Select feed item: Sample item proving the inbox layout can scan dense rows",
    });

    await user.click(selectedRow);

    expect(selectedRow).toHaveClass("feed-row-selected");
    expect(selectedRow).toHaveAttribute("aria-current", "true");
    expect(
      screen.getAllByText("Saved sample item used to validate the saved filter before real ingestion exists.").length,
    ).toBeGreaterThan(0);
    expect(screen.getByRole("link", { name: "Open source" })).toHaveAttribute(
      "href",
      "https://example.local/sample/pkn",
    );
    expect(screen.getByRole("link", { name: "https://example.local/sample/pkn" })).toHaveAttribute(
      "href",
      "https://example.local/sample/pkn",
    );
    expect(screen.getByText("Sample")).toBeInTheDocument();
  });

  it("opens the matching company workspace from an inbox feed item", async () => {
    const user = userEvent.setup();

    render(<App />);

    await screen.findByLabelText("Feed item details");
    await user.click(screen.getByRole("button", { name: "Open company" }));

    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Company workspace")).toBeInTheDocument();
    expect(screen.getByLabelText("Company feed item details")).toBeInTheDocument();
    expect(screen.getByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    })).toHaveClass("company-feed-row-selected");
  });

  it("creates a notebook draft from an inbox feed item with feed origins", async () => {
    const user = userEvent.setup();

    render(<App />);

    await screen.findByLabelText("Feed item details");
    await user.click(screen.getByRole("button", { name: "Note" }));

    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(screen.getByRole("heading", { name: "Notebooks" })).toBeInTheDocument();
    expect(screen.getByLabelText("Notebook screen note title")).toHaveValue(
      "Current report placeholder for watchlist company",
    );
    expect(screen.getByLabelText("Notebook screen note body")).toHaveValue(
      "Sample official report used to validate feed filtering and detail rendering.",
    );
    expect(screen.getByLabelText("Notebook screen note tags")).toHaveValue(
      "feed, official-report, gpw-espi/ebi",
    );

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_notebook_entry", {
        input: {
          companyId: "company_gpw_cdr",
          title: "Current report placeholder for watchlist company",
          body: "Sample official report used to validate feed filtering and detail rendering.",
          bodyFormat: "markdown",
          tags: ["feed", "official-report", "gpw-espi/ebi"],
          kind: "observation",
          claimStatus: null,
          eventDate: null,
          followUpAfter: null,
          followUpDate: null,
          origins: [
            {
              sourceType: "feed_item",
              sourceId: "feed_sample_cdr_report",
              sourceUrl: "https://www.gpw.pl/komunikaty",
              label: "GPW ESPI/EBI: Current report placeholder for watchlist company",
            },
          ],
        },
      });
    });

    const originFeedButton = await within(notebooksWorkspace).findByRole("button", {
      name: "Open origin feed item: GPW ESPI/EBI: Current report placeholder for watchlist company",
    });
    expect(
      within(notebooksWorkspace).getByRole("link", {
        name: "Open origin source: GPW ESPI/EBI: Current report placeholder for watchlist company",
      }),
    ).toHaveAttribute("href", "https://www.gpw.pl/komunikaty");

    await user.click(originFeedButton);

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(
      within(screen.getByLabelText("Feed item details")).getByRole("heading", {
        name: "Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();
  });

  it("selects inbox feed items with the keyboard", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedItem = await screen.findByRole("button", {
      name: "Select feed item: Sample item proving the inbox layout can scan dense rows",
    });

    feedItem.focus();
    await user.keyboard("{Enter}");

    expect(
      screen.getAllByText("Saved sample item used to validate the saved filter before real ingestion exists.").length,
    ).toBeGreaterThan(0);
    expect(screen.getByRole("link", { name: "https://example.local/sample/pkn" })).toBeInTheDocument();
  });

  it("moves through inbox feed items with arrow keys", async () => {
    const user = userEvent.setup();

    render(<App />);

    const firstFeedItem = await screen.findByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    });

    firstFeedItem.focus();
    await user.keyboard("{ArrowDown}");

    expect(
      screen.getByRole("button", {
        name: "Select feed item: Sample item proving the inbox layout can scan dense rows",
      }),
    ).toHaveFocus();
    expect(
      screen.getAllByText("Saved sample item used to validate the saved filter before real ingestion exists.").length,
    ).toBeGreaterThan(0);

    await user.keyboard("{ArrowUp}");

    expect(firstFeedItem).toHaveFocus();
    expect(
      screen.getAllByText("Sample official report used to validate feed filtering and detail rendering.").length,
    ).toBeGreaterThan(0);
  });

  it("shows feed details only in the inbox", async () => {
    const user = userEvent.setup();

    render(<App />);

    expect(await screen.findByLabelText("Feed item details")).toBeInTheDocument();
    expect(screen.getByRole("separator", { name: "Resize feed details" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Companies" }));

    expect(screen.queryByLabelText("Feed item details")).not.toBeInTheDocument();
    expect(screen.queryByRole("separator", { name: "Resize feed details" })).not.toBeInTheDocument();
  });

  it("resizes the inbox detail pane with keyboard controls", async () => {
    const user = userEvent.setup();

    render(<App />);

    const resizer = await screen.findByRole("separator", { name: "Resize feed details" });

    expect(resizer).toHaveAttribute("aria-valuenow", "360");

    resizer.focus();
    await user.keyboard("{ArrowLeft}");

    expect(resizer).toHaveAttribute("aria-valuenow", "384");
  });

  it("filters inbox sample items by watchlist", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");

    expect(
      await within(feedList).findByText("Sample item proving the inbox layout can scan dense rows"),
    ).toBeInTheDocument();

    await user.selectOptions(await screen.findByLabelText("Inbox watchlist"), "watchlist_main_gpw");

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).queryByText("Sample item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();
  });

  it("filters inbox sample items by status", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Unread" }));

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).queryByText("Sample item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Saved" }));

    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
    expect(within(feedList).getByText("Sample item proving the inbox layout can scan dense rows")).toBeInTheDocument();
  });

  it("moves selection to the next unread item after marking the current unread item read", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_unread",
        title: "Second unread report for review flow",
        summary: "Second unread item should become selected after the first one is marked read.",
        unread: true,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Unread" }));

    const firstUnreadRow = await screen.findByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    });

    expect(firstUnreadRow).toHaveClass("feed-row-selected");

    await user.click(screen.getByRole("button", { name: "Mark read" }));

    await waitFor(() => {
      expect(
        screen.getByRole("button", {
          name: "Select feed item: Second unread report for review flow",
        }),
      ).toHaveClass("feed-row-selected");
    });
    expect(screen.queryByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    })).not.toBeInTheDocument();
    expect(
      screen.getAllByText("Second unread item should become selected after the first one is marked read.").length,
    ).toBeGreaterThan(0);
  });

  it("marks all visible inbox items as read", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_unread",
        title: "Second unread report for bulk read flow",
        summary: "Second unread item should be marked read with the bulk action.",
        unread: true,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Mark all read" }));

    await waitFor(() => {
      expect(screen.getByLabelText("Inbox review summary")).toHaveTextContent("0 unread");
    });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_cdr_report",
        read: true,
        saved: false,
      },
    });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_cdr_second_unread",
        read: true,
        saved: false,
      },
    });
  });

  it("confirms and deletes unsaved feed items without refreshing sources", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    feedItemsResponse = [
      {
        ...initialFeedItems[0],
        id: "feed_unsaved_delete_candidate",
        title: "Unsaved report to delete",
        saved: false,
      },
      {
        ...initialFeedItems[1],
        id: "feed_saved_to_keep",
        title: "Saved report to keep",
        saved: true,
      },
    ];

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Unsaved report to delete");

    await user.click(screen.getByRole("button", { name: "Delete unsaved" }));

    expect(confirm).toHaveBeenCalledWith("Delete all unsaved feed items? Saved items will stay.");
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("delete_unsaved_feed_items");
    });
    await within(feedList).findByText("Saved report to keep");
    expect(within(feedList).queryByText("Unsaved report to delete")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("refresh_sources", expect.anything());

    confirm.mockRestore();
  });

  it("summarizes the current inbox review set", async () => {
    const user = userEvent.setup();

    render(<App />);

    await within(screen.getByLabelText("Feed items")).findByText(
      "Current report placeholder for watchlist company",
    );

    const summary = screen.getByLabelText("Inbox review summary");

    expect(within(summary).getByText("4")).toBeInTheDocument();
    expect(within(summary).getByText("visible")).toBeInTheDocument();
    expect(within(summary).getAllByText("1")).toHaveLength(2);
    expect(within(summary).getByText("unread")).toBeInTheDocument();
    expect(within(summary).getByText("saved")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Unread" }));

    expect(within(summary).getAllByText("1")).toHaveLength(2);
    expect(within(summary).getByText("visible")).toBeInTheDocument();
    expect(within(summary).getByText("unread")).toBeInTheDocument();
    expect(within(summary).getByText("0")).toBeInTheDocument();
  });

  it("filters inbox sample items by search query", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.type(screen.getByLabelText("Search feed"), "transcript");

    expect(within(feedList).getByText("Transcript-derived note candidate waits for future provider work")).toBeInTheDocument();
    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
  });

  it("filters inbox sample items by type and source", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.selectOptions(screen.getByLabelText("Inbox type"), "Transcript");

    expect(within(feedList).getByText("Transcript-derived note candidate waits for future provider work")).toBeInTheDocument();
    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Inbox type"), "all");
    await user.selectOptions(screen.getByLabelText("Inbox source"), "GPW ESPI/EBI");

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).queryByText("Transcript-derived note candidate waits for future provider work")).not.toBeInTheDocument();
  });

  it("clears active inbox filters", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.type(screen.getByLabelText("Search feed"), "does-not-match");

    expect(within(feedList).getByText("No feed items for selected filters.")).toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Clear filters" })[0]);

    expect(screen.getByLabelText("Search feed")).toHaveValue("");
    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).getByText("Sample item proving the inbox layout can scan dense rows")).toBeInTheDocument();
  });

  it("shows a first-run inbox empty state when no companies are tracked", async () => {
    const user = userEvent.setup();

    companiesResponse = [];
    feedItemsResponse = [];

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");

    expect(await within(feedList).findByText("No companies tracked yet.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Add company" }));

    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();
  });

  it("shows a no-feed inbox empty state with a source status path when companies exist", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [];

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");

    expect(await within(feedList).findByText("No stored feed items yet.")).toBeInTheDocument();
    expect(within(feedList).getByRole("button", { name: "Refresh sources" })).toBeEnabled();

    await user.click(within(feedList).getByRole("button", { name: "Open Sources" }));

    expect(screen.getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Source adapter details")).toBeInTheDocument();
  });

  it("refreshes source-backed feed items from the topbar", async () => {
    const user = userEvent.setup();

    render(<App />);

    await within(screen.getByLabelText("Feed items")).findByText(
      "Current report placeholder for watchlist company",
    );

    await user.click(screen.getByRole("button", { name: "Refresh sources" }));

    const refreshedFeedItem = await screen.findByRole("button", {
      name: "Select feed item: Refreshed GPW report from sample source",
    });
    await user.click(refreshedFeedItem);
    expect(screen.getByLabelText("Feed summary")).toHaveTextContent(
      "Refreshed GPW report from sample source",
    );
    expect(await screen.findAllByText("2026-05-30 17:13:31")).not.toHaveLength(0);
    expect(screen.getByText("2026-05-30 17:30:00")).toBeInTheDocument();
    const officialBody = screen.getByLabelText("Official report body");
    expect(officialBody).toHaveTextContent("Stored");
    expect(officialBody).not.toHaveAttribute("open");
    await user.click(within(officialBody).getByText("Official report body"));
    expect(officialBody).toHaveAttribute("open");
    expect(officialBody).toHaveTextContent(
      "Official GPW body text fetched from the detail page.",
    );
    expect(screen.getByText("report.pdf")).toBeInTheDocument();
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_sources", { input: { trigger: "manual" } }),
    );

    await user.click(screen.getByRole("button", { name: "Sources" }));
    const refreshSummary = await screen.findByLabelText("Last source refresh summary");

    expect(within(refreshSummary).getByLabelText("Fetched source items")).toHaveTextContent("2");
    expect(within(refreshSummary).getByLabelText("Matched source items")).toHaveTextContent("1");

    const unmatchedItems = await screen.findByLabelText("Unmatched source item diagnostics");
    expect(within(unmatchedItems).queryByText("LUBAWA S.A.")).not.toBeInTheDocument();

    await user.click(within(unmatchedItems).getByRole("button", { name: /Unmatched/i }));

    expect(within(unmatchedItems).getByText("LUBAWA S.A.")).toBeInTheDocument();
    expect(within(unmatchedItems).getByText("Unmatched GPW report from sample source")).toBeInTheDocument();
    expect(within(unmatchedItems).getByText("2026-05-30 17:13:31")).toBeInTheDocument();
  });

  it("shows source refresh failures in the topbar refresh control", async () => {
    const user = userEvent.setup();
    refreshSourcesError = "GPW HTTP request failed";

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Refresh sources" }));

    const failedRefresh = await screen.findByRole("button", { name: "Source refresh failed" });
    expect(failedRefresh).toHaveAttribute("title", "Source refresh failed: Error: GPW HTTP request failed");
    expect(failedRefresh).toHaveClass("source-refresh-button-danger");
  });

  it("backs off scheduled source polling after repeated refresh failures", async () => {
    const user = userEvent.setup();
    refreshSourcesError = "GPW HTTP request failed";

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Refresh sources" }));
    await user.click(await screen.findByRole("button", { name: "Source refresh failed" }));
    await user.click(screen.getByRole("button", { name: "Sources" }));
    await user.click(await screen.findByRole("button", { name: "Open source adapter: Bankier Giełda RSS" }));

    expect(
      within(await screen.findByLabelText("Source adapter details")).getByText("In-app · 15 min · backoff 30 min"),
    ).toBeInTheDocument();
  });

  it("refreshes source-backed feed items from the Sources screen", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sources" }));
    const sourcesPanel = screen.getByRole("region", { name: "Sources" });
    await user.click(within(sourcesPanel).getByRole("button", { name: "Refresh sources" }));

    expect(await screen.findByLabelText("Last source refresh summary")).toBeInTheDocument();
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_sources", { input: { trigger: "manual" } }),
    );
  });

  it("refreshes a single enabled source from source details", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    await user.click(within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: Bankier Giełda RSS",
    }));

    expect(within(await screen.findByLabelText("Source adapter details")).getByText("Public RSS")).toBeInTheDocument();

    await user.click(within(await screen.findByLabelText("Source adapter details")).getByRole("button", {
      name: "Refresh source",
    }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_source", {
        input: {
          adapterId: "bankier-market-rss",
          trigger: "manual",
        },
      }),
    );
    expect(await screen.findByLabelText("Last source refresh summary")).toBeInTheDocument();
  });

  it("shows independently jittered next poll times for enabled feed sources", async () => {
    const user = userEvent.setup();
    const randomSpy = vi
      .spyOn(Math, "random")
      .mockReturnValueOnce(0)
      .mockReturnValueOnce(0.5)
      .mockReturnValueOnce(0.9)
      .mockReturnValue(0.1);

    try {
      render(<App />);

      await user.click(screen.getByRole("button", { name: "Sources" }));

      const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
      await user.click(within(sourceAdaptersRegion).getByRole("button", {
        name: "Open source adapter: Bankier Company Komunikaty",
      }));
      const bankierCompanyNextPoll =
        within(await screen.findByLabelText("Source adapter details")).getByText(/^In 15 min \d+s$|^In 16 min$/)
          .textContent;

      await user.click(within(sourceAdaptersRegion).getByRole("button", {
        name: "Open source adapter: Bankier Giełda RSS",
      }));
      const bankierNextPoll =
        within(await screen.findByLabelText("Source adapter details")).getByText(/^In 15 min \d+s$|^In 16 min$/)
          .textContent;

      expect(bankierNextPoll).not.toEqual(bankierCompanyNextPoll);
    } finally {
      randomSpy.mockRestore();
    }
  });

  it("opens source status from the topbar source pill", async () => {
    const user = userEvent.setup();

    render(<App />);

    const sourceStatus = await screen.findByRole("button", { name: "Open source status" });

    expect(sourceStatus).toHaveTextContent("Sources");
    expect(sourceStatus).toHaveTextContent("3/7");

    await user.click(sourceStatus);

    expect(screen.getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    const sourceRow = within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: GPW Company Registry",
    });

    expect(sourceRow).toHaveClass("source-row-selected");
    expect(within(sourceAdaptersRegion).getByLabelText("Source adapter details")).toBeInTheDocument();
  });

  it("refreshes database-backed views from the DB status pill", async () => {
    const user = userEvent.setup();

    render(<App />);

    await within(screen.getByLabelText("Feed items")).findByText(
      "Current report placeholder for watchlist company",
    );

    vi.mocked(invoke).mockClear();

    await user.click(screen.getByRole("button", { name: "Refresh database-backed views" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("database_status");
      expect(invoke).toHaveBeenCalledWith("list_companies");
      expect(invoke).toHaveBeenCalledWith("list_watchlists");
      expect(invoke).toHaveBeenCalledWith("list_watchlist_memberships");
      expect(invoke).toHaveBeenCalledWith("list_feed_items");
      expect(invoke).toHaveBeenCalledWith("list_source_adapters");
      expect(invoke).toHaveBeenCalledWith("get_settings");
    });
  });

  it("shows SQLite-backed settings and persists theme changes", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Settings" }));

    const settingsRegion = await screen.findByLabelText("Application settings");

    expect(within(settingsRegion).getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Feed Cleanup" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "Import And Export" })).toBeInTheDocument();
    expect(within(settingsRegion).getByRole("heading", { name: "AI" })).toBeInTheDocument();
    expect(within(settingsRegion).getByText("sqlite")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("night-neon")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("accepted_deferred")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Feed cleanup")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("30 days")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Cleanup interval")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Daily")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("Last cleanup")).toBeInTheDocument();
    expect(within(settingsRegion).getAllByText("Not run this session")).toHaveLength(2);
    expect(within(settingsRegion).getByText("Saved")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Settings theme"), "light");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        theme: "light",
      },
    });
    expect(screen.getByLabelText("Settings theme")).toHaveValue("light");

    await user.selectOptions(screen.getByLabelText("Settings source poll interval"), "1800");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        pollIntervalSeconds: 1800,
      },
    });
    expect(screen.getByLabelText("Settings source poll interval")).toHaveValue("1800");
  });

  it("shows source adapter status", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    const sourceRow = await within(sourceAdaptersRegion).findByRole("button", {
      name: "Open source adapter: GPW ESPI/EBI",
    });

    expect(within(sourceAdaptersRegion).getByText("GPW ESPI/EBI")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("gpw-espi-ebi")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Official Reports · Public Page")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Bankier Giełda RSS")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getAllByText("Public Media · RSS")).toHaveLength(3);
    expect(within(sourceAdaptersRegion).getByText("Bankier Company Komunikaty")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Official Reports · Public JSON")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Bankier Firma RSS")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Bankier Wiadomosci RSS")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Portal Analiz")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Authenticated Research · Authenticated")).toBeInTheDocument();
    expect(within(sourceRow).getByText("Disabled")).toBeInTheDocument();

    await user.click(sourceRow);

    expect(sourceRow).toHaveClass("source-row-selected");
    const sourceDetails = await screen.findByLabelText("Source adapter details");
    expect(within(sourceDetails).getAllByText("Off")).not.toHaveLength(0);
    expect(within(sourceDetails).getByText("Next poll")).toBeInTheDocument();
    expect(within(sourceDetails).getAllByText("Off")).not.toHaveLength(0);
    expect(within(sourceDetails).getByText("Access")).toBeInTheDocument();
    expect(within(sourceDetails).getAllByText("Disabled")).not.toHaveLength(0);
    expect(within(sourceDetails).getByText("Manual")).toBeInTheDocument();
    expect(
      within(await screen.findByLabelText("Source adapter details")).getByText(
        "2 fetched · 1 created · 1 matched · 1 unmatched · details 1/1 stored · 0 failed",
      ),
    ).toBeInTheDocument();
    expect(
      within(await screen.findByLabelText("Source adapter details")).getByText(
        "Disabled while Bankier Company Komunikaty is the active official-report source",
      ),
    ).toBeInTheDocument();

    expect(
      within(await screen.findByLabelText("Source adapter details")).getByText(/Registered for later revisit/),
    ).toBeInTheDocument();
    const sourcePageButton = within(await screen.findByLabelText("Source adapter details")).getByRole("button", {
      name: "Open source page for GPW ESPI/EBI",
    });
    await user.click(sourcePageButton);
    expect(openUrl).toHaveBeenCalledWith("https://www.gpw.pl/komunikaty");

    const portalAnalizRow = within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: Portal Analiz",
    });
    expect(within(portalAnalizRow).getByText("Disabled")).toBeInTheDocument();
    await user.click(portalAnalizRow);
    const portalAnalizDetails = await screen.findByLabelText("Source adapter details");
    expect(within(portalAnalizDetails).getAllByText("Off")).not.toHaveLength(0);
    expect(within(portalAnalizDetails).getByText("Access")).toBeInTheDocument();
    expect(within(portalAnalizDetails).getAllByText("Disabled")).not.toHaveLength(0);
    expect(within(portalAnalizDetails).getByRole("button", { name: "Refresh source" })).toBeDisabled();
    expect(
      within(portalAnalizDetails).getByText(
        "Late-v1 disabled placeholder; no automated access until the authenticated-source implementation is explicitly built",
      ),
    ).toBeInTheDocument();

    await user.click(portalAnalizRow);

    expect(screen.queryByLabelText("Source adapter details")).not.toBeInTheDocument();
  });

  it("refreshes the GPW company registry from source details", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    const registryRow = within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: GPW Company Registry",
    });

    expect(within(registryRow).getByText("Company registry · Public GPW company list")).toBeInTheDocument();

    await user.click(registryRow);
    const registryDetails = await screen.findByLabelText("Source adapter details");
    expect(within(registryDetails).getByText("Next poll")).toBeInTheDocument();
    expect(within(registryDetails).getByText(/In 23h|In 1 day/)).toBeInTheDocument();
    expect(within(registryDetails).getByText("Cache result")).toBeInTheDocument();
    expect(within(registryDetails).getByText("400 cached entries · 400 refreshed or updated")).toBeInTheDocument();
    expect(within(registryDetails).getByText("Refresh policy")).toBeInTheDocument();
    expect(within(registryDetails).queryByText("Detail warning")).not.toBeInTheDocument();
    await user.click(within(await screen.findByLabelText("Company registry refresh")).getByRole("button", {
      name: "Refresh registry",
    }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_gpw_company_registry", {
        input: { trigger: "manual" },
      }),
    );
    expect(await screen.findByText("400/400 cached")).toBeInTheDocument();
  });

  it("lists cached GPW registry companies and adds an untracked company", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    await user.click(within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: GPW Company Registry",
    }));

    const registryPanel = await screen.findByLabelText("GPW company registry entries");
    expect(within(registryPanel).queryByText("DINO POLSKA S.A.")).not.toBeInTheDocument();

    await user.click(within(registryPanel).getByRole("button", { name: /Companies/i }));

    expect(await within(registryPanel).findByText("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(await within(registryPanel).findByText("DINO POLSKA S.A.")).toBeInTheDocument();

    await user.type(within(registryPanel).getByLabelText("Search GPW company registry"), "dnp");

    expect(within(registryPanel).queryByText("CD PROJEKT S.A.")).not.toBeInTheDocument();
    expect(await within(registryPanel).findByText("DINO POLSKA S.A.")).toBeInTheDocument();

    await user.click(within(registryPanel).getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("create_company", {
        input: {
          exchange: "GPW",
          ticker: "DNP",
          displayName: "DINO POLSKA S.A.",
          isin: "PLDINPL00011",
          cik: null,
          lei: null,
        },
      }),
    );
    expect(await within(registryPanel).findByTitle("GPW:DNP already added")).toBeDisabled();
  });

  it("expands and collapses source adapter details with keyboard controls", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceRow = await screen.findByRole("button", {
      name: "Open source adapter: GPW ESPI/EBI",
    });

    sourceRow.focus();
    await user.keyboard("{Enter}");

    expect(await screen.findByLabelText("Source adapter details")).toBeInTheDocument();

    await user.keyboard(" ");

    expect(screen.queryByLabelText("Source adapter details")).not.toBeInTheDocument();
  });

  it("shows the notebooks workspace and planned transcript placeholder screen", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [initialNotebookEntry];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Notebooks" }));

    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(screen.getByRole("heading", { name: "Notebooks" })).toBeInTheDocument();
    expect(
      await within(notebooksWorkspace).findByRole("button", {
        name: "Open notebook company: GPW:CDR",
      }),
    ).toBeInTheDocument();
    const notebookCompanyButton = within(notebooksWorkspace).getByRole("button", {
      name: "Open notebook company: GPW:CDR",
    });
    expect(notebookCompanyButton).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", { name: "Show open claims for GPW:CDR" }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", { name: "Show follow-ups for GPW:CDR" }),
    ).toBeInTheDocument();
    const notebookRow = await within(notebooksWorkspace).findByRole("button", {
      name: "Select notebook screen entry: Release schedule promise",
    });

    expect(notebookRow).toBeInTheDocument();
    expect(screen.queryByLabelText("Notebook screen selected body")).not.toBeInTheDocument();

    await user.click(notebookRow);

    expect(screen.getByLabelText("Notebook screen selected body")).toHaveTextContent(
      "Management promised a release milestone in the next two quarters.",
    );

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "New note" }));
    await user.type(screen.getByLabelText("Notebook screen note title"), "Notebook desk note");
    await user.type(screen.getByLabelText("Notebook screen note body"), "Created from the main notebooks pane.");
    await user.type(screen.getByLabelText("Notebook screen note tags"), "desk, workflow");
    await user.selectOptions(screen.getByLabelText("Notebook screen note kind"), "observation");
    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_notebook_entry", {
        input: {
          companyId: "company_gpw_cdr",
          title: "Notebook desk note",
          body: "Created from the main notebooks pane.",
          bodyFormat: "markdown",
          tags: ["desk", "workflow"],
          kind: "observation",
          claimStatus: null,
          eventDate: null,
          followUpAfter: null,
          followUpDate: null,
          origins: [
            {
              sourceType: "manual",
              sourceId: null,
              sourceUrl: null,
              label: "Manual note",
            },
          ],
        },
      });
    });

    const createdNotebookRow = await within(notebooksWorkspace).findByRole("button", {
      name: "Select notebook screen entry: Notebook desk note",
    });
    expect(createdNotebookRow).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Transcripts" }));

    const transcriptsPlaceholder = await screen.findByLabelText("Transcripts placeholder");

    expect(screen.getByRole("heading", { name: "Transcripts" })).toBeInTheDocument();
    expect(within(transcriptsPlaceholder).getByText("Gemini for YouTube transcription only")).toBeInTheDocument();
  });

  it("filters notebook screen entries by kind, status, tag, and follow-up scheduling", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [
      initialNotebookEntry,
      {
        ...initialNotebookEntry,
        id: "note_company_gpw_cdr_margin_observation",
        title: "Margin observation",
        body: "Desk note about margin pressure after the call.",
        tags: ["desk", "margin"],
        kind: "observation",
        claimStatus: null,
        followUpAfter: null,
        followUpDate: null,
      },
      {
        ...initialNotebookEntry,
        id: "note_company_gpw_cdr_capex_claim",
        title: "Capex delivery claim",
        body: "Management claimed capex should normalize.",
        tags: ["capex", "management-guidance"],
        kind: "claim",
        claimStatus: "delivered",
        followUpAfter: "2026-Q3",
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Notebooks" }));

    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(
      await within(notebooksWorkspace).findByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Show open claims for GPW:CDR" }));

    expect(screen.getByLabelText("Notebook claim status filter")).toHaveValue("open");
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).not.toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Show follow-ups for GPW:CDR" }));

    expect(screen.getByLabelText("Notebook follow-up filter")).toHaveValue("has_follow_up");
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).not.toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));

    await user.selectOptions(screen.getByLabelText("Notebook kind filter"), "observation");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.selectOptions(screen.getByLabelText("Notebook claim status filter"), "delivered");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Capex delivery claim",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.type(screen.getByLabelText("Notebook tag filter"), "desk");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Capex delivery claim",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.selectOptions(screen.getByLabelText("Notebook follow-up filter"), "no_follow_up");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();
  });

  it("renders notebook Markdown in read mode", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [
      {
        ...initialNotebookEntry,
        body: "# Release checklist\n- **Milestone** shipped\n- `Patch` ready",
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Notebooks" }));
    const notebookRow = await screen.findByRole("button", {
      name: "Select notebook screen entry: Release schedule promise",
    });

    expect(within(notebookRow).queryByText("# Release checklist")).not.toBeInTheDocument();

    await user.click(notebookRow);

    const notebookBody = screen.getByLabelText("Notebook screen selected body");

    expect(within(notebookBody).getByRole("heading", { name: "Release checklist" })).toBeInTheDocument();
    expect(within(notebookBody).getByText("Milestone")).toHaveProperty("tagName", "STRONG");
    expect(within(notebookBody).getByText("Patch")).toHaveProperty("tagName", "CODE");

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.clear(screen.getByLabelText("Notebook screen selected follow-up date"));
    await user.type(screen.getByLabelText("Notebook screen selected follow-up date"), "2026-12-15");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_notebook_entry", {
        input: {
          id: "note_company_gpw_cdr_release_schedule",
          title: "Release schedule promise",
          body: "# Release checklist\n- **Milestone** shipped\n- `Patch` ready",
          tags: ["management-guidance", "product"],
          kind: "claim",
          claimStatus: "open",
          eventDate: "2026-05-29",
          followUpAfter: "2026-Q4",
          followUpDate: "2026-12-15",
        },
      });
    });
  });

  it("updates sample read and saved state from the detail pane", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.click(screen.getByRole("button", { name: "Unread" }));
    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Mark read" }));
    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
    expect(within(feedList).getByText("No feed items for selected filters.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "All" }));
    await user.click(within(feedList).getByText("Current report placeholder for watchlist company"));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await user.click(screen.getByRole("button", { name: "Saved" }));

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
  });

  it("toggles feed item read state on double click", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    const feedTitle = await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.dblClick(feedTitle);
    await user.click(screen.getByRole("button", { name: "Unread" }));

    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
    expect(within(feedList).getByText("No feed items for selected filters.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "All" }));
    await user.dblClick(within(feedList).getByText("Current report placeholder for watchlist company"));
    await user.click(screen.getByRole("button", { name: "Unread" }));

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
  });

  it("fills company form from the GPW registry lookup", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.clear(screen.getByLabelText("Ticker"));
    await user.type(screen.getByLabelText("Ticker"), "CDR");
    await user.click(screen.getByRole("button", { name: "Lookup" }));

    expect(await screen.findByDisplayValue("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(screen.getByDisplayValue("PLOPTTC00011")).toBeInTheDocument();
    expect(screen.getByText("Filled from gpw_registry: GPW:CDR")).toBeInTheDocument();
  });

  it("selects a company from local GPW registry suggestions", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.clear(screen.getByLabelText("Name"));
    await user.type(screen.getByLabelText("Name"), "DINO");

    const suggestions = await screen.findByLabelText("Company registry suggestions");
    await user.click(within(suggestions).getByRole("button", { name: /GPW:DNP/ }));

    expect(screen.getByDisplayValue("DNP")).toBeInTheDocument();
    expect(screen.getByDisplayValue("DINO POLSKA S.A.")).toBeInTheDocument();
    expect(screen.getByDisplayValue("PLDINPL00011")).toBeInTheDocument();
    expect(screen.getByText("Selected from GPW registry: GPW:DNP")).toBeInTheDocument();
    expect(screen.queryByLabelText("Company registry suggestions")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Clear ticker" }));

    expect(screen.getByLabelText("Exchange")).toHaveValue("GPW");
    expect(screen.getByLabelText("Ticker")).toHaveValue("");
    expect(screen.getByLabelText("Name")).toHaveValue("DINO POLSKA S.A.");
    expect(screen.getByLabelText("ISIN")).toHaveValue("PLDINPL00011");
    expect(screen.queryByText("Selected from GPW registry: GPW:DNP")).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Ticker"), "DNP");

    await user.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("create_company", {
        input: {
          exchange: "GPW",
          ticker: "DNP",
          displayName: "DINO POLSKA S.A.",
          isin: "PLDINPL00011",
          cik: null,
          lei: null,
        },
      }),
    );
  });

  it("filters the tracked companies list", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const companyList = await screen.findByLabelText("Companies list");
    expect(within(companyList).getByText("GPW:CDR")).toBeInTheDocument();
    expect(within(companyList).getByText("GPW:PZU")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Search tracked companies"), "pzu");

    expect(within(companyList).queryByText("GPW:CDR")).not.toBeInTheDocument();
    expect(within(companyList).getByText("GPW:PZU")).toBeInTheDocument();
    expect(screen.getByText("1/4 companies")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Clear company search" }));

    expect(within(companyList).getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getByText("4/4 companies")).toBeInTheDocument();
  });

  it("confirms and deletes a company", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByTitle("Delete GPW:CDR"));

    expect(confirm).toHaveBeenCalledWith("Delete GPW:CDR from your local registry?");

    confirm.mockRestore();
  });

  it("opens a company workspace with company-scoped feed and metadata tabs", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    const companyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    await user.click(companyRow);

    const workspace = await screen.findByLabelText("Company workspace");

    expect(companyRow).toHaveClass("company-row-selected");
    expect(companyRow.compareDocumentPosition(workspace) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(workspace).getByText("GPW:CDR")).toBeInTheDocument();
    expect(within(workspace).getByText("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(within(screen.getByLabelText("Selected company metadata")).getByText("1 feed")).toBeInTheDocument();
    expect(within(screen.getByLabelText("Selected company metadata")).getByText("1 unread")).toBeInTheDocument();
    expect(within(screen.getByLabelText("Selected company metadata")).getByText("0 saved")).toBeInTheDocument();
    expect(
      within(screen.getByLabelText("Company feed")).getByRole("button", {
        name: "Open company feed item: Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();
    expect(within(screen.getByLabelText("Company feed")).queryByText("Sample item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();

    await user.click(within(workspace).getByRole("button", { name: "Metadata" }));

    expect(within(screen.getByLabelText("Company metadata")).getByText("PLOPTTC00011")).toBeInTheDocument();
  });

  it("lists and creates company notebook entries", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [initialNotebookEntry];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    await user.click(screen.getByRole("button", { name: "Notebook" }));

    const notebook = await screen.findByLabelText("Company notebook");

    expect(
      within(notebook).getByRole("button", { name: "Select notebook entry: Release schedule promise" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Selected notebook body")).toHaveTextContent(
      "Management promised a release milestone in the next two quarters.",
    );
    expect(within(notebook).getAllByText("management-guidance").length).toBeGreaterThan(0);
    expect(within(notebook).getAllByText("2026-Q4").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.clear(screen.getByLabelText("Selected notebook body"));
    await user.type(screen.getByLabelText("Selected notebook body"), "Management shifted the release language.");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_notebook_entry", {
        input: {
          id: "note_company_gpw_cdr_release_schedule",
          title: "Release schedule promise",
          body: "Management shifted the release language.",
          tags: ["management-guidance", "product"],
          kind: "claim",
          claimStatus: "open",
          eventDate: "2026-05-29",
          followUpAfter: "2026-Q4",
          followUpDate: "2026-11-30",
        },
      });
    });

    await user.click(screen.getByRole("button", { name: "New note" }));
    await user.clear(screen.getByLabelText("Notebook note title"));
    await user.type(screen.getByLabelText("Notebook note title"), "Conference note");
    await user.type(screen.getByLabelText("Notebook note body"), "Board mentioned margin pressure.");
    await user.type(screen.getByLabelText("Notebook note tags"), "conference, margin");
    await user.selectOptions(screen.getByLabelText("Notebook note kind"), "observation");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_notebook_entry", {
        input: {
          companyId: "company_gpw_cdr",
          title: "Conference note",
          body: "Board mentioned margin pressure.",
          bodyFormat: "markdown",
          tags: ["conference", "margin"],
          kind: "observation",
          claimStatus: null,
          eventDate: null,
          followUpAfter: null,
          followUpDate: null,
          origins: [
            {
              sourceType: "manual",
              sourceId: null,
              sourceUrl: null,
              label: "Manual note",
            },
          ],
        },
      });
    });
    const conferenceNoteRow = await within(notebook).findByRole("button", {
      name: "Select notebook entry: Conference note",
    });
    expect(conferenceNoteRow).toBeInTheDocument();
    await user.click(conferenceNoteRow);

    expect(screen.getByLabelText("Selected notebook body")).toHaveTextContent("Board mentioned margin pressure.");
  });

  it("lists company claims and updates claim status", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [initialNotebookEntry];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    await user.click(screen.getByRole("button", { name: "Claims" }));

    const claims = await screen.findByLabelText("Company claims");
    const claimRow = within(claims).getByRole("button", {
      name: "Open claim: Release schedule promise",
    });

    expect(claimRow).toBeInTheDocument();
    expect(within(claims).getByText("1 follow-up item for GPW:CDR")).toBeInTheDocument();

    await user.click(claimRow);

    expect(screen.getByLabelText("Claim detail")).toHaveTextContent(
      "Management promised a release milestone in the next two quarters.",
    );

    await user.selectOptions(screen.getByLabelText("Claim status"), "delivered");
    await user.click(within(screen.getByLabelText("Claim detail")).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_notebook_entry", {
        input: {
          id: "note_company_gpw_cdr_release_schedule",
          title: "Release schedule promise",
          body: "Management promised a release milestone in the next two quarters.",
          tags: ["management-guidance", "product"],
          kind: "claim",
          claimStatus: "delivered",
          eventDate: "2026-05-29",
          followUpAfter: "2026-Q4",
          followUpDate: "2026-11-30",
        },
      });
    });
  });

  it("hides the company workspace when the open company row is clicked again", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const companyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    await user.click(companyRow);
    expect(await screen.findByLabelText("Company workspace")).toBeInTheDocument();

    await user.click(companyRow);
    expect(screen.queryByLabelText("Company workspace")).not.toBeInTheDocument();
  });

  it("moves through company rows with arrow keys without expanding a collapsed workspace", async () => {
    const user = userEvent.setup();

    companiesResponse = [
      initialCompanies[0],
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
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const firstCompanyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    firstCompanyRow.focus();
    await user.keyboard("{ArrowDown}");

    const secondCompanyRow = screen.getByRole("button", { name: "Open GPW:PKN workspace" });

    expect(secondCompanyRow).toHaveFocus();
    expect(secondCompanyRow).not.toHaveClass("company-row-selected");
    expect(screen.queryByLabelText("Company workspace")).not.toBeInTheDocument();
  });

  it("moves an already-open company workspace with company row arrow keys", async () => {
    const user = userEvent.setup();

    companiesResponse = [
      initialCompanies[0],
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
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const firstCompanyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    await user.click(firstCompanyRow);
    firstCompanyRow.focus();
    await user.keyboard("{ArrowDown}");

    const secondCompanyRow = screen.getByRole("button", { name: "Open GPW:PKN workspace" });

    expect(secondCompanyRow).toHaveFocus();
    expect(secondCompanyRow).toHaveClass("company-row-selected");
    expect(within(await screen.findByLabelText("Company workspace")).getByText("GPW:PKN")).toBeInTheDocument();
  });

  it("shows an actionable company feed empty state for tracked companies without feed items", async () => {
    const user = userEvent.setup();

    companiesResponse = [
      {
        id: "company_gpw_lpp",
        exchange: "GPW",
        ticker: "LPP",
        qualifiedTicker: "GPW:LPP",
        displayName: "LPP S.A.",
        isin: "PLLPP0000011",
        cik: null,
        lei: null,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:LPP workspace" }));

    const companyFeed = await screen.findByLabelText("Company feed");

    expect(within(companyFeed).getByText("No stored feed items for GPW:LPP yet.")).toBeInTheDocument();
    expect(
      within(companyFeed).getByText(
        "This company is tracked locally, but no sample or ingested items are attached to it yet.",
      ),
    ).toBeInTheDocument();

    await user.click(within(companyFeed).getByRole("button", { name: "Open filtered Inbox" }));

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:LPP");
    expect(screen.getByText("No feed items for selected filters.")).toBeInTheDocument();
  });

  it("shows company feed item details inline and can open the item in the inbox", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    const companyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    await user.click(companyFeedRow);

    const companyFeedDetail = await screen.findByLabelText("Company feed item details");

    expect(companyFeedRow.compareDocumentPosition(companyFeedDetail) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(companyFeedDetail).getByLabelText("Feed summary")).toHaveTextContent(
      "Sample official report used to validate feed filtering and detail rendering.",
    );
    const companyOfficialBody = within(companyFeedDetail).getByLabelText("Official report body");
    expect(companyOfficialBody).toHaveTextContent("Not stored");
    expect(companyOfficialBody).not.toHaveAttribute("open");
    await user.click(within(companyOfficialBody).getByText("Official report body"));
    expect(companyOfficialBody).toHaveAttribute("open");
    expect(companyOfficialBody).toHaveTextContent(/No official report body is stored/);
    expect(within(companyFeedDetail).getByText("GPW ESPI/EBI")).toBeInTheDocument();
    expect(within(companyFeedDetail).getByRole("link", { name: "Open source" })).toHaveAttribute(
      "href",
      "https://www.gpw.pl/komunikaty",
    );

    await user.click(within(companyFeedDetail).getByRole("button", { name: "Open in Inbox" }));

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:CDR");
    expect(screen.getByLabelText("Feed item details")).toBeInTheDocument();
  });

  it("uses inbox unread and saved visual state in company feed rows", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    const companyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    expect(companyFeedRow).toHaveClass("unread");
    expect(within(companyFeedRow).getByTitle("Unread")).toBeInTheDocument();
  });

  it("hides company feed item details when the open feed row is clicked again", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    const companyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    await user.click(companyFeedRow);
    expect(await screen.findByLabelText("Company feed item details")).toBeInTheDocument();

    await user.click(companyFeedRow);
    expect(screen.queryByLabelText("Company feed item details")).not.toBeInTheDocument();
  });

  it("moves through collapsed company feed rows without expanding details", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_report",
        title: "Second CDR report for company feed keyboard navigation",
        summary: "Second company-scoped sample item.",
        unread: false,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    const firstCompanyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    firstCompanyFeedRow.focus();
    await user.keyboard("{ArrowDown}");

    const secondCompanyFeedRow = screen.getByRole("button", {
      name: "Open company feed item: Second CDR report for company feed keyboard navigation",
    });

    expect(secondCompanyFeedRow).toHaveFocus();
    expect(screen.queryByLabelText("Company feed item details")).not.toBeInTheDocument();
  });

  it("moves expanded company feed details with arrow keys and toggles details with space", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_report",
        title: "Second CDR report for company feed keyboard navigation",
        summary: "Second company-scoped sample item.",
        unread: false,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    const firstCompanyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    await user.click(firstCompanyFeedRow);
    firstCompanyFeedRow.focus();
    await user.keyboard("{ArrowDown}");

    const secondCompanyFeedRow = screen.getByRole("button", {
      name: "Open company feed item: Second CDR report for company feed keyboard navigation",
    });

    expect(secondCompanyFeedRow).toHaveFocus();
    expect(
      within(await screen.findByLabelText("Company feed item details")).getByText(
        "Second company-scoped sample item.",
      ),
    ).toBeInTheDocument();

    await user.keyboard(" ");
    expect(screen.queryByLabelText("Company feed item details")).not.toBeInTheDocument();
  });

  it("updates company feed item read and saved state from inline details", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    await user.click(await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    }));

    const companyFeedDetail = await screen.findByLabelText("Company feed item details");

    await user.click(within(companyFeedDetail).getByRole("button", { name: "Mark read" }));
    await user.click(within(companyFeedDetail).getByRole("button", { name: "Save" }));

    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_cdr_report",
        read: true,
        saved: false,
      },
    });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_cdr_report",
        read: true,
        saved: true,
      },
    });
  });

  it("creates a watchlist and assigns a company", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.type(screen.getByLabelText("Watchlist name"), "Main GPW");
    await user.click(screen.getByRole("button", { name: "Create" }));
    await user.click(
      within(await screen.findByRole("button", { name: "Open GPW:CDR workspace" })).getByRole(
        "button",
        { name: "Assign" },
      ),
    );

    expect(await within(screen.getByLabelText("Watchlist chips")).findByText("Main GPW")).toBeInTheDocument();
    expect(
      await within(screen.getByLabelText("Watchlist memberships for GPW:CDR")).findByText("Main GPW"),
    ).toBeInTheDocument();
    expect(await screen.findByRole("status", { name: "Assigned to Main GPW" })).toBeInTheDocument();
  });

  it("removes a company from a selected watchlist", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(
      within(await screen.findByRole("button", { name: "Open GPW:CDR workspace" })).getByRole(
        "button",
        { name: "Remove" },
      ),
    );

    expect(await screen.findByRole("status", { name: "Removed from Main GPW" })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("remove_company_from_watchlist", {
      input: {
        watchlistId: "watchlist_main_gpw",
        companyId: "company_gpw_cdr",
      },
    });
  });
});
