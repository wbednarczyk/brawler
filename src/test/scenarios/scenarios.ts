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

import { applyScenarioOverlays, type ScenarioOverlayName } from "./overlays";
import type { Company } from "../../api/types";
import type { CockpitLayout } from "../../api/generated/CockpitLayout";
import type { ExtractionOutcome } from "../../api/generated/ExtractionOutcome";
import type { FactProvenance } from "../../api/generated/FactProvenance";
import type { AlertRule } from "../../api/generated/AlertRule";
import type { AttentionEvent } from "../../api/generated/AttentionEvent";
import type { ReconciliationResult } from "../../api/generated/ReconciliationResult";
import type { MorningBriefing } from "../../api/generated/MorningBriefing";
import type { AutopilotRun, CompanyAutopilot } from "../../api/autopilot";
import type { KpiComparison } from "../../api/comparison";
import type { SectorPercentiles } from "../../api/generated/SectorPercentiles";
import type { ComparativeValuation } from "../../api/generated/ComparativeValuation";
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
  makeBackfillProgress,
  makeAlertRule,
  makeAttentionEvent,
  makeBackupStatus,
  makeClaimsToVerify,
  makeCompany,
  makeCredentialStatuses,
  makeDatabaseStatus,
  makeDiagnosticEvent,
  makeReconciliationResult,
  makeDiagnosticSummary,
  makeEvent,
  makeEvidenceLink,
  makeFeedItem,
  makeFrameworkEvaluation,
  makeFinancialFact,
  makeFinancialPeriod,
  makeIrReportResolution,
  makeKpiComparison,
  makeKpiDefinition,
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
  makeOwnershipOverview,
  makeCompanyHealth,
  makeInsiderOverview,
  makeAnalystRecommendationsView,
  makeRecommendationSignal,
  companyId,
  makeReportDocument,
  makeReportPreparation,
  makeReportSeasonEntry,
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
import type { IrReportResolution } from "../../api/ir";
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
import type { OwnershipOverview } from "../../api/ownership";
import type { CompanyHealth } from "../../api/companyHealth";
import type { InsiderOverview } from "../../api/insider";
import type { RedFlagsView } from "../../api/redFlags";
import type { AnalystRecommendationsView } from "../../api/analystRecommendations";
import type {
  PreReportCard,
  ReportPreparation,
  ReportSeasonEntry,
} from "../../api/reportSeason";
import type {
  EvidenceLink,
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
 * A base scenario plus composable overlays (ADR 0081 Q2). `buildScenario`
 * accepts either the bare `ScenarioName` (unchanged, pre-Q2 callers) or a
 * `ScenarioSpec` — never a bespoke seventh mega-scenario per overlay
 * combination.
 */
export type ScenarioSpec = {
  base: ScenarioName;
  overlays?: readonly ScenarioOverlayName[];
};

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
  watchlists: Watchlist[];
  watchlistMemberships: WatchlistMembership[];
  cockpitLayouts: CockpitLayout[];
  // Autonomous report pipeline (ADR 0055)
  autopilotModes: CompanyAutopilot[];
  autopilotRuns: AutopilotRun[];
  // Research workspace
  researchEvidence: ResearchEvidenceItem[];
  researchQuestions: ResearchQuestion[];
  evidenceLinks: EvidenceLink[];
  researchReminders: ResearchReminder[];
  researchReviewCheckpoints: ResearchReviewCheckpoint[];
  // Management claims
  managementClaims: ManagementClaim[];
  claimsToVerify: ClaimsToVerify;
  // Fundamentals
  financialPeriods: FinancialPeriod[];
  financialFacts: FinancialFact[];
  kpiDefinitions: KpiDefinition[];
  // Populated cross-company comparisons (ADR 0089 §A3). Optional seed matched by
  // (companyIds set, metricKeys, granularity); an unmatched request reads back
  // the empty computed default. Lets the browser/visual harness render the
  // kpi_comparison command without wiring confirmed facts to these companies.
  kpiComparisons?: KpiComparison[];
  // Comparative valuation L1 (ADR 0089 §B1/B2/B3). Optional seeds keyed by
  // companyId: a company with a seed reads it back verbatim; an unseeded company
  // falls through to the sector-derived computed default (every metric/method a
  // typed absence, since the mock store carries no resolvable ratios/multiples).
  // Lets the browser/visual harness render a populated football field + a thin
  // state without wiring confirmed facts to these companies.
  sectorPercentiles?: Record<string, SectorPercentiles>;
  comparativeValuations?: Record<string, ComparativeValuation>;
  kpiRelevance: KpiRelevance[];
  reportDocuments: ReportDocument[];
  // Ownership overviews per company (ADR 0072, v0.56 T6). Optional seed: a company
  // with no entry reads back the empty overview (freeFloatPct "100").
  ownershipOverviews?: OwnershipOverview[];
  // Company-health scores per company (ADR 0083, v0.57 T2). Optional seed: a
  // company with no entry reads back the empty read model (no latest, empty
  // history — the "no annual periods yet" state).
  companyHealthReports?: CompanyHealth[];
  // Insider overviews per company (ADR 0083 D7, v0.57 T6). Optional seed: a
  // company with no entry reads back the empty overview (no transactions/holdings,
  // both windows below the 2-transaction minimum).
  insiderOverviews?: InsiderOverview[];
  // Red-flags panel state per company (ADR 0083 D8, v0.57 T7). Optional seed: a
  // company with no entry reads back the empty view (no active flags, empty
  // history). `acknowledge_red_flag` mutates the matching entry in place.
  redFlagsByCompany?: Record<string, RedFlagsView>;
  // Analyst-recommendations panel view per company (ADR 0073, v0.58 A3). Optional
  // seed: a company with no entry reads back the empty view (no entries, no latest
  // target, no refresh — the "source published nothing yet" state).
  analystRecommendationsByCompany?: Record<string, AnalystRecommendationsView>;
  // Structured-first extraction provenance (ADR 0061) — optional seed: legacy
  // facts have no row, so the tier/validation badge + drift card simply don't
  // render. A provenance-seeded scenario exercises that UI (badges + the
  // "structure changed" diff).
  factProvenance?: FactProvenance[];
  // Non-emitting extraction outcomes (ADR 0061 decision 2, ADR 0084 decision
  // 4/6) — the periods the deterministic pipeline ran on and refused to record.
  // Optional seed: a company with no rows reads back `[]`, which is the GOOD
  // "nothing flagged" state, not a failure. `rerun_extraction_outcome` drops the
  // re-run row (the real backend updates it in place; a re-run that now emits
  // leaves the flagged list).
  flaggedExtractionOutcomes?: ExtractionOutcome[];
  reportPreparations: ReportPreparation[];
  // KNF short-selling register (v0.55 T4b). Raw seed rows; `list_short_positions`
  // derives the per-company view (aggregate/delta/history) from these.
  shortPositions: ScenarioShortPosition[];
  shortPositionEvents: ScenarioShortPositionEvent[];
  reportSeasonUpcoming: ReportSeasonEntry[];
  reportSeasonPast: ReportSeasonEntry[];
  preReportCards: PreReportCard[];
  // Quality frameworks
  qualityFrameworks: QualityFramework[];
  frameworkEvaluations: FrameworkEvaluation[];
  metricKeys: MetricKeyInfo[];
  // Attention routing — alert rules + fired events (ADR 0068)
  alertRules: AlertRule[];
  attentionEvents: AttentionEvent[];
  // Morning briefing (ADR 0068 decision 4, §T5): the real compose job runs on
  // the worker, not the mock, so every scenario starts with none composed —
  // `generate_morning_briefing` in `runtime.ts` populates this for the Today
  // card's on-demand generate → refetch flow.
  morningBriefing: MorningBriefing | null;
  // Sources
  sourceAdapters: SourceAdapter[];
  unmatchedSourceItems: UnmatchedSourceItem[];
  lastIngestionResults: SourceIngestionResult[];
  backfillProgress: ReturnType<typeof makeBackfillProgress>[];
  irResolutions: IrReportResolution[];
  // Platform singletons
  settings: UserSettings;
  // Dziś v2 visit anchor (F2 plan decision 4). `null` = never visited — the
  // same tolerant-absent-row default as the Rust `today_last_visit_at`
  // read; not part of `UserSettings`/`get_settings` on the real backend
  // either, so it stays a sibling field rather than a settings key.
  todayLastVisitAt: string | null;
  licenseStatus: LicenseStatus;
  providerCatalog: typeof AI_PROVIDER_CATALOG[number][];
  credentialStatuses: CredentialStatus[];
  metricsSnapshot: LocalMetricsSnapshot;
  diagnosticEvents: DiagnosticEvent[];
  reconciliationResults: ReconciliationResult[];
  diagnosticSummary: ReturnType<typeof makeDiagnosticSummary>;
  logEntries: LogEntry[];
  logStatus: ReturnType<typeof makeLogStatus>;
  databaseStatus: ReturnType<typeof makeDatabaseStatus>;
  backupStatus: BackupStatus;
}

/** A seeded KNF short-position mirror row (v0.55 T4b). `exitedAt` non-null =
 * dropped out of the register (below the 0.5% threshold). */
export interface ScenarioShortPosition {
  companyId: string;
  holderName: string;
  netPositionPct: number;
  positionDate: string;
  exitedAt: string | null;
}

/** A seeded KNF register change event (v0.55 T4b). */
export interface ScenarioShortPositionEvent {
  companyId: string;
  kind: "entered" | "increased" | "decreased" | "exited";
  holderName: string;
  fromPct: number | null;
  toPct: number | null;
  positionDate: string;
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
  todayLastVisitAt: null,
  licenseStatus: makeLicenseStatus("valid"),
  providerCatalog: AI_PROVIDER_CATALOG.map((entry) => ({ ...entry })),
  credentialStatuses: makeCredentialStatuses(),
  metricsSnapshot: makeLocalMetricsSnapshot(),
  diagnosticEvents: [makeDiagnosticEvent()],
  reconciliationResults: [makeReconciliationResult()],
  diagnosticSummary: makeDiagnosticSummary(),
  logEntries: [makeLogEntry()],
  logStatus: makeLogStatus(),
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

  for (const spec of specs) {
    for (let i = 0; i < density.feedPerCompany; i += 1) feedItems.push(makeFeedItem(spec, i));
    for (let i = 0; i < density.notebooksPerCompany; i += 1) notebookEntries.push(makeNotebookEntry(spec, i));
    signals.push(makeSignal(spec, true), makeSignal(spec, false), makeRecommendationSignal(spec));
    events.push(makeEvent(spec));
    transcriptJobs.push(makeTranscriptJob(spec));
    for (let i = 0; i < density.segmentsPerTranscript; i += 1) transcriptSegments.push(makeTranscriptSegment(spec, i));
  }

  const watchlists: Watchlist[] = [makeWatchlist("watchlist_sample_core", "Core holdings", specs.length)];
  const watchlistMemberships: WatchlistMembership[] = specs.map((spec) =>
    makeMembership("watchlist_sample_core", "Core holdings", spec),
  );

  const companies = specs.map(makeCompany);
  // Attention routing seed (ADR 0068): one watchlist-scoped rule + one fired
  // event referencing the first company, so every attention list command
  // returns non-trivial data and the rules manager renders a populated state.
  const alertRules: AlertRule[] = [
    makeAlertRule("alert_rule_sample_1", "signal_category", "watchlist_sample_core"),
  ];
  const attentionEvents: AttentionEvent[] = companies[0]
    ? [makeAttentionEvent("attn_sample_1", "alert_rule_sample_1", companies[0].id)]
    : [];

  return {
    companies,
    registry: COMPANY_SPECS.map((spec) => makeRegistryEntry(spec, specs.some((s) => s.key === spec.key))),
    feedItems,
    signals,
    events,
    notebookEntries,
    transcriptJobs,
    transcriptSegments,
    watchlists,
    watchlistMemberships,
    cockpitLayouts: [],
    autopilotModes: [],
    autopilotRuns: [],
    // Research
    researchEvidence: deep.map(makeResearchEvidenceItem),
    researchQuestions: deep.map(makeResearchQuestion),
    evidenceLinks: deep.map(makeEvidenceLink),
    researchReminders: deep.map(makeResearchReminder),
    researchReviewCheckpoints: deep.map(makeResearchReviewCheckpoint),
    // Claims
    managementClaims: deep.map(makeManagementClaim),
    claimsToVerify: makeClaimsToVerify(deep.length >= 3 ? deep : specs.slice(0, 3)),
    // Fundamentals
    financialPeriods: deep.flatMap((spec) => [makeFinancialPeriod(spec, 2025), makeFinancialPeriod(spec, 2026)]),
    financialFacts: deep.map((spec) => makeFinancialFact(spec, 2026)),
    kpiDefinitions: [makeKpiDefinition()],
    kpiComparisons: [makeKpiComparison()],
    kpiRelevance: deep.map(makeKpiRelevance),
    reportDocuments: deep.map(makeReportDocument),
    ownershipOverviews: deep.map(makeOwnershipOverview),
    companyHealthReports: deep.map(makeCompanyHealth),
    insiderOverviews: deep.map(makeInsiderOverview),
    analystRecommendationsByCompany: Object.fromEntries(
      deep.map((spec) => [companyId(spec), makeAnalystRecommendationsView(spec)]),
    ),
    reportPreparations: deep.map(makeReportPreparation),
    shortPositions: [],
    shortPositionEvents: [],
    reportSeasonUpcoming: deep.map((spec) => makeReportSeasonEntry(spec, true)),
    reportSeasonPast: deep.map((spec) => makeReportSeasonEntry(spec, false)),
    preReportCards: deep.map(makePreReportCard),
    // Quality frameworks
    qualityFrameworks: [makeQualityFramework()],
    frameworkEvaluations: deep.map(makeFrameworkEvaluation),
    metricKeys: AVAILABLE_METRIC_KEYS.map((entry) => ({ ...entry })),
    // Attention routing
    alertRules,
    attentionEvents,
    // Morning briefing
    morningBriefing: null,
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
    watchlists: [],
    watchlistMemberships: [],
    cockpitLayouts: [],
    autopilotModes: [],
    autopilotRuns: [],
    researchEvidence: [],
    researchQuestions: [],
    evidenceLinks: [],
    researchReminders: [],
    researchReviewCheckpoints: [],
    managementClaims: [],
    claimsToVerify: { due: [], overdue: [], upcoming: [] },
    financialPeriods: [],
    financialFacts: [],
    kpiDefinitions: [],
    kpiRelevance: [],
    reportDocuments: [],
    reportPreparations: [],
    shortPositions: [],
    shortPositionEvents: [],
    reportSeasonUpcoming: [],
    reportSeasonPast: [],
    preReportCards: [],
    qualityFrameworks: [],
    frameworkEvaluations: [],
    metricKeys: AVAILABLE_METRIC_KEYS.map((entry) => ({ ...entry })),
    alertRules: [],
    attentionEvents: [],
    morningBriefing: null,
    sourceAdapters: adapters,
    unmatchedSourceItems: [],
    lastIngestionResults: [],
    backfillProgress: [],
    irResolutions: [],
    ...EMPTY_SINGLETONS(0, adapters.length),
  };
}

/**
 * Build a fresh, deep-cloned dataset for the named scenario, optionally
 * layering Q2 overlays (`ScenarioSpec`). The clone guarantees mutation
 * isolation: a test that writes to its store cannot affect another test, and
 * two builds from the same spec never share array/object references.
 */
export function buildScenario(spec: ScenarioName | ScenarioSpec): ScenarioData {
  const name: ScenarioName = typeof spec === "string" ? spec : spec.base;
  const overlays: readonly ScenarioOverlayName[] = typeof spec === "string" ? [] : spec.overlays ?? [];
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

  data = applyScenarioOverlays(data, overlays);

  return structuredClone(data);
}
