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
import { SAMPLE_NOW } from "./entities";
import {
  legacyInvalidLicenseStatus,
  legacyLicenseStatus,
  legacyMissingLicenseStatus,
} from "./legacyMinimal";
import { buildScenario, type ScenarioData, type ScenarioName } from "./scenarios";
import type { ResearchEvidenceInput } from "../../api/researchTypes";

/** Narrow alias — the router treats every payload structurally. */
type Args = Record<string, unknown> | undefined;
type Handler = (data: ScenarioData, args: Args, ctx: RuntimeContext) => unknown;

interface RuntimeContext {
  nextId(prefix: string): string;
  /** Per-company IR report URLs (command-only state, not a seeded collection). */
  irReportUrls: Map<string, string>;
}

export interface MockRuntime {
  /** The live store. Tests may read/seed it directly. */
  data: ScenarioData;
  /** Route one command. Resolves with the command's contract return shape. */
  invoke(command: string, args?: Args): Promise<unknown>;
  /** Replace the store with a fresh scenario (per-test isolation). */
  reset(scenario?: ScenarioName): void;
  /** The scenario the store was last (re)built from. */
  scenario: ScenarioName;
}

/** `{ input: X }` → X, `{ companyId }` → the object, `undefined` → {}. */
function unwrap(args: Args): Record<string, unknown> {
  if (args && typeof args === "object" && "input" in args && args.input && typeof args.input === "object") {
    return args.input as Record<string, unknown>;
  }
  return (args ?? {}) as Record<string, unknown>;
}

function str(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

/**
 * Replace the first entity matching `match` with `patch(entity)`, returning a
 * NEW array. Store mutations MUST go through this (or an equivalent reassign) so
 * a UI read sees a changed reference and re-renders — an in-place field mutation
 * keeps the same array reference and React bails. Enforced by runtime.test.ts
 * "re-render safety"; see docs/testing.md → "mock runtime conventions".
 */
function mapReplace<T>(items: T[], match: (item: T) => boolean, patch: (item: T) => T): { next: T[]; updated: T | undefined } {
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
    const companyMatches = !input.companyId || item.companyId === input.companyId;
    const watchlistMatches = !input.watchlistId || watchlistCompanyIds.includes(item.companyId);
    const typeMatches = selectedTypes.size === 0 || selectedTypes.has(item.evidenceType);
    const changedSinceReview = input.watchlistId
      ? item.reviewState.changedSinceWatchlistReview
      : item.reviewState.changedSinceCompanyReview;
    const changedMatches = !input.changedSinceReviewOnly || changedSinceReview;
    return companyMatches && watchlistMatches && typeMatches && changedMatches;
  });
  const memberCompanyIds = input.watchlistId ? watchlistCompanyIds : [];
  const companySummaries = memberCompanyIds.map((companyId) => {
    const companyItems = filteredItems.filter((item) => item.companyId === companyId);
    return {
      companyId,
      total: companyItems.length,
      changedSinceReview: companyItems.filter((item) => item.reviewState.changedSinceWatchlistReview).length,
      lastReviewedAt: null,
    };
  });
  return {
    items: filteredItems.slice(0, input.limit ?? 100),
    summary: {
      total: filteredItems.length,
      changedSinceReview: filteredItems.filter((item) =>
        input.watchlistId ? item.reviewState.changedSinceWatchlistReview : item.reviewState.changedSinceCompanyReview,
      ).length,
      lastReviewedAt: data.researchReviewCheckpoints[0]?.reviewedAt ?? null,
      memberCompanyCount: memberCompanyIds.length,
      companiesWithChangedEvidence: companySummaries.filter((summary) => summary.changedSinceReview > 0).length,
      companySummaries,
    },
  };
}

/** Locate a KPI extraction proposal (and its job) by id across all jobs. */
function findKpiProposal(data: ScenarioData, proposalId: string | null) {
  if (!proposalId) return null;
  for (const job of data.kpiExtractionJobs) {
    const proposal = job.proposals.find((p) => p.id === proposalId);
    if (proposal) return { job, proposal };
  }
  return null;
}

/** The first feed-item evidence in a scope, used to ground generated citations. */
function scopeFeedEvidence(data: ScenarioData, scopeId: string) {
  return (
    data.researchEvidence.find((e) => e.companyId === scopeId && e.evidenceType === "feed_item") ??
    data.researchEvidence.find((e) => e.evidenceType === "feed_item") ??
    null
  );
}

// ---------------------------------------------------------------------------
// Search (ported intent of ADR 0032 FTS over the in-memory store)
// ---------------------------------------------------------------------------

function runSearch(data: ScenarioData, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return { groups: [] };
  const groups: { contentType: string; matches: unknown[] }[] = [];
  const push = (contentType: string, sourceId: string, companyId: string | null, title: string) => {
    let group = groups.find((g) => g.contentType === contentType);
    if (!group) {
      group = { contentType, matches: [] };
      groups.push(group);
    }
    group.matches.push({ contentType, sourceId, companyId, parentId: null, title, snippet: title, score: 1 });
  };
  for (const c of data.companies) {
    if (c.displayName.toLowerCase().includes(needle) || c.ticker.toLowerCase().includes(needle)) {
      push("company", c.id, c.id, c.displayName);
    }
  }
  for (const f of data.feedItems) {
    if (f.title.toLowerCase().includes(needle)) push("feed_item", f.id, null, f.title);
  }
  for (const n of data.notebookEntries) {
    if (n.title.toLowerCase().includes(needle)) push("notebook_entry", n.id, n.companyId, n.title);
  }
  return { groups };
}

type FrameworkCriterion = ScenarioData["qualityFrameworks"][number]["criteria"][number];

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
): { kind: "qualitative" | "quantitative"; expression: string; assessmentGuidance: string } {
  const kind = str(input.kind) === "qualitative"
    ? ("qualitative" as const)
    : str(input.kind) === "quantitative"
      ? ("quantitative" as const)
      : (existing?.kind ?? "quantitative");
  const assessmentGuidance = kind === "qualitative"
    ? (str(input.assessmentGuidance) ?? existing?.assessmentGuidance ?? "").trim()
    : "";
  const expression = kind === "qualitative"
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
    get_log_status: (d) => d.logStatus,
    list_log_entries: (d) => d.logEntries,
    get_embedding_model_status: (d) => d.embeddingModelStatus,
    list_ai_provider_catalog: (d) => d.providerCatalog,
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
            (displayName && displayName.length >= 3 && entry.displayName.toUpperCase().includes(displayName)),
        )
        .sort((left, right) => {
          const leftPreferred = left.exchange.toUpperCase() === exchange ? 0 : 1;
          const rightPreferred = right.exchange.toUpperCase() === exchange ? 0 : 1;
          return leftPreferred - rightPreferred || left.qualifiedTicker.localeCompare(right.qualifiedTicker);
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
        entry.qualifiedTicker === company.qualifiedTicker ? { ...entry, tracked: true } : entry,
      );
      void ctx;
      return company;
    },
    delete_company: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      d.companies = d.companies.filter((c) => c.id !== companyId);
      d.watchlistMemberships = d.watchlistMemberships.filter((m) => m.companyId !== companyId);
      return undefined;
    },

    // --- Cockpit layouts (ADR 0053) ---
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
    rename_cockpit_layout: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.cockpitLayouts,
        (l) => l.id === str(input.id),
        (l) => ({ ...l, name: str(input.name) ?? l.name, updatedAt: SAMPLE_NOW }),
      );
      d.cockpitLayouts = next;
      return updated ?? d.cockpitLayouts[0];
    },
    delete_cockpit_layout: (d, a) => {
      const layoutId = str(unwrap(a).layoutId);
      d.cockpitLayouts = d.cockpitLayouts.filter((l) => l.id !== layoutId);
      return undefined;
    },

    // --- Autopilot (autonomous report pipeline, ADR 0055) ---
    get_company_autopilot: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const mode = d.autopilotModes.find((m) => m.companyId === companyId)?.mode ?? "off";
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
      const companyIds = Array.isArray(input.companyIds) ? (input.companyIds as string[]) : [];
      const mode = str(input.mode) ?? "off";
      const ids = new Set(companyIds);
      const kept = d.autopilotModes.filter((entry) => !ids.has(entry.companyId));
      d.autopilotModes = [...kept, ...companyIds.map((companyId) => ({ companyId, mode }))];
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
      d.financialFacts = d.financialFacts.filter((f) => !producedFactIds.includes(f.id));
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
        .filter((adapter) => adapter.enabled && adapter.sourceType !== "company_registry")
        .forEach((adapter, index) => {
          sourceNextDueMs[adapter.id] = now + pollMs + (5 + index * 10) * 1000;
        });
      const registry = d.sourceAdapters.find(
        (adapter) => adapter.enabled && adapter.sourceType === "company_registry",
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
      return watchlistId ? d.watchlistMemberships.filter((m) => m.watchlistId === watchlistId) : d.watchlistMemberships;
    },
    create_watchlist: (d, a) => {
      const input = unwrap(a);
      const name = str(input.name) ?? "Watchlist";
      const watchlist = {
        id: `watchlist_${name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "")}`,
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
        (w) => ({ ...w, name: str(input.name) ?? w.name, description: str(input.description) }),
      );
      d.watchlists = next;
      return updated ?? d.watchlists[0];
    },
    delete_watchlist: (d, a) => {
      const watchlistId = str(unwrap(a).watchlistId);
      d.watchlists = d.watchlists.filter((w) => w.id !== watchlistId);
      d.watchlistMemberships = d.watchlistMemberships.filter((m) => m.watchlistId !== watchlistId);
      return undefined;
    },
    add_company_to_watchlist: (d, a) => {
      const input = unwrap(a);
      const watchlistId = str(input.watchlistId) ?? "";
      const companyId = str(input.companyId) ?? "";
      const watchlist = d.watchlists.find((w) => w.id === watchlistId);
      if (watchlist && !d.watchlistMemberships.some((m) => m.watchlistId === watchlistId && m.companyId === companyId)) {
        d.watchlistMemberships = [...d.watchlistMemberships, { watchlistId, watchlistName: watchlist.name, companyId }];
        d.watchlists = d.watchlists.map((w) => (w.id === watchlistId ? { ...w, companyCount: w.companyCount + 1 } : w));
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
      if (watchlist && d.watchlistMemberships.length < before) watchlist.companyCount = Math.max(0, watchlist.companyCount - 1);
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
    delete_unsaved_feed_items: (d) => {
      const before = d.feedItems.length;
      d.feedItems = d.feedItems.filter((f) => f.saved);
      return { itemsDeleted: before - d.feedItems.length, deletedAt: SAMPLE_NOW };
    },
    prune_old_feed_items: (d, a) => {
      const retentionDays = Number(unwrap(a).retentionDays ?? 30);
      return { retentionDays, itemsDeleted: 0, prunedAt: SAMPLE_NOW };
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
        ? d.watchlistMemberships.filter((m) => m.watchlistId === watchlistId).map((m) => m.companyId)
        : [];
      return d.events.filter((event) => {
        const companyMatches = !companyId || event.companyId === companyId;
        const typeMatches = !eventType || event.eventType === eventType;
        const statusMatches = !status || event.status === status;
        const dateFromMatches = !dateFrom || event.eventDate >= dateFrom;
        const dateToMatches = !dateTo || event.eventDate <= dateTo;
        const watchlistMatches = !watchlistId || watchlistCompanyIds.includes(event.companyId);
        return companyMatches && typeMatches && statusMatches && dateFromMatches && dateToMatches && watchlistMatches;
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
        (s) => (!companyId || s.companyId === companyId) && (!status || s.status === status),
      );
    },
    confirm_company_signal: (d, a) => {
      const id = str(unwrap(a).id);
      const { next, updated } = mapReplace(d.signals, (s) => s.id === id, (s) => ({ ...s, status: "confirmed" as const }));
      d.signals = next;
      return updated ?? d.signals[0];
    },
    reject_company_signal: (d, a) => {
      const id = str(unwrap(a).id);
      d.signals = d.signals.filter((s) => s.id !== id);
      return undefined;
    },
    confirm_derived_event: ok,
    run_ai_signal_classification: () => ({ enabled: true, examined: 2, proposed: 1, skipped: 0 }),
    run_ai_event_derivation: () => ({ enabled: true, examined: 2, derived: 1, skipped: 0 }),

    // --- AI analysis ---
    list_ai_analysis: (d, a) => {
      const feedItemId = str(unwrap(a).feedItemId);
      return d.aiAnalysisJobs.filter((j) => !feedItemId || j.feedItemId === feedItemId);
    },
    start_ai_analysis: (d, a, ctx) => {
      const input = unwrap(a);
      const feedItemId = str(input.feedItemId) ?? "";
      const item = d.feedItems.find((f) => f.id === feedItemId) ?? d.feedItems[0];
      const provider = d.settings.aiProviders.generalAnalysisProvider ?? "provider_gemini";
      const model = d.settings.aiProviders.generalAnalysisModel;
      const job = {
        id: ctx.nextId("ai_job"),
        feedItemId,
        promptPresetId: str(input.promptPresetId) ?? "default_summary",
        customQuestion: str(input.customQuestion),
        providerId: provider,
        model,
        promptVersion: "analysis_v1",
        status: "succeeded" as const,
        errorCode: null,
        error: null,
        createdAt: SAMPLE_NOW,
        startedAt: SAMPLE_NOW,
        finishedAt: SAMPLE_NOW,
        result: {
          id: `ai_result_${feedItemId}`,
          aiAnalysisJobId: null,
          feedItemId,
          providerId: provider,
          model,
          promptVersion: "analysis_v1",
          summary: `AI summary for ${item?.title ?? feedItemId}`,
          significance: "medium" as const,
          reasoning: "Grounded in the selected feed item summary and source metadata.",
          language: item?.language ?? "en",
          tags: ["analysis", "feed"],
          sourceReferences: [
            { id: `ai_source_${feedItemId}`, sourceUrl: item?.sourceUrl ?? null, label: item?.source ?? "Source", createdAt: SAMPLE_NOW },
          ],
          createdAt: SAMPLE_NOW,
        },
      };
      d.aiAnalysisJobs = [job, ...d.aiAnalysisJobs];
      return job;
    },
    retry_ai_analysis: (d, a) => {
      const jobId = str(unwrap(a).jobId);
      const job = d.aiAnalysisJobs.find((j) => j.id === jobId);
      if (job) job.status = "succeeded";
      return job ?? d.aiAnalysisJobs[0];
    },

    // --- Notebooks ---
    list_notebook_entries: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId ? d.notebookEntries.filter((n) => n.companyId === companyId) : d.notebookEntries;
    },
    create_notebook_entry: (d, a, ctx) => {
      const input = unwrap(a);
      const rawOrigins = Array.isArray(input.origins) ? (input.origins as Record<string, unknown>[]) : [];
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
    create_note_from_transcript_selection: (d, a, ctx) => handlers.create_notebook_entry(d, a, ctx),
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
      return companyId ? d.transcriptJobs.filter((j) => j.companyId === companyId) : d.transcriptJobs;
    },
    list_transcript_segments: (d, a) => {
      const jobId = str(unwrap(a).transcriptJobId);
      return d.transcriptSegments.filter((s) => !jobId || s.transcriptJobId === jobId);
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
        updated = { ...job, status: "completed", startedAt: SAMPLE_NOW, finishedAt: SAMPLE_NOW };
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
        updated = { ...job, sourceLabel: sourceLabel !== null ? sourceLabel : job.sourceLabel };
        return updated;
      });
      return updated ?? d.transcriptJobs[0];
    },
    delete_video_transcript_job: (d, a) => {
      const jobId = str(unwrap(a).jobId);
      d.transcriptJobs = d.transcriptJobs.filter((j) => j.id !== jobId);
      d.transcriptSegments = d.transcriptSegments.filter((s) => s.transcriptJobId !== jobId);
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
    list_research_evidence: (d, a) => buildTimeline(d, unwrap(a) as ResearchEvidenceInput),
    list_company_timeline: (d, a) => buildTimeline(d, { companyId: str(unwrap(a).companyId) } as ResearchEvidenceInput),
    list_watchlist_timeline: (d, a) =>
      buildTimeline(d, { watchlistId: str(unwrap(a).watchlistId) } as ResearchEvidenceInput),
    mark_research_scope_reviewed: (d, a, ctx) => {
      const input = unwrap(a);
      const scopeType = (str(input.scopeType) ?? "company") as "company" | "watchlist";
      const scopeId = str(input.scopeId) ?? "";
      const existing = d.researchReviewCheckpoints.find((c) => c.scopeType === scopeType && c.scopeId === scopeId);
      if (!existing) {
        const checkpoint = {
          id: ctx.nextId("checkpoint"),
          scopeType,
          scopeId,
          reviewedAt: SAMPLE_NOW,
          createdAt: SAMPLE_NOW,
          updatedAt: SAMPLE_NOW,
        };
        d.researchReviewCheckpoints = [...d.researchReviewCheckpoints, checkpoint];
        return checkpoint;
      }
      const { next, updated } = mapReplace(
        d.researchReviewCheckpoints,
        (c) => c.scopeType === scopeType && c.scopeId === scopeId,
        (c) => ({ ...c, reviewedAt: str(input.reviewedAt) ?? SAMPLE_NOW, updatedAt: SAMPLE_NOW }),
      );
      d.researchReviewCheckpoints = next;
      return updated ?? existing;
    },
    list_research_review_state: (d, a) => {
      const input = unwrap(a);
      return (
        d.researchReviewCheckpoints.find(
          (c) => c.scopeType === str(input.scopeType) && c.scopeId === str(input.scopeId),
        ) ?? null
      );
    },
    list_research_questions: (d, a) => {
      const input = unwrap(a);
      const scopeId = str(input.scopeId);
      const status = str(input.status);
      return d.researchQuestions.filter(
        (q) => (!scopeId || q.scopeId === scopeId) && (!status || q.status === status),
      );
    },
    create_research_question: (d, a) => {
      const input = unwrap(a);
      const scopeType = (str(input.scopeType) ?? "company") as "company" | "watchlist";
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
          reviewState: { changedSinceCompanyReview: true, changedSinceWatchlistReview: true },
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
          status: status === "open" || status === "answered" || status === "closed" ? status : q.status,
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
      return d.evidenceLinks.filter((l) => !endpointId || l.fromId === endpointId || l.toId === endpointId);
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
      const link = { id: ctx.nextId("evidence_link"), fromType, fromId, toType, toId, relationType, createdAt: SAMPLE_NOW };
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
        (r) => (!scopeId || r.scopeId === scopeId) && (!status || r.status === status),
      );
    },
    create_research_reminder: (d, a, ctx) => {
      const input = unwrap(a);
      const reminder = {
        id: ctx.nextId("reminder"),
        scopeType: (str(input.scopeType) ?? "company") as "company" | "watchlist",
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
          status: status === "open" || status === "completed" || status === "dismissed" ? status : r.status,
          dueAt: str(input.dueAt) !== null ? str(input.dueAt) : r.dueAt,
          snoozedUntil: str(input.snoozedUntil) !== null ? str(input.snoozedUntil) : r.snoozedUntil,
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
    list_research_briefs: (d, a) => {
      const input = unwrap(a);
      const scopeId = str(input.scopeId);
      return d.researchBriefJobs.filter((j) => !scopeId || j.scopeId === scopeId);
    },
    start_research_brief: (d, a, ctx) => {
      const input = unwrap(a);
      const scopeType = (str(input.scopeType) ?? "company") as "company" | "watchlist";
      const scopeId = str(input.scopeId) ?? "";
      const feed = scopeFeedEvidence(d, scopeId);
      const id = ctx.nextId("research_brief_job");
      const briefId = ctx.nextId("research_brief");
      const job = {
        id,
        scopeType,
        scopeId,
        providerId: "test_sample",
        model: "test-sample-analysis-v1",
        promptVersion: "m30.research_brief.v1",
        evidenceCollectorVersion: "m30.collector.v1",
        rendererVersion: "m30.renderer.v1",
        status: "succeeded" as const,
        errorCode: null,
        error: null,
        createdAt: SAMPLE_NOW,
        startedAt: SAMPLE_NOW,
        finishedAt: SAMPLE_NOW,
        brief: {
          id: briefId,
          jobId: id,
          scopeType,
          scopeId,
          providerId: "test_sample",
          model: "test-sample-analysis-v1",
          promptVersion: "m30.research_brief.v1",
          evidenceCollectorVersion: "m30.collector.v1",
          rendererVersion: "m30.renderer.v1",
          title: "Generated research brief",
          summary: "Source-grounded brief summary.",
          contentMarkdown: "## What changed\n\nReview cited evidence together. [E1]",
          language: "en",
          generatedAt: SAMPLE_NOW,
          createdAt: SAMPLE_NOW,
          citations: feed
            ? [
                {
                  id: `${briefId}_citation_1`,
                  briefId,
                  citationKey: "E1",
                  evidenceType: "feed_item" as const,
                  evidenceId: feed.sourceId,
                  label: feed.title,
                  snippet: feed.summary ?? null,
                  createdAt: SAMPLE_NOW,
                },
              ]
            : [],
        },
      };
      d.researchBriefJobs = [job, ...d.researchBriefJobs];
      return job;
    },
    list_research_digests: (d, a) => {
      const input = unwrap(a);
      const scopeId = str(input.scopeId);
      return d.researchDigestJobs.filter((j) => !scopeId || j.scopeId === scopeId);
    },
    start_research_digest: (d, a, ctx) => {
      const input = unwrap(a);
      const scopeType = (str(input.scopeType) ?? "company") as "company" | "watchlist";
      const scopeId = str(input.scopeId) ?? "";
      const feed = scopeFeedEvidence(d, scopeId);
      const id = ctx.nextId("research_digest_job");
      const digestId = ctx.nextId("research_digest");
      const job = {
        id,
        scopeType,
        scopeId,
        providerId: "test_sample",
        model: "test-sample-analysis-v1",
        promptVersion: "m31.research_digest.v1",
        evidenceCollectorVersion: "m31.collector.v1",
        rendererVersion: "m31.renderer.v1",
        status: "succeeded" as const,
        errorCode: null,
        error: null,
        createdAt: SAMPLE_NOW,
        startedAt: SAMPLE_NOW,
        finishedAt: SAMPLE_NOW,
        digest: {
          id: digestId,
          jobId: id,
          scopeType,
          scopeId,
          providerId: "test_sample",
          model: "test-sample-analysis-v1",
          promptVersion: "m31.research_digest.v1",
          evidenceCollectorVersion: "m31.collector.v1",
          rendererVersion: "m31.renderer.v1",
          title: "Research digest",
          summary: "Open reminders and changed evidence to review.",
          contentMarkdown: "## Today's review\n\nStart with open reminders. [E1]",
          language: "en",
          generatedAt: SAMPLE_NOW,
          createdAt: SAMPLE_NOW,
          citations: feed
            ? [
                {
                  id: `${digestId}_citation_1`,
                  digestId,
                  citationKey: "E1",
                  evidenceType: "feed_item" as const,
                  evidenceId: feed.sourceId,
                  label: feed.title,
                  snippet: feed.summary ?? null,
                  createdAt: SAMPLE_NOW,
                },
              ]
            : [],
        },
      };
      d.researchDigestJobs = [job, ...d.researchDigestJobs];
      return job;
    },

    // --- Management claims ---
    list_management_claims: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId ? d.managementClaims.filter((c) => c.companyId === companyId) : d.managementClaims;
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
        (c) => ({ ...c, statement: str(input.statement) ?? c.statement, updatedAt: SAMPLE_NOW }),
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
        status === "pending" || status === "delivered" || status === "partially_delivered" || status === "missed" || status === "revised";
      const { next, updated } = mapReplace(
        d.managementClaims,
        (c) => c.id === str(input.claimId),
        (c) => ({ ...c, status: validStatus ? status : c.status, verifyingFactId: str(input.verifyingFactId), updatedAt: SAMPLE_NOW }),
      );
      d.managementClaims = next;
      return updated ?? d.managementClaims[0];
    },
    list_claim_extraction: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId ? d.claimExtractionJobs.filter((j) => j.companyId === companyId) : d.claimExtractionJobs;
    },
    start_claim_extraction: (d, a, ctx) => {
      const job = { ...d.claimExtractionJobs[0] };
      job.id = ctx.nextId("claim_job");
      job.status = "succeeded";
      d.claimExtractionJobs = [...d.claimExtractionJobs, job];
      void a;
      return job;
    },
    retry_claim_extraction: (d, a) => {
      const jobId = str(unwrap(a).jobId);
      const job = d.claimExtractionJobs.find((j) => j.id === jobId);
      if (job) job.status = "succeeded";
      return job ?? d.claimExtractionJobs[0];
    },
    confirm_claim_proposal: (d) => d.managementClaims[0],
    reject_claim_proposal: (d, a) => {
      const proposalId = str(unwrap(a).proposalId);
      const job = d.claimExtractionJobs.find((j) => j.proposals.some((p) => p.id === proposalId));
      if (job) {
        const proposal = job.proposals.find((p) => p.id === proposalId);
        if (proposal) proposal.status = "rejected";
      }
      return job ?? d.claimExtractionJobs[0];
    },

    // --- Fundamentals: financials ---
    list_financial_periods: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId ? d.financialPeriods.filter((p) => p.companyId === companyId) : d.financialPeriods;
    },
    create_financial_period: (d, a, ctx) => {
      const base = { ...d.financialPeriods[0] };
      const input = unwrap(a);
      base.id = ctx.nextId("period");
      base.companyId = str(input.companyId) ?? base.companyId;
      if (typeof input.fiscalYear === "number") base.fiscalYear = input.fiscalYear;
      d.financialPeriods = [...d.financialPeriods, base];
      return base;
    },
    update_financial_period: (d, a) => {
      const input = unwrap(a);
      const period = d.financialPeriods.find((p) => p.id === str(input.id));
      if (period && typeof input.fiscalYear === "number") period.fiscalYear = input.fiscalYear;
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
        (f) => (!companyId || f.companyId === companyId) && (!periodId || f.periodId === periodId),
      );
    },
    create_financial_fact: (d, a, ctx) => {
      const base = { ...d.financialFacts[0] };
      const input = unwrap(a);
      base.id = ctx.nextId("fact");
      base.companyId = str(input.companyId) ?? base.companyId;
      base.valueNumeric = str(input.valueNumeric) ?? base.valueNumeric;
      d.financialFacts = [...d.financialFacts, base];
      return base;
    },
    update_financial_fact: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.financialFacts,
        (f) => f.id === str(input.id),
        (f) => ({ ...f, valueNumeric: str(input.valueNumeric) ?? f.valueNumeric, updatedAt: SAMPLE_NOW }),
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
      return companyId ? d.kpiRelevance.filter((r) => r.companyId === companyId) : d.kpiRelevance;
    },
    create_kpi_relevance: (d, a, ctx) => {
      const base = { ...d.kpiRelevance[0] };
      const input = unwrap(a);
      base.id = ctx.nextId("kpi_rel");
      base.companyId = str(input.companyId) ?? base.companyId;
      d.kpiRelevance = [...d.kpiRelevance, base];
      return base;
    },
    update_kpi_relevance: (d, a) => {
      const input = unwrap(a);
      const { next, updated } = mapReplace(
        d.kpiRelevance,
        (r) => r.id === str(input.id),
        (r) => ({ ...r, status: str(input.status) ?? r.status, updatedAt: SAMPLE_NOW }),
      );
      d.kpiRelevance = next;
      return updated ?? d.kpiRelevance[0];
    },
    delete_kpi_relevance: (d, a) => {
      const id = str(unwrap(a).id);
      d.kpiRelevance = d.kpiRelevance.filter((r) => r.id !== id);
      return undefined;
    },
    list_kpi_extraction: (d, a) => {
      const input = unwrap(a);
      const reportDocumentId = str(input.reportDocumentId);
      const companyId = str(input.companyId);
      if (reportDocumentId) {
        const matches = d.kpiExtractionJobs.filter((j) => j.reportDocumentId === reportDocumentId);
        // Fall back to any extraction job so the review UI always has proposals
        // even when the report id was minted by a fresh capture this session.
        return matches.length > 0 ? matches : d.kpiExtractionJobs;
      }
      if (companyId) return d.kpiExtractionJobs.filter((j) => j.companyId === companyId);
      return d.kpiExtractionJobs;
    },
    // Structured-first fundamentals provenance (ADR 0061). Provenance is an
    // optional seed on the store (default none), so legacy scenarios render no
    // badges and a provenance-seeded scenario exercises the tier/validation UI.
    list_fact_provenance: (d, a) => {
      const store = (d as { factProvenance?: Array<{ factId: string }> }).factProvenance ?? [];
      const ids = new Set(((unwrap(a).factIds as string[] | undefined) ?? []).map(String));
      return store.filter((p) => ids.has(p.factId));
    },
    list_flagged_fact_provenance: (d) =>
      ((d as { factProvenance?: Array<{ validationStatus: string }> }).factProvenance ?? []).filter(
        (p) => p.validationStatus === "flagged",
      ),
    run_structured_extraction: () => ({
      acceptance: "accepted",
      tier: "esef",
      emitted: false,
      producedFactIds: [],
      skippedFactIds: [],
      divergentCount: 0,
      driftJson: null,
      tier4: null,
      tier4Proposals: 0,
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
      driftJson: null,
      tier4: null,
      tier4Proposals: 0,
    }),
    start_kpi_extraction: (d, a, ctx) => {
      // Build a fresh extraction job for the requested report, carrying the
      // seeded proposals so the review UI has something to confirm/reject.
      const reportDocumentId = str(unwrap(a).reportDocumentId) ?? d.kpiExtractionJobs[0]?.reportDocumentId ?? "";
      const template = d.kpiExtractionJobs[0];
      const jobId = ctx.nextId("kpi_job");
      const job = {
        ...template,
        id: jobId,
        reportDocumentId,
        status: "succeeded",
        // Job-scoped proposal ids so confirm/reject target THIS job, not the
        // seeded template (which carries the same base proposal ids).
        proposals: (template?.proposals ?? []).map((p) => ({ ...p, id: `${jobId}_${p.id}`, jobId, status: "pending", factId: null })),
      };
      d.kpiExtractionJobs = [...d.kpiExtractionJobs.filter((j) => j.reportDocumentId !== reportDocumentId), job];
      return job;
    },
    retry_kpi_extraction: (d, a) => {
      const jobId = str(unwrap(a).jobId);
      let updated: ScenarioData["kpiExtractionJobs"][number] | undefined;
      d.kpiExtractionJobs = d.kpiExtractionJobs.map((job) => {
        if (job.id !== jobId) return job;
        updated = { ...job, status: "succeeded" };
        return updated;
      });
      return updated ?? d.kpiExtractionJobs[0];
    },
    confirm_kpi_proposal: (d, a) => {
      const input = unwrap(a);
      const proposalId = str(input.proposalId);
      const found = findKpiProposal(d, proposalId);
      const factId = `fact_confirmed_${d.financialFacts.length}`;
      if (found) {
        // Mark the proposal confirmed (new object refs for re-render).
        d.kpiExtractionJobs = d.kpiExtractionJobs.map((job) =>
          job.id !== found.job.id
            ? job
            : { ...job, proposals: job.proposals.map((p) => (p.id === proposalId ? { ...p, status: "confirmed", factId } : p)) },
        );
      }
      const fact = {
        id: factId,
        companyId: found?.job.companyId ?? d.companies[0]?.id ?? "",
        periodId: "period_cdr_2025_q3",
        definitionId: found ? `def_${found.proposal.metricKey}` : "def_revenue",
        valueNumeric: str(input.valueNumeric) ?? found?.proposal.valueNumeric ?? "0",
        currency: "PLN",
        statementBasis: "consolidated",
        attribution: "total",
        variant: "actual",
        measureWindow: "period",
        dataQuality: "final",
        asReportedValue: found?.proposal.asReportedValue ?? null,
        asReportedScale: found?.proposal.asReportedScale ?? null,
        reportingStandard: "IFRS",
        extractionMethod: "ai_confirmed",
        confidence: found?.proposal.confidence ?? "high",
        confirmationState: "confirmed",
        supersedesId: null,
        sourceDocumentRef: found?.job.reportDocumentId ?? null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      d.financialFacts = [...d.financialFacts, fact];
      // ADR 0077 T4.4: the confirm path returns the fact plus the validation
      // status recorded on its provenance row (the retired `none` is never
      // written). The mock reports the common lone-value verdict.
      return { fact, validationStatus: "unreviewed" };
    },
    reject_kpi_proposal: (d, a) => {
      const proposalId = str(unwrap(a).proposalId);
      const found = findKpiProposal(d, proposalId);
      if (found) {
        const rejected = { ...found.proposal, status: "rejected" };
        d.kpiExtractionJobs = d.kpiExtractionJobs.map((job) =>
          job.id !== found.job.id
            ? job
            : { ...job, proposals: job.proposals.map((p) => (p.id === proposalId ? rejected : p)) },
        );
        return rejected;
      }
      return d.kpiExtractionJobs[0]?.proposals[0];
    },
    // F5 review-queue read model (ADR 0077 §4/§5, T5.3b). Every pending proposal
    // for the company joined to its job's detected period and its source document
    // (id/title/url) — an inner join, so a proposal whose document is absent is
    // dropped, mirroring the Rust query. Per-row correctness is pinned Rust-side
    // (unit tests + the dual-execution fidelity corpus).
    list_pending_kpi_proposals: (d, a) => {
      const companyId = str(unwrap(a).companyId) ?? "";
      const docById = new Map(d.reportDocuments.map((doc) => [doc.id, doc]));
      const rows: Array<Record<string, unknown>> = [];
      for (const job of d.kpiExtractionJobs) {
        if (job.companyId !== companyId) continue;
        const doc = docById.get(job.reportDocumentId);
        if (!doc) continue;
        for (const p of job.proposals) {
          if (p.status !== "pending") continue;
          rows.push({
            id: p.id,
            jobId: job.id,
            metricKey: p.metricKey,
            label: p.label,
            valueNumeric: p.valueNumeric,
            unit: p.unit ?? null,
            currency: p.currency ?? null,
            sourceSnippet: p.sourceSnippet ?? null,
            status: p.status,
            fiscalYear: job.detectedFiscalYear ?? null,
            periodType: job.detectedPeriodType ?? null,
            documentId: doc.id,
            documentTitle: doc.title ?? null,
            documentUrl: doc.url,
            createdAt: p.createdAt,
            updatedAt: p.updatedAt,
          });
        }
      }
      return rows;
    },

    // --- Report documents / season ---
    list_report_documents: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return companyId ? d.reportDocuments.filter((r) => r.companyId === companyId) : d.reportDocuments;
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
      const key = (fy: number, pt: string): Key => `${fy} ${canonicalLabel(pt)}`;
      const rows = new Map<Key, { fiscalYear: number; periodType: string }>();

      // Reports: canonical periodic document per period (ssf beats jsf).
      const reports = new Map<Key, { documentId: string; docKind: string; title: string | null; structured: boolean; fetched: boolean }>();
      for (const doc of d.reportDocuments) {
        if (doc.companyId !== companyId) continue;
        if (doc.docKind !== "periodic_ssf" && doc.docKind !== "periodic_jsf") continue;
        const period = doc.periodId ? periodById.get(doc.periodId) : undefined;
        if (!period) continue;
        const k = key(period.fiscalYear, period.periodType);
        rows.set(k, { fiscalYear: period.fiscalYear, periodType: canonicalLabel(period.periodType) });
        const existing = reports.get(k);
        if (existing && existing.docKind === "periodic_ssf" && doc.docKind === "periodic_jsf") continue;
        reports.set(k, {
          documentId: doc.id,
          docKind: doc.docKind,
          title: doc.title ?? null,
          structured: (doc.contentType ?? "").includes("xhtml") || (doc.contentType ?? "").includes("html"),
          fetched: doc.fetchStatus === "fetched",
        });
      }

      // Facts grouped by period, split by provenance validation state.
      const provenance = new Map(
        ((d as { factProvenance?: Array<{ factId: string; validationStatus: string }> }).factProvenance ?? []).map(
          (p) => [p.factId, p.validationStatus],
        ),
      );
      const facts = new Map<Key, { total: number; validated: number; unvalidated: number; flagged: number }>();
      for (const fact of d.financialFacts) {
        if (fact.companyId !== companyId) continue;
        const period = periodById.get(fact.periodId);
        if (!period) continue;
        const k = key(period.fiscalYear, period.periodType);
        rows.set(k, { fiscalYear: period.fiscalYear, periodType: canonicalLabel(period.periodType) });
        const cell = facts.get(k) ?? { total: 0, validated: 0, unvalidated: 0, flagged: 0 };
        cell.total += 1;
        const status = provenance.get(fact.id);
        if (status === "passed" || status === "witness_confirmed") cell.validated += 1;
        else if (status === "flagged") cell.flagged += 1;
        else cell.unvalidated += 1;
        facts.set(k, cell);
      }

      // Pending proposals grouped by the job's detected period.
      const pending = new Map<Key, number>();
      for (const job of d.kpiExtractionJobs) {
        if (job.companyId !== companyId) continue;
        if (job.detectedFiscalYear == null || job.detectedPeriodType == null) continue;
        const count = job.proposals.filter((p) => p.status === "pending").length;
        if (count === 0) continue;
        const k = key(job.detectedFiscalYear, job.detectedPeriodType);
        rows.set(k, { fiscalYear: job.detectedFiscalYear, periodType: canonicalLabel(job.detectedPeriodType) });
        pending.set(k, (pending.get(k) ?? 0) + count);
      }

      const periods = [...rows.entries()]
        .map(([k, { fiscalYear, periodType }]) => {
          const factCell = facts.get(k) ?? { total: 0, validated: 0, unvalidated: 0, flagged: 0 };
          return {
            fiscalYear,
            periodType,
            report: reports.get(k) ?? null,
            facts: factCell,
            review: { pendingProposals: pending.get(k) ?? 0, flaggedFacts: factCell.flagged },
            skippedBudget: false,
          };
        })
        .sort((x, y) => y.fiscalYear - x.fiscalYear || periodIndex(y.periodType) - periodIndex(x.periodType) || y.periodType.localeCompare(x.periodType));

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
        return p ? { fiscalYear: p.fiscalYear, periodType: p.periodType } : null;
      };
      const key = (fy: number, pt: string) => `${fy} ${pt}`;
      const canonicalByKey = new Map<string, string>();
      for (const doc of docs) {
        if (doc.docKind !== "periodic_ssf" && doc.docKind !== "periodic_jsf") continue;
        const period = periodOf(doc);
        if (!period) continue;
        const k = key(period.fiscalYear, period.periodType);
        const currentId = canonicalByKey.get(k);
        if (currentId == null) {
          canonicalByKey.set(k, doc.id);
          continue;
        }
        const current = docs.find((x) => x.id === currentId);
        if (current?.docKind === "periodic_jsf" && doc.docKind === "periodic_ssf") {
          canonicalByKey.set(k, doc.id);
        }
      }
      const rows = docs.map((doc) => {
        const period = periodOf(doc);
        const canonical =
          period != null && canonicalByKey.get(key(period.fiscalYear, period.periodType)) === doc.id;
        return {
          document: doc,
          fiscalYear: period?.fiscalYear ?? null,
          periodType: period?.periodType ?? null,
          canonical,
        };
      });
      return { companyId, rows };
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
        companyId: str(input.companyId) ?? d.reportDocuments[0]?.companyId ?? "",
        url: str(input.url) ?? d.reportDocuments[0]?.url ?? "",
        title: str(input.title) ?? d.reportDocuments[0]?.title ?? "Captured report",
      };
      d.reportDocuments = [...d.reportDocuments, base];
      // Contract return shape is DocumentCaptureResult, not the document itself.
      return { documentId, localPath: `${documentId}.pdf`, success: true, error: null };
    },
    list_report_season: (d) => ({
      upcoming: d.reportSeasonUpcoming,
      past: d.reportSeasonPast,
      calendarFreshness: { lastFetchedAt: SAMPLE_NOW, stale: false },
    }),
    get_pre_report_card: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      return d.preReportCards.find((c) => c.companyId === companyId) ?? d.preReportCards[0] ?? null;
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
        (p) => ({ ...p, status: "processed" as const, processedAt: SAMPLE_NOW }),
      );
      d.reportPreparations = next;
      return updated ?? d.reportPreparations[0];
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
    resolve_ir_report: (d, a) => {
      const companyId = str(unwrap(a).companyId);
      return d.irResolutions.find((r) => r.document?.companyId === companyId) ?? d.irResolutions[0];
    },

    // --- Quality frameworks ---
    list_quality_frameworks: (d) => d.qualityFrameworks,
    get_quality_framework: (d, a) => {
      const id = str(unwrap(a).id);
      return d.qualityFrameworks.find((f) => f.id === id) ?? d.qualityFrameworks[0];
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
        (f) => ({ ...f, name: str(input.name) ?? f.name, description: str(input.description), updatedAt: SAMPLE_NOW }),
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
      const source = d.qualityFrameworks.find((f) => f.id === id) ?? d.qualityFrameworks[0];
      const clone = { ...source, id: ctx.nextId("framework"), origin: "user" as const, clonedFrom: source.id };
      d.qualityFrameworks = [...d.qualityFrameworks, clone];
      return clone;
    },
    reset_framework_to_template: (d, a) => {
      const id = str(unwrap(a).id);
      return d.qualityFrameworks.find((f) => f.id === id) ?? d.qualityFrameworks[0];
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
      const { kind, expression, assessmentGuidance: guidance } = resolveCriterionKindFields(input);
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
      const existing = d.qualityFrameworks.flatMap((f) => f.criteria).find((c) => c.id === id);
      // Resolve the EFFECTIVE kind/guidance/expression (input override else the
      // existing value) via the shared resolver (F9) — including a
      // qualitative→quantitative switch that must not keep the empty expression a
      // qualitative row carries (ADR 0075 T5).
      const { kind, expression, assessmentGuidance: guidance } = resolveCriterionKindFields(input, existing);
      let updated: ScenarioData["qualityFrameworks"][number]["criteria"][number] | undefined;
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
        d.frameworkEvaluations.find((e) => e.companyId === companyId) ?? d.frameworkEvaluations[0];
      if (!template) return undefined;
      let serial = d.frameworkEvaluations.length + 1;
      while (d.frameworkEvaluations.some((e) => e.id === `${template.id}_run${serial}`)) serial += 1;
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
        (e) => (!companyId || e.companyId === companyId) && (!frameworkId || e.frameworkId === frameworkId),
      );
    },
    get_framework_evaluation: (d, a) => {
      const id = str(unwrap(a).id);
      return d.frameworkEvaluations.find((e) => e.id === id) ?? d.frameworkEvaluations[0];
    },
    delete_framework_evaluation: (d, a) => {
      const id = str(unwrap(a).id);
      d.frameworkEvaluations = d.frameworkEvaluations.filter((e) => e.id !== id);
      return undefined;
    },
    validate_criterion_expression: (d, a) => {
      const expression = str(unwrap(a).expression) ?? "";
      const referencedMetricKeys = d.metricKeys.map((m) => m.key).filter((key) => expression.includes(key));
      return { ok: true, error: null, referencedMetricKeys };
    },
    // Qualitative assessment (ADR 0075). run/rerun enqueue an async job — no
    // synchronous result to surface (progress lands via the jobs read model),
    // matching the real enqueue commands.
    run_qualitative_assessment: () => undefined,
    rerun_qualitative_criterion: () => undefined,
    get_qualitative_assessment: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const frameworkId = str(input.frameworkId);
      // Current-state read: per criterion, the most-recent agent-assessed row
      // across all snapshots (ADR 0075 Decision 5). Mirror the real backend's
      // MAX(created_at). The forward scan keeps the row with the newest createdAt;
      // on an EQUAL createdAt it must keep the LATER-scanned row, because the
      // backend (quality_frameworks.rs get_qualitative_assessment) tie-breaks on
      // created_at THEN framework_evaluations.rowid — the last-inserted snapshot
      // wins. Array order == insertion order == rowid order, so `>=` (not strict
      // `>`) lets a later array element overwrite an equal-timestamp earlier one.
      type QualRow = ScenarioData["frameworkEvaluations"][number]["results"][number];
      const latestByCriterion = new Map<string, { row: QualRow; createdAt: string }>();
      for (const evaluation of d.frameworkEvaluations) {
        if (companyId && evaluation.companyId !== companyId) continue;
        if (frameworkId && evaluation.frameworkId !== frameworkId) continue;
        for (const result of evaluation.results) {
          if (result.source !== "agent" || !result.criterionId) continue;
          const seen = latestByCriterion.get(result.criterionId);
          if (!seen || evaluation.createdAt >= seen.createdAt) {
            latestByCriterion.set(result.criterionId, { row: result, createdAt: evaluation.createdAt });
          }
        }
      }
      return [...latestByCriterion.values()]
        .map((entry) => entry.row)
        .sort((x, y) => x.ordinal - y.ordinal || (x.id < y.id ? -1 : x.id > y.id ? 1 : 0));
    },
    // Lifecycle status of the durable assessment job (P1, ADR 0075). The mock
    // does not model the job queue, so it reports the two states derivable from
    // stored data — exactly what the real backend returns for a MISSING queue
    // row: `succeeded` when an agent assessment is already stored, else `idle`.
    // The `queued`/`running`/`failed` transitions live behind the queue and are
    // covered by the Rust status tests + the panel test's direct mock.
    get_qualitative_assessment_status: (d, a) => {
      const input = unwrap(a);
      const companyId = str(input.companyId);
      const frameworkId = str(input.frameworkId);
      const hasAssessment = d.frameworkEvaluations.some(
        (evaluation) =>
          (!companyId || evaluation.companyId === companyId) &&
          (!frameworkId || evaluation.frameworkId === frameworkId) &&
          evaluation.results.some((result) => result.source === "agent" && result.criterionId),
      );
      return {
        status: hasAssessment ? "succeeded" : "idle",
        attempts: 0,
        lastError: null,
      };
    },

    // --- Sources ---
    list_source_adapters: (d, a) => {
      const includeDeveloperOnly = unwrap(a).includeDeveloperOnly === true;
      return includeDeveloperOnly ? d.sourceAdapters : d.sourceAdapters.filter((s) => s.visibility !== "developer");
    },
    list_unmatched_source_items: (d, a) => {
      const adapterId = str(unwrap(a).adapterId);
      return adapterId ? d.unmatchedSourceItems.filter((i) => i.adapterId === adapterId) : d.unmatchedSourceItems;
    },
    set_source_adapter_enabled: (d, a) => {
      const input = unwrap(a);
      const id = str(input.adapterId) ?? str(input.id);
      const adapter = d.sourceAdapters.find((s) => s.id === id);
      if (!adapter || !adapter.userConfigurable) {
        throw new Error("source is not user configurable");
      }
      const enabled = input.enabled !== false;
      const healthStatus: ScenarioData["sourceAdapters"][number]["healthStatus"] = enabled ? "notRefreshed" : "off";
      const { next, updated } = mapReplace(
        d.sourceAdapters,
        (s) => s.id === id,
        (s) => ({ ...s, enabled, healthStatus }),
      );
      d.sourceAdapters = next;
      return updated ?? adapter;
    },
    refresh_source: (d, a) => {
      const adapterId = str(unwrap(a).adapterId) ?? d.sourceAdapters[0]?.id ?? "";
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
            { id: "feed_attachment_sample_report_pdf", label: "report.pdf", url: "https://www.gpw.pl/pub/GPW/ESPI/2026/report.pdf" },
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
        aiCallsUsed: 0,
        aiCallLimit: 30,
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
        aiCallsUsed: 0,
        aiCallLimit: 30,
        error: null,
        createdAt: SAMPLE_NOW,
        updatedAt: SAMPLE_NOW,
      };
      return { sweep, runsTotal: 0, runsDone: 0, runsFailed: 0 };
    },

    // --- Embedding / similarity ---
    find_similar_content: (d, a) => {
      const input = unwrap(a);
      const sourceId = str(input.contentId) ?? str(input.sourceId);
      return {
        strategyId: d.embeddingModelStatus.activeSimilarityStrategy,
        items: d.feedItems
          .filter((f) => f.id !== sourceId)
          .slice(0, 5)
          .map((f, index) => ({ contentType: "feed_item", contentId: f.id, score: 1 - index * 0.1 })),
      };
    },
    download_embedding_model: (d) => {
      d.embeddingModelStatus.weightsState = "ready";
      return d.embeddingModelStatus;
    },
    rebuild_embedding_index: (d) => d.embeddingModelStatus,
    set_similarity_strategy: (d, a) => {
      const strategy = str(unwrap(a).strategy);
      if (strategy === "static" || strategy === "embedding") d.embeddingModelStatus.activeSimilarityStrategy = strategy;
      return d.embeddingModelStatus;
    },

    // --- Settings / developer mode / diagnostics ---
    update_settings: (d, a) => {
      // The frontend sends a FLAT partial update; the backend maps the
      // AI-provider keys into the nested `aiProviders` block.
      const input = unwrap(a);
      const ai = d.settings.aiProviders;
      const pick = <T>(key: string, fallback: T): T => (key in input ? (input[key] as T) : fallback);
      d.settings = {
        ...d.settings,
        theme: pick("theme", d.settings.theme),
        accentPalette: pick("accentPalette", d.settings.accentPalette),
        locale: pick("locale", d.settings.locale),
        developerMode: pick("developerMode", d.settings.developerMode),
        pollIntervalSeconds: pick("pollIntervalSeconds", d.settings.pollIntervalSeconds),
        backfillYears: pick("backfillYears", d.settings.backfillYears),
        historySweepAiCallLimit: pick("historySweepAiCallLimit", d.settings.historySweepAiCallLimit),
        aiAnalysisMode: pick("aiAnalysisMode", d.settings.aiAnalysisMode),
        espiAiFallbackEnabled: pick("espiAiFallbackEnabled", d.settings.espiAiFallbackEnabled),
        shortcutBindings: pick("shortcutBindings", d.settings.shortcutBindings),
        capabilityProviders: pick("capabilityProviders", d.settings.capabilityProviders),
        pinnedCompanyIds: pick("pinnedCompanyIds", d.settings.pinnedCompanyIds),
        aiProviders: {
          ...ai,
          youtubeTranscriptionProvider: pick("youtubeTranscriptionProvider", ai.youtubeTranscriptionProvider),
          youtubeTranscriptionModel: pick("youtubeTranscriptionModel", ai.youtubeTranscriptionModel),
          youtubeTranscriptionTimeoutSeconds: pick("youtubeTranscriptionTimeoutSeconds", ai.youtubeTranscriptionTimeoutSeconds),
          generalAnalysisProvider: pick("generalAnalysisProvider", ai.generalAnalysisProvider),
          generalAnalysisModel: pick("generalAnalysisModel", ai.generalAnalysisModel),
          generalAnalysisTimeoutSeconds: pick("generalAnalysisTimeoutSeconds", ai.generalAnalysisTimeoutSeconds),
          openaiCompatibleBaseUrl: pick("openaiCompatibleBaseUrl", ai.openaiCompatibleBaseUrl),
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
    set_provider_api_key: (d, a) => {
      const providerId = str(unwrap(a).providerId);
      const credential = d.credentialStatuses.find((c) => c.providerId === providerId);
      if (credential) credential.configured = true;
      return credential ?? d.credentialStatuses[0];
    },
    clear_provider_api_key: (d, a) => {
      const providerId = str(unwrap(a).providerId);
      const credential = d.credentialStatuses.find((c) => c.providerId === providerId);
      if (credential) credential.configured = false;
      return credential ?? d.credentialStatuses[0];
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
    apply_research_import: () => ({ summary: { ...emptyApplySummary(), companiesCreated: 1 }, warnings: [] }),
    apply_settings_import: () => ({ summary: { ...emptyApplySummary(), settingsUpdated: 1 }, warnings: [] }),
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
  "get_log_status",
  "list_log_entries",
  "get_embedding_model_status",
  "list_ai_provider_catalog",
  "get_provider_credential_status",
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
  "list_research_briefs",
  "list_research_digests",
  "list_management_claims",
  "list_claims_to_verify",
  "list_claim_extraction",
  "list_financial_periods",
  "list_financial_facts",
  "list_kpi_definitions",
  "list_kpi_relevance",
  "list_kpi_extraction",
  "list_report_documents",
  "list_report_season",
  "list_quality_frameworks",
  "list_framework_evaluations",
  "list_source_adapters",
]);

export function createMockRuntime(scenario: ScenarioName = "minimal"): MockRuntime {
  let data = buildScenario(scenario);
  let counter = 0;
  const ctx: RuntimeContext = {
    nextId: (prefix) => {
      counter += 1;
      return `${prefix}_sample_new_${counter}`;
    },
    irReportUrls: new Map<string, string>(),
  };

  const runtime: MockRuntime = {
    get data() {
      return data;
    },
    set data(next: ScenarioData) {
      data = next;
    },
    scenario,
    invoke(command, args) {
      const handler = HANDLERS[command];
      if (!handler) {
        return Promise.reject(new Error(`Unhandled mock command: ${command}`));
      }
      try {
        return Promise.resolve(handler(data, args, ctx));
      } catch (error) {
        return Promise.reject(error instanceof Error ? error : new Error(String(error)));
      }
    },
    reset(nextScenario) {
      data = buildScenario(nextScenario ?? scenario);
      if (nextScenario) runtime.scenario = nextScenario;
      counter = 0;
      ctx.irReportUrls.clear();
    },
  };
  return runtime;
}

/** The set of commands the runtime knows how to handle (for coverage tests). */
export function knownCommands(): string[] {
  return Object.keys(HANDLERS).sort();
}
