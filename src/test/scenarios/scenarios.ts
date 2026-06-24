// Canonical named scenarios (ADR 0048 Keystone B/C, Radicle 749a5a8).
//
// `buildScenario(name)` composes the deterministic builders in `entities.ts`
// into one in-memory dataset — the single store both the Vitest harness and the
// Playwright browser-smoke runtime materialize. Three named scenarios:
//
//   empty   — singletons only (valid license, default settings, catalogs); every
//             collection empty, so empty-state rendering is exercised.
//   minimal — first 4 companies + ONE of every entity type, so every read/list
//             command returns non-trivial data and every screen renders.
//   rich    — all 28 companies + dense multiples + varied source-adapter health
//             and license kinds, for scale/pagination/filtering coverage.
//
// Each call returns a FRESH deep clone (`structuredClone`), so a test that mutates
// its dataset can never leak into another test or worker.

import type { Company } from "../../api/types";
import type { CockpitLayout } from "../../api/generated/CockpitLayout";
import {
  legacyCompanies,
  legacyCompanyEvents,
  legacyFeedItems,
  legacyGeminiCredential,
  legacyLicenseStatus,
  legacyMetricsSnapshot,
  legacyRegistry,
  legacyResearchEvidence,
  legacyResearchQuestions,
  legacyResearchReminders,
  legacySettings,
  legacySourceAdapters,
  legacyTranscriptJobs,
  legacyTranscriptSegments,
  legacyUnmatchedSourceItems,
  legacyWatchlistMemberships,
  legacyWatchlists,
} from "./legacyMinimal";
import {
  AI_PROVIDER_CATALOG,
  AVAILABLE_METRIC_KEYS,
  COMPANY_SPECS,
  type CompanySpec,
  makeAiAnalysisJob,
  makeBackfillProgress,
  makeBackupStatus,
  makeClaimExtractionJob,
  makeClaimsToVerify,
  makeCompany,
  makeCredentialStatuses,
  makeDatabaseStatus,
  makeDiagnosticEvent,
  makeDiagnosticSummary,
  makeEmbeddingModelStatus,
  makeEvent,
  makeEvidenceLink,
  makeFeedItem,
  makeFrameworkEvaluation,
  makeFinancialFact,
  makeFinancialPeriod,
  makeIrReportResolution,
  makeKpiDefinition,
  makeKpiExtractionJob,
  makeKpiRelevance,
  makeLicenseStatus,
  makeLocalMetricsSnapshot,
  makeLogEntry,
  makeLogStatus,
  makeManagementClaim,
  makeMembership,
  makeNotebookEntry,
  makePreReportCard,
  makeQualityFramework,
  makeRegistryEntry,
  makeReportDocument,
  makeReportPreparation,
  makeReportSeasonEntry,
  makeResearchBriefJob,
  makeResearchDigestJob,
  makeResearchEvidenceItem,
  makeResearchQuestion,
  makeResearchReminder,
  makeResearchReviewCheckpoint,
  makeSignal,
  makeSourceAdapters,
  makeSourceIngestionResult,
  makeTranscriptJob,
  makeTranscriptSegment,
  makeUnmatchedSourceItem,
  makeUserSettings,
  makeWatchlist,
} from "./entities";
import type {
  AiAnalysisJob,
  CompanyEvent,
  CompanyRegistryEntry,
  CompanySignal,
  CredentialStatus,
  DiagnosticEvent,
  FeedItem,
  LicenseStatus,
  LocalMetricsSnapshot,
  LogEntry,
  NotebookEntry,
  SourceAdapter,
  SourceIngestionResult,
  TranscriptJob,
  TranscriptSegment,
  UnmatchedSourceItem,
  UserSettings,
  Watchlist,
  WatchlistMembership,
} from "../../api/types";
import type { BackupStatus } from "../../api/backups";
import type { ClaimExtractionJob } from "../../api/claimExtraction";
import type { EmbeddingModelStatus } from "../../api/interpretation";
import type { IrReportResolution } from "../../api/ir";
import type { KpiExtractionJob } from "../../api/kpiExtraction";
import type {
  ClaimsToVerify,
  ManagementClaim,
} from "../../api/managementClaims";
import type {
  FinancialFact,
  FinancialPeriod,
  KpiDefinition,
  KpiRelevance,
} from "../../api/financialsTypes";
import type { ReportDocument } from "../../api/reportDocumentsTypes";
import type {
  PreReportCard,
  ReportPreparation,
  ReportSeasonEntry,
} from "../../api/reportSeason";
import type {
  EvidenceLink,
  ResearchBriefJob,
  ResearchDigestJob,
  ResearchEvidenceItem,
  ResearchQuestion,
  ResearchReminder,
  ResearchReviewCheckpoint,
} from "../../api/researchTypes";
import type {
  FrameworkEvaluation,
  MetricKeyInfo,
  QualityFramework,
} from "../../api/qualityFrameworksTypes";

export type ScenarioName = "empty" | "minimal" | "rich";

/**
 * The canonical mock store. Both test layers materialize one of these and route
 * every read command against it; every write command mutates it in place.
 */
export interface ScenarioData {
  // Core domain
  companies: Company[];
  registry: CompanyRegistryEntry[];
  feedItems: FeedItem[];
  signals: CompanySignal[];
  events: CompanyEvent[];
  notebookEntries: NotebookEntry[];
  transcriptJobs: TranscriptJob[];
  transcriptSegments: TranscriptSegment[];
  aiAnalysisJobs: AiAnalysisJob[];
  watchlists: Watchlist[];
  watchlistMemberships: WatchlistMembership[];
  cockpitLayouts: CockpitLayout[];
  // Research workspace
  researchEvidence: ResearchEvidenceItem[];
  researchQuestions: ResearchQuestion[];
  evidenceLinks: EvidenceLink[];
  researchReminders: ResearchReminder[];
  researchBriefJobs: ResearchBriefJob[];
  researchDigestJobs: ResearchDigestJob[];
  researchReviewCheckpoints: ResearchReviewCheckpoint[];
  // Management claims
  managementClaims: ManagementClaim[];
  claimsToVerify: ClaimsToVerify;
  claimExtractionJobs: ClaimExtractionJob[];
  // Fundamentals
  financialPeriods: FinancialPeriod[];
  financialFacts: FinancialFact[];
  kpiDefinitions: KpiDefinition[];
  kpiRelevance: KpiRelevance[];
  kpiExtractionJobs: KpiExtractionJob[];
  reportDocuments: ReportDocument[];
  reportPreparations: ReportPreparation[];
  reportSeasonUpcoming: ReportSeasonEntry[];
  reportSeasonPast: ReportSeasonEntry[];
  preReportCards: PreReportCard[];
  // Quality frameworks
  qualityFrameworks: QualityFramework[];
  frameworkEvaluations: FrameworkEvaluation[];
  metricKeys: MetricKeyInfo[];
  // Sources
  sourceAdapters: SourceAdapter[];
  unmatchedSourceItems: UnmatchedSourceItem[];
  lastIngestionResults: SourceIngestionResult[];
  backfillProgress: ReturnType<typeof makeBackfillProgress>[];
  irResolutions: IrReportResolution[];
  // Platform singletons
  settings: UserSettings;
  licenseStatus: LicenseStatus;
  providerCatalog: typeof AI_PROVIDER_CATALOG[number][];
  credentialStatuses: CredentialStatus[];
  metricsSnapshot: LocalMetricsSnapshot;
  diagnosticEvents: DiagnosticEvent[];
  diagnosticSummary: ReturnType<typeof makeDiagnosticSummary>;
  logEntries: LogEntry[];
  logStatus: ReturnType<typeof makeLogStatus>;
  embeddingModelStatus: EmbeddingModelStatus;
  databaseStatus: ReturnType<typeof makeDatabaseStatus>;
  backupStatus: BackupStatus;
}

/** How densely a roster is populated. */
interface Density {
  feedPerCompany: number;
  notebooksPerCompany: number;
  segmentsPerTranscript: number;
  /** How many companies also get research/claims/fundamentals depth. */
  deepCompanies: number;
  /**
   * Index of the first "deep" company. `minimal` skips the first company (cdr)
   * so the legacy empty-state screen tests that target it still see no
   * financials/briefs/digests, while later companies keep one of every object.
   */
  deepOffset?: number;
}

const EMPTY_SINGLETONS = (companies: number, adapters: number) => ({
  settings: makeUserSettings(),
  licenseStatus: makeLicenseStatus("valid"),
  providerCatalog: AI_PROVIDER_CATALOG.map((entry) => ({ ...entry })),
  credentialStatuses: makeCredentialStatuses(),
  metricsSnapshot: makeLocalMetricsSnapshot(),
  diagnosticEvents: [makeDiagnosticEvent()],
  diagnosticSummary: makeDiagnosticSummary(),
  logEntries: [makeLogEntry()],
  logStatus: makeLogStatus(),
  embeddingModelStatus: makeEmbeddingModelStatus(),
  databaseStatus: makeDatabaseStatus(companies, adapters),
  backupStatus: makeBackupStatus(),
});

function buildPopulated(specs: readonly CompanySpec[], density: Density): ScenarioData {
  const offset = density.deepOffset ?? 0;
  const deep = specs.slice(offset, offset + density.deepCompanies);
  const adapters = makeSourceAdapters();

  const feedItems: FeedItem[] = [];
  const notebookEntries: NotebookEntry[] = [];
  const signals: CompanySignal[] = [];
  const events: CompanyEvent[] = [];
  const transcriptJobs: TranscriptJob[] = [];
  const transcriptSegments: TranscriptSegment[] = [];
  const aiAnalysisJobs: AiAnalysisJob[] = [];

  for (const spec of specs) {
    for (let i = 0; i < density.feedPerCompany; i += 1) feedItems.push(makeFeedItem(spec, i));
    for (let i = 0; i < density.notebooksPerCompany; i += 1) notebookEntries.push(makeNotebookEntry(spec, i));
    signals.push(makeSignal(spec, true), makeSignal(spec, false));
    events.push(makeEvent(spec));
    transcriptJobs.push(makeTranscriptJob(spec));
    for (let i = 0; i < density.segmentsPerTranscript; i += 1) transcriptSegments.push(makeTranscriptSegment(spec, i));
    aiAnalysisJobs.push(makeAiAnalysisJob(spec));
  }

  const watchlists: Watchlist[] = [makeWatchlist("watchlist_sample_core", "Core holdings", specs.length)];
  const watchlistMemberships: WatchlistMembership[] = specs.map((spec) =>
    makeMembership("watchlist_sample_core", "Core holdings", spec),
  );

  return {
    companies: specs.map(makeCompany),
    registry: COMPANY_SPECS.map((spec) => makeRegistryEntry(spec, specs.some((s) => s.key === spec.key))),
    feedItems,
    signals,
    events,
    notebookEntries,
    transcriptJobs,
    transcriptSegments,
    aiAnalysisJobs,
    watchlists,
    watchlistMemberships,
    cockpitLayouts: [],
    // Research
    researchEvidence: deep.map(makeResearchEvidenceItem),
    researchQuestions: deep.map(makeResearchQuestion),
    evidenceLinks: deep.map(makeEvidenceLink),
    researchReminders: deep.map(makeResearchReminder),
    researchBriefJobs: deep.map(makeResearchBriefJob),
    researchDigestJobs: deep.map(makeResearchDigestJob),
    researchReviewCheckpoints: deep.map(makeResearchReviewCheckpoint),
    // Claims
    managementClaims: deep.map(makeManagementClaim),
    claimsToVerify: makeClaimsToVerify(deep.length >= 3 ? deep : specs.slice(0, 3)),
    claimExtractionJobs: deep.map(makeClaimExtractionJob),
    // Fundamentals
    financialPeriods: deep.flatMap((spec) => [makeFinancialPeriod(spec, 2025), makeFinancialPeriod(spec, 2026)]),
    financialFacts: deep.map((spec) => makeFinancialFact(spec, 2026)),
    kpiDefinitions: [makeKpiDefinition()],
    kpiRelevance: deep.map(makeKpiRelevance),
    kpiExtractionJobs: deep.map(makeKpiExtractionJob),
    reportDocuments: deep.map(makeReportDocument),
    reportPreparations: deep.map(makeReportPreparation),
    reportSeasonUpcoming: deep.map((spec) => makeReportSeasonEntry(spec, true)),
    reportSeasonPast: deep.map((spec) => makeReportSeasonEntry(spec, false)),
    preReportCards: deep.map(makePreReportCard),
    // Quality frameworks
    qualityFrameworks: [makeQualityFramework()],
    frameworkEvaluations: deep.map(makeFrameworkEvaluation),
    metricKeys: AVAILABLE_METRIC_KEYS.map((entry) => ({ ...entry })),
    // Sources
    sourceAdapters: adapters,
    unmatchedSourceItems: deep.map(makeUnmatchedSourceItem),
    lastIngestionResults: [makeSourceIngestionResult("bankier-company-komunikaty")],
    backfillProgress: deep.map(makeBackfillProgress),
    irResolutions: deep.map(makeIrReportResolution),
    // Singletons
    ...EMPTY_SINGLETONS(specs.length, adapters.length),
  };
}

/**
 * Override the heavily-asserted collections in `minimal` with the legacy seed so
 * the pre-existing Vitest screen tests stay stable. Collections NOT overridden
 * (signals, notebooks, claims, links, briefs, digests, financials, …) keep their
 * generic one-of-each data so `minimal` still contains one of every object.
 */
function applyLegacyOverrides(data: ScenarioData): void {
  data.companies = legacyCompanies.map((c) => ({ ...c }));
  data.feedItems = legacyFeedItems.map((f) => ({ ...f }));
  data.registry = legacyRegistry.map((r) => ({ ...r }));
  data.events = legacyCompanyEvents.map((e) => ({ ...e }));
  data.researchEvidence = legacyResearchEvidence.map((e) => ({ ...e }));
  data.researchQuestions = legacyResearchQuestions.map((q) => ({ ...q }));
  data.researchReminders = legacyResearchReminders.map((r) => ({ ...r }));
  data.sourceAdapters = legacySourceAdapters.map((a) => ({ ...a }));
  data.unmatchedSourceItems = legacyUnmatchedSourceItems.map((i) => ({ ...i }));
  data.settings = { ...legacySettings };
  data.metricsSnapshot = { ...legacyMetricsSnapshot };
  data.licenseStatus = { ...legacyLicenseStatus };
  data.transcriptJobs = legacyTranscriptJobs.map((j) => ({ ...j }));
  data.transcriptSegments = legacyTranscriptSegments.map((s) => ({ ...s }));
  data.watchlists = legacyWatchlists.map((w) => ({ ...w }));
  data.watchlistMemberships = legacyWatchlistMemberships.map((m) => ({ ...m }));
  data.credentialStatuses = [
    { ...legacyGeminiCredential },
    ...data.credentialStatuses.filter((c) => c.providerId !== "provider_gemini"),
  ];
  data.databaseStatus = makeDatabaseStatus(legacyCompanies.length, legacySourceAdapters.length);
}

function buildEmpty(): ScenarioData {
  const adapters = makeSourceAdapters();
  return {
    companies: [],
    registry: COMPANY_SPECS.map((spec) => makeRegistryEntry(spec, false)),
    feedItems: [],
    signals: [],
    events: [],
    notebookEntries: [],
    transcriptJobs: [],
    transcriptSegments: [],
    aiAnalysisJobs: [],
    watchlists: [],
    watchlistMemberships: [],
    cockpitLayouts: [],
    researchEvidence: [],
    researchQuestions: [],
    evidenceLinks: [],
    researchReminders: [],
    researchBriefJobs: [],
    researchDigestJobs: [],
    researchReviewCheckpoints: [],
    managementClaims: [],
    claimsToVerify: { due: [], overdue: [], upcoming: [] },
    claimExtractionJobs: [],
    financialPeriods: [],
    financialFacts: [],
    kpiDefinitions: [],
    kpiRelevance: [],
    kpiExtractionJobs: [],
    reportDocuments: [],
    reportPreparations: [],
    reportSeasonUpcoming: [],
    reportSeasonPast: [],
    preReportCards: [],
    qualityFrameworks: [],
    frameworkEvaluations: [],
    metricKeys: AVAILABLE_METRIC_KEYS.map((entry) => ({ ...entry })),
    sourceAdapters: adapters,
    unmatchedSourceItems: [],
    lastIngestionResults: [],
    backfillProgress: [],
    irResolutions: [],
    ...EMPTY_SINGLETONS(0, adapters.length),
  };
}

/**
 * Build a fresh, deep-cloned dataset for the named scenario. The clone guarantees
 * mutation isolation: a test that writes to its store cannot affect another test.
 */
export function buildScenario(name: ScenarioName): ScenarioData {
  let data: ScenarioData;
  switch (name) {
    case "empty":
      data = buildEmpty();
      break;
    case "minimal":
      data = buildPopulated(COMPANY_SPECS.slice(0, 4), {
        feedPerCompany: 2,
        notebooksPerCompany: 1,
        segmentsPerTranscript: 2,
        deepCompanies: 3,
        deepOffset: 1,
      });
      applyLegacyOverrides(data);
      break;
    case "rich":
      data = buildPopulated(COMPANY_SPECS, {
        feedPerCompany: 4,
        notebooksPerCompany: 3,
        segmentsPerTranscript: 4,
        deepCompanies: 8,
      });
      // Rich exercises varied license kinds beyond the default-valid path.
      data.licenseStatus = makeLicenseStatus("valid");
      break;
  }

  // Seed the pinned-company spine (ADR 0054) so the completeness guardrail
  // covers the new preference and screen tests have pinned entries. The empty
  // scenario stays unpinned to assert the no-pins resting state.
  if (name !== "empty" && data.companies.length > 0) {
    const pinnedCount = name === "rich" ? 3 : 1;
    data.settings = {
      ...data.settings,
      pinnedCompanyIds: data.companies.slice(0, pinnedCount).map((company) => company.id),
    };
  }

  return structuredClone(data);
}
