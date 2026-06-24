import { mockIPC } from "@tauri-apps/api/mocks";
import type {
  Company,
  CompanyEvent,
  CompanyRegistryEntry,
  CompanySignal,
  CredentialStatus,
  FeedItem,
  LicenseStatus,
  LocalMetricsSnapshot,
  NotebookEntry,
  SourceAdapter,
  UserSettings,
  Watchlist,
  WatchlistMembership,
} from "../api/types";
import type {
  FinancialFact,
  FinancialPeriod,
  KpiDefinition,
  KpiRelevance,
} from "../api/financialsTypes";
import type { KpiExtractionJob, KpiExtractionProposal } from "../api/kpiExtraction";
import type { ReportDocument } from "../api/reportDocumentsTypes";
import type { EmbeddingModelStatus } from "../api/interpretation";
import { createMockRuntime } from "./scenarios/runtime";
import type { ScenarioData, ScenarioName } from "./scenarios/scenarios";

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

// A realistic "results" report with several long-named PDF attachments, used to
// check the extraction panel's source-button layout under real-world filenames.
feedItems.push({
  id: "feed_results_report",
  company: "GPW:CDR",
  type: "Official report",
  source: "Bankier Company Komunikaty",
  time: "Today 08:20",
  title: "CD PROJEKT S.A. — wyniki za I półrocze 2026",
  unread: true,
  saved: false,
  sourceUrl: "https://example.test/source",
  language: "pl",
  publishedAt: "2026-06-08T20:12:13Z",
  fetchedAt: "2026-06-14T08:20:00Z",
  attribution: "Bankier Company Komunikaty",
  summary: "Półroczne wyniki finansowe wraz z raportem z przeglądu.",
  bodyText: "Treść raportu okresowego dostępna do podglądu.",
  attachments: [
    { id: "att_results_1", label: "H1_25_26_Sprawozdanie Zarządu.pdf", url: "https://example.test/H1_25_26_Sprawozdanie_Zarzadu.pdf" },
    { id: "att_results_2", label: "H1_25_26_Sprawozdanie_finansowe.pdf", url: "https://example.test/H1_25_26_Sprawozdanie_finansowe.pdf" },
    { id: "att_results_3", label: "AB S.A._31.03.2026_Raport z przeglądu śródrocznego skróconego JSF_MSSF.pdf", url: "https://example.test/AB_JSF_MSSF.pdf" },
    { id: "att_results_4", label: "GK AB_31.03.2026_Raport z przeglądu śródrocznego skróconego SSF_MSSF.pdf", url: "https://example.test/GK_AB_SSF_MSSF.pdf" },
  ],
});

// Typed ESPI signals for layout smoke: one confirmed rule signal and one
// AI proposal so the feed badges and the detail-pane confirm/reject render.
const companySignals: CompanySignal[] = [
  {
    id: "signal_feed_results_report_dividend",
    companyId: "company_gpw_cdr",
    company: "GPW:CDR",
    companyName: "CD PROJEKT S.A.",
    feedItemId: "feed_results_report",
    category: "dividend",
    categoryDisplayName: "Dividend",
    confidence: 0.92,
    classifiedBy: "rule",
    status: "confirmed",
    signalDate: "2026-06-08",
    providerId: null,
    modelId: null,
    derivedEventId: null,
    title: "CD PROJEKT S.A. — wyniki za I półrocze 2026",
    sourceUrl: "https://example.test/source",
    createdAt: "2026-06-08T20:12:13Z",
    updatedAt: "2026-06-08T20:12:13Z",
  },
  {
    id: `signal_${feedItems[0].id}_guidance_change`,
    companyId: companies[0].id,
    company: feedItems[0].company,
    companyName: companies[0].displayName,
    feedItemId: feedItems[0].id,
    category: "guidance_change",
    categoryDisplayName: "Guidance change",
    confidence: 0.74,
    classifiedBy: "ai",
    status: "proposed",
    signalDate: "2026-06-05",
    providerId: "provider_gemini",
    modelId: "gemini-2.5-flash",
    derivedEventId: null,
    title: feedItems[0].title,
    sourceUrl: "https://example.test/source",
    createdAt: "2026-06-05T09:15:00Z",
    updatedAt: "2026-06-05T09:15:00Z",
  },
];

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
    generalAnalysisProvider: "provider_gemini",
    generalAnalysisModel: "gemini-2.5-flash",
    generalAnalysisTimeoutSeconds: 90,
  },
  aiAnalysisMode: "source_grounded",
  espiAiFallbackEnabled: false,
  logs: { level: "info", maxFiles: 5, maxFileBytes: 5_242_880 },
  shortcutBindings: {},
  database: { maxConnections: 4, busyTimeoutMs: 5000, acquireTimeoutMs: 10000 },
  pinnedCompanyIds: [],
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
  configured: true,
  storage: "keychain",
  label: "Gemini API key",
  devFallbackAvailable: false,
  error: null,
};

const localMetricsSnapshot: LocalMetricsSnapshot = {
  collectedAt: "2026-06-05T09:00:00Z",
  samples: [],
};

// ---------------------------------------------------------------------------
// Fundamentals + AI KPI extraction mock state
//
// These power the v0.37 fundamentals panel and the KPI extraction review panel
// in browser-smoke mode. Handlers below are lightly stateful so click-through
// (confirm/reject a proposal, create a period/fact) reflects back into the UI
// the way the real Rust backend does. All data hangs off CD PROJEKT
// (company_gpw_cdr), whose first feed item is an "Official report" with a PDF
// attachment, so both panels are reachable without extra navigation.
// ---------------------------------------------------------------------------

const FUNDAMENTALS_COMPANY_ID = "company_gpw_cdr";
const NOW = "2026-06-05T09:00:00Z";

const kpiDefinitions: KpiDefinition[] = [
  kpiDefinition("def_revenue", "revenue", "Revenue", "monetary", null, "reported"),
  kpiDefinition("def_operating_profit", "operating_profit", "Operating profit", "monetary", null, "reported"),
  kpiDefinition("def_net_profit", "net_profit", "Net profit", "monetary", null, "reported"),
  kpiDefinition("def_ebitda", "ebitda", "EBITDA", "monetary", null, "reported"),
  kpiDefinition("def_eps", "eps", "EPS", "monetary", "per_share", "reported"),
  kpiDefinition("def_gross_margin", "gross_margin", "Gross margin", "percentage", null, "derived"),
];

const financialPeriods: FinancialPeriod[] = [
  financialPeriod("period_cdr_2024_annual", 2024, "annual", "2024-12-31"),
  financialPeriod("period_cdr_2025_q1", 2025, "q1", "2025-03-31"),
  financialPeriod("period_cdr_2025_q2", 2025, "q2", "2025-06-30"),
  financialPeriod("period_cdr_2025_q3", 2025, "q3", "2025-09-30"),
];

// valueNumeric is the raw base-unit integer the DB stores; asReportedValue /
// asReportedScale carry the original as-reported figure (the v0.37 540f931
// formatting work renders these instead of the raw integer).
const financialFacts: FinancialFact[] = [
  financialFact("fact_rev_2024", "period_cdr_2024_annual", "def_revenue", "1093600000", "1 093,6", "mln"),
  financialFact("fact_rev_q1", "period_cdr_2025_q1", "def_revenue", "228400000", "228,4", "mln"),
  financialFact("fact_rev_q2", "period_cdr_2025_q2", "def_revenue", "251900000", "251,9", "mln"),
  financialFact("fact_rev_q3", "period_cdr_2025_q3", "def_revenue", "319700000", "319,7", "mln"),
  financialFact("fact_np_2024", "period_cdr_2024_annual", "def_net_profit", "481100000", "481,1", "mln"),
  financialFact("fact_np_q3", "period_cdr_2025_q3", "def_net_profit", "112400000", "112,4", "mln"),
  financialFact("fact_eps_2024", "period_cdr_2024_annual", "def_eps", "478", "4,78", "", "PLN"),
  financialFact("fact_eps_q3", "period_cdr_2025_q3", "def_eps", "112", "1,12", "", "PLN"),
];

const kpiRelevance: KpiRelevance[] = [
  kpiRelevanceEntry("rel_revenue", "def_revenue", "primary"),
  kpiRelevanceEntry("rel_net_profit", "def_net_profit", "primary"),
  kpiRelevanceEntry("rel_eps", "def_eps", "secondary"),
];

const reportDocuments: ReportDocument[] = [
  reportDocument(
    "doc_cdr_q3_2025",
    "period_cdr_2025_q3",
    "26_06_17_formularz_do_wykonywana_prawa_glosu_przez_pelnomocnika_na_NWZ_cyber_Folks_S.A._w_dn._20.07.2026.pdf",
    "https://example.test/reports/CDPROJEKT_Q3_2025.pdf",
  ),
];

const irReportsUrls: Record<string, string> = {
  [FUNDAMENTALS_COMPANY_ID]: "https://www.cdprojekt.com/en/investors/financial-reports/",
};

// Extraction jobs keyed by report-document id.
const extractionJobs: Record<string, KpiExtractionJob[]> = {};

function seedExtractionJob(reportDocumentId: string): KpiExtractionJob {
  const jobId = `job_${reportDocumentId}`;
  const job: KpiExtractionJob = {
    id: jobId,
    companyId: FUNDAMENTALS_COMPANY_ID,
    reportDocumentId,
    providerId: "provider_gemini",
    model: "gemini-2.5-flash",
    promptVersion: "kpi-extraction.v1",
    periodHint: null,
    status: "succeeded",
    errorCode: null,
    error: null,
    detectedFiscalYear: 2025,
    detectedPeriodType: "Q3",
    detectedPeriodEndDate: "2025-09-30",
    detectedCurrency: "PLN",
    detectedLanguage: "pl",
    createdAt: NOW,
    startedAt: NOW,
    finishedAt: NOW,
    proposals: [
      proposal(jobId, "p_revenue", "revenue", "Revenue", "319700000", "319,7", "mln", "high", false,
        "Przychody ze sprzedaży wyniosły 319,7 mln zł w III kwartale 2025 r."),
      proposal(jobId, "p_net_profit", "net_profit", "Net profit", "112400000", "112,4", "mln", "high", false,
        "Zysk netto grupy kapitałowej wyniósł 112,4 mln zł."),
      proposal(jobId, "p_ebitda", "ebitda", "EBITDA", "168200000", "168,2", "mln", "medium", false,
        "EBITDA na poziomie 168,2 mln zł."),
      proposal(jobId, "p_backlog", "backlog", "Wishlist additions", "2400000", "2,4", "mln", "medium", true,
        "Liczba dodań do listy życzeń wyniosła 2,4 mln."),
    ],
  };
  extractionJobs[reportDocumentId] = [job];
  return job;
}


const embeddingModelStatus: EmbeddingModelStatus = {
  modelId: "intfloat/multilingual-e5-small",
  dim: 384,
  weightsState: "ready",
  downloadProgress: null,
  downloadError: null,
  activeSimilarityStrategy: "embedding",
  embeddedCounts: [{ contentType: "feed_item", count: 374 }],
  indexModelId: "intfloat/multilingual-e5-small",
  featureCompiled: true,
};


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

function kpiDefinition(
  id: string,
  metricKey: string,
  label: string,
  valueKind: string,
  unit: string | null,
  computation: string,
): KpiDefinition {
  return {
    id,
    // Matches the real seed: canonical KPIs are scope "canonical", not "global".
    scope: "canonical",
    companyId: null,
    sector: null,
    metricKey,
    label,
    valueKind,
    unit,
    computation,
    formula: computation === "derived" ? "gross_profit / revenue" : null,
    displayFormat: valueKind === "monetary" ? "millions" : null,
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function financialPeriod(
  id: string,
  fiscalYear: number,
  periodType: string,
  periodEndDate: string,
): FinancialPeriod {
  return {
    id,
    companyId: FUNDAMENTALS_COMPANY_ID,
    fiscalYear,
    periodType,
    periodEndDate,
    reportEvidenceRef: null,
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function financialFact(
  id: string,
  periodId: string,
  definitionId: string,
  valueNumeric: string,
  asReportedValue: string,
  asReportedScale: string,
  currency = "PLN",
): FinancialFact {
  return {
    id,
    companyId: FUNDAMENTALS_COMPANY_ID,
    periodId,
    definitionId,
    valueNumeric,
    currency,
    statementBasis: "consolidated",
    attribution: "total",
    variant: "actual",
    measureWindow: "period",
    dataQuality: "final",
    asReportedValue,
    asReportedScale,
    reportingStandard: "IFRS",
    extractionMethod: "ai_confirmed",
    confidence: "high",
    confirmationState: "confirmed",
    supersedesId: null,
    sourceDocumentRef: "doc_cdr_q3_2025",
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function kpiRelevanceEntry(id: string, definitionId: string, rank: string): KpiRelevance {
  return {
    id,
    companyId: FUNDAMENTALS_COMPANY_ID,
    definitionId,
    status: "active",
    source: "ai_extraction",
    rank,
    firstSeenPeriod: "period_cdr_2024_annual",
    lastSeenPeriod: "period_cdr_2025_q3",
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function reportDocument(
  id: string,
  periodId: string,
  title: string,
  url: string,
): ReportDocument {
  return {
    id,
    companyId: FUNDAMENTALS_COMPANY_ID,
    periodId,
    sourceType: "espi_attachment",
    originRef: "feed_company_gpw_cdr",
    url,
    localPath: `${id}.pdf`,
    contentType: "application/pdf",
    contentHash: id,
    byteSize: 482_311,
    title,
    attribution: "ESPI",
    fetchStatus: "fetched",
    fetchError: null,
    fetchedAt: NOW,
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function proposal(
  jobId: string,
  id: string,
  metricKey: string,
  label: string,
  valueNumeric: string,
  asReportedValue: string,
  asReportedScale: string,
  confidence: string,
  isProposedKpi: boolean,
  sourceSnippet: string,
): KpiExtractionProposal {
  return {
    id,
    jobId,
    metricKey,
    label,
    valueNumeric,
    unit: metricKey === "eps" ? "PLN" : null,
    currency: "PLN",
    asReportedValue,
    asReportedScale,
    measureWindow: "period",
    confidence,
    sourceSnippet,
    isProposedKpi,
    status: "pending",
    factId: null,
    createdAt: NOW,
    updatedAt: NOW,
  };
}

// ---------------------------------------------------------------------------
// Adapter onto the canonical mock runtime (ADR 0048 Keystone B/C, Radicle 749a5a8)
//
// The browser-smoke layer no longer carries its own command router: it builds
// the ONE canonical `createMockRuntime`, injects the browser-specific seed above
// into its store, and routes every IPC call through `runtime.invoke`. Only the
// Tauri plugin commands (path/dialog/fs/opener) are handled locally. A
// `window.__brawlerMockReset(scenario)` hook lets a Playwright test re-seed
// between interactions.
// ---------------------------------------------------------------------------

let activeRuntime: ReturnType<typeof createMockRuntime> | null = null;

function seedBrowserStore(data: ScenarioData) {
  data.companies = structuredClone(companies);
  data.watchlists = structuredClone(watchlists);
  data.watchlistMemberships = structuredClone(watchlistMemberships);
  data.feedItems = structuredClone(feedItems);
  data.signals = structuredClone(companySignals);
  data.sourceAdapters = structuredClone(sourceAdapters);
  data.registry = structuredClone(registryEntries);
  data.notebookEntries = structuredClone(notebookEntries);
  data.events = structuredClone(companyEvents);
  data.settings = structuredClone(settings);
  data.licenseStatus = structuredClone(licenseStatus);
  data.credentialStatuses = [structuredClone(credentialStatus)];
  data.metricsSnapshot = structuredClone(localMetricsSnapshot);
  data.embeddingModelStatus = structuredClone(embeddingModelStatus);
  data.kpiDefinitions = structuredClone(kpiDefinitions);
  data.financialPeriods = structuredClone(financialPeriods);
  data.financialFacts = structuredClone(financialFacts);
  data.kpiRelevance = structuredClone(kpiRelevance);
  data.reportDocuments = structuredClone(reportDocuments);
  data.kpiExtractionJobs = [structuredClone(seedExtractionJob("doc_cdr_q3_2025"))];
  data.irResolutions = [
    {
      document: null,
      candidates: [
        { url: "https://www.cdprojekt.com/en/wp-content/uploads/CDPROJEKT_Q3_2025.pdf", label: "Q3 2025 consolidated report" },
        { url: "https://www.cdprojekt.com/en/wp-content/uploads/CDPROJEKT_H1_2025.pdf", label: "H1 2025 report" },
      ],
      pickedUrl: null,
      confidence: "low",
    },
  ];
  data.transcriptJobs = [];
  data.databaseStatus = {
    appliedMigrations: 29,
    companies: companies.length,
    sourceAdapters: sourceAdapters.length,
    settings: 12,
  };
}

function seedIrReportUrls(runtime: ReturnType<typeof createMockRuntime>) {
  for (const [companyId, url] of Object.entries(irReportsUrls)) {
    void runtime.invoke("set_company_ir_reports_url", { input: { companyId, url } });
  }
}

export function installBrowserSmokeRuntime() {
  // Allow `?locale=pl` to preview the app in Polish (used by UI screenshot specs).
  const requestedLocale = new URLSearchParams(window.location.search).get("locale");
  if (requestedLocale === "pl" || requestedLocale === "en") {
    settings.locale = requestedLocale;
  }
  const runtime = createMockRuntime("rich");
  seedBrowserStore(runtime.data);
  seedIrReportUrls(runtime);
  activeRuntime = runtime;
  (window as unknown as { __brawlerMockReset?: (scenario?: ScenarioName) => void }).__brawlerMockReset = (scenario) => {
    runtime.reset(scenario ?? "rich");
    seedBrowserStore(runtime.data);
    seedIrReportUrls(runtime);
  };
  mockIPC((command, args) => dispatch(command, args as InvokeArgs), { shouldMockEvents: true });
}

function dispatch(command: string, args: InvokeArgs): unknown {
  switch (command) {
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
      return activeRuntime
        ? activeRuntime.invoke(command, args as Record<string, unknown> | undefined)
        : null;
  }
}
