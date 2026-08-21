// Canonical mock runtime (ADR 0048 Keystone B/C, Radicle 749a5a8).
//
// ONE stateful in-memory router over a `ScenarioData` store. Both test layers
// (the Vitest `appWorkflowHarness` and the Playwright browser-smoke runtime)
// drive `invoke(command, args)` against this — there is no second command table
// to drift from. Reads project the store; writes mutate it in place. An
// unhandled command throws (the add-a-case signal that keeps coverage honest).
//
// Determinism: IDs come from a per-runtime counter (reset with the store), never
// wall-clock/random, so parallel workers stay reproducible.

import packageJson from "../../../package.json";
import {
  COMPANY_SPECS,
  SAMPLE_NOW,
  companyId as companyIdFor,
} from "./entities";
import {
  legacyInvalidLicenseStatus,
  legacyLicenseStatus,
  legacyMissingLicenseStatus,
} from "./legacyMinimal";
import {
  buildScenario,
  type ScenarioData,
  type ScenarioName,
  type ScenarioSpec,
} from "./scenarios";
import {
  createControlledAsync,
  type MockRuntimeControls,
} from "./controlledAsync";
import type { ResearchEvidenceInput } from "../../api/researchTypes";
import type { OwnershipOverview } from "../../api/ownership";
import type { CompanyContext } from "../../api/generated/CompanyContext";
import type { CompanyHealth } from "../../api/generated/CompanyHealth";
import type { InsiderOverview } from "../../api/insider";
import type { RedFlagsView } from "../../api/redFlags";
import type { AnalystRecommendationsView } from "../../api/analystRecommendations";
import type { CommandError } from "../../api/generated/CommandError";
import type { UncrosswalkedConceptRow } from "../../api/generated/UncrosswalkedConceptRow";
import type { TodayView } from "../../api/generated/TodayView";
import type { TodayItem } from "../../api/generated/TodayItem";
import type { TodayClaim } from "../../api/generated/TodayClaim";

export type {
  MockRuntimeControls,
  InvocationPhase,
  InvocationMatch,
  PendingInvocation,
} from "./controlledAsync";

/** Narrow alias — the router treats every payload structurally. */
type Args = Record<string, unknown> | undefined;
type Handler = (data: ScenarioData, args: Args, ctx: RuntimeContext) => unknown;

interface RuntimeContext {
  nextId(prefix: string): string;
  /** Per-company IR report URLs (command-only state, not a seeded collection). */
  irReportUrls: Map<string, string>;
  /**
   * Manual sector overrides (ADR 0067 Decision 3); command-only, not seeded.
   * Absent from the map means "registry-sourced" — fall back to the
   * company's seeded `sector` field.
   */
  companySectors: Map<string, string>;
  /** Immutable decision journal entries (ADR 0071); command-only, not seeded. */
  decisionEntries: Record<string, unknown>[];
  /** Pre-report expectations (ADR 0071); command-only, not seeded. */
  reportExpectations: Record<string, unknown>[];
  /**
   * MCP bearer token (ADR 0078 M1); command-only, not seeded. Mirrors the
   * keychain: the plaintext leaves the runtime exactly once, at generation —
   * status payloads never carry it.
   */
  mcpToken: string | null;
  /** The kpi_acquisition token (ADR 0099 dec. 2) — same one-time-reveal rules. */
  mcpKpiToken: string | null;
  /**
   * MCP server live state (ADR 0078 M3). Command-only, not seeded. Mirrors the
   * lifecycle: `set_mcp_enabled` starts/stops, `mcp_status` reports. Enabling
   * without a token refuses (running:false + error), matching the real server.
   * The shape is `McpStatus { running, port, error }`; the Rust replayer
   * (mock_fidelity.rs) drives the real `McpLifecycle` for the same corpus, and
   * only `running`/`port` are asserted (error text differs across replayers).
   */
  mcpRunning: boolean;
  mcpError: string | null;
  /**
   * "Positions the program doesn't know yet" (ADR 0100 decision 10, epic
   * #398) for the ONE hardcoded company the browser narrow-window spec
   * exercises (`company_gpw_cdr`) — command-only, not a `ScenarioData`
   * seeded collection, since Layer 1 has no seeding command in the real
   * backend either. Mutated in place by `promote_uncrosswalked_concept` so a
   * re-fetch in the same test run reflects the promotion.
   */
  uncrosswalkedConcepts: UncrosswalkedConceptRow[];
}

export interface MockRuntime {
  /** The live store. Tests may read/seed it directly. */
  data: ScenarioData;
  /** Route one command. Resolves with the command's contract return shape. */
  invoke(command: string, args?: Args): Promise<unknown>;
  /** Replace the store with a fresh scenario (per-test isolation). */
  reset(scenario?: ScenarioName | ScenarioSpec): void;
  /** The scenario the store was last (re)built from. */
  scenario: ScenarioName;
  /**
   * Minimal failure-injection seam (Radicle 5be14c9, epic `0db7a7a`). Queue a
   * one-shot rejection for the NEXT invocation of `command`: instead of
   * running its handler, `invoke` settles with `error` UNCHANGED — the same
   * plain `{code, message}` shape (ADR 0070) a real typed backend rejection
   * uses, so the frontend's `isCommandError`/`CommandInvocationError` path is
   * exercised identically. `reset()` clears every queued failure. Q2's
   * controlled-async `reject(id, error)` (`controlledAsync.ts`) delegates to
   * this rather than reproducing the mapping — do not duplicate it elsewhere.
   */
  failNext(command: string, error: CommandError): void;
  /**
   * Persistent counterpart of `failNext` (epic #40 S1, ADR 0091): while a chaos
   * rule is installed for `command`, EVERY invocation of it settles with
   * `error` (the same untouched ADR 0070 envelope) instead of running its
   * handler — the "this read is broken for the whole session" state a
   * one-shot queue cannot express. A queued `failNext` for the same command
   * still wins (and is consumed) first. `clearChaos()` and `reset()` remove
   * every rule.
   */
  chaos(command: string, error: CommandError): void;
  /** Drop every persistent chaos rule; queued one-shot failures survive. */
  clearChaos(): void;
  /**
   * Controlled-async invocation control (ADR 0081 Q2, Radicle a9992e2):
   * hold/pending/release/reject around `invoke`. Wired ONCE here — never
   * inside individual handlers. See `controlledAsync.ts`.
   */
  controls: MockRuntimeControls;
}

/** `{ input: X }` → X, `{ companyId }` → the object, `undefined` → {}. */
function unwrap(args: Args): Record<string, unknown> {
  if (
    args &&
    typeof args === "object" &&
    "input" in args &&
    args.input &&
    typeof args.input === "object"
  ) {
    return args.input as Record<string, unknown>;
  }
  return (args ?? {}) as Record<string, unknown>;
}

function str(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

/** `day` (YYYY-MM-DD) minus `days`, same format. Mirrors the Rust read fn's
 * 30-day window so the dual-execution fidelity corpus agrees on the empty shape. */
function dateMinusDays(day: string, days: number): string {
  const date = new Date(`${day}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() - days);
  return date.toISOString().slice(0, 10);
}

/** Signed contribution of a KNF register-change event to the aggregate net short
 * % — mirrors `signed_event_delta` in `storage/short_positions.rs`. */
function signedEventDelta(
  kind: string,
  fromPct: number | null,
  toPct: number | null,
): number {
  switch (kind) {
    case "entered":
      return toPct ?? 0;
    case "increased":
    case "decreased":
      return (toPct ?? 0) - (fromPct ?? 0);
    case "exited":
      return -(fromPct ?? 0);
    default:
      return 0;
  }
}

/** Registry-sourced sector per company id (mirrors the Rust registry auto-populate, ADR 0067 Decision 3). */
const REGISTRY_SECTORS = new Map<string, string>(
  COMPANY_SPECS.map((spec) => [companyIdFor(spec), spec.sector]),
);

/**
 * CredentialStatus for the MCP bearer token (ADR 0078 M1), derived from the
 * command-only ctx state. Never includes the token itself — the plaintext is
 * returned exactly once by `regenerate_mcp_token` (mirror of the Rust
 * one-time-reveal semantics).
 */
function kpiTokenStatus(ctx: RuntimeContext) {
  const configured = ctx.mcpKpiToken !== null;
  return {
    providerId: "mcp",
    secretKind: "kpi_acquisition_token",
    configured,
    storage: configured ? "os_keychain" : "not_configured",
    label: "MCP acquisition token",
    devFallbackAvailable: false,
    error: null,
  };
}

/**
 * Mirror of the production restart-on-rotation composition (ADR 0099 dec. 2):
 * stop + ensure_started from stored-credential truth. Disabled stays down;
 * enabled without a primary token refuses; otherwise the server (re)starts.
 */
function restartMcpLifecycle(
  d: { settings: { mcp: { enabled: boolean } } },
  ctx: RuntimeContext,
) {
  if (!d.settings.mcp.enabled) {
    ctx.mcpRunning = false;
    ctx.mcpError = null;
    return;
  }
  if (ctx.mcpToken === null) {
    ctx.mcpRunning = false;
    ctx.mcpError = "MCP server auth token is not configured";
    return;
  }
  ctx.mcpRunning = true;
  ctx.mcpError = null;
}

/** `McpStatus` mirror incl. the acquisition-scope availability (ADR 0099). */
function mcpStatusOf(
  d: { settings: { mcp: { port: number } } },
  ctx: RuntimeContext,
) {
  return {
    running: ctx.mcpRunning,
    port: d.settings.mcp.port,
    error: ctx.mcpError,
    // False on digest collision with the primary (fail-closed guard).
    kpiAcquisitionConfigured:
      ctx.mcpRunning &&
      ctx.mcpKpiToken !== null &&
      ctx.mcpKpiToken !== ctx.mcpToken,
  };
}

function mcpTokenStatus(ctx: RuntimeContext) {
  const configured = ctx.mcpToken !== null;
  return {
    providerId: "mcp",
    secretKind: "auth_token",
    configured,
    storage: configured ? "os_keychain" : "not_configured",
    label: "MCP server auth token",
    devFallbackAvailable: false,
    error: null,
  };
}

/**
 * Replace the first entity matching `match` with `patch(entity)`, returning a
 * NEW array. Store mutations MUST go through this (or an equivalent reassign) so
 * a UI read sees a changed reference and re-renders — an in-place field mutation
 * keeps the same array reference and React bails. Enforced by runtime.test.ts
 * "re-render safety"; see docs/testing.md → "mock runtime conventions".
 */
function mapReplace<T>(
  items: T[],
  match: (item: T) => boolean,
  patch: (item: T) => T,
): { next: T[]; updated: T | undefined } {
  let updated: T | undefined;
  const next = items.map((item) => {
    if (!match(item)) return item;
    updated = patch(item);
    return updated;
  });
  return { next, updated };
}

// ---------------------------------------------------------------------------
// Research timeline (ported from workflowHarness/state.ts:buildResearchTimeline)
// ---------------------------------------------------------------------------

function buildTimeline(data: ScenarioData, input: ResearchEvidenceInput) {
  const selectedTypes = new Set(input.evidenceTypes ?? []);
  const watchlistCompanyIds = input.watchlistId
    ? data.watchlistMemberships
        .filter((membership) => membership.watchlistId === input.watchlistId)
        .map((membership) => membership.companyId)
    : [];
  const filteredItems = data.researchEvidence.filter((item) => {
    const companyMatches =
      !input.companyId || item.companyId === input.companyId;
    const watchlistMatches =
      !input.watchlistId || watchlistCompanyIds.includes(item.companyId);
    const typeMatches =
      selectedTypes.size === 0 || selectedTypes.has(item.evidenceType);
    const changedSinceReview = input.watchlistId
      ? item.reviewState.changedSinceWatchlistReview
      : item.reviewState.changedSinceCompanyReview;
    const changedMatches = !input.changedSinceReviewOnly || changedSinceReview;
    return companyMatches && watchlistMatches && typeMatches && changedMatches;
  });
  const memberCompanyIds = input.watchlistId ? watchlistCompanyIds : [];
  const companySummaries = memberCompanyIds.map((companyId) => {
    const companyItems = filteredItems.filter(
      (item) => item.companyId === companyId,
    );
    return {
      companyId,
      total: companyItems.length,
      changedSinceReview: companyItems.filter(
        (item) => item.reviewState.changedSinceWatchlistReview,
      ).length,
      lastReviewedAt: null,
    };
  });
  return {
    items: filteredItems.slice(0, input.limit ?? 100),
    summary: {
      total: filteredItems.length,
      changedSinceReview: filteredItems.filter((item) =>
        input.watchlistId
          ? item.reviewState.changedSinceWatchlistReview
          : item.reviewState.changedSinceCompanyReview,
      ).length,
      lastReviewedAt: data.researchReviewCheckpoints[0]?.reviewedAt ?? null,
      memberCompanyCount: memberCompanyIds.length,
      companiesWithChangedEvidence: companySummaries.filter(
        (summary) => summary.changedSinceReview > 0,
      ).length,
      companySummaries,
    },
  };
}

// ---------------------------------------------------------------------------
// Search (ported intent of ADR 0032 FTS over the in-memory store)
// ---------------------------------------------------------------------------

function runSearch(data: ScenarioData, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return { groups: [] };
  const groups: { contentType: string; matches: unknown[] }[] = [];
  const push = (
    contentType: string,
    sourceId: string,
    companyId: string | null,
    title: string,
  ) => {
    let group = groups.find((g) => g.contentType === contentType);
    if (!group) {
      group = { contentType, matches: [] };
      groups.push(group);
    }
    group.matches.push({
      contentType,
      sourceId,
      companyId,
      parentId: null,
      title,
      snippet: title,
      score: 1,
    });
  };
  for (const c of data.companies) {
    if (
      c.displayName.toLowerCase().includes(needle) ||
      c.ticker.toLowerCase().includes(needle)
    ) {
      push("company", c.id, c.id, c.displayName);
    }
  }
  for (const f of data.feedItems) {
    if (f.title.toLowerCase().includes(needle))
      push("feed_item", f.id, null, f.title);
  }
  for (const n of data.notebookEntries) {
    if (n.title.toLowerCase().includes(needle))
      push("notebook_entry", n.id, n.companyId, n.title);
  }
  return { groups };
}

type FrameworkCriterion =
  ScenarioData["qualityFrameworks"][number]["criteria"][number];

// Shared kind/guidance/expression resolution + validation for the
// create_/update_framework_criterion handlers (F9 — one resolver, no
// duplication). The effective kind is the input override else the existing
// row's kind (defaulting to quantitative on create, where `existing` is
// undefined — which makes this reduce to the create-path behaviour exactly).
// Mirror the storage validation (ADR 0075): a qualitative criterion requires
// non-empty guidance and stores no DSL expression; a quantitative one requires
// a non-empty predicate expression. The thrown messages MUST stay byte-identical
// — the dual-execution fidelity corpus replays these handlers.
function resolveCriterionKindFields(
  input: Record<string, unknown>,
  existing?: FrameworkCriterion,
): {
  kind: "qualitative" | "quantitative";
  expression: string;
  assessmentGuidance: string;
} {
  const kind =
    str(input.kind) === "qualitative"
      ? ("qualitative" as const)
      : str(input.kind) === "quantitative"
        ? ("quantitative" as const)
        : (existing?.kind ?? "quantitative");
  const assessmentGuidance =
    kind === "qualitative"
      ? (
          str(input.assessmentGuidance) ??
          existing?.assessmentGuidance ??
          ""
        ).trim()
      : "";
  const expression =
    kind === "qualitative"
      ? ""
      : (str(input.expression) ?? existing?.expression ?? "").trim();
  if (kind === "qualitative" && assessmentGuidance === "") {
    throw new Error("a qualitative criterion requires assessment guidance");
  }
  if (kind === "quantitative" && expression === "") {
    throw new Error("a quantitative criterion requires an expression");
  }
  return { kind, expression, assessmentGuidance };
}

// --- Judgment capture helpers (ADR 0071) ---

/** Build one expectation metric child row (mirrors the Rust insert scheme). */
function buildExpectationMetric(
  expectationId: string,
  metric: Record<string, unknown>,
  index: number,
): Record<string, unknown> {
  return {
    id: `${expectationId}_metric_${index + 1}`,
    expectationId,
    metricKey: str(metric.metricKey) ?? "",
    comparator: str(metric.comparator) ?? "",
    expectedValue: str(metric.expectedValue) ?? "",
    unit: str(metric.unit),
    createdAt: SAMPLE_NOW,
  };
}

/** Do any facts exist for the occurrence's period? The mock's freeze condition. */
function periodHasFacts(
  d: ScenarioData,
  companyId: string,
  fiscalYear: number,
  periodType: string,
): boolean {
  const periodIds = new Set(
    d.financialPeriods
      .filter(
        (p) =>
          p.companyId === companyId &&
          p.fiscalYear === fiscalYear &&
          p.periodType === periodType,
      )
      .map((p) => p.id),
  );
  return d.financialFacts.some(
    (f) => f.companyId === companyId && periodIds.has(f.periodId),
  );
}

/** The latest confirmed actual for a metric in the period, or null. */
function confirmedActual(
  d: ScenarioData,
  companyId: string,
  fiscalYear: number,
  periodType: string,
  metricKey: string,
): string | null {
  const definitionId = d.kpiDefinitions.find(
    (k) => k.metricKey === metricKey,
  )?.id;
  if (!definitionId) return null;
  const periodIds = new Set(
    d.financialPeriods
      .filter(
        (p) =>
          p.companyId === companyId &&
          p.fiscalYear === fiscalYear &&
          p.periodType === periodType,
      )
      .map((p) => p.id),
  );
  const fact = d.financialFacts.find(
    (f) =>
      f.companyId === companyId &&
      periodIds.has(f.periodId) &&
      f.definitionId === definitionId &&
      (f.confirmationState ?? "confirmed") === "confirmed",
  );
  return fact ? (str(fact.valueNumeric) ?? null) : null;
}

/** Mirror of the Rust comparator evaluator (report_expectations.rs). */
function evaluateExpectationOutcome(
  comparator: string,
  expected: string,
  actual: string,
): "met" | "missed" | "unknown" {
  const e = Number(expected.trim());
  const a = Number(actual.trim());
  if (
    expected.trim() === "" ||
    actual.trim() === "" ||
    !Number.isFinite(e) ||
    !Number.isFinite(a)
  ) {
    return "unknown";
  }
  let met: boolean;
  switch (comparator) {
    case "lt":
      met = a < e;
      break;
    case "lte":
      met = a <= e;
      break;
    case "eq":
      met = a === e;
      break;
    case "gte":
      met = a >= e;
      break;
    case "gt":
      met = a > e;
      break;
    default:
      return "unknown";
  }
  return met ? "met" : "missed";
}

// Ownership overview (ADR 0072, v0.56 T6). A company with no seeded overview
// reads back the empty overview (freeFloatPct "100"), mirroring the real
// backend's answer for a company with no disclosed stakes.
function emptyOwnershipOverview(companyId: string): OwnershipOverview {
  return {
    companyId,
    freeFloatPct: "100",
    disclosedSum: "0",
    holders: [],
    history: [],
    freeFloatHistory: [],
    residuals: [],
  };
}

function emptyCompanyHealth(companyId: string): CompanyHealth {
  return {
    companyId,
    statementType: "industrial",
    piotroskiVariant: "Piotroski F (2000)",
    altmanVariant: "Altman Z″ EM (1995)",
    history: [],
  };
}

// A company with no parsed insider substrate reads back the empty overview: no
// transactions, no holdings, both windows below the 2-transaction minimum.
function emptyInsiderOverview(companyId: string): InsiderOverview {
  return {
    companyId,
    transactions: [],
    holdings: [],
    window90d: { state: "belowMinimum", count: 0 },
    window12m: { state: "belowMinimum", count: 0 },
  };
}

function ensureOwnershipOverview(
  data: ScenarioData,
  companyId: string,
): OwnershipOverview {
  if (!data.ownershipOverviews) data.ownershipOverviews = [];
  let overview = data.ownershipOverviews.find(
    (entry) => entry.companyId === companyId,
  );
  if (!overview) {
    overview = emptyOwnershipOverview(companyId);
    data.ownershipOverviews.push(overview);
  }
  return overview;
}

// ---------------------------------------------------------------------------
// Handler table
// ---------------------------------------------------------------------------

function buildHandlers(): Record<string, Handler> {
  const ok = () => undefined;
  const handlers: Record<string, Handler> = {
    // --- System / platform reads ---
    // Version mirrors package.json so the browser-smoke brand chip never rots
    // behind the real app again (audit K12: it showed a stale hardcoded 0.3.0).
    health: () => ({ status: "ok", version: packageJson.version }),
    database_status: (d) => d.databaseStatus,
    get_settings: (d) => d.settings,
    get_license_status: (d) => d.licenseStatus,
    get_local_metrics_snapshot: (d) => d.metricsSnapshot,
    get_diagnostic_summary: (d) => d.diagnosticSummary,
    list_diagnostic_events: (d) => d.diagnosticEvents,
    list_source_reconciliation: (d) => d.reconciliationResults,
    get_log_status: (d) => d.logStatus,
    list_log_entries: (d) => d.logEntries,
    get_provider_credential_status: (d, a) => {
      const providerId = str(unwrap(a).providerId);
      const found = providerId
        ? d.credentialStatuses.find((c) => c.providerId === providerId)
        : undefined;
      if (found) return found;
      // Mock fidelity: the real command reports an UNCONFIGURED status for a
      // provider with no stored key — never another provider's row. Falling
      // back to statuses[0] painted every new catalog provider as "configured"
      // in tests (caught when Mistral joined the catalog, T4.1).
      return {
        providerId: providerId ?? "",
        secretKind: "api_key",
        configured: false,
        storage: "keychain",
        label: providerId ?? "",
        devFallbackAvailable: false,
        error: null,
      };
    },
    list_available_metric_keys: (d) => d.metricKeys,
    backup_status: (d) => d.backupStatus,

    // --- Companies / registry ---
    list_companies: (d) => d.companies,
    list_company_registry_entries: (d) => d.registry,
    lookup_company: (d, a) => {
      const input = unwrap(a);
      const exchange = (str(input.exchange) ?? "").trim().toUpperCase();
      const ticker = str(input.ticker)?.trim().toUpperCase();
      const isin = str(input.isin)?.trim().toUpperCase();
      const displayName = str(input.displayName)?.trim().toUpperCase();
      const match = d.registry
        .filter(
          (entry) =>
            (ticker && entry.ticker.toUpperCase() === ticker) ||
            (isin && entry.isin?.toUpperCase() === isin) ||
            (displayName &&
              displayName.length >= 3 &&
              entry.displayName.toUpperCase().includes(displayName)),
        )
        .sort((left, right) => {
          const leftPreferred =
            left.exchange.toUpperCase() === exchange ? 0 : 1;
          const rightPreferred =
            right.exchange.toUpperCase() === exchange ? 0 : 1;
          return (
            leftPreferred - rightPreferred ||
            left.qualifiedTicker.localeCompare(right.qualifiedTicker)
          );
        })[0];
      if (!match) return null;
      return {
        exchange: match.exchange,
        ticker: match.ticker,
        qualifiedTicker: match.qualifiedTicker,
        displayName: match.displayName,
        isin: match.isin ?? "",
        source: "company_directory",
      };
    },
    create_company: (d, a, ctx) => {
      const input = unwrap(a);
      const ticker = str(input.ticker) ?? "NEW";
      const exchange = str(input.exchange) ?? "GPW";
      const company = {
        id: `company_${exchange.toLowerCase()}_${ticker.toLowerCase()}`,
        exchange,
        ticker,
        qualifiedTicker: `${exchange}:${ticker}`,
        displayName: str(input.displayName) ?? ticker,
        isin: str(input.isin),
        cik: str(input.cik),
        lei: str(input.lei),
      };
      d.companies = [...d.companies, company];
      // Mark the matching directory entry tracked (drives the "already added" state).
      d.registry = d.registry.map((entry) =>
        entry.qualifiedTicker === company.qualifiedTicker
          ? { ...entry, tracked: true }
          : entry,
      );
      void ctx;
      return company;
    },
    delete_company: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      d.companies = d.companies.filter((c) => c.id !== companyId);
      d.watchlistMemberships = d.watchlistMemberships.filter(
        (m) => m.companyId !== companyId,
      );
      return undefined;
    },

    // Price context read model (v0.53 T5, ADR 0067/0082). The scenario data
    // carries no `daily_quotes` store yet, so every company reports its empty
    // state, mirroring the real backend's answer for an untouched fixture: a
    // GPW company (the mapped market, `commands::market_data`'s
    // `MAPPED_EXCHANGE`) reports "no_quotes" (mapped provider, no bars
    // fetched yet); any other exchange reports "unmapped_ticker".
    get_price_context: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const company = d.companies.find((c) => c.id === companyId);
      const emptyReason =
        company?.exchange.toUpperCase() === "GPW"
          ? "no_quotes"
          : "unmapped_ticker";
      return {
        lastClose: 0,
        lastDate: "",
        changeAbs: 0,
        changePct: 0,
        currency: "PLN",
        week52High: 0,
        week52Low: 0,
        week52HighDistPct: 0,
        week52LowDistPct: 0,
        marketCap: null,
        ratios: {
          pe: null,
          pbv: null,
          evEbitda: null,
          divYield: null,
          fcfYield: null,
          ownHistPercentile: null,
        },
        history: [],
        fetchedAt: "",
        emptyReason,
      };
    },

    // Cross-company comparison read model (v0.61 §A2, ADR 0089 dec. 1). The
    // scenario data carries no confirmed facts wired to the requested companies,
    // so the read model is the empty comparison the real backend also returns:
    // an empty axis and one empty series per (company, metric). Populated
    // alignment/delta/FX correctness is pinned by the Rust golden + proptest,
    // not the corpus (which asserts the empty-state parity).
    get_kpi_comparison: (d, a) => {
      const input = unwrap(a);
      const companyIds = Array.isArray(input.companyIds)
        ? (input.companyIds as string[])
        : [];
      const metricKeys = Array.isArray(input.metricKeys)
        ? (input.metricKeys as string[])
        : [];
      const granularity = str(input.granularity) ?? "annual";
      // A seeded populated comparison (ADR 0089 §A3) wins when the request set
      // matches by (companyIds set, metricKeys, granularity) — order-independent
      // on companies, mirroring the real read model. Otherwise the computed
      // empty default (no confirmed facts) is returned, keeping the fidelity
      // corpus's empty-state parity intact.
      const wanted = [...companyIds].sort().join(",");
      const seeded = d.kpiComparisons?.find(
        (comparison) =>
          comparison.granularity === granularity &&
          comparison.metricKeys.join(",") === metricKeys.join(",") &&
          [...new Set(comparison.series.map((series) => series.companyId))]
            .sort()
            .join(",") === wanted,
      );
      if (seeded) return seeded;
      const series = companyIds.flatMap((companyId) =>
        metricKeys.map((metricKey) => ({
          companyId,
          metricKey,
          valueKind: null,
          cells: [],
        })),
      );
      return { granularity, metricKeys, axis: [], series };
    },

    // --- Sector percentiles (v0.61 B1, ADR 0089 dec. 3) ---
    // Mirrors `commands::sector_percentiles::compute_company_sector_percentiles`:
    // the peer set is the tracked companies sharing the target's sector (the
    // company itself included); a company with no sector returns the typed
    // `no_sector` empty state. Scenario data carries no resolvable ratios/KPIs, so
    // every ranked metric is a typed absence — the fidelity corpus asserts the
    // empty-state parity; populated percentile math is pinned by the Rust golden +
    // proptest, not the corpus.
    get_sector_percentiles: (d, a, ctx) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      // A seeded populated/thin percentile payload (ADR 0089 §B3) wins for the
      // companies the browser/visual harness dresses; every other company reads
      // the sector-derived typed-absence default below (fidelity-corpus parity).
      const seeded = d.sectorPercentiles?.[companyId];
      if (seeded) return seeded;
      const sectorOf = (id: string): string | null =>
        ctx.companySectors.get(id) ?? REGISTRY_SECTORS.get(id) ?? null;
      const specs: { metricKey: string; kind: string }[] = [
        { metricKey: "pe_ratio", kind: "market_ratio" },
        { metricKey: "pbv_ratio", kind: "market_ratio" },
        { metricKey: "ev_ebitda", kind: "market_ratio" },
        { metricKey: "dividend_yield", kind: "market_ratio" },
        { metricKey: "fcf_yield", kind: "market_ratio" },
        { metricKey: "roe", kind: "canonical_kpi" },
        { metricKey: "roa", kind: "canonical_kpi" },
        { metricKey: "roic", kind: "canonical_kpi" },
        { metricKey: "fcf_margin", kind: "canonical_kpi" },
        { metricKey: "net_debt_to_ebitda", kind: "canonical_kpi" },
      ];
      const rawSector = sectorOf(companyId);
      const sector = rawSector?.trim() ? rawSector.trim() : null;
      if (sector === null) {
        return {
          companyId,
          sector: null,
          peerCount: 0,
          thin: true,
          emptyReason: "no_sector",
          metrics: [],
        };
      }
      const fold = sector.toLocaleLowerCase();
      const peers = d.companies.filter((c) => {
        const s = sectorOf(c.id);
        return s !== null && s.trim().toLocaleLowerCase() === fold;
      });
      // No resolvable values in the mock ⇒ every metric is `no_company_value`.
      const metrics = specs.map((spec) => ({
        metricKey: spec.metricKey,
        kind: spec.kind,
        value: null,
        percentile: null,
        median: null,
        sampleSize: 0,
        absentReason: "no_company_value",
      }));
      return {
        companyId,
        sector,
        peerCount: peers.length,
        thin: peers.length < 4,
        emptyReason: null,
        metrics,
      };
    },

    // --- Comparative valuation L1 (v0.61 B2, ADR 0089 dec. 4-5) ---
    // Mirrors `commands::valuation::compute_and_persist_comparative_valuation`:
    // the peer set is the tracked companies sharing the target's sector; a company
    // with no sector returns the typed `no_sector` empty state. Scenario data
    // carries no resolvable multiples/drivers, so every method is a typed absence
    // and no run persists — the fidelity corpus asserts that empty-state parity;
    // populated valuation math is pinned by the Rust golden + proptest.
    compute_comparative_valuation: (d, a, ctx) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      // A seeded populated/thin valuation payload (ADR 0089 §B3) wins for the
      // harness-dressed companies; every other company reads the typed-absence
      // default below (no resolvable multiples ⇒ fidelity-corpus parity).
      const seededValuation = d.comparativeValuations?.[companyId];
      if (seededValuation) return seededValuation;
      const sectorOf = (id: string): string | null =>
        ctx.companySectors.get(id) ?? REGISTRY_SECTORS.get(id) ?? null;
      const methodDefs: [string, string][] = [
        ["pe_multiple", "net_profit_ttm"],
        ["ev_ebitda_multiple", "ebitda_ttm"],
        ["pbv_multiple", "total_equity"],
      ];
      const methods = methodDefs.map(([method, driverKey]) => ({
        method,
        driverKey,
        driverValue: null,
        peerMultipleLow: null,
        peerMultipleBase: null,
        peerMultipleHigh: null,
        fairLow: null,
        fairBase: null,
        fairHigh: null,
        peerSampleSize: 0,
        absentReason: "insufficient_peers",
      }));
      const confidence = {
        grade: "D",
        composite: "0",
        dataCompleteness: "0",
        peerDepth: "0",
        methodConvergence: "0",
        validation: "0",
      };
      const rawSector = sectorOf(companyId);
      const sector = rawSector?.trim() ? rawSector.trim() : null;
      if (sector === null) {
        return {
          companyId,
          sector: null,
          peerCount: 0,
          thin: true,
          currentPrice: null,
          dataAsOf: "",
          emptyReason: "no_sector",
          methods,
          convergence: null,
          confidence,
        };
      }
      const fold = sector.toLocaleLowerCase();
      const peers = d.companies.filter((c) => {
        const s = sectorOf(c.id);
        return s !== null && s.trim().toLocaleLowerCase() === fold;
      });
      return {
        companyId,
        sector,
        peerCount: peers.length,
        thin: peers.length < 4,
        currentPrice: null,
        dataAsOf: "",
        emptyReason: null,
        methods,
        convergence: null,
        confidence,
      };
    },
    // No resolvable multiples in the mock ⇒ no run persists ⇒ empty history.
    list_valuation_runs: () => [],

    // --- Company health scores (v0.57 T2, ADR 0083) ---
    // A fresh company has no FY periods, so the read model is the empty default
    // (no latest, empty history); populated F/Z correctness is pinned by the
    // Rust golden + TS QualityPanel tests, not the corpus.
    get_company_health: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return (
        d.companyHealthReports?.find((h) => h.companyId === companyId) ??
        emptyCompanyHealth(companyId)
      );
    },

    // --- Company context (F1, ADR 0106 dec. 3) ---
    // The Inbox detail pane's composed company-context block: latest-period
    // facts (domain-date order, never created_at) + upcoming events + notebook
    // coverage + claims-due counts. Mirrors compute_company_context.
    get_company_context: (d, a): CompanyContext => {
      const companyId = str(unwrap(a).companyId) ?? "";

      const periods = d.financialPeriods.filter((p) => p.companyId === companyId);
      const domainKey = (p: (typeof periods)[number]) =>
        p.periodEndDate ?? `${p.fiscalYear}-12-31`;
      const latestPeriod = periods.reduce<(typeof periods)[number] | null>(
        (best, p) =>
          !best ||
          domainKey(p) > domainKey(best) ||
          (domainKey(p) === domainKey(best) && p.fiscalYear > best.fiscalYear)
            ? p
            : best,
        null,
      );
      const latestPeriodFacts = latestPeriod
        ? {
            periodLabel: `${latestPeriod.periodType} ${latestPeriod.fiscalYear}`,
            facts: d.financialFacts
              .filter((f) => f.periodId === latestPeriod.id)
              // Mirrors the Rust read model's `created_at DESC, id` ordering
              // (financials.rs) — the corpus only pins primitives, so keep
              // this in sync by hand (sol F1 finding 4).
              .sort(
                (x, y) =>
                  y.createdAt.localeCompare(x.createdAt) || x.id.localeCompare(y.id),
              )
              .slice(0, 6)
              .map((f) => ({
                metricKey: f.metricKey,
                valueNumeric: f.valueNumeric,
                currency: f.currency,
                sourceDocumentRef: f.sourceDocumentRef,
                createdAt: f.createdAt,
              })),
          }
        : null;

      const today = SAMPLE_NOW.slice(0, 10);
      const upcomingEvents = d.events
        .filter((e) => e.companyId === companyId && e.eventDate >= today)
        .sort(
          (x, y) =>
            x.eventDate.localeCompare(y.eventDate) ||
            (x.eventTime ?? "").localeCompare(y.eventTime ?? "") ||
            x.title.localeCompare(y.title),
        )
        .slice(0, 3)
        .map((e) => ({
          title: e.title,
          eventDate: e.eventDate,
          eventType: e.eventType,
        }));

      const notes = d.notebookEntries.filter((n) => n.companyId === companyId);
      const latestNote = notes.reduce<(typeof notes)[number] | null>(
        (best, n) => (!best || n.updatedAt > best.updatedAt ? n : best),
        null,
      );

      return {
        companyId,
        latestPeriodFacts,
        upcomingEvents,
        notebook: {
          count: notes.length,
          latestAt: latestNote?.updatedAt ?? null,
        },
        claimsDue: {
          // Company-scoped like the Rust `list_claims_to_verify(company_id)`
          // call — never the global bucket lengths (sol F1 finding 4).
          due: d.claimsToVerify.due.filter((c) => c.claim.companyId === companyId).length,
          overdue: d.claimsToVerify.overdue.filter((c) => c.claim.companyId === companyId).length,
        },
      };
    },

    // --- Dziś v2 composed read model (F2 S1, ADR 0106 dec. 3) ---
    // Flat items[] from four independent sections + a bulk claims-to-verify
    // scan. Mirrors `commands::today::compute_today_view`. Mock feed items
    // carry only a ticker string (no `feed_item_companies` join table), so
    // multi-company media-cluster membership is NOT replicated here — that
    // behavior is pinned by the Rust unit tests in `commands/today.rs`; the
    // mock stays faithful for the single-company case the corpus exercises.
    get_today_view: (d, a): TodayView => {
      const dayLimitRaw = unwrap(a).dayLimit;
      const dayLimit = Math.min(
        7,
        Math.max(1, typeof dayLimitRaw === "number" ? dayLimitRaw : 1),
      );
      const today = SAMPLE_NOW.slice(0, 10);
      const since = dateMinusDays(today, dayLimit);
      const horizon = (() => {
        const date = new Date(`${today}T00:00:00Z`);
        date.setUTCDate(date.getUTCDate() + dayLimit);
        return date.toISOString().slice(0, 10);
      })();

      const tickerToCompanyId = new Map(d.companies.map((c) => [c.qualifiedTicker, c.id]));
      const sinceStamp = `${since}T00:00:00Z`;
      const inWindow = d.feedItems.filter(
        (fi) => fi.publishedAt >= sinceStamp && tickerToCompanyId.has(fi.company),
      );

      const items: TodayItem[] = [];

      for (const fi of inWindow) {
        if (fi.type !== "Official report") continue;
        items.push({
          kind: "filing",
          feedItemId: fi.id,
          companyId: tickerToCompanyId.get(fi.company) ?? "",
          qualifiedTicker: fi.company,
          title: fi.title,
          publishedAt: fi.publishedAt,
          read: !fi.unread,
          presentationKind: fi.presentationKind,
        });
      }

      // Media clusters per (company, UTC day) — rows arrive in `feedItems`
      // insertion order, so no extra sort is needed for "most recent 3".
      const clusters = new Map<
        string,
        {
          companyId: string;
          qualifiedTicker: string;
          day: string;
          earliest: string;
          latest: string;
          topTitles: string[];
          feedItemIds: string[];
        }
      >();
      for (const fi of inWindow) {
        if (fi.type !== "Public media") continue;
        const companyId = tickerToCompanyId.get(fi.company) ?? "";
        const day = fi.publishedAt.slice(0, 10);
        const key = `${companyId}|${day}`;
        const acc = clusters.get(key) ?? {
          companyId,
          qualifiedTicker: fi.company,
          day,
          earliest: fi.publishedAt,
          latest: fi.publishedAt,
          topTitles: [],
          feedItemIds: [],
        };
        if (fi.publishedAt < acc.earliest) acc.earliest = fi.publishedAt;
        if (fi.publishedAt > acc.latest) acc.latest = fi.publishedAt;
        if (acc.topTitles.length < 3) acc.topTitles.push(fi.title);
        acc.feedItemIds.push(fi.id);
        clusters.set(key, acc);
      }
      for (const acc of clusters.values()) {
        items.push({
          kind: "mediaCluster",
          companyId: acc.companyId,
          qualifiedTicker: acc.qualifiedTicker,
          day: acc.day,
          count: acc.feedItemIds.length,
          earliestPublishedAt: acc.earliest,
          latestPublishedAt: acc.latest,
          topTitles: acc.topTitles,
          feedItemIds: acc.feedItemIds,
        });
      }

      // Non-arrival: `periodic_report` events past their date with no
      // witnessing report and no `report_delay` flag yet (shares the
      // deterministic flag-id shape `raise_flag`/`red_flags.rs` writes).
      for (const event of d.events) {
        if (event.eventType !== "periodic_report" || event.eventDate > today) continue;
        const witnessed = d.feedItems.some(
          (fi) =>
            fi.type === "Official report" &&
            fi.company === event.company &&
            fi.publishedAt >= event.eventDate,
        );
        if (witnessed) continue;
        const flagId = `rf:report_delay:${event.companyId}:${event.id}`;
        const flagged = d.redFlagsByCompany?.[event.companyId]?.active.some(
          (flag) => flag.flagId === flagId,
        );
        if (flagged) continue;
        items.push({
          kind: "nonArrival",
          eventKey: event.id,
          companyId: event.companyId,
          qualifiedTicker: event.company,
          eventDate: event.eventDate,
          title: event.title,
        });
      }

      // Calendar: company_events within [today, today + dayLimit].
      for (const event of d.events) {
        if (event.eventDate < today || event.eventDate > horizon) continue;
        items.push({
          kind: "calendar",
          eventKey: event.id,
          eventDate: event.eventDate,
          eventType: event.eventType,
          title: event.title,
          companyId: event.companyId,
          qualifiedTicker: event.company,
        });
      }

      // Autopilot: unread runs within the window.
      for (const run of d.autopilotRuns) {
        if (run.notificationState !== "unread" || run.createdAt < since) continue;
        items.push({ kind: "autopilotRun", run });
      }

      const todaySortKey = (item: TodayItem): string => {
        switch (item.kind) {
          case "filing":
            return item.publishedAt;
          case "mediaCluster":
            return item.latestPublishedAt;
          case "nonArrival":
          case "calendar":
            return item.eventDate;
          case "autopilotRun":
            return item.run.createdAt;
        }
      };
      items.sort((a, b) => todaySortKey(b).localeCompare(todaySortKey(a)));

      // Claims: the mock already stores the bulk bucket flat (no per-company
      // fan-out needed, unlike the Rust store's per-company loop).
      const tickerOf = (companyId: string) =>
        d.companies.find((company) => company.id === companyId)?.qualifiedTicker ?? "";
      const toVerify: TodayClaim[] = [
        ...d.claimsToVerify.overdue.map(
          (c): TodayClaim => ({
            claim: c.claim,
            qualifiedTicker: tickerOf(c.claim.companyId),
            bucket: "overdue",
          }),
        ),
        ...d.claimsToVerify.due.map(
          (c): TodayClaim => ({
            claim: c.claim,
            qualifiedTicker: tickerOf(c.claim.companyId),
            bucket: "due",
          }),
        ),
      ];

      let reportCount = 0;
      let filingCount = 0;
      let mediaCount = 0;
      for (const fi of inWindow) {
        if (fi.type === "Official report") {
          if (fi.presentationKind === "report") reportCount += 1;
          else filingCount += 1;
        } else if (fi.type === "Public media") {
          mediaCount += 1;
        }
      }

      return {
        items,
        toVerify,
        deltaSummary: { reportCount, filingCount, mediaCount },
        previousVisitAt: d.todayLastVisitAt,
        sectionErrors: {},
      };
    },

    // Dziś v2 visit anchor (F2 S2, plan decision 4): stamp with the mock's
    // deterministic clock (SAMPLE_NOW), mirroring the Rust command's own
    // backend-clock stamp, and return the new value.
    mark_today_visited: (d): string => {
      d.todayLastVisitAt = SAMPLE_NOW;
      return d.todayLastVisitAt;
    },

    // --- Red flags (v0.57 T7, ADR 0083 D8) ---
    // Computed panel state; a company with no seed reads back the empty view.
    get_red_flags: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return d.redFlagsByCompany?.[companyId] ?? { active: [], history: [] };
    },
    // Acknowledge: move the flag from active to history (idempotent), return the
    // refreshed view. The flag id encodes the company (`rf:<type>:<company>:…`).
    acknowledge_red_flag: (d, a) => {
      const flagId = str(unwrap(a).flagId) ?? "";
      const companyId = flagId.split(":")[2] ?? "";
      const view = d.redFlagsByCompany?.[companyId] ?? {
        active: [],
        history: [],
      };
      const flag = view.active.find((f) => f.flagId === flagId);
      const next: RedFlagsView = flag
        ? {
            active: view.active.filter((f) => f.flagId !== flagId),
            history: [{ ...flag, ackedAt: SAMPLE_NOW }, ...view.history],
          }
        : view;
      if (d.redFlagsByCompany) d.redFlagsByCompany[companyId] = next;
      return next;
    },

    // --- Analyst recommendations (v0.58 A3, ADR 0073) ---
    // Attributed third-party opinions; a company with no seed reads back the empty
    // view (no entries, no latest target, no refresh) — the adapter-populated
    // register is empty until a refresh ingests a page.
    get_analyst_recommendations: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return (
        d.analystRecommendationsByCompany?.[companyId] ??
        ({ companyId, entries: [] } satisfies AnalystRecommendationsView)
      );
    },

    // --- Insider overview (v0.57 T6, ADR 0083 D7) ---
    // Computed read model; a company with no parsed substrate reads back the empty
    // overview. Populated window-math correctness is pinned by the Rust golden +
    // TS component tests, not the corpus.
    get_insider_overview: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return (
        d.insiderOverviews?.find((o) => o.companyId === companyId) ??
        emptyInsiderOverview(companyId)
      );
    },

    // --- Ownership overview + review (v0.56 T6, ADR 0072) ---
    get_ownership_overview: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return (
        d.ownershipOverviews?.find((o) => o.companyId === companyId) ??
        emptyOwnershipOverview(companyId)
      );
    },
    // Deterministic extraction drains on the queue (not the mock); the CTA just
    // reports how many documents it would enqueue — none in the mock store.
    backfill_ownership_extraction: () => 0,
    set_ownership_holder_type: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? "";
      const holderKey = str(input.holderKey) ?? "";
      const holderType =
        input.holderType == null
          ? undefined
          : (str(input.holderType) ?? undefined);
      const overview = ensureOwnershipOverview(d, companyId);
      overview.holders = overview.holders.map((h) =>
        h.holderKey === holderKey ? { ...h, holderType } : h,
      );
      overview.history = overview.history.map((s) =>
        s.holderKey === holderKey ? { ...s, holderType } : s,
      );
      return overview;
    },
    list_cockpit_layouts: (d) => d.cockpitLayouts,
    save_cockpit_layout: (d, a, ctx) => {
      const input = unwrap(a);
      const name = (str(input.name) ?? "").trim();
      const existing = d.cockpitLayouts.find((l) => l.name === name);
      if (existing) {
        const { next, updated } = mapReplace(
          d.cockpitLayouts,
          (l) => l.id === existing.id,
          (l) => ({
            ...l,
            panelsJson: str(input.panelsJson) ?? l.panelsJson,
            layoutJson: str(input.layoutJson),
            dockviewVersion: str(input.dockviewVersion),
            updatedAt: SAMPLE_NOW,
          }),
        );
        d.cockpitLayouts = next;
        return updated;
      }
      const layout = {
        id: ctx.nextId("layout"),
        name,
        ordinal: d.cockpitLayouts.length,
        panelsJson: str(input.panelsJson) ?? "{}",
        layoutJson: str(input.layoutJson),
        dockviewVersion: str(input.dockviewVersion),
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      d.cockpitLayouts = [...d.cockpitLayouts, layout];
      return layout;
    },
    delete_cockpit_layout: (d, a) => {
      const layoutId = str(unwrap(a).layoutId);
      d.cockpitLayouts = d.cockpitLayouts.filter((l) => l.id !== layoutId);
      return undefined;
    },
    // Backend parity (storage/cockpit_layouts.rs rename_cockpit_layout, issue
    // #89): in-place rename keeping id/ordinal; empty → invalid name, another
    // layout with the target name → duplicate, unknown id → not found.
    rename_cockpit_layout: (d, a) => {
      const input = unwrap(a);
      const layoutId = str(input.layoutId) ?? "";
      const name = (str(input.name) ?? "").trim();
      if (!name) throw new Error("invalid_cockpit_layout_name");
      const existing = d.cockpitLayouts.find((l) => l.id === layoutId);
      if (!existing) throw new Error("cockpit_layout_not_found");
      const clash = d.cockpitLayouts.find((l) => l.name === name && l.id !== layoutId);
      if (clash) throw new Error("duplicate_cockpit_layout_name");
      const { next, updated } = mapReplace(
        d.cockpitLayouts,
        (l) => l.id === layoutId,
        (l) => ({ ...l, name, updatedAt: SAMPLE_NOW }),
      );
      d.cockpitLayouts = next;
      return updated;
    },

    // --- Autopilot (autonomous report pipeline, ADR 0055) ---
    get_company_autopilot: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const mode =
        d.autopilotModes.find((m) => m.companyId === companyId)?.mode ?? "off";
      return { companyId, mode };
    },
    set_company_autopilot: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? "";
      const mode = str(input.mode) ?? "off";
      const entry = { companyId, mode };
      const existing = d.autopilotModes.some((m) => m.companyId === companyId);
      if (existing) {
        const { next } = mapReplace(
          d.autopilotModes,
          (m) => m.companyId === companyId,
          () => entry,
        );
        d.autopilotModes = next;
      } else {
        d.autopilotModes = [...d.autopilotModes, entry];
      }
      return entry;
    },
    list_company_autopilot_modes: (d) => d.autopilotModes,
    set_companies_autopilot: (d, a) => {
      const input = unwrap(a);
      const companyIds = Array.isArray(input.companyIds)
        ? (input.companyIds as string[])
        : [];
      const mode = str(input.mode) ?? "off";
      const ids = new Set(companyIds);
      const kept = d.autopilotModes.filter(
        (entry) => !ids.has(entry.companyId),
      );
      d.autopilotModes = [
        ...kept,
        ...companyIds.map((companyId) => ({ companyId, mode })),
      ];
      return companyIds.length;
    },
    list_autopilot_runs: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const notificationState = str(input.notificationState);
      return d.autopilotRuns.filter(
        (run) =>
          (!companyId || run.companyId === companyId) &&
          (!notificationState || run.notificationState === notificationState),
      );
    },
    // Missing from the fidelity corpus (knip-flagged orphan) until the Today/Pulse
    // undo surface wired it up (issue 7062f7e) — mirrors the Rust command: revert
    // exactly the facts recorded on the run, then clear the run's produced list.
    // Idempotent, like the real command — an already-cleared run reverts nothing.
    undo_autopilot_run: (d, a) => {
      const runId = str(unwrap(a).runId) ?? "";
      const run = d.autopilotRuns.find((r) => r.id === runId);
      const producedFactIds = run?.producedFactIds ?? [];
      const revertedFactIds = d.financialFacts
        .filter((f) => producedFactIds.includes(f.id))
        .map((f) => f.id);
      d.financialFacts = d.financialFacts.filter(
        (f) => !producedFactIds.includes(f.id),
      );
      const { next } = mapReplace(
        d.autopilotRuns,
        (r) => r.id === runId,
        (r) => ({ ...r, producedFactIds: [] }),
      );
      d.autopilotRuns = next;
      return { runId, revertedFactIds };
    },

    // The Rust-side scheduler owns the refresh cadence (ADR 0055); the mock mirrors
    // it by publishing per-adapter next-due epoch-ms. Feed adapters share the global
    // poll interval with a small distinct per-adapter offset (the deterministic
    // jitter that lives in Rust); the registry uses its own interval.
    get_scheduler_status: (d) => {
      const now = Date.now();
      const pollMs = (d.settings.pollIntervalSeconds || 900) * 1000;
      const sourceNextDueMs: Record<string, number> = {};
      d.sourceAdapters
        .filter(
          (adapter) =>
            adapter.enabled && adapter.sourceType !== "company_registry",
        )
        .forEach((adapter, index) => {
          sourceNextDueMs[adapter.id] = now + pollMs + (5 + index * 10) * 1000;
        });
      const registry = d.sourceAdapters.find(
        (adapter) =>
          adapter.enabled && adapter.sourceType === "company_registry",
      );
      const registryNextDueMs = registry
        ? now + registry.defaultPollIntervalSeconds * 1000
        : null;
      return { sourceNextDueMs, registryNextDueMs };
    },

    // --- Watchlists ---
    list_watchlists: (d) => d.watchlists,
    list_watchlist_memberships: (d, a) => {
      const watchlistId = str(unwrap(a).watchlistId);
      return watchlistId
        ? d.watchlistMemberships.filter((m) => m.watchlistId === watchlistId)
        : d.watchlistMemberships;
    },
    create_watchlist: (d, a) => {
      const input = unwrap(a);
      const name = str(input.name) ?? "Watchlist";
      const watchlist = {
        id: `watchlist_${name
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, "_")
          .replace(/^_|_$/g, "")}`,
        name,
        description: str(input.description),
        companyCount: 0,
      };
      d.watchlists = [...d.watchlists, watchlist];
      return watchlist;
    },
    rename_watchlist: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.watchlists,
        (w) => w.id === str(input.id),
        (w) => ({
          ...w,
          name: str(input.name) ?? w.name,
          description: str(input.description),
        }),
      );
      d.watchlists = next;
      return updated ?? d.watchlists[0];
    },
    delete_watchlist: (d, a) => {
      const watchlistId = str(unwrap(a).watchlistId);
      d.watchlists = d.watchlists.filter((w) => w.id !== watchlistId);
      d.watchlistMemberships = d.watchlistMemberships.filter(
        (m) => m.watchlistId !== watchlistId,
      );
      return undefined;
    },
    add_company_to_watchlist: (d, a) => {
      const input = unwrap(a);
      const watchlistId = str(input.watchlistId) ?? "";
      const companyId = str(input.companyId) ?? "";
      const watchlist = d.watchlists.find((w) => w.id === watchlistId);
      if (
        watchlist &&
        !d.watchlistMemberships.some(
          (m) => m.watchlistId === watchlistId && m.companyId === companyId,
        )
      ) {
        d.watchlistMemberships = [
          ...d.watchlistMemberships,
          { watchlistId, watchlistName: watchlist.name, companyId },
        ];
        d.watchlists = d.watchlists.map((w) =>
          w.id === watchlistId ? { ...w, companyCount: w.companyCount + 1 } : w,
        );
      }
      return undefined;
    },
    remove_company_from_watchlist: (d, a) => {
      const input = unwrap(a);
      const watchlistId = str(input.watchlistId);
      const companyId = str(input.companyId);
      const before = d.watchlistMemberships.length;
      d.watchlistMemberships = d.watchlistMemberships.filter(
        (m) => !(m.watchlistId === watchlistId && m.companyId === companyId),
      );
      const watchlist = d.watchlists.find((w) => w.id === watchlistId);
      if (watchlist && d.watchlistMemberships.length < before)
        watchlist.companyCount = Math.max(0, watchlist.companyCount - 1);
      return undefined;
    },

    // --- Feed ---
    list_feed_items: (d) => d.feedItems,
    update_feed_item_state: (d, a) => {
      const input = unwrap(a);
      const id = str(input.id);
      let updated: ScenarioData["feedItems"][number] | undefined;
      d.feedItems = d.feedItems.map((item) => {
        if (item.id !== id) return item;
        updated = {
          ...item,
          unread: typeof input.read === "boolean" ? !input.read : item.unread,
          saved: typeof input.saved === "boolean" ? input.saved : item.saved,
        };
        return updated;
      });
      return updated ?? d.feedItems[0];
    },

    // --- Events / signals ---
    list_company_events: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const watchlistId = str(input.watchlistId);
      const eventType = str(input.eventType);
      const status = str(input.status);
      const dateFrom = str(input.dateFrom);
      const dateTo = str(input.dateTo);
      const watchlistCompanyIds = watchlistId
        ? d.watchlistMemberships
            .filter((m) => m.watchlistId === watchlistId)
            .map((m) => m.companyId)
        : [];
      return d.events.filter((event) => {
        const companyMatches = !companyId || event.companyId === companyId;
        const typeMatches = !eventType || event.eventType === eventType;
        const statusMatches = !status || event.status === status;
        const dateFromMatches = !dateFrom || event.eventDate >= dateFrom;
        const dateToMatches = !dateTo || event.eventDate <= dateTo;
        const watchlistMatches =
          !watchlistId || watchlistCompanyIds.includes(event.companyId);
        return (
          companyMatches &&
          typeMatches &&
          statusMatches &&
          dateFromMatches &&
          dateToMatches &&
          watchlistMatches
        );
      });
    },
    create_company_event: (d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? "";
      const company = d.companies.find((c) => c.id === companyId);
      const event = {
        id: ctx.nextId("event"),
        companyId,
        company: company?.qualifiedTicker ?? companyId,
        companyName: company?.displayName ?? companyId,
        eventType: str(input.eventType) ?? "shareholder_meeting",
        title: str(input.title) ?? "Event",
        eventDate: str(input.eventDate) ?? SAMPLE_NOW.slice(0, 10),
        eventTime: str(input.eventTime),
        status: str(input.status) ?? "scheduled",
        sourceType: str(input.sourceType) ?? "manual",
        sourceAdapterId: str(input.sourceAdapterId),
        sourceEventKey: str(input.sourceEventKey),
        sourceUrl: str(input.sourceUrl),
        attribution: str(input.attribution),
        fetchedAt: str(input.fetchedAt),
        manual: true,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      d.events = [...d.events, event];
      return event;
    },
    list_company_signals: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const status = str(input.status);
      return d.signals.filter(
        (s) =>
          (!companyId || s.companyId === companyId) &&
          (!status || s.status === status),
      );
    },
    confirm_company_signal: (d, a) => {
      const id = str(unwrap(a).id);
      const { next, updated } = mapReplace(
        d.signals,
        (s) => s.id === id,
        (s) => ({ ...s, status: "confirmed" as const }),
      );
      d.signals = next;
      return updated ?? d.signals[0];
    },
    reject_company_signal: (d, a) => {
      const id = str(unwrap(a).id);
      d.signals = d.signals.filter((s) => s.id !== id);
      return undefined;
    },
    confirm_derived_event: ok,
    list_alert_rules: (d) => d.alertRules,
    create_alert_rule: (d, a, ctx) => {
      const input = unwrap(a);
      const triggerType = str(input.triggerType) ?? "signal_category";
      const num = (value: unknown): number | null =>
        typeof value === "number" && Number.isFinite(value) ? value : null;
      const rule: ScenarioData["alertRules"][number] = {
        id: ctx.nextId("alert_rule"),
        triggerType:
          triggerType as ScenarioData["alertRules"][number]["triggerType"],
        signalCategory: str(input.signalCategory),
        priceMin: num(input.priceMin),
        priceMax: num(input.priceMax),
        scopeType: (str(input.scopeType) ??
          "company") as ScenarioData["alertRules"][number]["scopeType"],
        scopeRef: str(input.scopeRef) ?? "",
        enabled: true,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      // Mirrors the backend's typed DuplicateAlertRule rejection: an identical
      // trigger+scope(+prices) rule never inserts a twin (storage/attention.rs).
      const identical = d.alertRules.find(
        (existing) =>
          existing.triggerType === rule.triggerType &&
          existing.signalCategory === rule.signalCategory &&
          existing.priceMin === rule.priceMin &&
          existing.priceMax === rule.priceMax &&
          existing.scopeType === rule.scopeType &&
          existing.scopeRef === rule.scopeRef,
      );
      if (identical) {
        throw new Error(
          `an identical alert rule already exists: ${identical.id}`,
        );
      }
      d.alertRules = [...d.alertRules, rule];
      return rule;
    },
    update_alert_rule: (d, a) => {
      const input = unwrap(a);
      const id = str(input.id);
      const num = (value: unknown, current: number | null): number | null =>
        typeof value === "number" && Number.isFinite(value) ? value : current;
      const { next, updated } = mapReplace(
        d.alertRules,
        (r) => r.id === id,
        (r) => ({
          ...r,
          signalCategory:
            input.signalCategory != null
              ? str(input.signalCategory)
              : r.signalCategory,
          priceMin: num(input.priceMin, r.priceMin),
          priceMax: num(input.priceMax, r.priceMax),
          scopeType:
            input.scopeType != null
              ? (str(input.scopeType) as typeof r.scopeType)
              : r.scopeType,
          scopeRef:
            input.scopeRef != null
              ? (str(input.scopeRef) ?? r.scopeRef)
              : r.scopeRef,
          enabled:
            typeof input.enabled === "boolean" ? input.enabled : r.enabled,
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.alertRules = next;
      return updated ?? d.alertRules[0];
    },
    set_alert_rule_enabled: (d, a) => {
      const input = unwrap(a);
      const id = str(input.id);
      const enabled = input.enabled === true;
      const { next, updated } = mapReplace(
        d.alertRules,
        (r) => r.id === id,
        (r) => ({ ...r, enabled, updatedAt: SAMPLE_NOW }),
      );
      d.alertRules = next;
      return updated ?? d.alertRules[0];
    },
    delete_alert_rule: (d, a) => {
      const id = str(unwrap(a).id);
      d.alertRules = d.alertRules.filter((r) => r.id !== id);
      // Attention events CASCADE with their rule (ADR 0068 / data-model).
      d.attentionEvents = d.attentionEvents.filter((e) => e.ruleId !== id);
      return undefined;
    },
    list_attention_events: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const includeDismissed = input.includeDismissed === true;
      return d.attentionEvents
        .filter(
          (e) =>
            (!companyId || e.companyId === companyId) &&
            (includeDismissed || !e.dismissed),
        )
        .sort((left, right) =>
          left.firedAt < right.firedAt
            ? 1
            : left.firedAt > right.firedAt
              ? -1
              : 0,
        );
    },
    mark_attention_event_seen: (d, a) => {
      const id = str(unwrap(a).id);
      const { next } = mapReplace(
        d.attentionEvents,
        (e) => e.id === id,
        (e) => ({ ...e, seen: true }),
      );
      d.attentionEvents = next;
      return undefined;
    },
    // Batch "was on screen" marking (ADR 0097 dec. 5): flags exactly the given
    // ids; unknown ids are ignored, an empty batch is a no-op — the backend's
    // single UPDATE ... WHERE id IN (...) semantics.
    mark_attention_events_seen: (d, a) => {
      const raw = unwrap(a).ids;
      const ids = new Set(Array.isArray(raw) ? raw.map((value) => String(value)) : []);
      d.attentionEvents = d.attentionEvents.map((e) =>
        ids.has(e.id) ? { ...e, seen: true } : e,
      );
      return undefined;
    },
    dismiss_attention_event: (d, a) => {
      const id = str(unwrap(a).id);
      const { next } = mapReplace(
        d.attentionEvents,
        (e) => e.id === id,
        (e) => ({ ...e, dismissed: true, seen: true }),
      );
      d.attentionEvents = next;
      return undefined;
    },
    // Corpus-only setup bridge (mock_fidelity.rs dispatch calls the real store
    // hook). Fires enabled autopilot-completion rules for a company and returns
    // the resulting event, mirroring the inline evaluation the autopilot job runs
    // (ADR 0068 §T2). Not a registered tauri command — a fidelity seed step only.
    evaluate_autopilot_completion: (d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? "";
      const runId = str(input.runId) ?? "run_fidelity";
      const inWatchlist = (watchlistId: string) =>
        d.watchlistMemberships.some(
          (m) => m.watchlistId === watchlistId && m.companyId === companyId,
        );
      const matching = d.alertRules.filter(
        (r) =>
          r.enabled &&
          r.triggerType === "autopilot_run_completed" &&
          (r.scopeType === "company"
            ? r.scopeRef === companyId
            : inWatchlist(r.scopeRef)),
      );
      let firstEvent: ScenarioData["attentionEvents"][number] | undefined;
      for (const rule of matching) {
        const existing = d.attentionEvents.find(
          (e) =>
            e.ruleId === rule.id &&
            e.evidenceType === "autopilot_run" &&
            e.evidenceRef === runId,
        );
        if (existing) {
          firstEvent ??= existing;
          continue;
        }
        const firedRun = d.autopilotRuns.find((r) => r.id === runId);
        const event: ScenarioData["attentionEvents"][number] = {
          id: ctx.nextId("attn"),
          ruleId: rule.id,
          triggerType: "autopilot_run_completed",
          companyId,
          evidenceType: "autopilot_run",
          evidenceRef: runId,
          firedAt: SAMPLE_NOW,
          seen: false,
          dismissed: false,
          // `autopilot_run_completed` → notable (product-spec §Attention Routing /
          // `storage::severity`).
          severity: "notable",
          // Mirror the backend join (v0.60 D6): the processed report's title + the
          // run's status, so the event states WHAT the autopilot finished.
          evidenceTitle: firedRun?.reportDocumentTitle ?? null,
          evidenceDetail: firedRun?.status ?? null,
          witnessUrl: null,
        };
        d.attentionEvents = [...d.attentionEvents, event];
        firstEvent ??= event;
      }
      return firstEvent ?? null;
    },

    // Corpus-only setup bridge (epic #40 S3, ADR 0091 dec. 1): a background job
    // that exhausted its retries raises a SYSTEM `job_failed` event. There is no
    // user command to mint one — the real backend hook lives at the queue's single
    // terminal-failure point — so the bridge mirrors what that hook + the read
    // model produce: the failed job's kind as the detail, and the handler's
    // failure subject as the statement, falling back to the job's own error text.
    record_job_failure: (d, a, ctx) => {
      const input = unwrap(a);
      const jobId = str(input.jobId) ?? "job_fidelity";
      const kind = str(input.kind) ?? "history_sweep";
      const companyId = str(input.companyId) ?? null;
      const subject = str(input.subject) ?? null;
      const error = str(input.error) ?? "corpus failure";
      const existing = d.attentionEvents.find(
        (e) => e.triggerType === "job_failed" && e.evidenceRef === jobId,
      );
      // Dedup on (trigger, evidence) exactly like the backend's system partial
      // index: the same failed job never fires twice.
      if (existing) return existing;
      const event: ScenarioData["attentionEvents"][number] = {
        id: ctx.nextId("attn"),
        ruleId: null,
        triggerType: "job_failed",
        companyId,
        evidenceType: "job",
        evidenceRef: jobId,
        firedAt: SAMPLE_NOW,
        seen: false,
        dismissed: false,
        // Notable for every job kind (ADR 0091 dec. 1, owner decision).
        severity: "notable",
        evidenceTitle: subject ?? error,
        evidenceDetail: kind,
        witnessUrl: null,
      };
      d.attentionEvents = [...d.attentionEvents, event];
      return event;
    },

    // --- Morning briefing (ADR 0068 decision 4, §T5) ---
    // `generate` enqueues an async compose job on the durable queue; the real
    // backend job runs on the worker, not the mock. So the Today card's
    // generate -> refetch flow is deterministically testable without
    // simulating the worker/queue, the mock composes a minimal briefing
    // eagerly (one item citing the first watched company's newest signal, when
    // one exists) and stores it as the latest — mirroring the "immediate first
    // tick" poll idiom (`CompanyCoveragePanel`) the card's hook reuses.
    generate_morning_briefing: (d, _a, ctx) => {
      const company = d.companies[0];
      const signal = company
        ? d.signals.find((s) => s.companyId === company.id)
        : undefined;
      const briefingId = ctx.nextId("briefing");
      const items = company
        ? [
            {
              id: ctx.nextId("briefing_item"),
              briefingId,
              position: 0,
              itemType: "signal" as const,
              companyId: company.id,
              domainDate: SAMPLE_NOW.slice(0, 10),
              citationKey: "b1",
              evidenceType: "company_signal",
              evidenceRef: signal?.id ?? "sig_mock",
              title: signal?.title ?? "New signal since your last briefing",
              detail: signal?.categoryDisplayName ?? null,
              createdAt: SAMPLE_NOW,
            },
          ]
        : [];
      d.morningBriefing = {
        id: briefingId,
        composedAt: SAMPLE_NOW,
        since: d.morningBriefing?.composedAt ?? "",
        language: null,
        createdAt: SAMPLE_NOW,
        items,
      };
      return undefined;
    },
    get_latest_morning_briefing: (d) => d.morningBriefing ?? null,

    list_notebook_entries: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId
        ? d.notebookEntries.filter((n) => n.companyId === companyId)
        : d.notebookEntries;
    },
    create_notebook_entry: (d, a, ctx) => {
      const input = unwrap(a);
      const rawOrigins = Array.isArray(input.origins)
        ? (input.origins as Record<string, unknown>[])
        : [];
      const entry = {
        id: ctx.nextId("note"),
        companyId: str(input.companyId) ?? "",
        title: str(input.title) ?? "Note",
        body: str(input.body) ?? "",
        bodyFormat: str(input.bodyFormat) ?? "markdown",
        tags: Array.isArray(input.tags) ? (input.tags as string[]) : [],
        kind: str(input.kind) ?? "note",
        claimStatus: str(input.claimStatus),
        eventDate: str(input.eventDate),
        followUpAfter: str(input.followUpAfter),
        followUpDate: str(input.followUpDate),
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
        origins: rawOrigins.map((origin, index) => ({
          id: `${ctx.nextId("note_origin")}_${index}`,
          sourceType: str(origin.sourceType) ?? "manual",
          sourceId: str(origin.sourceId),
          sourceUrl: str(origin.sourceUrl),
          label: str(origin.label),
          createdAt: SAMPLE_NOW,
        })),
      };
      d.notebookEntries = [...d.notebookEntries, entry];
      return entry;
    },
    create_note_from_transcript_selection: (d, a, ctx) =>
      handlers.create_notebook_entry(d, a, ctx),
    update_notebook_entry: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.notebookEntries,
        (n) => n.id === str(input.id),
        (n) => ({
          ...n,
          title: str(input.title) ?? n.title,
          body: str(input.body) ?? n.body,
          tags: Array.isArray(input.tags) ? (input.tags as string[]) : n.tags,
          kind: str(input.kind) ?? n.kind,
          claimStatus: str(input.claimStatus),
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.notebookEntries = next;
      return updated ?? d.notebookEntries[0];
    },
    delete_notebook_entry: (d, a) => {
      const id = str(unwrap(a).id);
      d.notebookEntries = d.notebookEntries.filter((n) => n.id !== id);
      return undefined;
    },

    // --- Transcripts ---
    list_video_transcript_jobs: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId
        ? d.transcriptJobs.filter((j) => j.companyId === companyId)
        : d.transcriptJobs;
    },
    list_transcript_segments: (d, a) => {
      const jobId = str(unwrap(a).transcriptJobId);
      return d.transcriptSegments.filter(
        (s) => !jobId || s.transcriptJobId === jobId,
      );
    },
    create_video_transcript_job: (d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const company = d.companies.find((c) => c.id === companyId);
      void ctx;
      const job = {
        id: "transcript_job_created",
        companyId,
        company: company?.qualifiedTicker ?? null,
        companyName: company?.displayName ?? null,
        providerId: str(input.providerId) ?? "provider_gemini",
        sourceType: "youtube",
        sourceUrl: str(input.sourceUrl) ?? "",
        sourceLabel: str(input.sourceLabel),
        companyResolutionStatus: companyId ? "resolved" : "pending",
        recognizedCompanyCandidates: [],
        status: "queued",
        errorCode: null,
        createdAt: SAMPLE_NOW,
        startedAt: null,
        finishedAt: null,
        error: null,
      };
      d.transcriptJobs = [...d.transcriptJobs, job];
      return job;
    },
    run_video_transcript_job: (d, a) => {
      const input = unwrap(a);
      const id = str(input.jobId) ?? str(input.id);
      let updated: ScenarioData["transcriptJobs"][number] | undefined;
      d.transcriptJobs = d.transcriptJobs.map((job) => {
        if (job.id !== id) return job;
        updated = {
          ...job,
          status: "completed",
          startedAt: SAMPLE_NOW,
          finishedAt: SAMPLE_NOW,
        };
        return updated;
      });
      return updated ?? d.transcriptJobs[0];
    },
    update_video_transcript_job: (d, a) => {
      const input = unwrap(a);
      const id = str(input.jobId) ?? str(input.id);
      const sourceLabel = str(input.sourceLabel);
      let updated: ScenarioData["transcriptJobs"][number] | undefined;
      d.transcriptJobs = d.transcriptJobs.map((job) => {
        if (job.id !== id) return job;
        updated = {
          ...job,
          sourceLabel: sourceLabel !== null ? sourceLabel : job.sourceLabel,
        };
        return updated;
      });
      return updated ?? d.transcriptJobs[0];
    },
    delete_video_transcript_job: (d, a) => {
      const jobId = str(unwrap(a).jobId);
      d.transcriptJobs = d.transcriptJobs.filter((j) => j.id !== jobId);
      d.transcriptSegments = d.transcriptSegments.filter(
        (s) => s.transcriptJobId !== jobId,
      );
      return undefined;
    },
    resolve_transcript_job_company: (d, a) => {
      const input = unwrap(a);
      const id = str(input.jobId);
      const companyId = str(input.companyId);
      const company = d.companies.find((c) => c.id === companyId);
      let updated: ScenarioData["transcriptJobs"][number] | undefined;
      d.transcriptJobs = d.transcriptJobs.map((job) => {
        if (job.id !== id) return job;
        updated = {
          ...job,
          companyId: companyId ?? job.companyId,
          company: company?.qualifiedTicker ?? job.company,
          companyName: company?.displayName ?? job.companyName,
          companyResolutionStatus: "resolved",
        };
        return updated;
      });
      return updated ?? d.transcriptJobs[0];
    },

    // --- Research workspace ---
    list_research_evidence: (d, a) =>
      buildTimeline(d, unwrap(a) as ResearchEvidenceInput),
    list_company_timeline: (d, a) =>
      buildTimeline(d, {
        companyId: str(unwrap(a).companyId),
      } as ResearchEvidenceInput),
    list_watchlist_timeline: (d, a) =>
      buildTimeline(d, {
        watchlistId: str(unwrap(a).watchlistId),
      } as ResearchEvidenceInput),
    mark_research_scope_reviewed: (d, a, ctx) => {
      const input = unwrap(a);
      const scopeType = (str(input.scopeType) ?? "company") as
        "company" | "watchlist";
      const scopeId = str(input.scopeId) ?? "";
      const existing = d.researchReviewCheckpoints.find(
        (c) => c.scopeType === scopeType && c.scopeId === scopeId,
      );
      if (!existing) {
        const checkpoint = {
          id: ctx.nextId("checkpoint"),
          scopeType,
          scopeId,
          reviewedAt: SAMPLE_NOW,
          createdAt: SAMPLE_NOW,
          updatedAt: SAMPLE_NOW,
        };
        d.researchReviewCheckpoints = [
          ...d.researchReviewCheckpoints,
          checkpoint,
        ];
        return checkpoint;
      }
      const { next, updated } = mapReplace(
        d.researchReviewCheckpoints,
        (c) => c.scopeType === scopeType && c.scopeId === scopeId,
        (c) => ({
          ...c,
          reviewedAt: str(input.reviewedAt) ?? SAMPLE_NOW,
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.researchReviewCheckpoints = next;
      return updated ?? existing;
    },
    list_research_review_state: (d, a) => {
      const input = unwrap(a);
      return (
        d.researchReviewCheckpoints.find(
          (c) =>
            c.scopeType === str(input.scopeType) &&
            c.scopeId === str(input.scopeId),
        ) ?? null
      );
    },
    list_research_questions: (d, a) => {
      const input = unwrap(a);
      const scopeId = str(input.scopeId);
      const status = str(input.status);
      return d.researchQuestions.filter(
        (q) =>
          (!scopeId || q.scopeId === scopeId) &&
          (!status || q.status === status),
      );
    },
    create_research_question: (d, a) => {
      const input = unwrap(a);
      const scopeType = (str(input.scopeType) ?? "company") as
        "company" | "watchlist";
      const scopeId = str(input.scopeId) ?? "";
      const question = {
        id: `research_question_${scopeType}_${scopeId}_${d.researchQuestions.length + 1}`,
        scopeType,
        scopeId,
        title: str(input.title) ?? "Question",
        body: str(input.body) ?? "",
        status: "open" as const,
        closedAt: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      d.researchQuestions = [...d.researchQuestions, question];
      // Surface the new question on the research evidence timeline.
      d.researchEvidence = [
        ...d.researchEvidence,
        {
          id: `evidence_research_question_${question.id}`,
          evidenceType: "research_question",
          sourceDomain: "research",
          sourceId: question.id,
          companyId: question.scopeId,
          occurredAt: question.updatedAt,
          title: question.title,
          summary: question.body || null,
          sourceUrl: null,
          attribution: null,
          trustCategory: "user_note",
          reviewState: {
            changedSinceCompanyReview: true,
            changedSinceWatchlistReview: true,
          },
        },
      ];
      return question;
    },
    update_research_question: (d, a) => {
      const input = unwrap(a);
      const status = str(input.status);
      const { next, updated } = mapReplace(
        d.researchQuestions,
        (q) => q.id === str(input.id),
        (q) => ({
          ...q,
          title: str(input.title) ?? q.title,
          body: str(input.body) ?? q.body,
          status:
            status === "open" || status === "answered" || status === "closed"
              ? status
              : q.status,
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.researchQuestions = next;
      return updated ?? d.researchQuestions[0];
    },
    delete_research_question: (d, a) => {
      const id = str(unwrap(a).id);
      d.researchQuestions = d.researchQuestions.filter((q) => q.id !== id);
      return undefined;
    },
    list_evidence_links: (d, a) => {
      const input = unwrap(a);
      const endpointId = str(input.endpointId);
      return d.evidenceLinks.filter(
        (l) => !endpointId || l.fromId === endpointId || l.toId === endpointId,
      );
    },
    create_evidence_link: (d, a, ctx) => {
      const input = unwrap(a);
      const fromType = (str(input.fromType) ?? "feed_item") as never;
      const fromId = str(input.fromId) ?? "";
      const toType = (str(input.toType) ?? "notebook_entry") as never;
      const toId = str(input.toId) ?? "";
      const relationType = (str(input.relationType) ?? "cites") as never;
      // Idempotent: an identical link returns the existing row (a non-deduped
      // create would re-fire on every render and loop the UI).
      const existing = d.evidenceLinks.find(
        (link) =>
          link.fromType === fromType &&
          link.fromId === fromId &&
          link.toType === toType &&
          link.toId === toId &&
          link.relationType === relationType,
      );
      if (existing) return existing;
      const link = {
        id: ctx.nextId("evidence_link"),
        fromType,
        fromId,
        toType,
        toId,
        relationType,
        createdAt: SAMPLE_NOW,
      };
      d.evidenceLinks = [...d.evidenceLinks, link];
      return link;
    },
    delete_evidence_link: (d, a) => {
      const id = str(unwrap(a).id);
      d.evidenceLinks = d.evidenceLinks.filter((l) => l.id !== id);
      return undefined;
    },
    list_research_reminders: (d, a) => {
      const input = unwrap(a);
      const scopeId = str(input.scopeId);
      const status = str(input.status);
      return d.researchReminders.filter(
        (r) =>
          (!scopeId || r.scopeId === scopeId) &&
          (!status || r.status === status),
      );
    },
    create_research_reminder: (d, a, ctx) => {
      const input = unwrap(a);
      const reminder = {
        id: ctx.nextId("reminder"),
        scopeType: (str(input.scopeType) ?? "company") as
          "company" | "watchlist",
        scopeId: str(input.scopeId) ?? "",
        companyId: str(input.companyId),
        reminderKind: (str(input.reminderKind) ?? "manual_research") as never,
        sourceType: (str(input.sourceType) ?? null) as never,
        sourceId: str(input.sourceId),
        title: str(input.title) ?? "Reminder",
        body: str(input.body) ?? "",
        dueAt: str(input.dueAt),
        status: "open" as const,
        snoozedUntil: null,
        completedAt: null,
        dismissedAt: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      d.researchReminders = [...d.researchReminders, reminder];
      return reminder;
    },
    update_research_reminder: (d, a) => {
      const input = unwrap(a);
      const status = str(input.status);
      const { next, updated } = mapReplace(
        d.researchReminders,
        (r) => r.id === str(input.id),
        (r) => ({
          ...r,
          status:
            status === "open" ||
            status === "completed" ||
            status === "dismissed"
              ? status
              : r.status,
          dueAt: str(input.dueAt) !== null ? str(input.dueAt) : r.dueAt,
          snoozedUntil:
            str(input.snoozedUntil) !== null
              ? str(input.snoozedUntil)
              : r.snoozedUntil,
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.researchReminders = next;
      return updated ?? d.researchReminders[0];
    },
    delete_research_reminder: (d, a) => {
      const id = str(unwrap(a).id);
      d.researchReminders = d.researchReminders.filter((r) => r.id !== id);
      return undefined;
    },
    list_management_claims: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId
        ? d.managementClaims.filter((c) => c.companyId === companyId)
        : d.managementClaims;
    },
    list_claims_to_verify: (d) => d.claimsToVerify,
    create_management_claim: (d, a, ctx) => {
      const base = { ...d.managementClaims[0] };
      const input = unwrap(a);
      base.id = ctx.nextId("claim");
      base.companyId = str(input.companyId) ?? base.companyId;
      base.statement = str(input.statement) ?? base.statement;
      base.status = "pending";
      d.managementClaims = [...d.managementClaims, base];
      return base;
    },
    update_management_claim: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.managementClaims,
        (c) => c.id === str(input.id),
        (c) => ({
          ...c,
          statement: str(input.statement) ?? c.statement,
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.managementClaims = next;
      return updated ?? d.managementClaims[0];
    },
    delete_management_claim: (d, a) => {
      const id = str(unwrap(a).id);
      d.managementClaims = d.managementClaims.filter((c) => c.id !== id);
      return undefined;
    },
    set_claim_verdict: (d, a) => {
      const input = unwrap(a);
      const status = str(input.status);
      const validStatus =
        status === "pending" ||
        status === "delivered" ||
        status === "partially_delivered" ||
        status === "missed" ||
        status === "revised";
      const { next, updated } = mapReplace(
        d.managementClaims,
        (c) => c.id === str(input.claimId),
        (c) => ({
          ...c,
          status: validStatus ? status : c.status,
          verifyingFactId: str(input.verifyingFactId),
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.managementClaims = next;
      return updated ?? d.managementClaims[0];
    },
    list_financial_periods: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId
        ? d.financialPeriods.filter((p) => p.companyId === companyId)
        : d.financialPeriods;
    },
    create_financial_period: (d, a, ctx) => {
      const base = { ...d.financialPeriods[0] };
      const input = unwrap(a);
      base.id = ctx.nextId("period");
      base.companyId = str(input.companyId) ?? base.companyId;
      if (typeof input.fiscalYear === "number")
        base.fiscalYear = input.fiscalYear;
      base.periodType = str(input.periodType) ?? base.periodType;
      base.periodEndDate = str(input.periodEndDate) ?? null;
      d.financialPeriods = [...d.financialPeriods, base];
      return base;
    },
    update_financial_period: (d, a) => {
      const input = unwrap(a);
      const period = d.financialPeriods.find((p) => p.id === str(input.id));
      if (period && typeof input.fiscalYear === "number")
        period.fiscalYear = input.fiscalYear;
      return period ?? d.financialPeriods[0];
    },
    delete_financial_period: (d, a) => {
      const id = str(unwrap(a).id);
      d.financialPeriods = d.financialPeriods.filter((p) => p.id !== id);
      return undefined;
    },
    list_financial_facts: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const periodId = str(input.periodId);
      return d.financialFacts.filter(
        (f) =>
          (!companyId || f.companyId === companyId) &&
          (!periodId || f.periodId === periodId),
      );
    },
    create_financial_fact: (d, a, ctx) => {
      const base = { ...d.financialFacts[0] };
      const input = unwrap(a);
      base.id = ctx.nextId("fact");
      base.companyId = str(input.companyId) ?? base.companyId;
      base.periodId = str(input.periodId) ?? base.periodId;
      base.definitionId = str(input.definitionId) ?? base.definitionId;
      base.valueNumeric = str(input.valueNumeric) ?? base.valueNumeric;
      base.supersedesId = str(input.supersedesId) ?? null;
      // Mirrors the Rust normalizer (#156): empty/whitespace -> null.
      base.annotation = str(input.annotation)?.trim() || null;
      d.financialFacts = [...d.financialFacts, base];
      return base;
    },
    update_financial_fact: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.financialFacts,
        (f) => f.id === str(input.id),
        (f) => ({
          ...f,
          valueNumeric: str(input.valueNumeric) ?? f.valueNumeric,
          // Keep / clear / replace (#156): absent keeps, "" clears, text replaces.
          annotation:
            input.annotation === undefined
              ? f.annotation
              : str(input.annotation)?.trim() || null,
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.financialFacts = next;
      return updated ?? d.financialFacts[0];
    },
    delete_financial_fact: (d, a) => {
      const id = str(unwrap(a).id);
      d.financialFacts = d.financialFacts.filter((f) => f.id !== id);
      return undefined;
    },

    // --- Fundamentals: KPIs ---
    list_kpi_definitions: (d) => d.kpiDefinitions,
    create_kpi_definition: (d, a, ctx) => {
      const base = { ...d.kpiDefinitions[0] };
      const input = unwrap(a);
      base.id = ctx.nextId("kpi_def");
      base.metricKey = str(input.metricKey) ?? base.metricKey;
      base.label = str(input.label) ?? base.label;
      d.kpiDefinitions = [...d.kpiDefinitions, base];
      return base;
    },
    list_kpi_relevance: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId
        ? d.kpiRelevance.filter((r) => r.companyId === companyId)
        : d.kpiRelevance;
    },
    // Upsert on (companyId, definitionId), mirroring the backend: since company
    // creation seeds the core KPI set, curating a core metric restates its row
    // instead of adding a second one (epic #229 T7).
    create_kpi_relevance: (d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? d.kpiRelevance[0]?.companyId;
      const definitionId =
        str(input.definitionId) ?? d.kpiRelevance[0]?.definitionId;
      const existing = d.kpiRelevance.find(
        (r) => r.companyId === companyId && r.definitionId === definitionId,
      );
      const base = {
        ...(existing ?? d.kpiRelevance[0]),
        id: existing?.id ?? ctx.nextId("kpi_rel"),
        companyId,
        definitionId,
        status: "active",
        source: str(input.source) ?? d.kpiRelevance[0]?.source,
        updatedAt: SAMPLE_NOW,
      };
      d.kpiRelevance = existing
        ? d.kpiRelevance.map((r) => (r.id === existing.id ? base : r))
        : [...d.kpiRelevance, base];
      return base;
    },
    update_kpi_relevance: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.kpiRelevance,
        (r) => r.id === str(input.id),
        (r) => ({
          ...r,
          status: str(input.status) ?? r.status,
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.kpiRelevance = next;
      return updated ?? d.kpiRelevance[0];
    },
    delete_kpi_relevance: (d, a) => {
      const id = str(unwrap(a).id);
      d.kpiRelevance = d.kpiRelevance.filter((r) => r.id !== id);
      return undefined;
    },
    list_fact_provenance: (d, a) => {
      const store =
        (d as { factProvenance?: Array<{ factId: string }> }).factProvenance ??
        [];
      const ids = new Set(
        ((unwrap(a).factIds as string[] | undefined) ?? []).map(String),
      );
      return store.filter((p) => ids.has(p.factId));
    },
    // Flagged FACTS with the context a reviewer needs (epic #229 T5): the real
    // backend JOINs provenance → fact → period → definition, so the mock derives
    // the same row from the same seeds instead of carrying a parallel one.
    // `companyId` omitted = every company (the MCP data-quality surface).
    list_flagged_fact_provenance: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      const provenance =
        (
          d as {
            factProvenance?: Array<{
              factId: string;
              sourceTier: string;
              validationStatus: string;
              driftJson: string | null;
              citation: string | null;
            }>;
          }
        ).factProvenance ?? [];
      return provenance
        .filter((p) => p.validationStatus === "flagged")
        .flatMap((p) => {
          const fact = d.financialFacts.find((f) => f.id === p.factId);
          if (!fact) return [];
          // `null` (absent / explicitly null) = every company, mirroring the
          // backend's `Option<String>` scope.
          if (companyId !== null && fact.companyId !== companyId) return [];
          const period = d.financialPeriods.find((x) => x.id === fact.periodId);
          const definition = d.kpiDefinitions.find(
            (x) => x.id === fact.definitionId,
          );
          return [
            {
              factId: p.factId,
              companyId: fact.companyId,
              metricKey: definition?.metricKey ?? "",
              label: definition?.label ?? "",
              valueNumeric: fact.valueNumeric,
              currency: fact.currency ?? null,
              fiscalYear: period?.fiscalYear ?? 0,
              periodType: period?.periodType ?? "",
              sourceTier: p.sourceTier,
              validationStatus: p.validationStatus,
              driftJson: p.driftJson ?? null,
              citation: p.citation ?? null,
            },
          ];
        });
    },
    // The company's NON-EMITTING extraction outcomes, newest attempt first (ADR
    // 0061 decision 2). Clean periods are excluded, and an absent row means
    // "never attempted" — so `[]` is the honest "nothing flagged" state, never a
    // stand-in for a failed read.
    list_flagged_extraction_outcomes: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return [...(d.flaggedExtractionOutcomes ?? [])]
        .filter((o) => o.companyId === companyId && o.reasonCode !== "emitted")
        .sort((left, right) =>
          right.lastAttemptedAt.localeCompare(left.lastAttemptedAt),
        );
    },
    // "Try again" on a flagged period. The company/document/period come from the
    // STORED row, so the retry cannot target a different slot than the one shown.
    // The real backend updates that row in place; the mock models the outcome the
    // UI depends on — a period whose cause is fixed LEAVES the flagged list.
    rerun_extraction_outcome: (d, a) => {
      const outcomeId = str(unwrap(a).outcomeId) ?? "";
      const outcome = (d.flaggedExtractionOutcomes ?? []).find(
        (o) => o.id === outcomeId,
      );
      if (!outcome) {
        throw new Error(`no extraction outcome with id ${outcomeId}`);
      }
      d.flaggedExtractionOutcomes = (d.flaggedExtractionOutcomes ?? []).filter(
        (o) => o.id !== outcomeId,
      );
      return {
        acceptance: "accepted",
        tier: outcome.tier ?? "pdf",
        emitted: true,
        producedFactIds: [],
        skippedFactIds: [],
        divergentCount: 0,
        reasonCode: "emitted",
        driftJson: null,
      };
    },
    run_structured_extraction: () => ({
      acceptance: "accepted",
      tier: "esef",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: [],
      divergentCount: 0,
      reasonCode: "emitted",
      driftJson: null,
      tier4: null,
    }),
    // Per-document structured extraction (ADR 0061 S5) — the period is derived
    // server-side; the mock returns the same summary shape as the raw pipeline.
    extract_report_document_data: () => ({
      acceptance: "accepted",
      tier: "pdf",
      emitted: true,
      producedFactIds: [],
      skippedFactIds: [],
      divergentCount: 0,
      reasonCode: "emitted",
      driftJson: null,
      tier4: null,
    }),
    list_report_documents: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId
        ? d.reportDocuments.filter((r) => r.companyId === companyId)
        : d.reportDocuments;
    },
    // Classification is deterministic Rust code (classify_doc_kind); the mock does
    // NOT re-derive kinds (a TS port would drift and the dual-execution corpus
    // pins correctness Rust-side). It reports the already-stored kinds grouped,
    // and updated: 0 — a read-shaped no-op over the seeded docKind values.
    reclassify_report_documents: (d) => {
      const byKind: Record<string, number> = {};
      for (const doc of d.reportDocuments) {
        if (doc.docKind == null) continue;
        byKind[doc.docKind] = (byKind[doc.docKind] ?? 0) + 1;
      }
      return { total: d.reportDocuments.length, updated: 0, byKind };
    },
    // Coverage read model (ADR 0077 §2). A simplified union over scenario data:
    // periodic-docKind documents keyed by their period, plus facts grouped by
    // period. Mirrors the Rust DTO shape; per-cell correctness is pinned by the
    // Rust unit tests + the dual-execution fidelity corpus. skippedBudget is
    // always false (the F3/T5.2 budget substrate is not modelled).
    get_fundamentals_coverage: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const periodIndex = (pt: string): number =>
        ({ Q1: 1, Q2: 2, H1: 2, Q3: 3, "9M": 3, Q4: 4, H2: 4, FY: 4 })[pt] ?? 0;
      const periodById = new Map(d.financialPeriods.map((p) => [p.id, p]));
      // Mirror of the Rust canonical_period_label: stored labels drift (legacy
      // `annual`, lowercase `q3`), and raw keys would split one period into two
      // rows. Uppercase + ANNUAL→FY; merged cells sum.
      const canonicalLabel = (pt: string): string => {
        const upper = pt.trim().toUpperCase();
        return upper === "ANNUAL" ? "FY" : upper;
      };
      type Key = string;
      const key = (fy: number, pt: string): Key =>
        `${fy} ${canonicalLabel(pt)}`;
      const rows = new Map<Key, { fiscalYear: number; periodType: string }>();

      // Reports: canonical periodic document per period (ssf beats jsf).
      const reports = new Map<
        Key,
        {
          documentId: string;
          docKind: string;
          title: string | null;
          structured: boolean;
          fetched: boolean;
        }
      >();
      for (const doc of d.reportDocuments) {
        if (doc.companyId !== companyId) continue;
        if (doc.docKind !== "periodic_ssf" && doc.docKind !== "periodic_jsf")
          continue;
        const period = doc.periodId ? periodById.get(doc.periodId) : undefined;
        if (!period) continue;
        const k = key(period.fiscalYear, period.periodType);
        rows.set(k, {
          fiscalYear: period.fiscalYear,
          periodType: canonicalLabel(period.periodType),
        });
        const existing = reports.get(k);
        if (
          existing &&
          existing.docKind === "periodic_ssf" &&
          doc.docKind === "periodic_jsf"
        )
          continue;
        reports.set(k, {
          documentId: doc.id,
          docKind: doc.docKind,
          title: doc.title ?? null,
          structured:
            (doc.contentType ?? "").includes("xhtml") ||
            (doc.contentType ?? "").includes("html"),
          fetched: doc.fetchStatus === "fetched",
        });
      }

      // Facts grouped by period, split by provenance validation state.
      const provenance = new Map(
        (
          (
            d as {
              factProvenance?: Array<{
                factId: string;
                validationStatus: string;
              }>;
            }
          ).factProvenance ?? []
        ).map((p) => [p.factId, p.validationStatus]),
      );
      const facts = new Map<
        Key,
        {
          total: number;
          validated: number;
          unvalidated: number;
          flagged: number;
        }
      >();
      for (const fact of d.financialFacts) {
        if (fact.companyId !== companyId) continue;
        const period = periodById.get(fact.periodId);
        if (!period) continue;
        const k = key(period.fiscalYear, period.periodType);
        rows.set(k, {
          fiscalYear: period.fiscalYear,
          periodType: canonicalLabel(period.periodType),
        });
        const cell = facts.get(k) ?? {
          total: 0,
          validated: 0,
          unvalidated: 0,
          flagged: 0,
        };
        cell.total += 1;
        const status = provenance.get(fact.id);
        if (status === "passed" || status === "witness_confirmed")
          cell.validated += 1;
        else if (status === "flagged") cell.flagged += 1;
        else cell.unvalidated += 1;
        facts.set(k, cell);
      }

      const periods = [...rows.entries()]
        .map(([k, { fiscalYear, periodType }]) => {
          const factCell = facts.get(k) ?? {
            total: 0,
            validated: 0,
            unvalidated: 0,
            flagged: 0,
          };
          return {
            fiscalYear,
            periodType,
            report: reports.get(k) ?? null,
            facts: factCell,
            review: { flaggedFacts: factCell.flagged },
            skippedBudget: false,
          };
        })
        .sort(
          (x, y) =>
            y.fiscalYear - x.fiscalYear ||
            periodIndex(y.periodType) - periodIndex(x.periodType) ||
            y.periodType.localeCompare(x.periodType),
        );

      return { companyId, periods };
    },
    // Report-documents view read model (ADR 0077 §1/§2, Panel B). Every stored
    // document for the company, tagged with its fiscal period and whether it is
    // that period's canonical report. The mock derives the period from the
    // document's LINKED financial period (periodId → financialPeriods) rather
    // than re-deriving it from the title/URL the way the Rust `document_period`
    // does — a TS port of that parser would drift, and the dual-execution
    // fidelity corpus (empty company) plus the Rust unit tests pin real
    // derivation. Canonical = the first periodic document per period, ssf
    // preferred over jsf (mirror of canonical_reports_per_period's kind rank).
    get_report_documents_view: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const periodById = new Map(d.financialPeriods.map((p) => [p.id, p]));
      const docs = d.reportDocuments.filter((r) => r.companyId === companyId);
      const periodOf = (doc: (typeof docs)[number]) => {
        const p = doc.periodId ? periodById.get(doc.periodId) : undefined;
        return p
          ? { fiscalYear: p.fiscalYear, periodType: p.periodType }
          : null;
      };
      const key = (fy: number, pt: string) => `${fy} ${pt}`;
      const canonicalByKey = new Map<string, string>();
      for (const doc of docs) {
        if (doc.docKind !== "periodic_ssf" && doc.docKind !== "periodic_jsf")
          continue;
        const period = periodOf(doc);
        if (!period) continue;
        const k = key(period.fiscalYear, period.periodType);
        const currentId = canonicalByKey.get(k);
        if (currentId == null) {
          canonicalByKey.set(k, doc.id);
          continue;
        }
        const current = docs.find((x) => x.id === currentId);
        if (
          current?.docKind === "periodic_jsf" &&
          doc.docKind === "periodic_ssf"
        ) {
          canonicalByKey.set(k, doc.id);
        }
      }
      const rows = docs.map((doc) => {
        const period = periodOf(doc);
        const canonical =
          period != null &&
          canonicalByKey.get(key(period.fiscalYear, period.periodType)) ===
            doc.id;
        return {
          document: doc,
          fiscalYear: period?.fiscalYear ?? null,
          periodType: period?.periodType ?? null,
          canonical,
        };
      });
      // Coverage roll-up (#174, epic #229 T3) — mirrors the Rust
      // `companies_lacking_periodic_coverage` predicate: `hasPeriodicCoverage`
      // is true only when a periodic document has ACTUALLY been fetched, so it
      // can be false while `periodicCount` (any fetch state) is > 0.
      const isPeriodic = (doc: (typeof docs)[number]) =>
        doc.docKind === "periodic_ssf" || doc.docKind === "periodic_jsf";
      const totals = {
        documents: docs.length,
        fetched: docs.filter((doc) => doc.fetchStatus === "fetched").length,
        pending: docs.filter((doc) => doc.fetchStatus === "pending").length,
        metadataOnly: docs.filter((doc) => doc.fetchStatus === "metadata_only")
          .length,
        periodicCount: docs.filter(isPeriodic).length,
        hasPeriodicCoverage: docs.some(
          (doc) => isPeriodic(doc) && doc.fetchStatus === "fetched",
        ),
      };
      return { companyId, rows, totals };
    },

    // --- Report-over-report diff (ADR 0052) ---
    list_report_diff_candidates: (_d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return {
        companyId,
        candidates: [
          {
            statementType: "ssf",
            sourceFormat: "pdf",
            older: {
              reportDocumentId: "rd_older",
              title: "2025 Q3 SSF",
              periodLabel: "2025 Q3",
              extractionStatus: "extracted",
            },
            newer: {
              reportDocumentId: "rd_newer",
              title: "2026 Q1 SSF",
              periodLabel: "2026 Q1",
              extractionStatus: "extracted",
            },
          },
        ],
      };
    },
    fetch_report_document: (_d, a) => ({
      reportDocumentId: str(unwrap(a).reportDocumentId) ?? "",
      fetched: true,
    }),
    extract_report_sections: () => ({
      extractionState: "extracted",
      sourceFormat: "pdf",
      sectionCount: 5,
      skipped: false,
    }),
    get_report_diff: () => ({
      statementType: "ssf",
      status: "ok",
      diff: {
        alignedCount: 12,
        sections: [
          {
            status: "changed",
            heading:
              "skonsolidowane sprawozdanie z zysków i strat oraz innych całkowitych dochodów",
            olderOrdinal: 1,
            newerOrdinal: 1,
            addedLines: 43,
            removedLines: 54,
          },
          {
            status: "only_older",
            heading: "informacje o standardach i interpretacjach mssf",
            olderOrdinal: 2,
            newerOrdinal: null,
            addedLines: 0,
            removedLines: 0,
          },
          {
            status: "changed",
            heading: "skonsolidowane sprawozdanie z sytuacji finansowej",
            olderOrdinal: 3,
            newerOrdinal: 3,
            addedLines: 42,
            removedLines: 43,
          },
        ],
      },
    }),
    capture_report_document: (d, a, ctx) => {
      const input = unwrap(a);
      const documentId = ctx.nextId("report_doc");
      const base = {
        ...d.reportDocuments[0],
        id: documentId,
        companyId:
          str(input.companyId) ?? d.reportDocuments[0]?.companyId ?? "",
        url: str(input.url) ?? d.reportDocuments[0]?.url ?? "",
        title:
          str(input.title) ?? d.reportDocuments[0]?.title ?? "Captured report",
      };
      d.reportDocuments = [...d.reportDocuments, base];
      // Contract return shape is DocumentCaptureResult, not the document itself.
      return {
        documentId,
        localPath: `${documentId}.pdf`,
        success: true,
        error: null,
      };
    },
    list_report_season: (d) => ({
      upcoming: d.reportSeasonUpcoming,
      past: d.reportSeasonPast,
      calendarFreshness: { lastFetchedAt: SAMPLE_NOW, stale: false },
    }),
    get_pre_report_card: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      return (
        d.preReportCards.find((c) => c.companyId === companyId) ??
        d.preReportCards[0] ??
        null
      );
    },
    mark_report_prepared: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.reportPreparations,
        (p) => p.companyId === str(input.companyId),
        (p) => ({ ...p, status: "prepared" as const }),
      );
      d.reportPreparations = next;
      return updated ?? d.reportPreparations[0];
    },
    mark_report_processed: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.reportPreparations,
        (p) => p.companyId === str(input.companyId),
        (p) => ({
          ...p,
          status: "processed" as const,
          processedAt: SAMPLE_NOW,
        }),
      );
      d.reportPreparations = next;
      return updated ?? d.reportPreparations[0];
    },

    // --- Decision journal (ADR 0071). Immutable, command-only state in ctx. ---
    create_decision_entry: (d, a, ctx) => {
      const input = unwrap(a);
      const entry = {
        id: ctx.nextId("decision_entry"),
        companyId: str(input.companyId) ?? "",
        kind: str(input.kind) ?? "",
        rationaleMd: str(input.rationaleMd) ?? "",
        decidedAt: str(input.decidedAt) ?? "",
        supersededByEntryId: str(input.supersededByEntryId),
        createdAt: SAMPLE_NOW,
      };
      ctx.decisionEntries = [...ctx.decisionEntries, entry];
      // Surface the entry on the research timeline (J2 wired the Rust UNION arm;
      // the mock must mirror it so timeline-driven UI/tests see it). occurredAt is
      // the decision's own date so it slots into chronology by decided_at.
      d.researchEvidence = [
        ...d.researchEvidence,
        {
          id: `evidence_decision_entry_${entry.id}`,
          evidenceType: "decision_entry",
          sourceDomain: "research",
          sourceId: entry.id,
          companyId: entry.companyId,
          occurredAt: entry.decidedAt || SAMPLE_NOW,
          title: entry.rationaleMd.split("\n")[0] || entry.kind,
          summary: null,
          sourceUrl: null,
          attribution: null,
          trustCategory: "user_note",
          reviewState: {
            changedSinceCompanyReview: true,
            changedSinceWatchlistReview: true,
          },
        },
      ];
      return entry;
    },
    list_decision_entries: (_d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const kind = str(input.kind);
      // Chronology of DECISIONS: decided_at DESC, id DESC (never insertion).
      return ctx.decisionEntries
        .filter(
          (e) =>
            (!companyId || e.companyId === companyId) &&
            (!kind || e.kind === kind),
        )
        .sort((x, y) => {
          const dx = String(x.decidedAt);
          const dy = String(y.decidedAt);
          if (dx !== dy) return dx < dy ? 1 : -1;
          return String(x.id) < String(y.id) ? 1 : -1;
        });
    },

    // --- KNF short selling (v0.55 T4b, ADR 0069 decision 3). Read model derived
    // from the seeded register rows/events; mirrors storage/short_positions.rs. ---
    list_short_positions: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? "";
      const today = SAMPLE_NOW.slice(0, 10);
      const cutoff = dateMinusDays(today, 30);

      const active = d.shortPositions
        .filter((p) => p.companyId === companyId && p.exitedAt === null)
        .sort(
          (x, y) =>
            y.netPositionPct - x.netPositionPct ||
            (x.holderName < y.holderName ? -1 : 1),
        );
      const changedHolders = new Set(
        d.shortPositionEvents
          .filter((e) => e.companyId === companyId && e.positionDate >= cutoff)
          .map((e) => e.holderName),
      );
      const events = d.shortPositionEvents
        .filter((e) => e.companyId === companyId)
        .sort((x, y) =>
          x.positionDate < y.positionDate
            ? 1
            : x.positionDate > y.positionDate
              ? -1
              : 0,
        )
        .slice(0, 50)
        .map((e) => ({
          kind: e.kind,
          holderName: e.holderName,
          fromPct: e.fromPct,
          toPct: e.toPct,
          positionDate: e.positionDate,
        }));
      const exits = d.shortPositions
        .filter((p) => p.companyId === companyId && p.exitedAt !== null)
        .sort((x, y) => (String(x.exitedAt) < String(y.exitedAt) ? 1 : -1));
      const lastExit = exits.length
        ? {
            holderName: exits[0].holderName,
            exitedOn: String(exits[0].exitedAt).slice(0, 10),
          }
        : null;

      return {
        positions: active.map((p) => ({
          holderName: p.holderName,
          netPositionPct: p.netPositionPct,
          positionDate: p.positionDate,
          recentlyChanged: changedHolders.has(p.holderName),
        })),
        events,
        lastExit,
        aggregatePct: active.reduce((sum, p) => sum + p.netPositionPct, 0),
        delta30dPp: d.shortPositionEvents
          .filter((e) => e.companyId === companyId && e.positionDate >= cutoff)
          .reduce(
            (sum, e) => sum + signedEventDelta(e.kind, e.fromPct, e.toPct),
            0,
          ),
        // Mirrors source_adapters.last_success_at for knf-short-selling; the
        // sample world has no adapter-run state, so expose the sample clock when
        // any register data exists, null otherwise (matches a never-pulled DB).
        registerUpdatedAt:
          d.shortPositions.length > 0 || d.shortPositionEvents.length > 0
            ? SAMPLE_NOW
            : null,
      };
    },

    // --- Pre-report expectations (ADR 0071). Command-only state in ctx. ---
    create_report_expectation: (_d, a, ctx) => {
      const input = unwrap(a);
      const id = ctx.nextId("report_expectation");
      const metricsIn = Array.isArray(input.metrics)
        ? (input.metrics as Record<string, unknown>[])
        : [];
      const expectation = {
        id,
        companyId: str(input.companyId) ?? "",
        eventKey: str(input.eventKey) ?? "",
        fiscalYear: typeof input.fiscalYear === "number" ? input.fiscalYear : 0,
        periodType: str(input.periodType) ?? "",
        stanceMd: str(input.stanceMd) ?? "",
        frozenAt: null as string | null,
        resolutionNoteMd: null as string | null,
        resolvedAt: null as string | null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
        metrics: metricsIn.map((m, i) => buildExpectationMetric(id, m, i)),
      };
      ctx.reportExpectations = [...ctx.reportExpectations, expectation];
      return expectation;
    },
    update_report_expectation: (d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const eventKey = str(input.eventKey);
      const found = ctx.reportExpectations.find(
        (e) => e.companyId === companyId && e.eventKey === eventKey,
      );
      if (!found) throw new Error("report expectation not found");
      // Freeze check shares the read: once the period's facts land, the edit is
      // refused and frozenAt stamped (mirror of the Rust in-transaction check).
      if (
        periodHasFacts(
          d,
          String(found.companyId),
          Number(found.fiscalYear),
          String(found.periodType),
        )
      ) {
        ctx.reportExpectations = ctx.reportExpectations.map((e) =>
          e === found
            ? { ...e, frozenAt: (e.frozenAt as string | null) ?? SAMPLE_NOW }
            : e,
        );
        throw new Error("report expectation is frozen");
      }
      const nextMetrics = Array.isArray(input.metrics)
        ? (input.metrics as Record<string, unknown>[]).map((m, i) =>
            buildExpectationMetric(String(found.id), m, i),
          )
        : (found.metrics as unknown[]);
      const updated = {
        ...found,
        stanceMd:
          typeof input.stanceMd === "string" ? input.stanceMd : found.stanceMd,
        metrics: nextMetrics,
        updatedAt: SAMPLE_NOW,
      };
      ctx.reportExpectations = ctx.reportExpectations.map((e) =>
        e === found ? updated : e,
      );
      return updated;
    },
    list_report_expectations: (d, a, ctx) => {
      const companyId = str(unwrap(a).companyId);
      // Freeze-on-read: stamp frozenAt for any expectation whose facts arrived.
      ctx.reportExpectations = ctx.reportExpectations.map((e) => {
        if (e.frozenAt) return e;
        const frozen = periodHasFacts(
          d,
          String(e.companyId),
          Number(e.fiscalYear),
          String(e.periodType),
        );
        return frozen ? { ...e, frozenAt: SAMPLE_NOW } : e;
      });
      return ctx.reportExpectations.filter(
        (e) => !companyId || e.companyId === companyId,
      );
    },
    expectation_review: (d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const eventKey = str(input.eventKey);
      const found = ctx.reportExpectations.find(
        (e) => e.companyId === companyId && e.eventKey === eventKey,
      );
      if (!found) throw new Error("report expectation not found");
      const factsAvailable = periodHasFacts(
        d,
        String(found.companyId),
        Number(found.fiscalYear),
        String(found.periodType),
      );
      if (factsAvailable && !found.frozenAt) {
        ctx.reportExpectations = ctx.reportExpectations.map((e) =>
          e === found ? { ...e, frozenAt: SAMPLE_NOW } : e,
        );
      }
      const current =
        ctx.reportExpectations.find(
          (e) => e.companyId === companyId && e.eventKey === eventKey,
        ) ?? found;
      const metrics = (current.metrics as Record<string, unknown>[]).map(
        (m) => {
          const actual = confirmedActual(
            d,
            String(current.companyId),
            Number(current.fiscalYear),
            String(current.periodType),
            String(m.metricKey),
          );
          const outcome =
            actual == null
              ? "unknown"
              : evaluateExpectationOutcome(
                  String(m.comparator),
                  String(m.expectedValue),
                  actual,
                );
          return {
            metricKey: m.metricKey,
            comparator: m.comparator,
            expectedValue: m.expectedValue,
            unit: m.unit ?? null,
            actualValue: actual,
            outcome,
          };
        },
      );
      return {
        companyId: current.companyId,
        eventKey: current.eventKey,
        fiscalYear: current.fiscalYear,
        periodType: current.periodType,
        stanceMd: current.stanceMd,
        frozenAt: (current.frozenAt as string | null) ?? null,
        factsAvailable,
        resolutionNoteMd: (current.resolutionNoteMd as string | null) ?? null,
        resolvedAt: (current.resolvedAt as string | null) ?? null,
        metrics,
      };
    },
    record_expectation_resolution: (_d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const eventKey = str(input.eventKey);
      const found = ctx.reportExpectations.find(
        (e) => e.companyId === companyId && e.eventKey === eventKey,
      );
      if (!found) throw new Error("report expectation not found");
      const updated = {
        ...found,
        resolutionNoteMd: str(input.resolutionNoteMd) ?? "",
        resolvedAt: (found.resolvedAt as string | null) ?? SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      ctx.reportExpectations = ctx.reportExpectations.map((e) =>
        e === found ? updated : e,
      );
      return updated;
    },

    get_company_ir_reports_url: (d, a, ctx) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return ctx.irReportUrls.get(companyId) ?? null;
    },
    set_company_ir_reports_url: (d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? "";
      const url = str(input.url);
      if (url) ctx.irReportUrls.set(companyId, url);
      else ctx.irReportUrls.delete(companyId);
      return url;
    },
    get_company_sector: (_d, a, ctx) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const override = ctx.companySectors.get(companyId);
      if (override !== undefined) return override;
      return REGISTRY_SECTORS.get(companyId) ?? null;
    },
    set_company_sector: (_d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? "";
      const sector = str(input.sector);
      if (sector) {
        ctx.companySectors.set(companyId, sector);
        return sector;
      }
      ctx.companySectors.delete(companyId);
      return REGISTRY_SECTORS.get(companyId) ?? null;
    },
    list_company_sectors: () => {
      // Mirrors the real command: case variants fold to one entry (most
      // frequent spelling wins), case-insensitive sort.
      const variantsByFold = new Map<string, Map<string, number>>();
      for (const sector of REGISTRY_SECTORS.values()) {
        const fold = sector.toLocaleLowerCase();
        const variants = variantsByFold.get(fold) ?? new Map<string, number>();
        variants.set(sector, (variants.get(sector) ?? 0) + 1);
        variantsByFold.set(fold, variants);
      }
      return [...variantsByFold.values()]
        .map(
          (variants) =>
            [...variants.entries()].sort((a, b) => b[1] - a[1])[0][0],
        )
        .sort((a, b) =>
          a.toLocaleLowerCase().localeCompare(b.toLocaleLowerCase()),
        );
    },
    // Basic info read model (v0.53 follow-up): identity facts + sector with
    // provenance + latest shares_outstanding fact, mirroring
    // `commands::companies::compute_company_basic_info`.
    get_company_basic_info: (d, a, ctx) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const company = d.companies.find((c) => c.id === companyId);
      if (!company) throw new Error(`no tracked company for id ${companyId}`);
      const manual = ctx.companySectors.get(companyId) ?? null;
      const registry = REGISTRY_SECTORS.get(companyId) ?? null;
      const periodsById = new Map(
        d.financialPeriods
          .filter((p) => p.companyId === companyId)
          .map((p) => [p.id, p]),
      );
      // Mirror `latest_shares_outstanding` exactly (mock-fidelity, ADR 0049):
      // exclude superseded facts, then order by fiscal year, then period end
      // date (null last), then recency — never fiscal year alone.
      const supersededIds = new Set(
        d.financialFacts
          .map((f) => f.supersedesId)
          .filter((id): id is string => Boolean(id)),
      );
      const sharesFacts = d.financialFacts
        .filter(
          (f) =>
            f.companyId === companyId &&
            f.definitionId === "kpidef_shares_outstanding" &&
            periodsById.has(f.periodId) &&
            !supersededIds.has(f.id),
        )
        .sort((a2, b2) => {
          const pa = periodsById.get(a2.periodId);
          const pb = periodsById.get(b2.periodId);
          return (
            (pb?.fiscalYear ?? 0) - (pa?.fiscalYear ?? 0) ||
            (pb?.periodEndDate ?? "").localeCompare(pa?.periodEndDate ?? "") ||
            b2.createdAt.localeCompare(a2.createdAt)
          );
        });
      const latest = sharesFacts[0];
      const latestPeriod = latest
        ? periodsById.get(latest.periodId)
        : undefined;
      return {
        displayName: company.displayName,
        exchange: company.exchange,
        ticker: company.ticker,
        qualifiedTicker: company.qualifiedTicker,
        isin: company.isin ?? null,
        sector: manual ?? registry,
        sectorSource: manual ? "manual" : registry ? "registry" : null,
        sharesOutstanding: latest ? latest.valueNumeric : null,
        sharesOutstandingPeriod: latestPeriod
          ? `${latestPeriod.fiscalYear} ${latestPeriod.periodType.toUpperCase()}`
          : null,
      };
    },
    resolve_ir_report: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      // IrReportResolution is candidates-only after ADR 0084 (the AI pick is gone),
      // so there is nothing company-scoped to match on: return the single sample.
      void companyId;
      return d.irResolutions[0];
    },

    // --- Quality frameworks ---
    list_quality_frameworks: (d) => d.qualityFrameworks,
    get_quality_framework: (d, a) => {
      const id = str(unwrap(a).id);
      return (
        d.qualityFrameworks.find((f) => f.id === id) ?? d.qualityFrameworks[0]
      );
    },
    create_quality_framework: (d, a, ctx) => {
      const input = unwrap(a);
      const framework = {
        ...d.qualityFrameworks[0],
        id: ctx.nextId("framework"),
        name: str(input.name) ?? "Framework",
        description: str(input.description),
        origin: "user" as const,
        templateKey: null,
        criteria: [],
      };
      d.qualityFrameworks = [...d.qualityFrameworks, framework];
      return framework;
    },
    update_quality_framework: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.qualityFrameworks,
        (f) => f.id === str(input.id),
        (f) => ({
          ...f,
          name: str(input.name) ?? f.name,
          description: str(input.description),
          updatedAt: SAMPLE_NOW,
        }),
      );
      d.qualityFrameworks = next;
      return updated ?? d.qualityFrameworks[0];
    },
    delete_quality_framework: (d, a) => {
      const id = str(unwrap(a).id);
      d.qualityFrameworks = d.qualityFrameworks.filter((f) => f.id !== id);
      return undefined;
    },
    clone_framework: (d, a, ctx) => {
      const id = str(unwrap(a).id ?? unwrap(a).frameworkId);
      const source =
        d.qualityFrameworks.find((f) => f.id === id) ?? d.qualityFrameworks[0];
      const clone = {
        ...source,
        id: ctx.nextId("framework"),
        origin: "user" as const,
        clonedFrom: source.id,
      };
      d.qualityFrameworks = [...d.qualityFrameworks, clone];
      return clone;
    },
    reset_framework_to_template: (d, a) => {
      const id = str(unwrap(a).id);
      return (
        d.qualityFrameworks.find((f) => f.id === id) ?? d.qualityFrameworks[0]
      );
    },
    create_framework_criterion: (d, a, ctx) => {
      const input = unwrap(a);
      const frameworkId = str(input.frameworkId) ?? "";
      const framework = d.qualityFrameworks.find((f) => f.id === frameworkId);
      // Shared resolution + validation (F9). NOTE: this mirrors PRESENCE, not DSL
      // *semantics* — the real backend's validate_predicate rejects a malformed
      // non-empty expression that this mock accepts. The fidelity corpus only
      // replays valid inputs (the Rust replayer expects success), so a
      // malformed-predicate case is out of its scope by design; the UI gates on
      // validate_criterion_expression before create.
      const {
        kind,
        expression,
        assessmentGuidance: guidance,
      } = resolveCriterionKindFields(input);
      const criterion = {
        id: ctx.nextId("criterion"),
        frameworkId,
        ordinal: framework?.criteria.length ?? 0,
        label: str(input.label) ?? "Criterion",
        expression,
        weight: str(input.weight),
        partialBand: str(input.partialBand),
        kind,
        assessmentGuidance: kind === "qualitative" ? guidance : null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      framework?.criteria.push(criterion);
      return criterion;
    },
    update_framework_criterion: (d, a) => {
      const input = unwrap(a);
      const id = str(input.id);
      const existing = d.qualityFrameworks
        .flatMap((f) => f.criteria)
        .find((c) => c.id === id);
      // Resolve the EFFECTIVE kind/guidance/expression (input override else the
      // existing value) via the shared resolver (F9) — including a
      // qualitative→quantitative switch that must not keep the empty expression a
      // qualitative row carries (ADR 0075 T5).
      const {
        kind,
        expression,
        assessmentGuidance: guidance,
      } = resolveCriterionKindFields(input, existing);
      let updated:
        | ScenarioData["qualityFrameworks"][number]["criteria"][number]
        | undefined;
      d.qualityFrameworks = d.qualityFrameworks.map((framework) =>
        !framework.criteria.some((c) => c.id === id)
          ? framework
          : {
              ...framework,
              criteria: framework.criteria.map((c) => {
                if (c.id !== id) return c;
                updated = {
                  ...c,
                  label: str(input.label) ?? c.label,
                  expression,
                  kind,
                  assessmentGuidance: kind === "qualitative" ? guidance : null,
                  updatedAt: SAMPLE_NOW,
                };
                return updated;
              }),
            },
      );
      return updated ?? d.qualityFrameworks[0]?.criteria[0];
    },
    delete_framework_criterion: (d, a) => {
      const id = str(unwrap(a).id);
      for (const framework of d.qualityFrameworks) {
        framework.criteria = framework.criteria.filter((c) => c.id !== id);
      }
      return undefined;
    },
    evaluate_framework: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      // The backend persists a NEW immutable snapshot with a unique id on every
      // run (quality_frameworks.rs `evaluation_id` + unique_suffix). Returning a
      // seeded row verbatim would hand the panel a repeated id — duplicate React
      // keys in the history list. Mint a fresh snapshot from the newest matching
      // one and prepend it (newest-first, like ORDER BY created_at DESC).
      const template =
        d.frameworkEvaluations.find((e) => e.companyId === companyId) ??
        d.frameworkEvaluations[0];
      if (!template) return undefined;
      let serial = d.frameworkEvaluations.length + 1;
      while (
        d.frameworkEvaluations.some(
          (e) => e.id === `${template.id}_run${serial}`,
        )
      )
        serial += 1;
      const id = `${template.id}_run${serial}`;
      const minted = {
        ...template,
        id,
        results: template.results.map((result, index) => ({
          ...result,
          id: `${id}_r${index}`,
          evaluationId: id,
        })),
      };
      d.frameworkEvaluations = [minted, ...d.frameworkEvaluations];
      return minted;
    },
    list_framework_evaluations: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const frameworkId = str(input.frameworkId);
      return d.frameworkEvaluations.filter(
        (e) =>
          (!companyId || e.companyId === companyId) &&
          (!frameworkId || e.frameworkId === frameworkId),
      );
    },
    get_framework_evaluation: (d, a) => {
      const id = str(unwrap(a).id);
      return (
        d.frameworkEvaluations.find((e) => e.id === id) ??
        d.frameworkEvaluations[0]
      );
    },
    delete_framework_evaluation: (d, a) => {
      const id = str(unwrap(a).id);
      d.frameworkEvaluations = d.frameworkEvaluations.filter(
        (e) => e.id !== id,
      );
      return undefined;
    },
    validate_criterion_expression: (d, a) => {
      const expression = str(unwrap(a).expression) ?? "";
      const referencedMetricKeys = d.metricKeys
        .map((m) => m.key)
        .filter((key) => expression.includes(key));
      return { ok: true, error: null, referencedMetricKeys };
    },
    list_source_adapters: (d, a) => {
      const includeDeveloperOnly = unwrap(a).includeDeveloperOnly === true;
      return includeDeveloperOnly
        ? d.sourceAdapters
        : d.sourceAdapters.filter((s) => s.visibility !== "developer");
    },
    list_unmatched_source_items: (d, a) => {
      const adapterId = str(unwrap(a).adapterId);
      return adapterId
        ? d.unmatchedSourceItems.filter((i) => i.adapterId === adapterId)
        : d.unmatchedSourceItems;
    },
    set_source_adapter_enabled: (d, a) => {
      const input = unwrap(a);
      const id = str(input.adapterId) ?? str(input.id);
      const adapter = d.sourceAdapters.find((s) => s.id === id);
      if (!adapter || !adapter.userConfigurable) {
        throw new Error("source is not user configurable");
      }
      const enabled = input.enabled !== false;
      const healthStatus: ScenarioData["sourceAdapters"][number]["healthStatus"] =
        enabled ? "notRefreshed" : "off";
      const { next, updated } = mapReplace(
        d.sourceAdapters,
        (s) => s.id === id,
        (s) => ({ ...s, enabled, healthStatus }),
      );
      d.sourceAdapters = next;
      return updated ?? adapter;
    },
    refresh_source: (d, a) => {
      const adapterId =
        str(unwrap(a).adapterId) ?? d.sourceAdapters[0]?.id ?? "";
      return {
        adapterId,
        itemsFetched: 2,
        itemsCreated: 1,
        itemsMatched: 1,
        itemsUnmatched: 0,
        detailItemsAttempted: 0,
        detailItemsStored: 0,
        detailItemsFailed: 0,
        fetchedAt: "2026-05-30T17:30:00Z",
      };
    },
    refresh_sources: (d) => {
      // The source refresh surfaces a freshly-ingested, detail-enriched item.
      d.feedItems = [
        {
          ...d.feedItems[0],
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
        ...d.feedItems,
      ];
      return {
        adapterId: "gpw-espi-ebi",
        itemsFetched: 2,
        itemsCreated: 2,
        itemsMatched: 1,
        itemsUnmatched: 1,
        detailItemsAttempted: 1,
        detailItemsStored: 1,
        detailItemsFailed: 0,
        fetchedAt: "2026-05-30T17:30:00Z",
      };
    },
    refresh_gpw_company_registry: () => ({
      adapterId: "company-directories",
      entriesFetched: 750,
      entriesUpserted: 750,
      entriesDeactivated: 0,
      fetchedAt: "2026-05-31T12:00:00Z",
    }),
    refresh_gpw_company_registry_if_stale: (d) => ({
      adapterId: "gpw-company-registry",
      entriesFetched: d.registry.length,
      entriesUpserted: 0,
      entriesDeactivated: 0,
      fetchedAt: SAMPLE_NOW,
    }),
    backfill_company_history: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return (
        d.backfillProgress.find((p) => p.companyId === companyId) ?? {
          companyId,
          status: "completed" as const,
          pagesFetched: 1,
          itemsIngested: 0,
          documentsStored: 0,
          detailErrors: 0,
          truncated: false,
          // A completed backfill eagerly chains a history sweep (ADR 0077 §3);
          // its id matches the one get_history_sweep_progress reports, so the
          // coverage panel can poll THIS sweep specifically.
          chainedSweepId: `history_sweep:${companyId}:mock`,
          error: null,
          startedAt: SAMPLE_NOW,
          updatedAt: SAMPLE_NOW,
        }
      );
    },
    get_backfill_progress: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return d.backfillProgress.find((p) => p.companyId === companyId) ?? null;
    },

    // History sweep (ADR 0077 §3). A manual sweep enqueues extraction runs for the
    // company's fetched periodic reports. The mock synthesizes the sweep from
    // scenario docs (per-run/per-cell correctness is pinned by the Rust unit tests
    // + the dual-execution fidelity corpus). run_history_sweep returns a freshly
    // queued sweep; get_history_sweep_progress returns the completed sweep.
    run_history_sweep: (_d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return {
        id: `history_sweep:${companyId}:mock`,
        companyId,
        trigger: "manual",
        status: "queued",
        candidatesTotal: 0,
        runsEnqueued: 0,
        skippedExisting: 0,
        runsFailed: 0,
        skippedReason: null,
        enqueuedRunIds: [],
        error: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
    },
    get_history_sweep_progress: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      // Fetched canonical periodic reports for this company are the sweep's
      // candidates (the deterministic runs it would enqueue).
      const candidates = d.reportDocuments.filter(
        (doc) =>
          doc.companyId === companyId &&
          (doc.docKind === "periodic_ssf" || doc.docKind === "periodic_jsf") &&
          doc.fetchStatus === "fetched",
      ).length;
      const sweep = {
        id: `history_sweep:${companyId}:mock`,
        companyId,
        trigger: "backfill" as const,
        status: "completed" as const,
        candidatesTotal: candidates,
        runsEnqueued: candidates,
        skippedExisting: 0,
        runsFailed: 0,
        skippedReason: null,
        enqueuedRunIds: [],
        error: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      return { sweep, runsTotal: 0, runsDone: 0, runsFailed: 0 };
    },

    // Version-aware re-extraction (epic #398 Item B). Re-arms the company's
    // successful ESEF-tier runs whose stored pipeline version is stale — NOT
    // gated on automation mode (it reprocesses already-stored documents on
    // explicit request, the "Try again" posture, not new automation). The
    // mock has no stale-run population to select from (per-candidate
    // correctness is pinned by the Rust unit tests + the dual-execution
    // fidelity corpus); run_pipeline_reextraction returns a freshly queued
    // batch, get_pipeline_reextraction_progress the completed batch.
    run_pipeline_reextraction: (_d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      return {
        id: `pipeline_reextraction:${companyId}:mock`,
        companyId,
        status: "queued",
        candidatesTotal: 0,
        runsEnqueued: 0,
        runsFailed: 0,
        enqueuedRunIds: [],
        error: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
    },
    get_pipeline_reextraction_progress: (_d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const batch = {
        id: `pipeline_reextraction:${companyId}:mock`,
        companyId,
        status: "completed" as const,
        candidatesTotal: 0,
        runsEnqueued: 0,
        runsFailed: 0,
        enqueuedRunIds: [],
        error: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      return { batch, runsTotal: 0, runsDone: 0, runsFailed: 0 };
    },

    // Layer 1 raw-tagged-fact read model + promotion (ADR 0100, epic #398
    // final slice). Report documents/tagged facts are seeded only through the
    // (unmodeled) extraction pipeline, never a command, so this stays a
    // command-only state map — not a `ScenarioData` seeded collection — the
    // same idiom `irReportUrls`/`companySectors` above use. Every company
    // reads the empty state EXCEPT one hardcoded company (CD Projekt), which
    // carries a small fixed sample so the narrow-window browser spec
    // (`tests/browser/coverage-raw-capture.spec.ts`) has real content to
    // measure overflow against; per-row/per-bucket correctness is pinned by
    // the Rust unit tests and the dual-execution fidelity corpus.
    get_report_tagged_fact_coverage: (_d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      if (companyId !== "company_gpw_cdr") {
        return { rawStored: 0, projected: 0, comparative: 0, dimensional: 0, noteLevel: 0, awaitingName: 0, conflicting: 0, unparsed: 0, repeated: 0 };
      }
      // Bucket split mirrors the Rust read model (sol review finding 8):
      // `projected` covers only each filing's own period; comparatives and
      // unparsed rows have their own stated reasons.
      return { rawStored: 426, projected: 68, comparative: 12, dimensional: 228, noteLevel: 51, awaitingName: 2, conflicting: 1, unparsed: 0, repeated: 4 };
    },
    list_uncrosswalked_concepts: (_d, a, ctx) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      if (companyId !== "company_gpw_cdr") return [];
      return ctx.uncrosswalkedConcepts;
    },
    promote_uncrosswalked_concept: (_d, a, ctx) => {
      const input = unwrap(a);
      const companyId = str(input.companyId) ?? "";
      const conceptLocalName = str(input.conceptLocalName) ?? "";
      const conceptNamespaceUri = str(input.conceptNamespaceUri) ?? "";
      // Identity is (namespace, local name), mirroring the real command.
      const row = ctx.uncrosswalkedConcepts.find(
        (c) =>
          c.conceptLocalName === conceptLocalName &&
          c.conceptNamespaceUri === conceptNamespaceUri,
      );
      if (companyId !== "company_gpw_cdr" || !row) {
        throw new Error("concept_not_captured");
      }
      row.alreadyPromoted = true;
      row.promotedDefinitionId = ctx.nextId("kpidef");
      return {
        definitionId: row.promotedDefinitionId,
        metricKey: row.conceptLocalName,
        label: row.humanLabel,
        labelSource: row.labelSource,
        factsProjected: 1,
      };
    },

    // --- Settings / developer mode / diagnostics ---
    update_settings: (d, a) => {
      // The frontend sends a FLAT partial update; the backend maps the
      // AI-provider keys into the nested `aiProviders` block.
      const input = unwrap(a);
      const ai = d.settings.aiProviders;
      const pick = <T>(key: string, fallback: T): T =>
        key in input ? (input[key] as T) : fallback;
      d.settings = {
        ...d.settings,
        theme: pick("theme", d.settings.theme),
        accentPalette: pick("accentPalette", d.settings.accentPalette),
        locale: pick("locale", d.settings.locale),
        developerMode: pick("developerMode", d.settings.developerMode),
        pollIntervalSeconds: pick(
          "pollIntervalSeconds",
          d.settings.pollIntervalSeconds,
        ),
        backfillYears: pick("backfillYears", d.settings.backfillYears),
        shortcutBindings: pick("shortcutBindings", d.settings.shortcutBindings),
        pinnedCompanyIds: pick("pinnedCompanyIds", d.settings.pinnedCompanyIds),
        todayReviewedDays: pick("todayReviewedDays", d.settings.todayReviewedDays),
        mcp: {
          enabled: pick("mcpEnabled", d.settings.mcp.enabled),
          // Mirror of the backend clamp ([1024, 65535], ADR 0078).
          port: Math.min(
            65_535,
            Math.max(1024, pick("mcpPort", d.settings.mcp.port)),
          ),
          // The act-tier write gate (ADR 0088 M3), default off.
          writesEnabled: pick("mcpWritesEnabled", d.settings.mcp.writesEnabled),
          // The kpi_acquisition scope gate (ADR 0099 dec. 2), default off.
          kpiAcquisitionEnabled: pick(
            "kpiAcquisitionEnabled",
            d.settings.mcp.kpiAcquisitionEnabled,
          ),
        },
        aiProviders: {
          ...ai,
          youtubeTranscriptionProvider: pick(
            "youtubeTranscriptionProvider",
            ai.youtubeTranscriptionProvider,
          ),
          youtubeTranscriptionModel: pick(
            "youtubeTranscriptionModel",
            ai.youtubeTranscriptionModel,
          ),
          youtubeTranscriptionTimeoutSeconds: pick(
            "youtubeTranscriptionTimeoutSeconds",
            ai.youtubeTranscriptionTimeoutSeconds,
          ),
        },
      };
      return d.settings;
    },
    unlock_developer_mode: (d) => {
      d.settings.developerMode = true;
      return d.settings;
    },
    disable_developer_mode: (d) => {
      d.settings.developerMode = false;
      return d.settings;
    },
    open_logs_directory: ok,
    clear_diagnostic_events: (d) => {
      const eventsDeleted = d.diagnosticEvents.length;
      d.diagnosticEvents = [];
      return { eventsDeleted };
    },

    // --- License / credentials ---
    submit_license_key: (d, a) => {
      const licenseKey = str(unwrap(a).licenseKey) ?? "";
      d.licenseStatus = licenseKey.includes("valid-friend-license")
        ? { ...legacyLicenseStatus }
        : { ...legacyInvalidLicenseStatus };
      return d.licenseStatus;
    },
    clear_license_key: (d) => {
      d.licenseStatus = { ...legacyMissingLicenseStatus };
      return d.licenseStatus;
    },
    // --- MCP server token (ADR 0078 M1). Mirrors the Rust commands: the
    // plaintext token is returned exactly once (regenerate); status/revoke
    // report only configuration state. Deterministic pseudo-token: the shared
    // id counter hex-padded to the real 64-char width, unique per call.
    regenerate_mcp_token: (d, _a, ctx) => {
      const seq = ctx.nextId("mcp_token").replace(/\D/g, "") || "0";
      const token = Number(seq).toString(16).padStart(64, "0");
      ctx.mcpToken = token;
      // Rotation restarts the listener (ADR 0099 dec. 2) — mirror it.
      restartMcpLifecycle(d, ctx);
      return { token, status: mcpTokenStatus(ctx) };
    },
    revoke_mcp_token: (d, _a, ctx) => {
      ctx.mcpToken = null;
      // Revoking the primary stops the server (restart refuses: no token).
      restartMcpLifecycle(d, ctx);
      return mcpTokenStatus(ctx);
    },
    mcp_token_status: (_d, _a, ctx) => mcpTokenStatus(ctx),
    regenerate_kpi_acquisition_token: (d, _a, ctx) => {
      const seq = ctx.nextId("mcp_kpi_token").replace(/\D/g, "") || "0";
      const token = Number(seq).toString(16).padStart(64, "1");
      ctx.mcpKpiToken = token;
      restartMcpLifecycle(d, ctx);
      return { token, status: kpiTokenStatus(ctx) };
    },
    revoke_kpi_acquisition_token: (d, _a, ctx) => {
      ctx.mcpKpiToken = null;
      // The server keeps running; only the scope becomes unavailable.
      restartMcpLifecycle(d, ctx);
      return kpiTokenStatus(ctx);
    },
    kpi_acquisition_token_status: (_d, _a, ctx) => kpiTokenStatus(ctx),

    // --- MCP server lifecycle (ADR 0078 M3). Mirrors `set_mcp_enabled_impl`:
    // persists `mcp.enabled` AND flips the live listener; enabling without a
    // token refuses (running:false + the real backend's error wording), never
    // throws. `mcp_status` reports the live state.
    set_mcp_enabled: (d, a, ctx) => {
      const enabled = Boolean(unwrap(a).enabled);
      d.settings = { ...d.settings, mcp: { ...d.settings.mcp, enabled } };
      if (enabled && ctx.mcpToken === null) {
        ctx.mcpRunning = false;
        ctx.mcpError = "MCP server auth token is not configured";
      } else {
        ctx.mcpRunning = enabled;
        ctx.mcpError = null;
      }
      return mcpStatusOf(d, ctx);
    },
    mcp_status: (d, _a, ctx) => mcpStatusOf(d, ctx),

    // Mock fidelity: the real commands store/clear the key for EXACTLY the
    // named provider and report that provider's status — never another row.
    // A provider with no seeded status row upserts one, mirroring first-time
    // configuration.
    set_provider_api_key: (d, a) => {
      const providerId = str(unwrap(a).providerId) ?? "";
      const existing = d.credentialStatuses.find(
        (c) => c.providerId === providerId,
      );
      const credential = existing ?? {
        providerId,
        secretKind: "api_key",
        configured: false,
        storage: "keychain",
        label: providerId,
        devFallbackAvailable: false,
        error: null,
      };
      if (!existing) d.credentialStatuses.push(credential);
      credential.configured = true;
      return credential;
    },
    clear_provider_api_key: (d, a) => {
      const providerId = str(unwrap(a).providerId) ?? "";
      const credential = d.credentialStatuses.find(
        (c) => c.providerId === providerId,
      );
      if (credential) credential.configured = false;
      return (
        credential ?? {
          providerId,
          secretKind: "api_key",
          configured: false,
          storage: "keychain",
          label: providerId,
          devFallbackAvailable: false,
          error: null,
        }
      );
    },

    // --- Backups ---
    create_backup: (d) => d.backupStatus,
    restore_backup: ok,

    // --- Import / export ---
    export_research_data: () => ({
      fileName: "brawler-research-data-2026-06-05.json",
      mediaType: "application/json",
      contents: '{"schemaVersion":1}',
      summary: emptyExportSummary(),
    }),
    export_settings_data: () => ({
      fileName: "brawler-settings-2026-06-05.yaml",
      mediaType: "application/x-yaml",
      contents: "schemaVersion: 1\nsettings:\n  theme: dark\n",
      summary: emptyExportSummary(),
    }),
    // Backend parity (commands/import_export.rs write_export_file, issue
    // #106): extension whitelist enforced, absolute path required, returns the
    // final path. The mock performs no IO — the contract is path policy.
    write_export_file: (_d, a) => {
      const input = unwrap(a);
      const path = (str(input.path) ?? "").trim();
      if (!path || !path.startsWith("/")) throw new Error(`invalid_export_path: ${path}`);
      const allowed = Array.isArray(input.allowedExtensions)
        ? (input.allowedExtensions as string[])
        : [];
      if (allowed.length === 0) throw new Error("invalid_export_path: empty extension whitelist");
      const lower = path.toLowerCase();
      const hasAllowed = allowed.some((ext) => lower.endsWith(`.${ext.toLowerCase()}`));
      return hasAllowed ? path : `${path}.${str(input.defaultExtension) ?? ""}`;
    },
    preview_research_import: () => ({
      valid: true,
      summary: { ...emptyApplySummary(), companiesCreated: 1 },
      warnings: [],
      errors: [],
    }),
    preview_settings_import: () => ({
      valid: true,
      summary: { ...emptyApplySummary(), settingsUpdated: 1 },
      warnings: [],
      errors: [],
    }),
    apply_research_import: () => ({
      summary: { ...emptyApplySummary(), companiesCreated: 1 },
      warnings: [],
    }),
    apply_settings_import: () => ({
      summary: { ...emptyApplySummary(), settingsUpdated: 1 },
      warnings: [],
    }),
  };
  return handlers;
}

function emptyExportSummary() {
  return {
    companies: 0,
    watchlists: 0,
    memberships: 0,
    notebookEntries: 0,
    managementClaims: 0,
    researchQuestions: 0,
    evidenceLinks: 0,
    aiResearchBriefs: 0,
    aiResearchBriefCitations: 0,
    researchReminders: 0,
    aiResearchDigests: 0,
    aiResearchDigestCitations: 0,
    qualityFrameworks: 0,
    userMetrics: 0,
    settings: 0,
  };
}

function emptyApplySummary() {
  return {
    companiesCreated: 0,
    companiesMerged: 0,
    watchlistsCreated: 0,
    watchlistsMerged: 0,
    membershipsCreated: 0,
    notebookEntriesCreated: 0,
    notebookEntriesSkipped: 0,
    managementClaimsCreated: 0,
    managementClaimsSkipped: 0,
    researchQuestionsCreated: 0,
    researchQuestionsMerged: 0,
    evidenceLinksCreated: 0,
    evidenceLinksSkipped: 0,
    aiResearchBriefsCreated: 0,
    aiResearchBriefsSkipped: 0,
    aiResearchBriefCitationsCreated: 0,
    aiResearchBriefCitationsSkipped: 0,
    researchRemindersCreated: 0,
    researchRemindersSkipped: 0,
    aiResearchDigestsCreated: 0,
    aiResearchDigestsSkipped: 0,
    aiResearchDigestCitationsCreated: 0,
    aiResearchDigestCitationsSkipped: 0,
    qualityFrameworksCreated: 0,
    qualityFrameworksSkipped: 0,
    userMetricsCreated: 0,
    userMetricsSkipped: 0,
    settingsUpdated: 0,
  };
}

// The handler table is built once; handlers are pure functions of (data, args).
const HANDLERS = buildHandlers();

// `search` is intentionally outside the table builder so it can reference runSearch.
HANDLERS.search = (d, a) => runSearch(d, str(unwrap(a).query) ?? "");

/** Commands that the runtime intentionally treats as read/list endpoints. */
export const READ_COMMANDS: readonly string[] = Object.freeze([
  "database_status",
  "get_settings",
  "get_license_status",
  "get_local_metrics_snapshot",
  "get_diagnostic_summary",
  "list_diagnostic_events",
  "list_source_reconciliation",
  "get_log_status",
  "list_log_entries",
  "get_provider_credential_status",
  "mcp_token_status",
  "mcp_status",
  "list_available_metric_keys",
  "backup_status",
  "list_companies",
  "list_company_registry_entries",
  "list_watchlists",
  "list_watchlist_memberships",
  "list_feed_items",
  "list_company_events",
  "list_company_signals",
  "list_notebook_entries",
  "list_video_transcript_jobs",
  "list_research_questions",
  "list_research_reminders",
  "list_management_claims",
  "list_claims_to_verify",
  "list_financial_periods",
  "list_financial_facts",
  "list_kpi_definitions",
  "list_kpi_relevance",
  "list_report_documents",
  "list_report_season",
  "list_quality_frameworks",
  "list_framework_evaluations",
  "list_source_adapters",
  "list_alert_rules",
  "list_attention_events",
]);

export function createMockRuntime(
  spec: ScenarioName | ScenarioSpec = "minimal",
): MockRuntime {
  const scenario: ScenarioName = typeof spec === "string" ? spec : spec.base;
  let data = buildScenario(spec);
  let counter = 0;
  // Deliverable A seam (5be14c9): one-shot rejections queued per command name.
  const queuedFailures = new Map<string, CommandError[]>();
  // Epic #40 S1 seam (ADR 0091): persistent rejections per command name. Same
  // envelope, no consumption — the one-shot queue is checked first.
  const chaosRules = new Map<string, CommandError>();
  const ctx: RuntimeContext = {
    nextId: (prefix) => {
      counter += 1;
      return `${prefix}_sample_new_${counter}`;
    },
    irReportUrls: new Map<string, string>(),
    companySectors: new Map<string, string>(),
    decisionEntries: [],
    reportExpectations: [],
    mcpToken: null,
    mcpKpiToken: null,
    mcpRunning: false,
    mcpError: null,
    uncrosswalkedConcepts: [
      {
        conceptLocalName: "SomeStandardConceptNotYetCurated",
        conceptNamespaceUri: "http://xbrl.ifrs.org/taxonomy/2023/ifrs-full",
        companyCount: 6,
        occurrenceCount: 2,
        statementGroup: "balance",
        periodNature: "instant",
        humanLabel: "SomeStandardConceptNotYetCurated",
        labelSource: "technical",
        alreadyPromoted: false,
        promotedDefinitionId: null,
      },
      {
        conceptLocalName: "PozostaleUslugiObce",
        conceptNamespaceUri: "http://issuer.example.com/2025-12-31",
        companyCount: 1,
        occurrenceCount: 1,
        statementGroup: "income",
        periodNature: "duration",
        humanLabel: "Pozostałe usługi obce",
        labelSource: "issuer",
        alreadyPromoted: false,
        promotedDefinitionId: null,
      },
    ],
  };

  /** Raw settlement layer: the Deliverable A seam, then the handler table. */
  function rawInvoke(command: string, args: Args): Promise<unknown> {
    const queue = queuedFailures.get(command);
    if (queue && queue.length > 0) {
      const error = queue.shift()!;
      if (queue.length === 0) queuedFailures.delete(command);
      return Promise.reject(error);
    }
    const chaosRule = chaosRules.get(command);
    if (chaosRule) {
      return Promise.reject(chaosRule);
    }
    const handler = HANDLERS[command];
    if (!handler) {
      return Promise.reject(new Error(`Unhandled mock command: ${command}`));
    }
    try {
      return Promise.resolve(handler(data, args, ctx));
    } catch (error) {
      return Promise.reject(
        error instanceof Error ? error : new Error(String(error)),
      );
    }
  }

  function failNext(command: string, error: CommandError): void {
    const queue = queuedFailures.get(command) ?? [];
    queue.push(error);
    queuedFailures.set(command, queue);
  }

  function chaos(command: string, error: CommandError): void {
    chaosRules.set(command, error);
  }

  function clearChaos(): void {
    chaosRules.clear();
  }

  // Wired ONCE around the raw settlement layer (ADR 0081 Q2) — never inside
  // individual handlers. `runtime.invoke` below IS the controlled invoke.
  const controlledAsync = createControlledAsync(rawInvoke, failNext);

  const runtime: MockRuntime = {
    get data() {
      return data;
    },
    set data(next: ScenarioData) {
      data = next;
    },
    scenario,
    failNext,
    chaos,
    clearChaos,
    invoke: controlledAsync.invoke,
    controls: controlledAsync.controls,
    reset(nextScenario) {
      data = buildScenario(nextScenario ?? scenario);
      if (nextScenario) {
        runtime.scenario =
          typeof nextScenario === "string" ? nextScenario : nextScenario.base;
      }
      counter = 0;
      queuedFailures.clear();
      chaosRules.clear();
      controlledAsync.reset();
      ctx.irReportUrls.clear();
      ctx.companySectors.clear();
      ctx.decisionEntries = [];
      ctx.reportExpectations = [];
      // MCP token + lifecycle state (ADR 0078 M1/M3, ADR 0099).
      ctx.mcpToken = null;
      ctx.mcpKpiToken = null;
      ctx.mcpRunning = false;
      ctx.mcpError = null;
    },
  };
  return runtime;
}

/** The set of commands the runtime knows how to handle (for coverage tests). */
export function knownCommands(): string[] {
  return Object.keys(HANDLERS).sort();
}
