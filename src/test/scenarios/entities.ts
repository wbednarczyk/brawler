// Canonical test entity builders (ADR 0048 Keystone B/C, Radicle 749a5a8).
//
// One source of truth for realistic mock data, typed against the ts-rs-GENERATED
// api DTOs so a seed cannot drift from the contract. Builders are deterministic
// (fixed IDs/timestamps, no wall-clock/random) and pure (return fresh objects);
// `scenarios.ts` composes them into empty/minimal/rich datasets that the mock
// runtime clones per test. IDs use a single `*_sample_*` scheme.

import type {
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
  PresentationKind,
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
import type { AlertRule } from "../../api/generated/AlertRule";
import type { AttentionEvent } from "../../api/generated/AttentionEvent";
import type { AttentionSeverity } from "../../api/generated/AttentionSeverity";
import type { ReconciliationResult } from "../../api/generated/ReconciliationResult";
import type { BackupStatus } from "../../api/backups";
import type { IrReportResolution } from "../../api/ir";
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
import type {
  KpiComparison,
  KpiComparisonCell,
  KpiComparisonPeriod,
  KpiComparisonSeries,
} from "../../api/comparison";
import type { ReportDocument } from "../../api/reportDocumentsTypes";
import type { OwnershipOverview } from "../../api/ownership";
import type { CompanyHealth } from "../../api/companyHealth";
import type { InsiderOverview } from "../../api/insider";
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
  {
    key: "dnp",
    ticker: "DNP",
    name: "Dino Polska",
    sector: "Consumer Staples",
  },
  { key: "lpp", ticker: "LPP", name: "LPP", sector: "Consumer Discretionary" },
  {
    key: "cps",
    ticker: "CPS",
    name: "Cyfrowy Polsat",
    sector: "Communication",
  },
  { key: "ccc", ticker: "CCC", name: "CCC", sector: "Consumer Discretionary" },
  { key: "alr", ticker: "ALR", name: "Alior Bank", sector: "Financials" },
  { key: "opl", ticker: "OPL", name: "Orange Polska", sector: "Communication" },
  {
    key: "spl",
    ticker: "SPL",
    name: "Santander Bank Polska",
    sector: "Financials",
  },
  { key: "mbk", ticker: "MBK", name: "mBank", sector: "Financials" },
  { key: "kru", ticker: "KRU", name: "Kruk", sector: "Financials" },
  { key: "tpe", ticker: "TPE", name: "Tauron", sector: "Energy" },
  { key: "ena", ticker: "ENA", name: "Enea", sector: "Energy" },
  {
    key: "jsw",
    ticker: "JSW",
    name: "Jastrzebska Spolka Weglowa",
    sector: "Materials",
  },
  { key: "gpw", ticker: "GPW", name: "GPW Exchange", sector: "Financials" },
  {
    key: "ale",
    ticker: "ALE",
    name: "Allegro",
    sector: "Consumer Discretionary",
  },
  { key: "atc", ticker: "ATC", name: "Arctic Paper", sector: "Materials" },
  { key: "bdx", ticker: "BDX", name: "Budimex", sector: "Industrials" },
  { key: "cbf", ticker: "CBF", name: "Cuprum Bank Fund", sector: "Financials" },
  {
    key: "ats",
    ticker: "ATS",
    name: "Amica",
    sector: "Consumer Discretionary",
  },
  { key: "kty", ticker: "KTY", name: "Grupa Kety", sector: "Materials" },
  { key: "mil", ticker: "MIL", name: "Bank Millennium", sector: "Financials" },
  {
    key: "pge",
    ticker: "PGE",
    name: "PGE Polska Grupa Energetyczna",
    sector: "Energy",
  },
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

export function makeRegistryEntry(
  spec: CompanySpec,
  tracked: boolean,
): CompanyRegistryEntry {
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

// Mirrors the backend derivation (`PresentationKind::derive`, F1 S1): real
// data only ever carries the three production `type` strings, so an unmapped
// mock-only pseudo-type (e.g. legacy "News"/"Transcript" fixtures) falls back
// to "media" rather than the backend's "filing" default — that keeps their
// summaries visible instead of wrongly suppressed by the dead-literal guard.
export function presentationKindFor(itemType: string, hasAttachments: boolean): PresentationKind {
  switch (itemType) {
    case "Red flag":
      return "redFlag";
    case "Official report":
      return hasAttachments ? "report" : "filing";
    default:
      return "media";
  }
}

export function makeFeedItem(spec: CompanySpec, index: number): FeedItem {
  const kinds = [
    {
      type: "Official report",
      source: "GPW ESPI/EBI",
      attribution: "GPW",
      language: "pl",
    },
    {
      type: "News",
      source: "Bankier RSS",
      attribution: "Bankier",
      language: "pl",
    },
    {
      type: "Transcript",
      source: "Sample transcript",
      attribution: "Sample",
      language: "en",
    },
  ];
  const kind = kinds[index % kinds.length];
  const hasAttachments = index % 3 === 0;
  return {
    id: `feed_sample_${spec.key}_${index}`,
    company: qualifiedTicker(spec),
    type: kind.type,
    presentationKind: presentationKindFor(kind.type, hasAttachments),
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
    attachments: hasAttachments
      ? [
          {
            id: `attach_${spec.key}_${index}`,
            label: "Attachment",
            url: `https://example.test/doc/${spec.key}.pdf`,
          },
        ]
      : [],
  };
}

export function makeSignal(
  spec: CompanySpec,
  confirmed: boolean,
): CompanySignal {
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

// Ownership overview seed (ADR 0072, v0.56 T6). Mirrors the storyboard's real
// CBF case: two founders + two OFE + treasury (the donut's coloured groups) with
// a derived 46.8% free float, two multi-year founder trajectories (the
// stakes-over-time charts), one holder awaiting AI classification (pending
// proposal), and one unreadable document (residual → OCR/AI review queue).
export function makeOwnershipOverview(spec: CompanySpec): OwnershipOverview {
  // The middle point is an `espi_filing`: a holder files one only on crossing a
  // statutory band, so it renders as a threshold-crossing marker on the
  // trajectory (ADR 0072 decision 5). The surrounding points are ordinary
  // periodic-report samples, so the seed exercises both marked and unmarked.
  const founderPoints = (pct: string) => [
    { asOf: "2023-12-31", capitalPct: pct, source: "report_document" },
    { asOf: "2024-12-31", capitalPct: pct, source: "espi_filing" },
    { asOf: "2025-12-31", capitalPct: pct, source: "report_document" },
  ];
  return {
    companyId: companyId(spec),
    asOf: "2025-12-31",
    source: "report_document",
    freeFloatPct: "46.8",
    disclosedSum: "53.2",
    freeFloatHistory: [
      { asOf: "2023-12-31", pct: "52.4" },
      { asOf: "2024-12-31", pct: "49.1" },
      { asOf: "2025-12-31", pct: "46.8" },
    ],
    holders: [
      {
        holderKey: "JACEK DUCH",
        name: "Jacek Duch",
        holderType: "founder_insider",
        capitalPct: "25.5",
        votesPct: "25.5",
        asOf: "2025-12-31",
        source: "report_document",
        skinInTheGame: { person: "Jacek Duch" },
      },
      {
        holderKey: "JAKUB DWERNICKI",
        name: "Jakub Dwernicki",
        holderType: "founder_insider",
        capitalPct: "15.9",
        votesPct: "15.9",
        asOf: "2025-12-31",
        source: "report_document",
        skinInTheGame: {
          person: "Jakub Dwernicki",
          via: "Dwernicki Fundacja Rodzinna",
        },
      },
      {
        holderKey: "NN PTE",
        name: "NN PTE",
        holderType: "ofe_pension",
        capitalPct: "6.0",
        votesPct: "6.0",
        asOf: "2025-12-31",
        source: "report_document",
      },
      {
        holderKey: "PTE ALLIANZ POLSKA",
        name: "PTE Allianz Polska",
        holderType: "ofe_pension",
        capitalPct: "5.3",
        votesPct: "5.3",
        asOf: "2025-12-31",
        source: "report_document",
      },
      {
        holderKey: "CYBER_FOLKS S.A.",
        name: "cyber_Folks S.A.",
        holderType: "treasury_shares",
        capitalPct: "0.5",
        votesPct: "0.5",
        asOf: "2025-12-31",
        source: "report_document",
      },
      {
        holderKey: "ITEMA VENTURES UAB",
        name: "Itema Ventures UAB",
        asOf: "2025-12-31",
        source: "report_document",
      },
    ],
    history: [
      {
        holderKey: "JACEK DUCH",
        name: "Jacek Duch",
        holderType: "founder_insider",
        points: founderPoints("25.5"),
      },
      {
        holderKey: "JAKUB DWERNICKI",
        name: "Jakub Dwernicki",
        holderType: "founder_insider",
        points: founderPoints("15.9"),
      },
    ],
    residuals: [
      // Unreadable residual → the warnbox reports it as a flagged gap.
      {
        reportDocumentId: `doc_${spec.key}_2023`,
        parseState: "glyph_encoded",
        detectedAsOf: "2023-12-31",
        matchedHeading: "Akcjonariat",
      },
      // A second unreadable residual: an honest flagged gap with no OCR action
      // (ADR 0084).
      {
        reportDocumentId: `doc_${spec.key}_2022`,
        parseState: "glyph_encoded",
        detectedAsOf: "2022-12-31",
        matchedHeading: "Akcjonariat",
      },
    ],
  };
}

/// A populated company-health read model (ADR 0083): a full-input latest FY
/// (F headline + Z″ safe) plus a prior FY whose Piotroski is insufficient
/// (missing long_term_debt) — so the Quality-panel health tiles, the
/// per-component breakdown, and the insufficient-data state all render.
export function makeCompanyHealth(spec: CompanySpec): CompanyHealth {
  const id = companyId(spec);
  return {
    companyId: id,
    statementType: "industrial",
    piotroskiVariant: "Piotroski F (2000)",
    altmanVariant: "Altman Z″ EM (1995)",
    latest: {
      periodId: `${id}_fy2025`,
      fiscalYear: 2025,
      piotroski: {
        state: "headline",
        score: 8,
        signals: [
          {
            code: "F1",
            name: "roa_positive",
            passed: true,
            points: 1,
            inputs: [
              { key: "net_profit@FY2025", value: "100" },
              { key: "total_assets@FY2025", value: "1000" },
            ],
          },
          {
            code: "F9",
            name: "turnover_improved",
            passed: false,
            points: 0,
            inputs: [
              { key: "revenue@FY2025", value: "1000" },
              { key: "total_assets@FY2025", value: "1000" },
              { key: "basis", value: "two_year_average" },
            ],
          },
        ],
      },
      // Altman is insufficient here (a real coverage gap — retained_earnings is
      // often note-level) so the panel exercises the insufficient-data render +
      // missing-input list alongside the Piotroski headline breakdown.
      altman: {
        state: "insufficient_data",
        components: [
          {
            code: "X1",
            name: "working_capital_to_assets",
            weight: "6.56",
            ratio: "0.25",
            contribution: "1.64",
            inputs: [
              { key: "working_capital@FY2025", value: "250" },
              { key: "total_assets@FY2025", value: "1000" },
            ],
          },
        ],
        missing: [{ metric: "retained_earnings", period: "FY2025" }],
      },
    },
    history: [
      {
        periodId: `${id}_fy2025`,
        fiscalYear: 2025,
        piotroski: { state: "insufficient_data", signals: [], missing: [] },
        altman: { state: "insufficient_data", components: [], missing: [] },
      },
    ],
  };
}

/// A populated insider overview (ADR 0083 D7): a management-holdings group (a
/// founder holding indirectly via a foundation with an unstated share count, and
/// a management member with a real count), a multi-transaction timeline (buy /
/// sell / a directionless filing / a closely-associated filing dated via its
/// filing signal), and both windows computed above the 2-transaction minimum with
/// the volume-coverage note. Mirrors the ownership seed's founder (Jakub
/// Dwernicki via his family foundation) so the two Ownership-area blocks agree.
export function makeInsiderOverview(spec: CompanySpec): InsiderOverview {
  const id = companyId(spec);
  return {
    companyId: id,
    holdings: [
      {
        person: "Jakub Dwernicki",
        role: "management",
        shares: null,
        indirectVia: "Dwernicki Fundacja Rodzinna",
        asOf: "2025-12-31",
      },
      {
        person: "Anna Nowak",
        role: "supervisory",
        shares: "12000",
        indirectVia: null,
        asOf: "2025-12-31",
      },
    ],
    transactions: [
      {
        id: `insidertx_${spec.key}_1`,
        person: "Jakub Dwernicki",
        role: "management",
        relatedPdmr: null,
        direction: "buy",
        instrument: "shares",
        volume: "5000",
        price: "24.80",
        currency: "PLN",
        txDate: "2026-05-20",
        effectiveDate: "2026-05-20",
        dateSource: "transaction",
        feedItemId: `feed_${spec.key}_ins1`,
        sourceUrl: "https://www.bankier.pl/wiadomosc/insider-1.html",
      },
      {
        id: `insidertx_${spec.key}_2`,
        person: "Anna Nowak",
        role: "supervisory",
        relatedPdmr: null,
        direction: "sell",
        instrument: "shares",
        volume: "1500",
        price: "25.10",
        currency: "PLN",
        txDate: "2026-05-12",
        effectiveDate: "2026-05-12",
        dateSource: "transaction",
        feedItemId: `feed_${spec.key}_ins2`,
        sourceUrl: "https://www.bankier.pl/wiadomosc/insider-2.html",
      },
      {
        id: `insidertx_${spec.key}_3`,
        person: "Dwernicki Fundacja Rodzinna",
        role: "closely_associated",
        relatedPdmr: "Jakub Dwernicki",
        direction: "buy",
        instrument: "shares",
        volume: null,
        price: null,
        currency: null,
        txDate: null,
        effectiveDate: "2026-04-30",
        dateSource: "filing",
        feedItemId: `feed_${spec.key}_ins3`,
        sourceUrl: "https://www.bankier.pl/wiadomosc/insider-3.html",
      },
    ],
    // 3 in-window (2 buys, 1 sell) → net +1; two of three disclosed a volume.
    window90d: {
      state: "computed",
      count: 3,
      buys: 2,
      sells: 1,
      undetermined: 0,
      net: 1,
      buyVolume: "5000",
      sellVolume: "1500",
      volumeKnown: 2,
      volumeTotal: 3,
    },
    window12m: {
      state: "computed",
      count: 3,
      buys: 2,
      sells: 1,
      undetermined: 0,
      net: 1,
      buyVolume: "5000",
      sellVolume: "1500",
      volumeKnown: 2,
      volumeTotal: 3,
    },
  };
}

// Analyst recommendations panel view (v0.58 A3, ADR 0073): a small attributed
// history — an upgrade (with derived prior rating/target), an initiate that keeps
// a target, and a partial entry with neither target nor broker PDF. The newest
// target-carrying entry drives the "vs target" readout; `lastRefreshedAt` feeds
// the footer's honest refresh line. Third-party opinions — never advice.
export function makeAnalystRecommendationsView(
  spec: CompanySpec,
): AnalystRecommendationsView {
  const source = `https://www.biznesradar.pl/rekomendacje-spolki/${spec.ticker}`;
  return {
    companyId: companyId(spec),
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
        reportUrl: `https://static.example/rec/${spec.key}-noble-2026-06.pdf`,
        sourceUrl: source,
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
        reportUrl: `https://static.example/rec/${spec.key}-bos-2026-02.pdf`,
        sourceUrl: source,
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
        sourceUrl: source,
      },
    ],
    latestTarget: {
      firm: "Noble Securities",
      targetPrice: "250.00",
      targetCurrency: "PLN",
      publishedAt: "2026-06-18T08:40:00",
    },
    lastRefreshedAt: SAMPLE_NOW,
  };
}

// A `recommendation_change` filing signal (v0.58 A3, ADR 0073) so the category
// renders in the feed/Inbox badges, digests, and the Alerts dropdown. Emitted
// directly by the adapter (rule-classified, confirmed), never AI-proposed.
export function makeRecommendationSignal(spec: CompanySpec): CompanySignal {
  return {
    id: `signal_sample_${spec.key}_recommendation`,
    companyId: companyId(spec),
    company: qualifiedTicker(spec),
    companyName: spec.name,
    feedItemId: `feed_sample_${spec.key}_0`,
    category: "recommendation_change",
    categoryDisplayName: "Analyst recommendation change",
    confidence: 1.0,
    classifiedBy: "rule",
    status: "confirmed",
    signalDate: SAMPLE_NOW,
    providerId: null,
    modelId: null,
    derivedEventId: null,
    title: `${spec.name} — Noble Securities: akumuluj (cel 250,00 zł)`,
    sourceUrl: `https://www.biznesradar.pl/rekomendacje-spolki/${spec.ticker}`,
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

export function makeNotebookEntry(
  spec: CompanySpec,
  index: number,
): NotebookEntry {
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

export function makeTranscriptSegment(
  spec: CompanySpec,
  index: number,
): TranscriptSegment {
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

export function makeWatchlist(
  id: string,
  name: string,
  companyCount: number,
): Watchlist {
  return { id, name, description: null, companyCount };
}

export function makeMembership(
  watchlistId: string,
  watchlistName: string,
  spec: CompanySpec,
): WatchlistMembership {
  return { watchlistId, watchlistName, companyId: companyId(spec) };
}

// ============================================================================
// Attention routing — alert rules + fired events (ADR 0068)
// ============================================================================

export function makeAlertRule(
  id: string,
  triggerType: AlertRule["triggerType"],
  scopeRef: string,
): AlertRule {
  return {
    id,
    triggerType,
    signalCategory: triggerType === "signal_category" ? "profit_warning" : null,
    priceMin: triggerType === "price_enters_range" ? 20 : null,
    priceMax: triggerType === "price_enters_range" ? 30 : null,
    scopeType: scopeRef.startsWith("watchlist") ? "watchlist" : "company",
    scopeRef,
    enabled: true,
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeAttentionEvent(
  id: string,
  ruleId: string,
  companyId: string,
  // Mirrors the backend mapping (product-spec §Attention Routing / `storage::severity`):
  // the paired sample rule is `signal_category` + `profit_warning`, which maps to
  // `urgent`. Callers override for other trigger types.
  severity: AttentionSeverity = "urgent",
): AttentionEvent {
  return {
    id,
    ruleId,
    triggerType: "signal_category",
    companyId,
    evidenceType: "company_signal",
    evidenceRef: `signal_sample_${companyId}`,
    firedAt: SAMPLE_NOW,
    seen: false,
    dismissed: false,
    severity,
    // The filing's own title — the concrete statement a stream row shows (v0.60 D6).
    evidenceTitle: "Powiadomienie o transakcjach, o których mowa w art. 19 ust. 1 MAR",
    evidenceDetail: null,
    witnessUrl: null,
  };
}

// ============================================================================
// Research workspace
// ============================================================================

export function makeResearchEvidenceItem(
  spec: CompanySpec,
): ResearchEvidenceItem {
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
    reviewState: {
      changedSinceCompanyReview: true,
      changedSinceWatchlistReview: true,
    },
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

export function makeResearchReviewCheckpoint(
  spec: CompanySpec,
): ResearchReviewCheckpoint {
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
    verifyingFactCandidate: {
      factId: `fact_sample_${spec.key}_revenue`,
      valueNumeric: "1050000000",
    },
  };
}

export function makeClaimsToVerify(
  specs: readonly CompanySpec[],
): ClaimsToVerify {
  return {
    due: specs.slice(0, 1).map(makeClaimToVerify),
    overdue: specs.slice(1, 2).map(makeClaimToVerify),
    upcoming: specs.slice(2, 3).map(makeClaimToVerify),
  };
}

// ============================================================================
// Fundamentals (financials + KPIs + report documents)
// ============================================================================

export function makeFinancialPeriod(
  spec: CompanySpec,
  fiscalYear: number,
): FinancialPeriod {
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

export function makeFinancialFact(
  spec: CompanySpec,
  fiscalYear: number,
): FinancialFact {
  return {
    id: `fact_sample_${spec.key}_revenue`,
    companyId: companyId(spec),
    periodId: `period_sample_${spec.key}_${fiscalYear}_FY`,
    definitionId: "kpi_def_revenue",
    metricKey: "revenue",
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
    confidence: "high",
    confirmationState: "confirmed",
    supersedesId: null,
    sourceDocumentRef: `report_doc_sample_${spec.key}`,
    annotation: null,
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
    origin: "seed",
    statementGroup: "other",
    periodNature: "duration",
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

// A populated cross-company comparison (ADR 0089 dec. 1) for the Compare screen
// (§A3) — a fixed two-company × revenue × annual read model with three aligned
// FY periods, so the browser/visual harness renders the success table (the
// runtime mock's computed default is the empty comparison, since the base
// scenario carries no confirmed facts wired to these companies). PKN reports in
// PLN (native, delta-defined); TPE reports in EUR (FX chip + one fx_missing
// gap on the latest period), exercising the typed flags + EUR→PLN basis.
export function makeKpiComparison(
  companyA = "company_gpw_pkn",
  companyB = "company_gpw_tpe",
): KpiComparison {
  const axis: KpiComparisonPeriod[] = [
    { fiscalYear: 2022, periodType: "FY", key: "2022:FY" },
    { fiscalYear: 2023, periodType: "FY", key: "2023:FY" },
    { fiscalYear: 2024, periodType: "FY", key: "2024:FY" },
  ];
  const cell = (over: Partial<KpiComparisonCell> & { fiscalYear: number }): KpiComparisonCell => ({
    periodType: "FY",
    factId: null,
    value: null,
    currency: null,
    valuePln: null,
    fxBasis: null,
    validationStatus: null,
    deltaQoQ: null,
    deltaYoY: null,
    flags: [],
    ...over,
  });
  return {
    granularity: "annual",
    metricKeys: ["revenue"],
    axis,
    series: [
      {
        companyId: companyA,
        metricKey: "revenue",
        valueKind: "currency",
        cells: [
          cell({ fiscalYear: 2022, factId: "fact_cmp_pkn_2022", value: "953", currency: "PLN", valuePln: "953", fxBasis: "native_pln", validationStatus: "confirmed" }),
          cell({ fiscalYear: 2023, factId: "fact_cmp_pkn_2023", value: "1230", currency: "PLN", valuePln: "1230", fxBasis: "native_pln", validationStatus: "confirmed", deltaYoY: "29.07" }),
          cell({ fiscalYear: 2024, factId: "fact_cmp_pkn_2024", value: "985", currency: "PLN", valuePln: "985", fxBasis: "native_pln", validationStatus: "confirmed", deltaYoY: "-19.92" }),
        ],
      },
      {
        companyId: companyB,
        metricKey: "revenue",
        valueKind: "currency",
        cells: [
          cell({ fiscalYear: 2022, factId: "fact_cmp_tpe_2022", value: "388", currency: "EUR", valuePln: "1746", fxBasis: "period_average", validationStatus: "confirmed" }),
          cell({ fiscalYear: 2023, factId: "fact_cmp_tpe_2023", value: "412", currency: "EUR", valuePln: "1854", fxBasis: "period_average", validationStatus: "confirmed", deltaYoY: "6.19" }),
          cell({ fiscalYear: 2024, factId: "fact_cmp_tpe_2024", value: "451", currency: "EUR", valuePln: null, fxBasis: null, validationStatus: "confirmed", flags: ["fx_missing"] }),
        ],
      },
    ],
  };
}

// Profil comparison for the Compare screen's default view (v0.61 §A7). Pivots
// the whole seeded canonical catalog (`kpiDefinitions` order) over two companies
// at annual granularity, exercising every Różnica path: monetary multiples, a
// p.p. margin delta, a per-share "—", and one fx_missing gap. `companyA` reports
// in PLN, `companyB` in EUR (converted, plus one missing-rate cell). Matched by
// the runtime mock on (companyIds set, all six metricKeys, granularity="annual").
export function makeProfileComparison(
  companyA = "company_gpw_cdr",
  companyB = "company_gpw_cbf",
): KpiComparison {
  const axis: KpiComparisonPeriod[] = [
    { fiscalYear: 2022, periodType: "FY", key: "2022:FY" },
    { fiscalYear: 2023, periodType: "FY", key: "2023:FY" },
    { fiscalYear: 2024, periodType: "FY", key: "2024:FY" },
  ];
  const cell = (over: Partial<KpiComparisonCell> & { fiscalYear: number }): KpiComparisonCell => ({
    periodType: "FY",
    factId: null,
    value: null,
    currency: null,
    valuePln: null,
    fxBasis: null,
    validationStatus: null,
    deltaQoQ: null,
    deltaYoY: null,
    flags: [],
    ...over,
  });
  // PLN monetary series (companyA): valuePln == native value.
  const pln = (metricKey: string, values: number[]): KpiComparisonSeries => ({
    companyId: companyA,
    metricKey,
    valueKind: "monetary",
    cells: axis.map((period, index) =>
      cell({
        fiscalYear: period.fiscalYear,
        factId: `fact_${companyA}_${metricKey}_${period.fiscalYear}`,
        value: String(values[index]),
        currency: "PLN",
        valuePln: String(values[index]),
        fxBasis: "native_pln",
        validationStatus: "confirmed",
      }),
    ),
  });
  // EUR monetary series (companyB): converted valuePln; `null` marks an
  // fx_missing gap (no comparable number that period).
  const eur = (metricKey: string, valuesPln: (number | null)[]): KpiComparisonSeries => ({
    companyId: companyB,
    metricKey,
    valueKind: "monetary",
    cells: axis.map((period, index) => {
      const converted = valuesPln[index];
      if (converted === null) {
        return cell({
          fiscalYear: period.fiscalYear,
          factId: `fact_${companyB}_${metricKey}_${period.fiscalYear}`,
          value: "10",
          currency: "EUR",
          validationStatus: "confirmed",
          flags: ["fx_missing"],
        });
      }
      return cell({
        fiscalYear: period.fiscalYear,
        factId: `fact_${companyB}_${metricKey}_${period.fiscalYear}`,
        value: String(Math.round(converted / 4.3)),
        currency: "EUR",
        valuePln: String(converted),
        fxBasis: "period_average",
        validationStatus: "confirmed",
      });
    }),
  });
  // Per-share EPS series for companyB (native PLN per share).
  const perShare = (companyId: string, values: number[]): KpiComparisonSeries => ({
    companyId,
    metricKey: "eps",
    valueKind: "monetary",
    cells: axis.map((period, index) =>
      cell({
        fiscalYear: period.fiscalYear,
        factId: `fact_${companyId}_eps_${period.fiscalYear}`,
        value: String(values[index]),
        currency: "PLN",
        valuePln: String(values[index]),
        fxBasis: "native_pln",
        validationStatus: "confirmed",
      }),
    ),
  });
  // Percentage series: currency-less native value (the read model tags a
  // currency_unknown flag; the native value is the comparable number).
  const pct = (companyId: string, metricKey: string, values: number[]): KpiComparisonSeries => ({
    companyId,
    metricKey,
    valueKind: "percentage",
    cells: axis.map((period, index) =>
      cell({
        fiscalYear: period.fiscalYear,
        factId: `fact_${companyId}_${metricKey}_${period.fiscalYear}`,
        value: String(values[index]),
        validationStatus: "confirmed",
        flags: ["currency_unknown"],
      }),
    ),
  });
  return {
    granularity: "annual",
    metricKeys: ["revenue", "operating_profit", "net_profit", "ebitda", "eps", "gross_margin"],
    axis,
    series: [
      pln("revenue", [900, 950, 985]),
      eur("revenue", [360, 430, 110]),
      pln("operating_profit", [300, 350, 410]),
      eur("operating_profit", [150, 170, 41]),
      pln("net_profit", [400, 450, 481]),
      eur("net_profit", [140, 160, 38]),
      pln("ebitda", [500, 560, 620]),
      eur("ebitda", [220, 240, null]),
      // Per-share EPS (both in native currency): Różnica is always "—" (the def's
      // `per_share` unit makes an absolute multiple meaningless).
      pln("eps", [4.0, 4.5, 4.78]),
      perShare(companyB, [15.0, 15.5, 16.1]),
      pct(companyA, "gross_margin", [39.0, 40.0, 41.0]),
      pct(companyB, "gross_margin", [34.0, 34.5, 35.2]),
    ],
  };
}

// N=1 comparison for the Fundamentals periods × deltas section (v0.61 §A5). The
// panel requests its own KPI set (`["revenue", "net_profit", "eps"]` for the
// browser-harness CDR facts) at one granularity; this returns a populated,
// deliberately wide axis with inline deltas, one no_fact gap, and one
// undefined-delta flag so the section renders every rendering path.
export function makeCompanyPeriodsComparison(
  companyId = "company_gpw_cdr",
  granularity: "annual" | "quarterly" = "quarterly",
): KpiComparison {
  const axis: KpiComparisonPeriod[] =
    granularity === "annual"
      ? [
          { fiscalYear: 2022, periodType: "FY", key: "2022:FY" },
          { fiscalYear: 2023, periodType: "FY", key: "2023:FY" },
          { fiscalYear: 2024, periodType: "FY", key: "2024:FY" },
        ]
      : [
          { fiscalYear: 2024, periodType: "Q2", key: "2024:Q2" },
          { fiscalYear: 2024, periodType: "Q3", key: "2024:Q3" },
          { fiscalYear: 2024, periodType: "Q4", key: "2024:Q4" },
          { fiscalYear: 2025, periodType: "Q1", key: "2025:Q1" },
          { fiscalYear: 2025, periodType: "Q2", key: "2025:Q2" },
          { fiscalYear: 2025, periodType: "Q3", key: "2025:Q3" },
        ];
  const quarterly = granularity === "quarterly";
  const cell = (over: Partial<KpiComparisonCell> & { fiscalYear: number; periodType: string }): KpiComparisonCell => ({
    factId: null,
    value: null,
    currency: null,
    valuePln: null,
    fxBasis: null,
    validationStatus: null,
    deltaQoQ: null,
    deltaYoY: null,
    flags: [],
    ...over,
  });
  const monetary = (metricKey: string, base: number, step: number, mutate?: (index: number, spec: Partial<KpiComparisonCell>) => void): KpiComparisonSeries => ({
    companyId,
    metricKey,
    valueKind: "monetary",
    cells: axis.map((period, index) => {
      const value = String(base + index * step);
      const spec: Partial<KpiComparisonCell> = {
        factId: `fact_${companyId}_${metricKey}_${period.key.replace(":", "_")}`,
        value,
        currency: "PLN",
        valuePln: value,
        fxBasis: "native_pln",
        validationStatus: "confirmed",
        deltaYoY: index > 0 ? (index * 3.1).toFixed(2) : null,
        deltaQoQ: quarterly && index > 0 ? (index * 1.4).toFixed(2) : null,
      };
      mutate?.(index, spec);
      return cell({ fiscalYear: period.fiscalYear, periodType: period.periodType, ...spec });
    }),
  });
  return {
    granularity,
    metricKeys: ["revenue", "net_profit", "eps"],
    axis,
    series: [
      monetary("revenue", 228, 24),
      // Net profit's second period has an undefined YoY (non-positive base).
      monetary("net_profit", 41, 14, (index, spec) => {
        if (index === 1) {
          spec.deltaYoY = null;
          spec.flags = ["delta_yoy_undefined"];
        }
      }),
      // EPS's first period is a no_fact gap.
      monetary("eps", 1, 1, (index, spec) => {
        if (index === 0) {
          spec.factId = null;
          spec.value = null;
          spec.currency = null;
          spec.valuePln = null;
          spec.fxBasis = null;
          spec.validationStatus = null;
          spec.deltaYoY = null;
          spec.deltaQoQ = null;
          spec.flags = ["no_fact"];
        }
      }),
    ],
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
    docKind: "periodic_ssf",
    // Migration 0121 (epic #229 T2): NULL = bytes not sniffed yet.
    detectedContainer: null,
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

export function makeReportSeasonEntry(
  spec: CompanySpec,
  upcoming: boolean,
): ReportSeasonEntry {
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
    sourceUrl:
      "https://www.bankier.pl/gielda/notowania/akcje/{TICKER}/komunikaty",
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
    role: "primary",
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
    lastError:
      spec.healthStatus === "attention" ? "Sample fetch warning" : null,
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

export function makeUnmatchedSourceItem(
  spec: CompanySpec,
): UnmatchedSourceItem {
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

export function makeSourceIngestionResult(
  adapterId: string,
): SourceIngestionResult {
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
    truncated: false,
    chainedSweepId: `history_sweep:${companyId(spec)}:mock`,
    error: null,
    startedAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
}

export function makeIrReportResolution(spec: CompanySpec): IrReportResolution {
  return {
    candidates: [
      {
        url: `https://example.test/ir/${spec.key}/annual-2026.pdf`,
        label: "Annual report 2026",
      },
      {
        url: `https://example.test/ir/${spec.key}/q1-2026.pdf`,
        label: "Q1 2026 report",
      },
    ],
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
    backfillYears: 3,
    settingsSource: "sample",
    settingsImportExportFormat: "yaml",
    yamlImportExportStatus: "accepted_deferred",
    aiProviders: {
      youtubeTranscriptionProvider: "provider_gemini",
      youtubeTranscriptionModel: "gemini-2.5-flash",
      youtubeTranscriptionTimeoutSeconds: 300,
    },
    logs: { level: "info", maxFiles: 5, maxFileBytes: 5_242_880 },
    shortcutBindings: {},
    database: {
      maxConnections: 4,
      busyTimeoutMs: 5000,
      acquireTimeoutMs: 10000,
    },
    queue: { sourcesWorkers: 2, autopilotWorkers: 3 },
    pinnedCompanyIds: [],
    todayReviewedDays: [],
    mcp: { enabled: false, port: 8317, writesEnabled: false, kpiAcquisitionEnabled: false },
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
    models: [
      "gemini-3.5-flash",
      "gemini-3.1-pro-preview",
      "gemini-2.5-flash",
      "gemini-2.5-flash-lite",
    ],
    defaultModel: "gemini-3.5-flash",
    requiresCredential: true,
  },
  {
    providerId: "provider_anthropic",
    label: "Claude (Anthropic)",
    models: [
      "claude-sonnet-4-6",
      "claude-opus-4-8",
      "claude-haiku-4-5-20251001",
    ],
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
  {
    providerId: "provider_openai_compatible",
    label: "OpenAI-compatible (custom)",
    models: [],
    defaultModel: "",
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

export function makeReconciliationResult(): ReconciliationResult {
  return {
    id: "recon_sample_0001",
    witnessAdapterId: "gpw-espi-ebi",
    companyId: "company_sample",
    qualifiedTicker: "GPW:CDR",
    reportNumber: "15/2026",
    reportType: "Bieżący",
    disclosureDate: "2026-07-14",
    witnessTitle: "Sample reconciled ESPI report.",
    witnessUrl: "https://www.gpw.pl/komunikaty?id=15",
    status: "matched",
    primaryFeedItemId: "feed_sample",
    createdAt: SAMPLE_NOW,
    updatedAt: SAMPLE_NOW,
  };
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

export function makeDatabaseStatus(
  companies: number,
  sourceAdapters: number,
): DatabaseStatus {
  return { appliedMigrations: 60, companies, sourceAdapters, settings: 1 };
}

export function makeBackupStatus(): BackupStatus {
  return {
    lastBackupAt: SAMPLE_NOW,
    backupCount: 2,
    backups: [
      {
        fileName: "brawler-2026-06-08.snapshot.sqlite",
        createdAt: SAMPLE_NOW,
        kind: "snapshot",
        sizeBytes: 2_097_152,
      },
      {
        fileName: "brawler-rotating-01.sqlite",
        createdAt: SAMPLE_NOW,
        kind: "rotating",
        sizeBytes: 1_048_576,
      },
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
        kind: "quantitative",
        assessmentGuidance: null,
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
        kind: "quantitative",
        assessmentGuidance: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      },
      // A qualitative criterion with owner-authored guidance at REALISTIC length
      // (mirrors the shipped Kroeze template). Prose this long is exactly what
      // once forced panel-internal horizontal scroll — layout gates need it in
      // the sample data to bite (guardrail, ADR 0045).
      {
        id: "criterion_sample_moat",
        frameworkId: id,
        ordinal: 2,
        label: "Durable moat",
        expression: "",
        weight: null,
        partialBand: null,
        kind: "qualitative",
        assessmentGuidance:
          "Assess whether the company has a durable competitive advantage — brand, network effects, switching costs, scale, or regulatory barriers — that protects returns on capital over time. Ground the judgment in evidence of pricing, market share, or margin durability from the supplied sources.",
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      },
    ],
  };
}

export function makeFrameworkEvaluation(
  spec: CompanySpec,
): FrameworkEvaluation {
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
        reasoning: null,
        citations: null,
        confidence: null,
        promptVersion: null,
        source: "engine",
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
        reasoning: null,
        citations: null,
        confidence: null,
        promptVersion: null,
        source: "engine",
      },
    ],
  };
}

export const AVAILABLE_METRIC_KEYS: readonly MetricKeyInfo[] = [
  {
    key: "revenue",
    label: "Revenue",
    unit: "PLN",
    valueKind: "currency",
    computation: "reported",
    scope: "global",
  },
  {
    key: "ebitda_margin",
    label: "EBITDA margin",
    unit: "ratio",
    valueKind: "ratio",
    computation: "derived",
    scope: "global",
  },
  {
    key: "roic",
    label: "ROIC",
    unit: "ratio",
    valueKind: "ratio",
    computation: "derived",
    scope: "global",
  },
  {
    key: "operating_margin",
    label: "Operating margin",
    unit: "ratio",
    valueKind: "ratio",
    computation: "derived",
    scope: "global",
  },
] as const;
