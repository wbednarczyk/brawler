import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import { renderTool, type ToolRenderContext } from "./toolRegistry";
import { TOOL_KINDS, type Tool } from "./route";
import { ResearchProvider } from "../../app/state/screenViewModels";
import type { ResearchScreenProps } from "../Research/ResearchScreen";
import { COMPANY_SPECS, makeCompany } from "../../test/scenarios/entities";
import type { FeedItem } from "../../api/types";
import { ToastProvider } from "../../ui";

// Ported from the deleted `src/screens/Cockpit/paneLandmarks.test.tsx` (issue
// #142) after the docking engine + cockpit retirement (ADR 0108). The
// strategy is unchanged (docs/ui-authoring.md § Landmarks): a landmark
// belongs to the SCREEN, never to a tool hosted in the Spółka workshop.
// Inside a tool a titled block is a `role="group"` carrying its name —
// announced, but not a landmark — so no number of same-kind tools (or a
// future multi-instance host) can collide on a duplicate accessible name.
//
// This guard walks every one of the 15 `Tool["t"]` kinds (`route.ts`) the
// same way the Spółka workshop renders them (`renderTool`, the function
// `toolPrimaries.test.tsx` also drives) and asserts its tool frame
// contributes no landmark. It reddens the moment a tool body reintroduces a
// named <section>/<aside>/role="region" — the class, not the one tool that
// had it.

const LANDMARK_SELECTOR = [
  "section[aria-label]",
  "section[aria-labelledby]",
  "[role='region']",
  "main",
  "nav",
  "aside",
].join(", ");

const companySpec = COMPANY_SPECS.find((spec) => spec.key === "cdr")!;
const company = makeCompany(companySpec);
const feedItem: FeedItem = {
  id: "feed_1",
  company: company.qualifiedTicker,
  type: "Official report",
  source: "GPW ESPI/EBI",
  time: "Today 09:12",
  title: "Report 1",
  unread: false,
  saved: false,
  sourceUrl: "https://example.test/feed/1",
  language: "pl",
  publishedAt: "2026-08-19T09:00:00Z",
  fetchedAt: "2026-08-19T09:00:00Z",
  attribution: "GPW",
  summary: "Sample feed item summary.",
  bodyText: "Body text.",
  attachments: [],
  presentationKind: "report",
};

// A minimal, inert stub — the Research tool hosts the real `ResearchScreen`,
// which reads this context; no interaction is exercised here.
const researchViewModelStub: ResearchScreenProps = {
  companies: [],
  watchlists: [],
  watchlistMemberships: [],
  mode: "company",
  selectedCompanyId: null,
  selectedWatchlistId: null,
  selectedWatchlistCompanyId: null,
  cascadeToCompanies: false,
  selectedEvidenceTypes: [],
  changedOnly: false,
  timeline: null,
  questions: [],
  selectedQuestionId: null,
  questionTitle: "",
  questionBody: "",
  questionLinks: [],
  reminders: [],
  error: null,
  loading: false,
  reviewInFlight: false,
  questionInFlight: false,
  reminderInFlight: false,
  setMode: () => {},
  setSelectedCompanyId: () => {},
  setSelectedWatchlistId: () => {},
  setSelectedWatchlistCompanyId: () => {},
  openCompanyWorkspaceById: () => {},
  setSelectedQuestionId: () => {},
  setQuestionTitle: () => {},
  setQuestionBody: () => {},
  setCascadeToCompanies: () => {},
  setChangedOnly: () => {},
  toggleEvidenceType: () => {},
  clearEvidenceTypes: () => {},
  refreshTimeline: () => {},
  markReviewed: () => {},
  createQuestion: () => {},
  updateQuestionStatus: () => {},
  deleteQuestion: () => {},
  linkEvidence: () => {},
  unlinkEvidence: () => {},
  createReminder: () => {},
  completeReminder: () => {},
  snoozeReminder: () => {},
  reopenReminder: () => {},
  deleteReminder: () => {},
  openEvidence: () => {},
  openEvidenceUrl: () => {},
  formatTimestamp: (v) => v ?? "",
};

const invokeMock = vi.mocked(invoke);

function toolFor(kind: Tool["t"]): Tool {
  switch (kind) {
    case "feedItem":
      return { t: "feedItem", feedItemId: feedItem.id };
    default:
      return { t: kind } as Tool;
  }
}

const ctx: ToolRenderContext = {
  companyId: company.id,
  company,
  feedItems: [feedItem],
  rootHighlightClaimId: null,
  onOpenTool: () => {},
  onOpenDocument: () => {},
  onOpenFeedItem: () => {},
  onCloseTool: () => {},
};

beforeEach(() => {
  invokeMock.mockReset();
  // Same minimal, proven-safe fallback `toolPrimaries.test.tsx` uses to mount
  // every one of the 15 tools without crashing — a bare `Promise.resolve([])`
  // covers every list-shaped command a tool might call; a handful of named
  // commands need a specific (object, not array) shape. `list_quality_frameworks`
  // is non-empty so the Quality tool's "Evaluate" action is reachable (needed
  // below to render its evaluation history).
  invokeMock.mockImplementation((command: unknown) => {
    if (command === "list_claims_to_verify") {
      return Promise.resolve({ due: [], overdue: [], upcoming: [] });
    }
    if (command === "get_ownership_overview") {
      return Promise.resolve({ holders: [], residuals: [], insiderHoldersCount: 0, lastUpdatedAt: null });
    }
    if (command === "get_company_basic_info") {
      return Promise.resolve({
        displayName: company.displayName,
        exchange: "GPW",
        ticker: company.qualifiedTicker.split(":")[1] ?? company.qualifiedTicker,
        qualifiedTicker: company.qualifiedTicker,
        isin: null,
        sector: null,
        sectorSource: null,
        sharesOutstanding: null,
        sharesOutstandingPeriod: null,
      });
    }
    if (command === "get_report_documents_view") {
      return Promise.resolve({ rows: [], totals: null });
    }
    if (command === "get_fundamentals_coverage") {
      return Promise.resolve({ periods: [] });
    }
    if (command === "get_price_context") {
      return Promise.resolve({
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
        ratios: {},
        history: [],
        fetchedAt: "",
        emptyReason: "no_quotes",
      });
    }
    if (command === "get_insider_overview") {
      return Promise.resolve({
        companyId: company.id,
        transactions: [],
        holdings: [],
        window90d: { buyCount: 0, sellCount: 0, netValue: null },
        window12m: { buyCount: 0, sellCount: 0, netValue: null },
      });
    }
    if (command === "get_analyst_recommendations") {
      return Promise.resolve({ companyId: company.id, entries: [] });
    }
    if (command === "get_company_context") {
      return Promise.resolve({
        companyId: company.id,
        latestPeriodFacts: null,
        upcomingEvents: [],
        notebook: { count: 0, latestAt: null },
        claimsDue: { due: 0, overdue: 0 },
      });
    }
    if (command === "list_quality_frameworks") {
      return Promise.resolve([
        {
          id: "qf_1",
          name: "Owner checklist",
          description: null,
          origin: "user",
          templateKey: null,
          clonedFrom: null,
          version: 1,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
          criteria: [],
        },
      ]);
    }
    if (command === "list_financial_periods") {
      return Promise.resolve([
        {
          id: "fp_1",
          companyId: company.id,
          fiscalYear: 2025,
          periodType: "annual",
          periodEndDate: null,
          reportEvidenceRef: null,
          createdAt: "2026-01-01T00:00:00Z",
          updatedAt: "2026-01-01T00:00:00Z",
        },
      ]);
    }
    if (command === "evaluate_framework") {
      return Promise.resolve({
        id: "eval_1",
        frameworkId: "qf_1",
        frameworkVersion: 1,
        companyId: company.id,
        periodId: null,
        passCount: 1,
        partialCount: 0,
        failCount: 0,
        unavailableCount: 0,
        engineVersion: "1",
        createdAt: "2026-01-01T00:00:00Z",
        results: [],
      });
    }
    return Promise.resolve([]);
  });
});

describe("Spółka workshop tool landmarks", () => {
  // Rendering + settling 15 tools runs a few seconds alone, well past the 5s
  // default once the suite runs at full parallelism.
  it("no workshop tool contributes a landmark of its own", { timeout: 30_000 }, async () => {
    const offenders: Record<string, string[]> = {};

    for (const kind of TOOL_KINDS) {
      // Each kind gets its own mount/unmount: unlike `toolPrimaries.test.tsx`
      // (a separate `it.each` test per kind, cleaned up automatically
      // between tests), this single test walks all 15 kinds — an un-unmounted
      // render would leave every prior tool's frame in the DOM too, and
      // `findByRole` below would match more than one.
      const { unmount } = render(
        <ToastProvider>
          <ResearchProvider value={researchViewModelStub}>
            {renderTool(toolFor(kind), ctx)}
          </ResearchProvider>
        </ToastProvider>,
      );
      const frame = await screen.findByRole("group", { name: "Workshop tool" });

      // Let every mocked fetch settle before inspecting the DOM.
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      // sol R1 finding 10 (ported from the cockpit-era guard): Quality's
      // evaluation history (a `role="group"` div, not a landmark) only
      // renders once `history.length > 0` — never exercised by this guard
      // before, so a regression there would pass vacuously. Produce one real
      // evaluation through the actual "Evaluate" action before checking this
      // tool for landmarks.
      if (kind === "jakosc") {
        const evaluateButton = screen.queryByRole("button", { name: "Evaluate" });
        if (evaluateButton) {
          await act(async () => {
            evaluateButton.click();
            await new Promise((resolve) => setTimeout(resolve, 0));
          });
          await screen.findByText("Evaluation history");
        }
      }

      const landmarks = Array.from(frame.querySelectorAll(LANDMARK_SELECTOR)).map(
        (el) =>
          el.getAttribute("aria-label") ??
          el.getAttribute("aria-labelledby") ??
          el.tagName.toLowerCase(),
      );
      if (landmarks.length > 0) {
        offenders[kind] = [...new Set(landmarks)];
      }
      unmount();
    }

    expect(
      offenders,
      'A Spółka workshop tool must not add a landmark: two tools of the same kind (or a future multi-instance host) would collide on the same accessible name. Use role="group" + aria-label instead.',
    ).toEqual({});
  });
});
