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
import type { FactProvenance } from "../api/generated/FactProvenance";
import type { ReportDocument } from "../api/reportDocumentsTypes";
import type { AutopilotRun } from "../api/autopilot";
import type { CockpitLayout } from "../api/generated/CockpitLayout";
import {
  createMockRuntime,
  type InvocationMatch,
  type PendingInvocation,
} from "./scenarios/runtime";
import type {
  ScenarioData,
  ScenarioName,
  ScenarioSpec,
} from "./scenarios/scenarios";
import {
  applyScenarioOverlays,
  type ScenarioOverlayName,
} from "./scenarios/overlays";
import { makeCompanyPeriodsComparison, makeKpiComparison, makeProfileComparison, makeReconciliationResult, presentationKindFor } from "./scenarios/entities";
import type { CommandError } from "../api/generated/CommandError";

type InvokeArgs = Record<string, unknown> | undefined;

const companies: Company[] = [
  company("company_gpw_cdr", "GPW", "CDR", "CD PROJEKT S.A.", "PLOPTTC00011"),
  company("company_gpw_pkn", "GPW", "PKN", "ORLEN S.A.", "PLPKN0000018"),
  company(
    "company_gpw_kgh",
    "GPW",
    "KGH",
    "KGHM POLSKA MIEDZ S.A.",
    "PLKGHM000017",
  ),
  company("company_gpw_pzu", "GPW", "PZU", "PZU S.A.", "PLPZU0000011"),
  company("company_gpw_dnp", "GPW", "DNP", "DINO POLSKA S.A.", "PLDINPL00011"),
  company(
    "company_gpw_acp",
    "GPW",
    "ACP",
    "ASSECO POLAND S.A.",
    "PLSOFTB00016",
  ),
  company(
    "company_gpw_bft",
    "GPW",
    "BFT",
    "BENEFIT SYSTEMS S.A.",
    "PLBNFTS00107",
  ),
  company(
    "company_gpw_cbf",
    "GPW",
    "CBF",
    "CYFROWY POLSAT S.A.",
    "PLCFRPT00013",
  ),
  company("company_gpw_cmp", "GPW", "CMP", "COMPREMUM S.A.", "PLCOMP000001"),
  company(
    "company_gpw_nc4",
    "NC",
    "4MB",
    "4MOBILITY SPOLKA AKCYJNA",
    "PLESLTN00010",
  ),
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
  {
    id: "watchlist_main_gpw",
    name: "Main GPW",
    description: null,
    companyCount: 6,
  },
  {
    id: "watchlist_followups",
    name: "Follow-up",
    description: null,
    companyCount: 3,
  },
];

const watchlistMemberships: WatchlistMembership[] = [
  ...companies
    .slice(0, 16)
    .map((entry) => membership("watchlist_main_gpw", "Main GPW", entry.id)),
  membership("watchlist_followups", "Follow-up", "company_gpw_cdr"),
  membership("watchlist_followups", "Follow-up", "company_gpw_acp"),
  membership("watchlist_followups", "Follow-up", "company_gpw_bft"),
];

const feedItems: FeedItem[] = companies.slice(0, 7).map((entry, index) => ({
  id: `feed_${entry.id}`,
  company: entry.qualifiedTicker,
  type: index % 2 === 0 ? "Official report" : "News",
  presentationKind: presentationKindFor(index % 2 === 0 ? "Official report" : "News", true),
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
  summary:
    "Deterministic source item used by browser UI regression smoke tests.",
  bodyText: "Longer body text remains available for detail-pane layout checks.",
  attachments: [
    {
      id: `attachment_${entry.id}`,
      label: "report.pdf",
      url: "https://example.test/report.pdf",
    },
  ],
}));

// A realistic "results" report with several long-named PDF attachments, used to
// check the extraction panel's source-button layout under real-world filenames.
feedItems.push({
  id: "feed_results_report",
  company: "GPW:CDR",
  type: "Official report",
  presentationKind: presentationKindFor("Official report", true),
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
    {
      id: "att_results_1",
      label: "H1_25_26_Sprawozdanie Zarządu.pdf",
      url: "https://example.test/H1_25_26_Sprawozdanie_Zarzadu.pdf",
    },
    {
      id: "att_results_2",
      label: "H1_25_26_Sprawozdanie_finansowe.pdf",
      url: "https://example.test/H1_25_26_Sprawozdanie_finansowe.pdf",
    },
    {
      // URL-shaped label (real bankier shape): the human title must come from
      // the decoded basename, never the mangled full URL (sol F1 finding 7/9).
      id: "att_results_url_label",
      label:
        "https://example.test/static/att/emitent/2026-08/20260820_054808_0123456789_PZU_SA_Raport_z_przegladu_srodrocznego_JSF_2026_sigAB.pdf",
      url: "https://example.test/static/att/emitent/2026-08/20260820_054808_0123456789_PZU_SA_Raport_z_przegladu_srodrocznego_JSF_2026_sigAB.pdf",
    },
    {
      id: "att_results_3",
      label:
        "AB S.A._31.03.2026_Raport z przeglądu śródrocznego skróconego JSF_MSSF.pdf",
      url: "https://example.test/AB_JSF_MSSF.pdf",
    },
    {
      id: "att_results_4",
      label:
        "GK AB_31.03.2026_Raport z przeglądu śródrocznego skróconego SSF_MSSF.pdf",
      url: "https://example.test/GK_AB_SSF_MSSF.pdf",
    },
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
  // Analyst-recommendation change (v0.58 A3, ADR 0073): a rule-classified,
  // confirmed signal so the `recommendation_change` badge renders in feed/Inbox.
  {
    id: "signal_feed_results_report_recommendation_change",
    companyId: "company_gpw_cdr",
    company: "GPW:CDR",
    companyName: "CD PROJEKT S.A.",
    feedItemId: "feed_results_report",
    category: "recommendation_change",
    categoryDisplayName: "Analyst recommendation change",
    confidence: 1.0,
    classifiedBy: "rule",
    status: "confirmed",
    signalDate: "2026-06-18",
    providerId: null,
    modelId: null,
    derivedEventId: null,
    title: "CD PROJEKT S.A. — Noble Securities: akumuluj (cel 250,00 zł)",
    sourceUrl: "https://www.biznesradar.pl/rekomendacje-spolki/CDR",
    createdAt: "2026-06-18T08:40:00Z",
    updatedAt: "2026-06-18T08:40:00Z",
  },
];

const sourceAdapters: SourceAdapter[] = [
  sourceAdapter(
    "gpw-company-registry",
    "GPW Company Directory",
    "company_registry",
    "required",
    true,
    ["GPW"],
    470,
  ),
  sourceAdapter(
    "newconnect-company-directory",
    "NewConnect Company Directory",
    "company_registry",
    "required",
    true,
    ["NEWCONNECT"],
    350,
  ),
  sourceAdapter(
    "bankier-company-komunikaty",
    "Bankier Company Komunikaty",
    "official_report",
    "optional",
    true,
    ["GPW"],
    7,
  ),
  sourceAdapter(
    "bankier-market-rss",
    "Bankier Gielda RSS",
    "public_media",
    "optional",
    false,
    ["GPW"],
    3,
  ),
  sourceAdapter(
    "gpw-market-events-rss",
    "GPW Market Events RSS",
    "official_calendar",
    "optional",
    true,
    ["GPW"],
    5,
  ),
  sourceAdapter(
    "bankier-kalendarium-html",
    "Bankier Kalendarium",
    "public_calendar",
    "optional",
    true,
    ["GPW"],
    4,
  ),
  sourceAdapter(
    "portal-analiz",
    "Portal Analiz",
    "authenticated_research",
    "developer",
    false,
    ["GPW"],
    null,
  ),
];

const registryEntries: CompanyRegistryEntry[] = companies.map((entry) => ({
  sourceAdapterId:
    entry.exchange === "NC"
      ? "newconnect-company-directory"
      : "gpw-company-registry",
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

const companyEvents: CompanyEvent[] = companies
  .slice(0, 4)
  .map((entry, index) => ({
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

// KNF short-selling register (v0.55 T4b). CD PROJEKT carries active positions +
// change history (populated panel); ORLEN carries only a remembered exit (the
// common empty state). Dates sit inside the mock's 30-day window (SAMPLE_NOW =
// 2026-06-08) so the "changed" chip + non-zero delta render.
const shortPositions = [
  {
    companyId: "company_gpw_cdr",
    holderName: "Qube Research & Technologies Ltd",
    netPositionPct: 1.81,
    positionDate: "2026-06-01",
    exitedAt: null as string | null,
  },
  {
    companyId: "company_gpw_cdr",
    holderName: "Marshall Wace LLP",
    netPositionPct: 0.59,
    positionDate: "2026-04-15",
    exitedAt: null as string | null,
  },
  {
    companyId: "company_gpw_pkn",
    holderName: "Point72 Asset Management",
    netPositionPct: 0.62,
    positionDate: "2024-11-03",
    exitedAt: "2024-11-03T06:30:00Z" as string | null,
  },
];

// Red-flags panel state (v0.57 T7, ADR 0083 D8). CD PROJEKT carries two active
// flags (a high auditor flag with an evidence item + a medium fund-exit) and one
// acknowledged flag in history — exercising the severity chips, the evidence
// "Open" affordance, the acknowledge flow, and the collapsed history group.
const redFlagsByCompany: NonNullable<ScenarioData["redFlagsByCompany"]> = {
  company_gpw_cdr: {
    active: [
      {
        flagId: "rf:auditor_red_flag:company_gpw_cdr:fi_cdr_audit",
        flagType: "auditor_red_flag",
        severity: "high",
        title: "Opinia biegłego rewidenta z zastrzeżeniem",
        raisedDate: "2026-05-01",
        evidenceUrl: "https://example.test/audit",
        evidenceFeedItemId: "feed_cdr_1",
        ackedAt: null,
      },
      {
        flagId: "rf:fund_exit:company_gpw_cdr:beta@2026-03-31",
        flagType: "fund_exit",
        severity: "medium",
        title: "Wyjście z akcjonariatu: Nationale-Nederlanden OFE",
        raisedDate: "2026-03-31",
        evidenceUrl: null,
        evidenceFeedItemId: null,
        ackedAt: null,
      },
    ],
    history: [
      {
        flagId: "rf:report_delay:company_gpw_cdr:evt_old",
        flagType: "report_delay",
        severity: "high",
        title: "Opóźniony raport okresowy: Raport za IV kwartał",
        raisedDate: "2026-02-01",
        evidenceUrl: null,
        evidenceFeedItemId: null,
        ackedAt: "2026-02-10T09:00:00Z",
      },
    ],
  },
};

// Analyst recommendations (v0.58 A3, ADR 0073). CD PROJEKT carries an attributed
// history — an upgrade with a target (+ broker PDF), an initiate that keeps a
// target, and a partial entry with neither target nor PDF — for the panel journey
// and narrow-pane overflow assertion. ORLEN has none (empty-state coverage).
const analystRecommendationsByCompany: NonNullable<
  ScenarioData["analystRecommendationsByCompany"]
> = {
  company_gpw_cdr: {
    companyId: "company_gpw_cdr",
    entries: [
      {
        firm: "Noble Securities",
        analyst: "Mateusz Chrzanowski",
        rating: "akumuluj",
        ratingPrev: "trzymaj",
        direction: "upgrade",
        targetPrice: "250.00",
        targetCurrency: "PLN",
        targetPrev: "230.00",
        priceAtIssue: "232.00",
        publishedAt: "2026-06-18T08:40:00",
        reportUrl: "https://static.example/rec/cdr-noble-2026-06.pdf",
        sourceUrl: "https://www.biznesradar.pl/rekomendacje-spolki/CDR",
      },
      {
        firm: "BOŚ DM",
        analyst: "Tomasz Rodak",
        rating: "trzymaj",
        ratingPrev: null,
        direction: "initiate",
        targetPrice: "270.00",
        targetCurrency: "PLN",
        targetPrev: null,
        priceAtIssue: "228.00",
        publishedAt: "2026-02-27T07:30:00",
        reportUrl: "https://static.example/rec/cdr-bos-2026-02.pdf",
        sourceUrl: "https://www.biznesradar.pl/rekomendacje-spolki/CDR",
      },
      {
        firm: "BM mBank",
        analyst: null,
        rating: "trzymaj",
        ratingPrev: null,
        direction: "initiate",
        targetPrice: null,
        targetCurrency: null,
        targetPrev: null,
        priceAtIssue: null,
        publishedAt: "2025-11-26T00:00:00",
        reportUrl: null,
        sourceUrl: "https://www.biznesradar.pl/rekomendacje-spolki/CDR",
      },
    ],
    latestTarget: {
      firm: "Noble Securities",
      targetPrice: "250.00",
      targetCurrency: "PLN",
      publishedAt: "2026-06-18T08:40:00",
    },
    lastRefreshedAt: "2026-07-19T08:12:00Z",
  },
};

const shortPositionEvents = [
  {
    companyId: "company_gpw_cdr",
    kind: "increased" as const,
    holderName: "Qube Research & Technologies Ltd",
    fromPct: 1.49,
    toPct: 1.81,
    positionDate: "2026-06-01",
  },
  {
    companyId: "company_gpw_cdr",
    kind: "entered" as const,
    holderName: "Qube Research & Technologies Ltd",
    fromPct: null as number | null,
    toPct: 1.49,
    positionDate: "2026-05-20",
  },
  {
    companyId: "company_gpw_cdr",
    kind: "exited" as const,
    holderName: "AQR Capital Management",
    fromPct: 0.55,
    toPct: null as number | null,
    positionDate: "2026-05-12",
  },
  {
    companyId: "company_gpw_cdr",
    kind: "entered" as const,
    holderName: "Marshall Wace LLP",
    fromPct: null as number | null,
    toPct: 0.59,
    positionDate: "2026-04-15",
  },
];

const settings: UserSettings = {
  theme: "dark",
  locale: "en",
  accentPalette: "night-neon",
  developerMode: false,
  pollIntervalSeconds: 900,
  backfillYears: 3,
  settingsSource: "browser-smoke",
  settingsImportExportFormat: "yaml",
  yamlImportExportStatus: "accepted_deferred",
  aiProviders: {
    youtubeTranscriptionProvider: "provider_gemini",
    youtubeTranscriptionModel: "gemini-2.5-flash",
    youtubeTranscriptionTimeoutSeconds: 300,
  },
  logs: { level: "info", maxFiles: 5, maxFileBytes: 5_242_880 },
  shortcutBindings: {},
  database: { maxConnections: 4, busyTimeoutMs: 5000, acquireTimeoutMs: 10000 },
  queue: { sourcesWorkers: 2, autopilotWorkers: 3 },
  pinnedCompanyIds: [],
  todayReviewedDays: [],
  mcp: { enabled: false, port: 8317, writesEnabled: false, kpiAcquisitionEnabled: false },
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
  kpiDefinition(
    "def_revenue",
    "revenue",
    "Revenue",
    "monetary",
    null,
    "reported",
  ),
  kpiDefinition(
    "def_operating_profit",
    "operating_profit",
    "Operating profit",
    "monetary",
    null,
    "reported",
  ),
  kpiDefinition(
    "def_net_profit",
    "net_profit",
    "Net profit",
    "monetary",
    null,
    "reported",
  ),
  kpiDefinition("def_ebitda", "ebitda", "EBITDA", "monetary", null, "reported"),
  kpiDefinition("def_eps", "eps", "EPS", "monetary", "per_share", "reported"),
  kpiDefinition(
    "def_gross_margin",
    "gross_margin",
    "Gross margin",
    "percentage",
    null,
    "derived",
  ),
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
  financialFact(
    "fact_rev_2024",
    "period_cdr_2024_annual",
    "def_revenue",
    "revenue",
    "1093600000",
    "1 093,6",
    "mln",
  ),
  financialFact(
    "fact_rev_q1",
    "period_cdr_2025_q1",
    "def_revenue",
    "revenue",
    "228400000",
    "228,4",
    "mln",
  ),
  financialFact(
    "fact_rev_q2",
    "period_cdr_2025_q2",
    "def_revenue",
    "revenue",
    "251900000",
    "251,9",
    "mln",
  ),
  financialFact(
    "fact_rev_q3",
    "period_cdr_2025_q3",
    "def_revenue",
    "revenue",
    "319700000",
    "319,7",
    "mln",
  ),
  financialFact(
    "fact_np_2024",
    "period_cdr_2024_annual",
    "def_net_profit",
    "net_profit",
    "481100000",
    "481,1",
    "mln",
  ),
  financialFact(
    "fact_np_q3",
    "period_cdr_2025_q3",
    "def_net_profit",
    "net_profit",
    "112400000",
    "112,4",
    "mln",
  ),
  financialFact(
    "fact_eps_2024",
    "period_cdr_2024_annual",
    "def_eps",
    "eps",
    "478",
    "4,78",
    "",
    "PLN",
  ),
  financialFact(
    "fact_eps_q3",
    "period_cdr_2025_q3",
    "def_eps",
    "eps",
    "112",
    "1,12",
    "",
    "PLN",
  ),
];

// Structured-first extraction provenance (ADR 0061): Revenue Q3 is a clean,
// validated ESEF fact (tier + validation badges, no drift); Net profit Q3 is a
// flagged PDF parse whose layout drifted from the profile (renders the "structure
// changed" diff card). Facts without a row (EPS, older periods) show no badges —
// the legacy/unvalidated default.
const factProvenance: FactProvenance[] = [
  {
    factId: "fact_rev_q3",
    sourceTier: "esef",
    validationStatus: "passed",
    driftJson: null,
    citation: "Przychody ze sprzedaży",
    // Positively corroborated by the aggregator (epic #229 T5): the issuer's
    // figure and the independent web read agree.
    witnessValue: "412000000",
    witnessPageUrl: "https://www.biznesradar.pl/raporty-finansowe-rachunek-zyskow-i-strat/CDR",
    corroboratedAt: "2026-07-29T06:12:00Z",
  },
  {
    factId: "fact_np_q3",
    sourceTier: "pdf",
    validationStatus: "flagged",
    driftJson: JSON.stringify({
      added_labels: ["Zysk netto z działalności kontynuowanej"],
      removed_labels: ["Zysk netto"],
      unit_changed: null,
    }),
    citation: null,
    // Never corroborated — absence is "no witness", never "disagreed".
    witnessValue: null,
    witnessPageUrl: null,
    corroboratedAt: null,
  },
];

// Flagged extraction outcomes (ADR 0061 decision 2, ADR 0084 decision 4/6) — the
// Coverage panel's "Flagged periods" section. Three shapes on purpose, so the
// browser preview + narrow-pane overflow assertion see every visual variant:
// a validation failure WITH failing-check detail, a drift-flagged period (the
// "Layout changed" chip), and the SENTINEL period of a `no_period_derived`
// failure (fiscalYear 0 / empty periodType — renders as "Period unknown", never
// as a real period). ORLEN has none, covering the reassuring empty state.
const flaggedExtractionOutcomes: NonNullable<
  ScenarioData["flaggedExtractionOutcomes"]
> = [
  {
    id: "outcome_cdr_2025_h1",
    companyId: FUNDAMENTALS_COMPANY_ID,
    reportDocumentId: "doc_cdr_h1_2025",
    fiscalYear: 2025,
    periodType: "H1",
    periodEnd: "2025-06-30",
    tier: "pdf",
    acceptance: "flagged",
    reasonCode: "validation_failed",
    detailJson: JSON.stringify({
      check: "aktywa = zobowiązania + kapitał własny",
      expected: "12 480 100",
      actual: "12 468 900",
      residual: "11 200",
    }),
    driftJson: null,
    structureChanged: false,
    factCount: 0,
    attemptCount: 2,
    firstAttemptedAt: "2026-07-02T08:15:00Z",
    lastAttemptedAt: "2026-07-14T09:30:00Z",
  },
  {
    id: "outcome_cdr_2024_annual",
    companyId: FUNDAMENTALS_COMPANY_ID,
    reportDocumentId: "doc_cdr_annual_2024",
    fiscalYear: 2024,
    periodType: "annual",
    periodEnd: "2024-12-31",
    tier: "esef",
    acceptance: "flagged",
    reasonCode: "structure_drift",
    detailJson: JSON.stringify({
      added_labels: ["Zysk netto z działalności kontynuowanej"],
      removed_labels: ["Zysk netto"],
    }),
    driftJson: JSON.stringify({ unit_changed: null }),
    structureChanged: true,
    factCount: 0,
    attemptCount: 1,
    firstAttemptedAt: "2026-07-10T11:00:00Z",
    lastAttemptedAt: "2026-07-10T11:00:00Z",
  },
  {
    id: "outcome_cdr_no_period",
    companyId: FUNDAMENTALS_COMPANY_ID,
    reportDocumentId: "doc_cdr_unclassified",
    // SENTINEL period: a period-less failure has no slot key.
    fiscalYear: 0,
    periodType: "",
    periodEnd: "",
    tier: null,
    acceptance: "flagged",
    reasonCode: "no_period_derived",
    detailJson: null,
    driftJson: null,
    structureChanged: false,
    factCount: 0,
    attemptCount: 1,
    firstAttemptedAt: "2026-07-08T07:45:00Z",
    lastAttemptedAt: "2026-07-08T07:45:00Z",
  },
];

const kpiRelevance: KpiRelevance[] = [
  kpiRelevanceEntry("rel_revenue", "def_revenue", "primary"),
  kpiRelevanceEntry("rel_net_profit", "def_net_profit", "primary"),
  kpiRelevanceEntry("rel_eps", "def_eps", "secondary"),
];

// One autopilot run (ADR 0055), so the Today/Pulse "Autopilot" card — and its
// Undo action — render in the visual browser-smoke preview, not just Vitest's
// jsdom tests. Autopilot mode + a non-empty producedFactIds makes it Undo-eligible
// (undo_autopilot_run reverts exactly the facts it produced); reuses fact_np_q3
// (already flagged/drifted in factProvenance above) so the run also renders a
// "Structure changed" section.
const autopilotRuns: AutopilotRun[] = [
  {
    id: "run_cdr_q3_2025",
    companyId: FUNDAMENTALS_COMPANY_ID,
    reportDocumentId: "doc_cdr_q3_2025",
    trigger: "detected",
    sweepId: null,
    mode: "autopilot",
    status: "succeeded",
    stage: "notify",
    // Legacy/fallback only (contracts.md § Autonomous Report Pipeline) — the Today
    // card composes its own localized sentence from kpiDeltaJson/reportDiffRef/
    // crossRefsJson (bug e77a1a2 part 2), never this raw English string.
    summaryText:
      "CD PROJEKT Q3 2025: net profit auto-confirmed from the new report.",
    kpiDeltaJson: JSON.stringify({
      extractionAvailable: true,
      structured: true,
      structureChanged: true,
      // Honest counts (bug e77a1a2 part 1): match producedFactIds below (1 fact).
      factsProposed: 1,
      factsAutoConfirmed: 1,
      driftJson: JSON.stringify({
        added_labels: ["Zysk netto z działalności kontynuowanej"],
        removed_labels: ["Zysk netto"],
        unit_changed: null,
      }),
    }),
    reportDiffRef: null,
    crossRefsJson: null,
    producedFactIds: ["fact_np_q3"],
    notificationState: "unread",
    lastError: null,
    // status `succeeded` → routine (product-spec §Attention Routing / `storage::severity`).
    severity: "routine",
    // The processed report's title (v0.60 D6): the Today row states WHICH report.
    reportDocumentTitle: "CD PROJEKT — Raport kwartalny Q3 2025",
    createdAt: "2026-06-20T09:00:00Z",
    updatedAt: "2026-06-20T09:00:00Z",
  },
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
  [FUNDAMENTALS_COMPANY_ID]:
    "https://www.cdprojekt.com/en/investors/financial-reports/",
};

function company(
  id: string,
  exchange: string,
  ticker: string,
  displayName: string,
  isin: string,
): Company {
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

function membership(
  watchlistId: string,
  watchlistName: string,
  companyId: string,
): WatchlistMembership {
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
    role: "primary",
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

function notebookEntry(
  id: string,
  companyId: string,
  title: string,
  kind: string,
): NotebookEntry {
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
    // Matches the real seed: canonical KPIs backfill origin "seed" (0129).
    origin: "seed",
    statementGroup: "other",
    periodNature: "duration",
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
  metricKey: string,
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
    metricKey,
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
    annotation: null,
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function kpiRelevanceEntry(
  id: string,
  definitionId: string,
  rank: string,
): KpiRelevance {
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
    docKind: "periodic_ssf",
    // Migration 0121 (epic #229 T2): NULL = bytes not sniffed yet.
    detectedContainer: null,
  };
}

// ---------------------------------------------------------------------------
// Adapter onto the canonical mock runtime (ADR 0048 Keystone B/C, Radicle 749a5a8)
//
// The browser-smoke layer no longer carries its own command router: it builds
// the ONE canonical `createMockRuntime`, injects the browser-specific seed above
// into its store, and routes every IPC call through `runtime.invoke`. Only the
// Tauri plugin commands (path/dialog/fs/opener) are handled locally. A typed
// `window.__brawlerMock` bridge (ADR 0081 Q2, Radicle a9992e2) lets a
// Playwright test re-seed (optionally with overlays) and drive controlled
// async between interactions.
//
// Setup order is ALWAYS base scenario -> `seedBrowserStore` (this file's
// browser-specific projection) -> Q2 overlays. Overlays run last on both
// initial install AND every `reset()` so `seedBrowserStore` can never
// silently clobber hostile/dense/partial/stale/conflicting/mixed-locale data
// applied by an overlay.
// ---------------------------------------------------------------------------

let activeRuntime: ReturnType<typeof createMockRuntime> | null = null;

// Source-reconciliation rows for the developer-gated Diagnostics screen. One row
// per status so the reconciliation table exercises all three StatusChip labels —
// the `espi_only` "Missed by primary" / "Pominięte przez główne" chip is the long
// one that overran the fixed severity-sized grid track (bug 228762e); its
// no-overlap layout is asserted in diagnostics-reconciliation-layout.spec.ts.
const reconciliationResults = [
  { ...makeReconciliationResult(), id: "recon_matched", status: "matched" as const, qualifiedTicker: "GPW:CDR" },
  {
    ...makeReconciliationResult(),
    id: "recon_espi_only",
    status: "espi_only" as const,
    qualifiedTicker: "GPW:PKN",
    reportNumber: "22/2026",
    witnessTitle: "ESPI report the primary feed never surfaced.",
  },
  {
    ...makeReconciliationResult(),
    id: "recon_bankier_only",
    status: "bankier_only" as const,
    qualifiedTicker: "GPW:PKO",
    reportNumber: "8/2026",
    witnessTitle: "Bankier-only witness with no ESPI counterpart.",
  },
];

// Comparative valuation L1 (ADR 0089 §B3). CD PROJEKT carries a populated
// 5-peer percentile + 3-method valuation (two drawable methods, P/BV a typed
// absence) so the football field + grade breakdown render in specs/visuals;
// CYFROWY POLSAT carries the honest thin state (N=2 < 4) so the threshold
// message renders. Keyed by companyId; every other company reads the computed
// typed-absence default. Matches the generated B1/B2 DTO shapes exactly.
const sectorPercentilesSeed: NonNullable<ScenarioData["sectorPercentiles"]> = {
  company_gpw_cdr: {
    companyId: "company_gpw_cdr",
    sector: "Technology",
    peerCount: 5,
    thin: false,
    emptyReason: null,
    metrics: [
      { metricKey: "pe_ratio", kind: "market_ratio", value: "18.40", percentile: "37.00", median: "21.10", sampleSize: 5, absentReason: null },
      { metricKey: "ev_ebitda", kind: "market_ratio", value: "12.30", percentile: "52.00", median: "12.00", sampleSize: 5, absentReason: null },
      { metricKey: "pbv_ratio", kind: "market_ratio", value: "3.10", percentile: "61.00", median: "2.60", sampleSize: 5, absentReason: null },
      { metricKey: "roe", kind: "canonical_kpi", value: "0.18", percentile: "48.00", median: "0.17", sampleSize: 5, absentReason: null },
      { metricKey: "fcf_yield", kind: "market_ratio", value: null, percentile: null, median: null, sampleSize: 0, absentReason: "no_company_value" },
    ],
  },
  company_gpw_cbf: {
    companyId: "company_gpw_cbf",
    sector: "Communication",
    peerCount: 2,
    thin: true,
    emptyReason: null,
    metrics: [
      { metricKey: "pe_ratio", kind: "market_ratio", value: "9.20", percentile: null, median: null, sampleSize: 2, absentReason: "insufficient_peers" },
    ],
  },
};

const comparativeValuationsSeed: NonNullable<ScenarioData["comparativeValuations"]> = {
  company_gpw_cdr: {
    companyId: "company_gpw_cdr",
    sector: "Technology",
    peerCount: 5,
    thin: false,
    currentPrice: "132",
    dataAsOf: "2026-06-30",
    emptyReason: null,
    methods: [
      { method: "pe_multiple", driverKey: "net_profit_ttm", driverValue: "481", peerMultipleLow: "16.0", peerMultipleBase: "20.0", peerMultipleHigh: "24.5", fairLow: "96", fairBase: "122", fairHigh: "148", peerSampleSize: 4, absentReason: null },
      { method: "ev_ebitda_multiple", driverKey: "ebitda_ttm", driverValue: "640", peerMultipleLow: "9.0", peerMultipleBase: "11.0", peerMultipleHigh: "13.0", fairLow: "108", fairBase: "131", fairHigh: "154", peerSampleSize: 4, absentReason: null },
      { method: "pbv_multiple", driverKey: "total_equity", driverValue: null, peerMultipleLow: null, peerMultipleBase: null, peerMultipleHigh: null, fairLow: null, fairBase: null, fairHigh: null, peerSampleSize: 0, absentReason: "insufficient_peers" },
    ],
    convergence: { baseLow: "122", baseHigh: "131", spreadPct: "7.11", methodCount: 2 },
    confidence: { grade: "B", composite: "0.72", dataCompleteness: "0.80", peerDepth: "0.60", methodConvergence: "0.70", validation: "0.75" },
  },
  company_gpw_cbf: {
    companyId: "company_gpw_cbf",
    sector: "Communication",
    peerCount: 2,
    thin: true,
    currentPrice: "42",
    dataAsOf: "2026-06-30",
    emptyReason: null,
    methods: [
      { method: "pe_multiple", driverKey: "net_profit_ttm", driverValue: "310", peerMultipleLow: null, peerMultipleBase: null, peerMultipleHigh: null, fairLow: null, fairBase: null, fairHigh: null, peerSampleSize: 1, absentReason: "insufficient_peers" },
      { method: "ev_ebitda_multiple", driverKey: "ebitda_ttm", driverValue: "980", peerMultipleLow: null, peerMultipleBase: null, peerMultipleHigh: null, fairLow: null, fairBase: null, fairHigh: null, peerSampleSize: 1, absentReason: "insufficient_peers" },
      { method: "pbv_multiple", driverKey: "total_equity", driverValue: "5400", peerMultipleLow: null, peerMultipleBase: null, peerMultipleHigh: null, fairLow: null, fairBase: null, fairHigh: null, peerSampleSize: 1, absentReason: "insufficient_peers" },
    ],
    convergence: null,
    confidence: { grade: "D", composite: "0.22", dataCompleteness: "0.50", peerDepth: "0.10", methodConvergence: "0.00", validation: "0.25" },
  },
};

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
  data.kpiDefinitions = structuredClone(kpiDefinitions);
  data.financialPeriods = structuredClone(financialPeriods);
  data.financialFacts = structuredClone(financialFacts);
  data.factProvenance = structuredClone(factProvenance);
  data.flaggedExtractionOutcomes = structuredClone(flaggedExtractionOutcomes);
  data.kpiRelevance = structuredClone(kpiRelevance);
  data.reconciliationResults = structuredClone(reconciliationResults);
  // Populated cross-company comparison (ADR 0089 §A3) over two browser-harness
  // companies exercising the kpi_comparison command (CDR reports in PLN, CBF in
  // EUR — the FX chip + fx_missing flag). Matched by the runtime mock on
  // (companyIds set, metricKeys=["revenue"], granularity="annual").
  // Plus the N=1 CDR comparisons the Fundamentals periods × deltas section
  // (§A5) requests — its own KPI set at annual (default) and quarterly (the
  // wide, overflow-tested view). Matched on (companyIds={CDR}, metricKeys=
  // ["revenue","net_profit","eps"], granularity).
  // The Profil default view (§A7) requests the whole canonical catalog at annual;
  // makeProfileComparison matches it on (companyIds={CDR,CBF}, all six metricKeys,
  // annual). makeKpiComparison still serves the single-metric Trend view.
  data.kpiComparisons = [
    makeProfileComparison("company_gpw_cdr", "company_gpw_cbf"),
    makeKpiComparison("company_gpw_cdr", "company_gpw_cbf"),
    makeCompanyPeriodsComparison("company_gpw_cdr", "annual"),
    makeCompanyPeriodsComparison("company_gpw_cdr", "quarterly"),
  ];
  data.sectorPercentiles = structuredClone(sectorPercentilesSeed);
  data.comparativeValuations = structuredClone(comparativeValuationsSeed);
  data.reportDocuments = structuredClone(reportDocuments);
  data.autopilotRuns = structuredClone(autopilotRuns);
  data.shortPositions = structuredClone(shortPositions);
  data.shortPositionEvents = structuredClone(shortPositionEvents);
  data.redFlagsByCompany = structuredClone(redFlagsByCompany);
  data.analystRecommendationsByCompany = structuredClone(
    analystRecommendationsByCompany,
  );
  data.irResolutions = [
    {
      candidates: [
        {
          url: "https://www.cdprojekt.com/en/wp-content/uploads/CDPROJEKT_Q3_2025.pdf",
          label: "Q3 2025 consolidated report",
        },
        {
          url: "https://www.cdprojekt.com/en/wp-content/uploads/CDPROJEKT_H1_2025.pdf",
          label: "H1 2025 report",
        },
      ],
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
    void runtime.invoke("set_company_ir_reports_url", {
      input: { companyId, url },
    });
  }
}

/** base scenario -> browser projection -> overlays (fixed order, see above). */
function seedAndOverlay(
  runtime: ReturnType<typeof createMockRuntime>,
  overlays: readonly ScenarioOverlayName[],
) {
  seedBrowserStore(runtime.data);
  seedIrReportUrls(runtime);
  if (overlays.length > 0) {
    runtime.data = applyScenarioOverlays(runtime.data, overlays);
  }
}

/**
 * Typed test-only bridge (ADR 0081 Q2, Radicle a9992e2) a Playwright test
 * drives via `page.evaluate`. See `tests/browser/helpers/mockRuntime.ts` for
 * the typed wrapper.
 */
export interface BrawlerMockBridge {
  reset(spec?: ScenarioName | ScenarioSpec): void;
  hold(match: InvocationMatch): string;
  pending(): readonly PendingInvocation[];
  release(id: string): void;
  /** Playwright arguments are JSON-serialized, so only the plain ADR 0070
   * envelope crosses the bridge — never an `Error` instance. */
  reject(id: string, error: CommandError): void;
  releaseAll(): void;
  /** One-shot rejection of the NEXT invocation of `command` (Radicle 5be14c9). */
  failNext(command: string, error: CommandError): void;
  /** Persistent rejection of EVERY invocation of `command` (epic #40 S1). */
  chaos(command: string, error: CommandError): void;
  /** Drop every persistent chaos rule. */
  clearChaos(): void;
  /** Appends a raw `cockpit_layouts` row (F3a S3, ADR 0107 decision 5): the
   * frozen cockpit's "Add panel"/"Save dashboard"/"+ New view" surface is
   * gone, so a browser test can no longer build a named view (with follow
   * panels) through the UI — this seeds one directly, the browser-E2E
   * equivalent of `saveCockpitLayout` in a jsdom component test
   * (paneLandmarks.test.tsx). Must run BEFORE `openApp` (the sidebar's
   * "Views" group reads the layout list once at boot, not reactively). */
  seedCockpitLayout(layout: CockpitLayout): void;
}

declare global {
  interface Window {
    __brawlerMock?: BrawlerMockBridge;
  }
}

// Reads a smoke-appearance override from localStorage, tolerating environments
// where storage access throws (never a reason to fail app boot).
function readSmokeOverride(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

// The closed ADR 0070 code set, as a runtime guard for the `?chaos=` param
// (a URL carries arbitrary text; an unknown code falls back to `internal`).
const COMMAND_ERROR_CODES: readonly CommandError["code"][] = [
  "not_found",
  "invalid_input",
  "missing_credential",
  "network",
  "provider",
  "conflict",
  "writes_disabled",
  "provenance_required",
  "internal",
];

/**
 * `?chaos=<command>[:<code>]` → the persistent rule to install (epic #40 S1).
 * The message NAMES the failing command so a broken read surfaces as a named
 * failure on screen rather than an indistinguishable empty state.
 */
function parseChaosParam(
  raw: string | null,
): { command: string; error: CommandError } | null {
  if (!raw) return null;
  const separator = raw.indexOf(":");
  const command = (separator === -1 ? raw : raw.slice(0, separator)).trim();
  if (!command) return null;
  const requested = separator === -1 ? "" : raw.slice(separator + 1).trim();
  const code = (COMMAND_ERROR_CODES as readonly string[]).includes(requested)
    ? (requested as CommandError["code"])
    : "internal";
  return {
    command,
    error: { code, message: `chaos seam: ${command} fails on every call` },
  };
}

export function installBrowserSmokeRuntime() {
  const params = new URLSearchParams(window.location.search);
  // Allow `?locale=pl` to preview the app in Polish (used by UI screenshot specs).
  const requestedLocale = params.get("locale");
  if (requestedLocale === "pl" || requestedLocale === "en") {
    settings.locale = requestedLocale;
  }
  // Allow `?dev=1` to seed the developer-gated surfaces (Diagnostics) reachable
  // in a browser spec. The real unlock is a hidden chord → passphrase → backend
  // command (mocked here as accept-any), which is racy to drive across the
  // viewport matrix; this test-only override makes Diagnostics deterministically
  // reachable. Harness-only — never present in production code.
  if (params.get("dev") === "1") {
    settings.developerMode = true;
  }
  // Allow the seeded appearance to be forced deterministically, so the Playwright
  // `chromium-compact-light` project can drive the whole suite under the light
  // theme (ADR 0076 D3). Precedence: `?theme=`/`?palette=` query param (dev
  // preview), then a localStorage key set via the project's storageState (survives
  // navigations within the context, which a query param does not). The app then
  // applies data-theme/data-palette from these persisted settings on first render.
  const requestedTheme =
    params.get("theme") ?? readSmokeOverride("brawler:smoke:theme");
  if (
    requestedTheme === "light" ||
    requestedTheme === "dark" ||
    requestedTheme === "system"
  ) {
    settings.theme = requestedTheme;
  }
  const requestedPalette =
    params.get("palette") ?? readSmokeOverride("brawler:smoke:palette");
  if (
    requestedPalette === "night-neon" ||
    requestedPalette === "midnight-horizon"
  ) {
    settings.accentPalette = requestedPalette;
  }
  // Allow `?chaos=<command>[:<code>]` to break a command for the WHOLE session
  // (epic #40 S1, ADR 0091) — parsed with its sibling params, installed below
  // once the runtime exists but strictly before the app's bootstrap reads, so
  // even a once-at-boot fetch sees the failure.
  const chaosRule = parseChaosParam(params.get("chaos"));
  const runtime = createMockRuntime("rich");
  seedAndOverlay(runtime, []);
  if (chaosRule) {
    runtime.chaos(chaosRule.command, chaosRule.error);
  }
  activeRuntime = runtime;
  window.__brawlerMock = {
    reset(spec) {
      const base: ScenarioName =
        spec === undefined
          ? "rich"
          : typeof spec === "string"
            ? spec
            : spec.base;
      const overlays: readonly ScenarioOverlayName[] =
        spec === undefined || typeof spec === "string"
          ? []
          : (spec.overlays ?? []);
      runtime.reset(base);
      seedAndOverlay(runtime, overlays);
    },
    hold: (match) => runtime.controls.hold(match),
    pending: () => runtime.controls.pending(),
    release: (id) => runtime.controls.release(id),
    reject: (id, error) => runtime.controls.reject(id, error),
    releaseAll: () => runtime.controls.releaseAll(),
    failNext: (command, error) => runtime.failNext(command, error),
    chaos: (command, error) => runtime.chaos(command, error),
    clearChaos: () => runtime.clearChaos(),
    seedCockpitLayout: (layout) => {
      runtime.data.cockpitLayouts = [...runtime.data.cockpitLayouts, layout];
    },
  };
  mockIPC((command, args) => dispatch(command, args as InvokeArgs), {
    shouldMockEvents: true,
  });
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
        ? activeRuntime.invoke(
            command,
            args as Record<string, unknown> | undefined,
          )
        : null;
  }
}
