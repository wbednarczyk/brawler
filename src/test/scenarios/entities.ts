// Canonical test entity builders (ADR 0048 Keystone B/C, Radicle 749a5a8).
//
// One source of truth for realistic mock data, typed against the ts-rs-GENERATED
// api DTOs so a seed cannot drift from the contract. Builders are deterministic
// (fixed IDs/timestamps, no wall-clock/random) and pure (return fresh objects);
// `scenarios.ts` composes them into empty/minimal/rich datasets that the mock
// runtime clones per test. IDs use a single `*_sample_*` scheme.

import type {
  AiAnalysisJob,
  AiAnalysisResult,
  AiProviderCatalogEntry,
  BackfillProgress,
  Company,
  CompanyEvent,
  CompanyRegistryEntry,
  CompanySignal,
  CredentialStatus,
  DatabaseStatus,
  DiagnosticEvent,
  DiagnosticSummary,
  FeedItem,
  LicenseStatus,
  LicenseStatusKind,
  LocalMetricsSnapshot,
  LogEntry,
  LogStatus,
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
  ClaimToVerify,
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

// Fixed clock anchor — every timestamp derives from this so datasets are stable
// across parallel workers. (No `new Date()` / `Date.now()` in seeds.)
export const SAMPLE_NOW = "2026-06-08T10:00:00Z";

/** One company in the roster. `minimal` uses the first 4; `rich` uses all. */
export type CompanySpec = {
  key: string; // short slug, e.g. "cdr"
  ticker: string;
  name: string;
  sector: string;
};

export const COMPANY_SPECS: readonly CompanySpec[] = [
  { key: "cdr", ticker: "CDR", name: "CD Projekt", sector: "Technology" },
  { key: "pkn", ticker: "PKN", name: "PKN Orlen", sector: "Energy" },
  { key: "kgh", ticker: "KGH", name: "KGHM Polska Miedz", sector: "Materials" },
  { key: "pzu", ticker: "PZU", name: "PZU Group", sector: "Financials" },
  { key: "peo", ticker: "PEO", name: "Bank Pekao", sector: "Financials" },
  { key: "pko", ticker: "PKO", name: "PKO Bank Polski", sector: "Financials" },
  { key: "dnp", ticker: "DNP", name: "Dino Polska", sector: "Consumer Staples" },
  { key: "lpp", ticker: "LPP", name: "LPP", sector: "Consumer Discretionary" },
  { key: "cps", ticker: "CPS", name: "Cyfrowy Polsat", sector: "Communication" },
  { key: "ccc", ticker: "CCC", name: "CCC", sector: "Consumer Discretionary" },
  { key: "alr", ticker: "ALR", name: "Alior Bank", sector: "Financials" },
  { key: "opl", ticker: "OPL", name: "Orange Polska", sector: "Communication" },
  { key: "spl", ticker: "SPL", name: "Santander Bank Polska", sector: "Financials" },
  { key: "mbk", ticker: "MBK", name: "mBank", sector: "Financials" },
  { key: "kru", ticker: "KRU", name: "Kruk", sector: "Financials" },
  { key: "tpe", ticker: "TPE", name: "Tauron", sector: "Energy" },
  { key: "ena", ticker: "ENA", name: "Enea", sector: "Energy" },
  { key: "jsw", ticker: "JSW", name: "Jastrzebska Spolka Weglowa", sector: "Materials" },
  { key: "gpw", ticker: "GPW", name: "GPW Exchange", sector: "Financials" },
  { key: "ale", ticker: "ALE", name: "Allegro", sector: "Consumer Discretionary" },
  { key: "atc", ticker: "ATC", name: "Arctic Paper", sector: "Materials" },
  { key: "bdx", ticker: "BDX", name: "Budimex", sector: "Industrials" },
  { key: "cbf", ticker: "CBF", name: "Cuprum Bank Fund", sector: "Financials" },
  { key: "ats", ticker: "ATS", name: "Amica", sector: "Consumer Discretionary" },
  { key: "kty", ticker: "KTY", name: "Grupa Kety", sector: "Materials" },
  { key: "mil", ticker: "MIL", name: "Bank Millennium", sector: "Financials" },
  { key: "pge", ticker: "PGE", name: "PGE Polska Grupa Energetyczna", sector: "Energy" },
  { key: "txt", ticker: "TXT", name: "Text", sector: "Technology" },
] as const;

export const companyId = (spec: CompanySpec) => `company_gpw_${spec.key}`;
export const qualifiedTicker = (spec: CompanySpec) => `GPW:${spec.ticker}`;

// ============================================================================
// Core domain
// ============================================================================

export function makeCompany(spec: CompanySpec): Company {
  return {
    id: companyId(spec),
    exchange: "GPW",
    ticker: spec.ticker,
    qualifiedTicker: qualifiedTicker(spec),
    displayName: spec.name,
    isin: `PLSAMPL00${spec.ticker}`,
    cik: null,
    lei: null,
  };
}

export function makeRegistryEntry(spec: CompanySpec, tracked: boolean): CompanyRegistryEntry {
  return {
    sourceAdapterId: "gpw-company-registry",
    exchange: "GPW",
    ticker: spec.ticker,
    qualifiedTicker: qualifiedTicker(spec),
    displayName: spec.name,
    isin: `PLSAMPL00${spec.ticker}`,
    sourceUrl: "https://www.gpw.pl/spolki",
    fetchedAt: SAMPLE_NOW,
    tracked,
  };
}

export function makeFeedItem(spec: CompanySpec, index: number): FeedItem {
  const kinds = [
    { type: "Official report", source: "GPW ESPI/EBI", attribution: "GPW", language: "pl" },
    { type: "News", source: "Bankier RSS", attribution: "Bankier", language: "pl" },
    { type: "Transcript", source: "Sample transcript", attribution: "Sample", language: "en" },
  ];
  const kind = kinds[index % kinds.length];
  return {
    id: `feed_sample_${spec.key}_${index}`,
    company: qualifiedTicker(spec),
    type: kind.type,
    source: kind.source,
    time: "Today 09:12",
    title: `${spec.name} sample ${kind.type.toLowerCase()} #${index}`,
    unread: index % 2 === 0,
    saved: index % 3 === 0,
    sourceUrl: `https://example.test/feed/${spec.key}/${index}`,
    language: kind.language,
    publishedAt: SAMPLE_NOW,
    fetchedAt: SAMPLE_NOW,
    attribution: kind.attribution,
    summary: `Sample ${kind.type.toLowerCase()} item validating feed filtering and detail rendering.`,
    bodyText: `Body text for ${spec.name} sample item ${index}.`,
    attachments:
      index % 3 === 0
        ? [{ id: `attach_${spec.key}_${index}`, label: "Attachment", url: `https://example.test/doc/${spec.key}.pdf` }]
        : [],
  };
}

export function makeSignal(spec: CompanySpec, confirmed: boolean): CompanySignal {
  return {
    id: `signal_sample_${spec.key}_${confirmed ? "dividend" : "guidance"}`,
    companyId: companyId(spec),
    company: qualifiedTicker(spec),
    companyName: spec.name,
    feedItemId: `feed_sample_${spec.key}_0`,
    category: confirmed ? "dividend" : "guidance_change",
    categoryDisplayName: confirmed ? "Dividend" : "Guidance change",
    confidence: confirmed ? 0.95 : 0.6,
    classifiedBy: confirmed ? "rule" : "ai",
    status: confirmed ? "confirmed" : "proposed",
    signalDate: confirmed ? SAMPLE_NOW : null,
    providerId: confirmed ? null : "provider_gemini",
    modelId: confirmed ? null : "gemini-2.5-flash",
    derivedEventId: null,
    title: `${spec.name} ${confirmed ? "dividend" : "guidance"} signal`,
    sourceUrl: `https://example.test/feed/${spec.key}/0`,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeEvent(spec: CompanySpec): CompanyEvent {
  return {
    id: `event_sample_${spec.key}_meeting`,
    companyId: companyId(spec),
    company: qualifiedTicker(spec),
    companyName: spec.name,
    eventType: "shareholder_meeting",
    title: `${spec.name} annual shareholder meeting`,
    eventDate: "2026-06-20",
    eventTime: "10:00",
    status: "scheduled",
    sourceType: "market_calendar",
    sourceAdapterId: "gpw-company-registry",
    sourceEventKey: `${spec.key}-meeting-2026`,
    sourceUrl: `https://example.test/events/${spec.key}-meeting`,
    attribution: "GPW calendar",
    fetchedAt: SAMPLE_NOW,
    manual: false,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeNotebookEntry(spec: CompanySpec, index: number): NotebookEntry {
  return {
    id: `note_sample_${spec.key}_${index}`,
    companyId: companyId(spec),
    title: `${spec.name} research note #${index}`,
    body: `Notebook body for ${spec.name} entry ${index}.`,
    bodyFormat: "markdown",
    tags: index % 2 === 0 ? ["thesis"] : ["risk", "follow-up"],
    kind: index % 4 === 0 ? "claim" : "note",
    claimStatus: index % 4 === 0 ? "pending" : null,
    eventDate: null,
    followUpAfter: null,
    followUpDate: null,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
    origins: [],
  };
}

export function makeTranscriptJob(spec: CompanySpec): TranscriptJob {
  return {
    id: `transcript_sample_${spec.key}`,
    companyId: companyId(spec),
    company: qualifiedTicker(spec),
    companyName: spec.name,
    providerId: "provider_gemini",
    sourceType: "youtube",
    sourceUrl: `https://example.test/video/${spec.key}`,
    sourceLabel: `${spec.name} earnings call`,
    companyResolutionStatus: "resolved",
    recognizedCompanyCandidates: [],
    status: "completed",
    errorCode: null,
    createdAt: SAMPLE_NOW,
    startedAt: SAMPLE_NOW,
    finishedAt: SAMPLE_NOW,
    error: null,
  };
}

export function makeTranscriptSegment(spec: CompanySpec, index: number): TranscriptSegment {
  return {
    id: `segment_sample_${spec.key}_${index}`,
    transcriptJobId: `transcript_sample_${spec.key}`,
    companyId: companyId(spec),
    startSeconds: index * 30,
    endSeconds: index * 30 + 28,
    speaker: index % 2 === 0 ? "CEO" : "CFO",
    text: `Transcript segment ${index} for ${spec.name}.`,
    language: "en",
    createdAt: SAMPLE_NOW,
  };
}

export function makeAiAnalysisResult(spec: CompanySpec): AiAnalysisResult {
  return {
    id: `ai_result_sample_${spec.key}`,
    aiAnalysisJobId: `ai_job_sample_${spec.key}`,
    feedItemId: `feed_sample_${spec.key}_0`,
    providerId: "provider_gemini",
    model: "gemini-2.5-flash",
    promptVersion: "m13.source_grounded.v1",
    summary: `AI summary for ${spec.name}.`,
    significance: "medium",
    reasoning: "Source-grounded reasoning over the official report.",
    language: "en",
    tags: ["earnings"],
    sourceReferences: [
      { id: `ai_ref_${spec.key}`, sourceUrl: `https://example.test/feed/${spec.key}/0`, label: "Report", createdAt: SAMPLE_NOW },
    ],
    createdAt: SAMPLE_NOW,
  };
}

export function makeAiAnalysisJob(spec: CompanySpec): AiAnalysisJob {
  return {
    id: `ai_job_sample_${spec.key}`,
    feedItemId: `feed_sample_${spec.key}_0`,
    promptPresetId: "default_summary",
    customQuestion: null,
    providerId: "provider_gemini",
    model: "gemini-2.5-flash",
    promptVersion: "m13.source_grounded.v1",
    status: "succeeded",
    errorCode: null,
    error: null,
    createdAt: SAMPLE_NOW,
    startedAt: SAMPLE_NOW,
    finishedAt: SAMPLE_NOW,
    result: makeAiAnalysisResult(spec),
  };
}

export function makeWatchlist(id: string, name: string, companyCount: number): Watchlist {
  return { id, name, description: null, companyCount };
}

export function makeMembership(watchlistId: string, watchlistName: string, spec: CompanySpec): WatchlistMembership {
  return { watchlistId, watchlistName, companyId: companyId(spec) };
}

// ============================================================================
// Research workspace
// ============================================================================

export function makeResearchEvidenceItem(spec: CompanySpec): ResearchEvidenceItem {
  return {
    id: `evidence_sample_${spec.key}`,
    evidenceType: "feed_item",
    sourceDomain: "feed",
    sourceId: `feed_sample_${spec.key}_0`,
    companyId: companyId(spec),
    occurredAt: SAMPLE_NOW,
    title: `${spec.name} evidence on the timeline`,
    summary: `Evidence summary for ${spec.name}.`,
    sourceUrl: `https://example.test/feed/${spec.key}/0`,
    attribution: "GPW",
    trustCategory: "official_report",
    reviewState: { changedSinceCompanyReview: true, changedSinceWatchlistReview: true },
  };
}

export function makeResearchQuestion(spec: CompanySpec): ResearchQuestion {
  return {
    id: `question_sample_${spec.key}`,
    scopeType: "company",
    scopeId: companyId(spec),
    title: `What is the ${spec.name} margin trajectory?`,
    body: `Open research question for ${spec.name}.`,
    status: "open",
    closedAt: null,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeEvidenceLink(spec: CompanySpec): EvidenceLink {
  return {
    id: `evlink_sample_${spec.key}`,
    fromType: "feed_item",
    fromId: `feed_sample_${spec.key}_0`,
    toType: "notebook_entry",
    toId: `note_sample_${spec.key}_0`,
    relationType: "cites",
    createdAt: SAMPLE_NOW,
  };
}

export function makeResearchReminder(spec: CompanySpec): ResearchReminder {
  return {
    id: `reminder_sample_${spec.key}`,
    scopeType: "company",
    scopeId: companyId(spec),
    companyId: companyId(spec),
    reminderKind: "claim_follow_up",
    sourceType: "claim",
    sourceId: `claim_sample_${spec.key}`,
    title: `Follow up on ${spec.name} guidance`,
    body: `Reminder body for ${spec.name}.`,
    dueAt: SAMPLE_NOW,
    status: "open",
    snoozedUntil: null,
    completedAt: null,
    dismissedAt: null,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeResearchBriefJob(spec: CompanySpec): ResearchBriefJob {
  const briefId = `brief_sample_${spec.key}`;
  return {
    id: `brief_job_sample_${spec.key}`,
    scopeType: "company",
    scopeId: companyId(spec),
    providerId: "provider_gemini",
    model: "gemini-2.5-flash",
    promptVersion: "research_brief.v1",
    evidenceCollectorVersion: "collector.v1",
    rendererVersion: "renderer.v1",
    status: "succeeded",
    errorCode: null,
    error: null,
    createdAt: SAMPLE_NOW,
    startedAt: SAMPLE_NOW,
    finishedAt: SAMPLE_NOW,
    brief: {
      id: briefId,
      jobId: `brief_job_sample_${spec.key}`,
      scopeType: "company",
      scopeId: companyId(spec),
      providerId: "provider_gemini",
      model: "gemini-2.5-flash",
      promptVersion: "research_brief.v1",
      evidenceCollectorVersion: "collector.v1",
      rendererVersion: "renderer.v1",
      title: `${spec.name} research brief`,
      summary: `Brief summary for ${spec.name}.`,
      contentMarkdown: `# ${spec.name}\n\nGenerated brief content [^1].`,
      language: "en",
      generatedAt: SAMPLE_NOW,
      createdAt: SAMPLE_NOW,
      citations: [
        {
          id: `brief_cite_${spec.key}`,
          briefId,
          citationKey: "1",
          evidenceType: "feed_item",
          evidenceId: `feed_sample_${spec.key}_0`,
          label: `${spec.name} official report`,
          snippet: "Cited snippet.",
          createdAt: SAMPLE_NOW,
        },
      ],
    },
  };
}

export function makeResearchDigestJob(spec: CompanySpec): ResearchDigestJob {
  const digestId = `digest_sample_${spec.key}`;
  return {
    id: `digest_job_sample_${spec.key}`,
    scopeType: "company",
    scopeId: companyId(spec),
    providerId: "provider_gemini",
    model: "gemini-2.5-flash",
    promptVersion: "research_digest.v1",
    evidenceCollectorVersion: "collector.v1",
    rendererVersion: "renderer.v1",
    status: "succeeded",
    errorCode: null,
    error: null,
    createdAt: SAMPLE_NOW,
    startedAt: SAMPLE_NOW,
    finishedAt: SAMPLE_NOW,
    digest: {
      id: digestId,
      jobId: `digest_job_sample_${spec.key}`,
      scopeType: "company",
      scopeId: companyId(spec),
      providerId: "provider_gemini",
      model: "gemini-2.5-flash",
      promptVersion: "research_digest.v1",
      evidenceCollectorVersion: "collector.v1",
      rendererVersion: "renderer.v1",
      title: `${spec.name} weekly digest`,
      summary: `Digest summary for ${spec.name}.`,
      contentMarkdown: `# ${spec.name}\n\nGenerated digest content [^1].`,
      language: "en",
      generatedAt: SAMPLE_NOW,
      createdAt: SAMPLE_NOW,
      citations: [
        {
          id: `digest_cite_${spec.key}`,
          digestId,
          citationKey: "1",
          evidenceType: "feed_item",
          evidenceId: `feed_sample_${spec.key}_0`,
          label: `${spec.name} official report`,
          snippet: "Cited snippet.",
          createdAt: SAMPLE_NOW,
        },
      ],
    },
  };
}

export function makeResearchReviewCheckpoint(spec: CompanySpec): ResearchReviewCheckpoint {
  return {
    id: `checkpoint_sample_${spec.key}`,
    scopeType: "company",
    scopeId: companyId(spec),
    reviewedAt: SAMPLE_NOW,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

// ============================================================================
// Management claims (ADR 0040)
// ============================================================================

export function makeManagementClaim(spec: CompanySpec): ManagementClaim {
  return {
    id: `claim_sample_${spec.key}`,
    companyId: companyId(spec),
    statement: `${spec.name} targets revenue above 1.0bn next year`,
    body: `Claim body for ${spec.name}.`,
    bodyFormat: "markdown",
    madeAt: SAMPLE_NOW,
    sourcePeriodId: `period_sample_${spec.key}_2025_FY`,
    dueFiscalYear: 2026,
    duePeriodType: "FY",
    status: "pending",
    sourceEvidenceType: "transcript_segment",
    sourceEvidenceId: `segment_sample_${spec.key}_0`,
    extractionProposalId: null,
    targetMetricKey: "revenue",
    targetComparator: "gte",
    targetValueNumeric: "1000000000",
    targetUnit: "PLN",
    verifyingFactId: null,
    revisesClaimId: null,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeClaimToVerify(spec: CompanySpec): ClaimToVerify {
  return {
    claim: makeManagementClaim(spec),
    arrivedPeriodId: `period_sample_${spec.key}_2026_FY`,
    verifyingFactCandidate: { factId: `fact_sample_${spec.key}_revenue`, valueNumeric: "1050000000" },
  };
}

export function makeClaimsToVerify(specs: readonly CompanySpec[]): ClaimsToVerify {
  return {
    due: specs.slice(0, 1).map(makeClaimToVerify),
    overdue: specs.slice(1, 2).map(makeClaimToVerify),
    upcoming: specs.slice(2, 3).map(makeClaimToVerify),
  };
}

export function makeClaimExtractionJob(spec: CompanySpec): ClaimExtractionJob {
  return {
    id: `claim_job_sample_${spec.key}`,
    companyId: companyId(spec),
    sourceType: "transcript",
    sourceId: `transcript_sample_${spec.key}`,
    providerId: "provider_gemini",
    model: "gemini-2.5-flash",
    promptVersion: "claim_extraction.v1",
    status: "succeeded",
    errorCode: null,
    error: null,
    createdAt: SAMPLE_NOW,
    startedAt: SAMPLE_NOW,
    finishedAt: SAMPLE_NOW,
    proposals: [
      {
        id: `claim_proposal_sample_${spec.key}`,
        jobId: `claim_job_sample_${spec.key}`,
        statement: `${spec.name} expects EBITDA margin to expand`,
        dueFiscalYear: 2026,
        duePeriodType: "FY",
        targetMetricKey: "ebitda_margin",
        targetComparator: "gte",
        targetValueNumeric: "0.18",
        targetUnit: "ratio",
        confidence: "0.72",
        sourceSnippet: "We expect margins to expand next year.",
        sourceEvidenceType: "transcript_segment",
        sourceEvidenceId: `segment_sample_${spec.key}_0`,
        status: "pending",
        claimId: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      },
    ],
  };
}

// ============================================================================
// Fundamentals (financials + KPIs + report documents)
// ============================================================================

export function makeFinancialPeriod(spec: CompanySpec, fiscalYear: number): FinancialPeriod {
  return {
    id: `period_sample_${spec.key}_${fiscalYear}_FY`,
    companyId: companyId(spec),
    fiscalYear,
    periodType: "FY",
    periodEndDate: `${fiscalYear}-12-31`,
    reportEvidenceRef: `report_doc_sample_${spec.key}`,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeFinancialFact(spec: CompanySpec, fiscalYear: number): FinancialFact {
  return {
    id: `fact_sample_${spec.key}_revenue`,
    companyId: companyId(spec),
    periodId: `period_sample_${spec.key}_${fiscalYear}_FY`,
    definitionId: "kpi_def_revenue",
    valueNumeric: "1050000000",
    currency: "PLN",
    statementBasis: "consolidated",
    attribution: "Annual report",
    variant: "reported",
    measureWindow: "FY",
    dataQuality: "reported",
    asReportedValue: "1050.0",
    asReportedScale: "million",
    reportingStandard: "IFRS",
    extractionMethod: "ai_assisted",
    confidence: "0.9",
    confirmationState: "confirmed",
    supersedesId: null,
    sourceDocumentRef: `report_doc_sample_${spec.key}`,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeKpiDefinition(): KpiDefinition {
  return {
    id: "kpi_def_revenue",
    scope: "global",
    companyId: null,
    sector: null,
    metricKey: "revenue",
    label: "Revenue",
    valueKind: "currency",
    unit: "PLN",
    computation: "reported",
    formula: null,
    displayFormat: "currency",
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeKpiRelevance(spec: CompanySpec): KpiRelevance {
  return {
    id: `kpi_rel_sample_${spec.key}`,
    companyId: companyId(spec),
    definitionId: "kpi_def_revenue",
    status: "active",
    source: "ai",
    rank: "1",
    firstSeenPeriod: `period_sample_${spec.key}_2025_FY`,
    lastSeenPeriod: `period_sample_${spec.key}_2026_FY`,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeKpiExtractionJob(spec: CompanySpec): KpiExtractionJob {
  return {
    id: `kpi_job_sample_${spec.key}`,
    companyId: companyId(spec),
    reportDocumentId: `report_doc_sample_${spec.key}`,
    providerId: "provider_gemini",
    model: "gemini-2.5-flash",
    promptVersion: "kpi_extraction.v1",
    periodHint: "2026-FY",
    status: "succeeded",
    errorCode: null,
    error: null,
    detectedFiscalYear: 2026,
    detectedPeriodType: "FY",
    detectedPeriodEndDate: "2026-12-31",
    detectedCurrency: "PLN",
    detectedLanguage: "pl",
    createdAt: SAMPLE_NOW,
    startedAt: SAMPLE_NOW,
    finishedAt: SAMPLE_NOW,
    proposals: [
      {
        id: `kpi_proposal_sample_${spec.key}`,
        jobId: `kpi_job_sample_${spec.key}`,
        metricKey: "revenue",
        label: "Revenue",
        valueNumeric: "1050000000",
        unit: "PLN",
        currency: "PLN",
        asReportedValue: "1050.0",
        asReportedScale: "million",
        measureWindow: "FY",
        confidence: "0.9",
        sourceSnippet: "Revenue of PLN 1,050m.",
        isProposedKpi: false,
        status: "pending",
        factId: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      },
    ],
  };
}

export function makeReportDocument(spec: CompanySpec): ReportDocument {
  return {
    id: `report_doc_sample_${spec.key}`,
    companyId: companyId(spec),
    periodId: `period_sample_${spec.key}_2026_FY`,
    sourceType: "ir_page",
    originRef: `https://example.test/ir/${spec.key}`,
    url: `https://example.test/ir/${spec.key}/annual-2026.pdf`,
    localPath: null,
    contentType: "application/pdf",
    contentHash: `hash_${spec.key}`,
    byteSize: 1048576,
    title: `${spec.name} annual report 2026`,
    attribution: `${spec.name} IR`,
    fetchStatus: "fetched",
    fetchError: null,
    fetchedAt: SAMPLE_NOW,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeReportPreparation(spec: CompanySpec): ReportPreparation {
  return {
    companyId: companyId(spec),
    eventKey: `${spec.key}-earnings-2026`,
    status: "prepared",
    preparedAt: SAMPLE_NOW,
    processedAt: null,
    linkedReportDocumentId: `report_doc_sample_${spec.key}`,
  };
}

export function makeReportSeasonEntry(spec: CompanySpec, upcoming: boolean): ReportSeasonEntry {
  return {
    companyId: companyId(spec),
    qualifiedTicker: qualifiedTicker(spec),
    displayName: spec.name,
    eventKey: `${spec.key}-earnings-2026`,
    eventDate: upcoming ? "2026-07-15" : "2026-04-15",
    eventTime: "08:00",
    title: `${spec.name} ${upcoming ? "Q2" : "Q1"} 2026 results`,
    preparationStatus: upcoming ? "prepared" : "processed",
  };
}

export function makePreReportCard(spec: CompanySpec): PreReportCard {
  return {
    companyId: companyId(spec),
    eventKey: `${spec.key}-earnings-2026`,
    eventDate: "2026-07-15",
    preparationStatus: "prepared",
    linkedReportDocumentId: `report_doc_sample_${spec.key}`,
    openQuestions: [makeResearchQuestion(spec)],
    unresolvedClaims: makeClaimsToVerify([spec]),
    lastPeriodKpis: [
      {
        periodId: `period_sample_${spec.key}_2025_FY`,
        metricKey: "revenue",
        label: "Revenue",
        unit: "PLN",
        valueNumeric: "980000000",
      },
    ],
    recentEvidence: [makeResearchEvidenceItem(spec)],
  };
}

// ============================================================================
// Sources
// ============================================================================

type SourceAdapterSpec = {
  id: string;
  displayName: string;
  sourceType: string;
  fetchMode: string;
  visibility: SourceAdapter["visibility"];
  userConfigurable: boolean;
  healthStatus: SourceAdapter["healthStatus"];
  enabled: boolean;
  sourceUrl: string;
  markets: string[];
};

const SOURCE_ADAPTER_SPECS: readonly SourceAdapterSpec[] = [
  {
    id: "gpw-company-registry",
    displayName: "GPW Company Directory",
    sourceType: "company_registry",
    fetchMode: "public_page",
    visibility: "required",
    userConfigurable: false,
    healthStatus: "healthy",
    enabled: true,
    sourceUrl: "https://www.gpw.pl/spolki",
    markets: ["GPW"],
  },
  {
    id: "bankier-company-komunikaty",
    displayName: "Bankier Company Komunikaty",
    sourceType: "official_report",
    fetchMode: "public_json",
    visibility: "required",
    userConfigurable: true,
    healthStatus: "healthy",
    enabled: true,
    sourceUrl: "https://www.bankier.pl/gielda/notowania/akcje/{TICKER}/komunikaty",
    markets: ["GPW"],
  },
  {
    id: "bankier-market-rss",
    displayName: "Bankier Giełda RSS",
    sourceType: "public_media",
    fetchMode: "rss",
    visibility: "optional",
    userConfigurable: true,
    healthStatus: "attention",
    enabled: true,
    sourceUrl: "https://www.bankier.pl/rss/gielda.xml",
    markets: ["GPW"],
  },
  {
    id: "bankier-wiadomosci-rss",
    displayName: "Bankier Wiadomosci RSS",
    sourceType: "public_media",
    fetchMode: "rss",
    visibility: "optional",
    userConfigurable: true,
    healthStatus: "notRefreshed",
    enabled: false,
    sourceUrl: "https://www.bankier.pl/rss/wiadomosci.xml",
    markets: ["GPW"],
  },
  {
    id: "portal-analiz",
    displayName: "Portal Analiz",
    sourceType: "authenticated_research",
    fetchMode: "authenticated",
    visibility: "developer",
    userConfigurable: false,
    healthStatus: "off",
    enabled: false,
    sourceUrl: "https://portalanaliz.pl/",
    markets: ["GPW"],
  },
];

export function makeSourceAdapter(spec: SourceAdapterSpec): SourceAdapter {
  const healthy = spec.healthStatus === "healthy";
  return {
    id: spec.id,
    displayName: spec.displayName,
    sourceType: spec.sourceType,
    fetchMode: spec.fetchMode,
    visibility: spec.visibility,
    userConfigurable: spec.userConfigurable,
    healthStatus: spec.healthStatus,
    enabled: spec.enabled,
    defaultPollIntervalSeconds: spec.enabled ? 900 : 0,
    sourceUrl: spec.sourceUrl,
    rateLimitPolicy: "Manual refresh plus normal in-app source scheduler",
    policyNote: `Sample policy note for ${spec.displayName}.`,
    lastAttemptAt: spec.enabled ? SAMPLE_NOW : null,
    lastTrigger: spec.enabled ? "scheduler" : null,
    lastSuccessAt: healthy ? SAMPLE_NOW : null,
    lastErrorAt: spec.healthStatus === "attention" ? SAMPLE_NOW : null,
    lastError: spec.healthStatus === "attention" ? "Sample fetch warning" : null,
    lastItemsFetched: healthy ? 12 : null,
    lastItemsCreated: healthy ? 4 : null,
    lastItemsMatched: healthy ? 3 : null,
    lastItemsUnmatched: healthy ? 1 : null,
    lastDetailItemsAttempted: healthy ? 4 : null,
    lastDetailItemsStored: healthy ? 4 : null,
    lastDetailItemsFailed: healthy ? 0 : null,
    lastDetailWarning: null,
    markets: spec.markets,
  };
}

export function makeSourceAdapters(): SourceAdapter[] {
  return SOURCE_ADAPTER_SPECS.map(makeSourceAdapter);
}

export function makeUnmatchedSourceItem(spec: CompanySpec): UnmatchedSourceItem {
  return {
    id: `unmatched_sample_${spec.key}`,
    adapterId: "bankier-market-rss",
    companyName: `${spec.name} (unmatched)`,
    title: `Unmatched headline mentioning ${spec.name}`,
    sourceUrl: `https://example.test/unmatched/${spec.key}`,
    publishedAt: SAMPLE_NOW,
    fetchedAt: SAMPLE_NOW,
  };
}

export function makeSourceIngestionResult(adapterId: string): SourceIngestionResult {
  return {
    adapterId,
    itemsFetched: 12,
    itemsCreated: 4,
    itemsMatched: 3,
    itemsUnmatched: 1,
    detailItemsAttempted: 4,
    detailItemsStored: 4,
    detailItemsFailed: 0,
    fetchedAt: SAMPLE_NOW,
  };
}

export function makeBackfillProgress(spec: CompanySpec): BackfillProgress {
  return {
    companyId: companyId(spec),
    status: "completed",
    pagesFetched: 3,
    itemsIngested: 24,
    documentsStored: 6,
    detailErrors: 0,
    error: null,
    startedAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeIrReportResolution(spec: CompanySpec): IrReportResolution {
  return {
    document: makeReportDocument(spec),
    candidates: [
      { url: `https://example.test/ir/${spec.key}/annual-2026.pdf`, label: "Annual report 2026" },
      { url: `https://example.test/ir/${spec.key}/q1-2026.pdf`, label: "Q1 2026 report" },
    ],
    pickedUrl: `https://example.test/ir/${spec.key}/annual-2026.pdf`,
    confidence: "high",
  };
}

// ============================================================================
// Platform singletons
// ============================================================================

export function makeUserSettings(): UserSettings {
  return {
    theme: "dark",
    locale: "en",
    accentPalette: "night-neon",
    developerMode: false,
    pollIntervalSeconds: 900,
    settingsSource: "sample",
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
    queue: { sourcesWorkers: 2, autopilotWorkers: 3, aiWorkers: 2, aiProviderConcurrency: 2 },
    pinnedCompanyIds: [],
  };
}

export function makeLicenseStatus(kind: LicenseStatusKind): LicenseStatus {
  const valid = kind === "valid";
  return {
    status: kind,
    canUseApp: valid,
    reason: valid ? null : `License is ${kind}`,
    license: valid
      ? {
          licenseId: "lic_sample_0001",
          holder: "Sample Holder",
          channel: "direct",
          edition: "standard",
          features: ["research", "transcripts"],
          issuedAt: SAMPLE_NOW,
          expiresAt: "2027-06-08T10:00:00Z",
          appVersionRange: ">=0.40.0 <1.0.0",
          keyId: "key_sample_01",
        }
      : null,
    checkedAt: SAMPLE_NOW,
  };
}

export const AI_PROVIDER_CATALOG: readonly AiProviderCatalogEntry[] = [
  {
    providerId: "provider_gemini",
    label: "Gemini",
    models: ["gemini-3.5-flash", "gemini-3.1-pro-preview", "gemini-2.5-flash", "gemini-2.5-flash-lite"],
    defaultModel: "gemini-3.5-flash",
    requiresCredential: true,
  },
  {
    providerId: "provider_anthropic",
    label: "Claude (Anthropic)",
    models: ["claude-sonnet-4-6", "claude-opus-4-8", "claude-haiku-4-5-20251001"],
    defaultModel: "claude-sonnet-4-6",
    requiresCredential: true,
  },
  {
    providerId: "provider_openai",
    label: "OpenAI (ChatGPT)",
    models: ["gpt-5.5", "gpt-5.1"],
    defaultModel: "gpt-5.5",
    requiresCredential: true,
  },
] as const;

export function makeCredentialStatuses(): CredentialStatus[] {
  return AI_PROVIDER_CATALOG.map((entry) => ({
    providerId: entry.providerId,
    secretKind: "api_key",
    configured: entry.providerId === "provider_gemini",
    storage: "keychain",
    label: `${entry.label} API key`,
    devFallbackAvailable: entry.providerId === "provider_gemini",
    error: null,
  }));
}

export function makeLocalMetricsSnapshot(): LocalMetricsSnapshot {
  return {
    collectedAt: SAMPLE_NOW,
    samples: [
      {
        name: "feed_items_total",
        description: "Total feed items stored",
        kind: "gauge",
        unit: "count",
        value: 42,
        labels: [],
        collectedAt: SAMPLE_NOW,
      },
    ],
  };
}

export function makeDiagnosticEvent(): DiagnosticEvent {
  return {
    id: "diag_sample_0001",
    occurredAt: SAMPLE_NOW,
    module: "sources",
    scope: { type: "adapter", id: "bankier-market-rss" },
    stage: "fetch",
    severity: "info",
    message: "Sample diagnostic event for the developer console.",
    metadata: { itemsFetched: 12 },
    createdAt: SAMPLE_NOW,
  };
}

export function makeDiagnosticSummary(): DiagnosticSummary {
  return { summary: "1 diagnostic event recorded.", eventCount: 1 };
}

export function makeLogEntry(): LogEntry {
  return {
    fileName: "brawler.log",
    lineNumber: 1,
    record: { level: "info", message: "Sample log line", target: "brawler" },
  };
}

export function makeLogStatus(): LogStatus {
  return {
    logsDir: "/sample/logs",
    currentFileBytes: 4096,
    rotatedFileCount: 1,
    level: "info",
    maxFiles: 5,
    maxFileBytes: 5_242_880,
  };
}

export function makeEmbeddingModelStatus(): EmbeddingModelStatus {
  return {
    modelId: "bge-small-en-v1.5",
    dim: 384,
    weightsState: "ready",
    downloadProgress: null,
    downloadError: null,
    activeSimilarityStrategy: "embedding",
    embeddedCounts: [
      { contentType: "feed_item", count: 12 },
      { contentType: "notebook_entry", count: 6 },
    ],
    indexModelId: "bge-small-en-v1.5",
    featureCompiled: true,
  };
}

export function makeDatabaseStatus(companies: number, sourceAdapters: number): DatabaseStatus {
  return { appliedMigrations: 60, companies, sourceAdapters, settings: 1 };
}

export function makeBackupStatus(): BackupStatus {
  return {
    lastBackupAt: SAMPLE_NOW,
    backupCount: 2,
    backups: [
      { fileName: "brawler-2026-06-08.snapshot.sqlite", createdAt: SAMPLE_NOW, kind: "snapshot", sizeBytes: 2_097_152 },
      { fileName: "brawler-rotating-01.sqlite", createdAt: SAMPLE_NOW, kind: "rotating", sizeBytes: 1_048_576 },
    ],
  };
}

// ============================================================================
// Quality frameworks (ADR 0044)
// ============================================================================

export function makeQualityFramework(): QualityFramework {
  const id = "framework_sample_moat";
  return {
    id,
    name: "Economic moat checklist",
    description: "Sample qualitative framework for moat assessment.",
    origin: "app_template",
    templateKey: "moat_v1",
    clonedFrom: null,
    version: 1,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
    criteria: [
      {
        id: "criterion_sample_roic",
        frameworkId: id,
        ordinal: 0,
        label: "ROIC above cost of capital",
        expression: "roic > 0.10",
        weight: "2",
        partialBand: "0.02",
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      },
      {
        id: "criterion_sample_margin",
        frameworkId: id,
        ordinal: 1,
        label: "Stable operating margin",
        expression: "operating_margin > 0.15",
        weight: "1",
        partialBand: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      },
    ],
  };
}

export function makeFrameworkEvaluation(spec: CompanySpec): FrameworkEvaluation {
  const id = `evaluation_sample_${spec.key}`;
  return {
    id,
    frameworkId: "framework_sample_moat",
    frameworkVersion: 1,
    companyId: companyId(spec),
    periodId: `period_sample_${spec.key}_2026_FY`,
    passCount: 1,
    partialCount: 1,
    failCount: 0,
    unavailableCount: 0,
    engineVersion: "framework_engine.v1",
    createdAt: SAMPLE_NOW,
    results: [
      {
        id: `crit_result_sample_${spec.key}_roic`,
        evaluationId: id,
        criterionId: "criterion_sample_roic",
        ordinal: 0,
        label: "ROIC above cost of capital",
        expression: "roic > 0.10",
        verdict: "pass",
        measuredValue: "0.14",
        measuredUnit: "ratio",
        threshold: "0.10",
        inputsJson: '{"roic":0.14}',
        note: null,
      },
      {
        id: `crit_result_sample_${spec.key}_margin`,
        evaluationId: id,
        criterionId: "criterion_sample_margin",
        ordinal: 1,
        label: "Stable operating margin",
        expression: "operating_margin > 0.15",
        verdict: "partial",
        measuredValue: "0.14",
        measuredUnit: "ratio",
        threshold: "0.15",
        inputsJson: '{"operating_margin":0.14}',
        note: "Just below threshold.",
      },
    ],
  };
}

export const AVAILABLE_METRIC_KEYS: readonly MetricKeyInfo[] = [
  { key: "revenue", label: "Revenue", unit: "PLN", valueKind: "currency", computation: "reported", scope: "global" },
  { key: "ebitda_margin", label: "EBITDA margin", unit: "ratio", valueKind: "ratio", computation: "derived", scope: "global" },
  { key: "roic", label: "ROIC", unit: "ratio", valueKind: "ratio", computation: "derived", scope: "global" },
  { key: "operating_margin", label: "Operating margin", unit: "ratio", valueKind: "ratio", computation: "derived", scope: "global" },
] as const;
