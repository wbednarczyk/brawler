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

// At most ONE primary action per hosted tool at a time (plan §9, sol R1
// finding 9): a parametrized sweep over all 15 `Tool` kinds, rendering each
// tool body the SAME way the Spółka workshop does (`renderTool`), the class
// this test exists to pin — not just the one tool (Claims) an earlier probe
// happened to cover.

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
  // The same minimal, proven-safe fallback `SpolkaScreen.test.tsx` uses to
  // mount every one of the 15 tools without crashing — a bare
  // `Promise.resolve([])` covers every list-shaped command a panel might
  // call; a handful of named commands need a specific (object, not array)
  // shape. Two commands get NON-EMPTY data here (list_quality_frameworks,
  // list_financial_periods) so the Quality and Fundamentals tools reach the
  // state where their SECOND form/action becomes reachable — the exact
  // state finding 9 flagged as rendering two simultaneous primaries.
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
    // Finding 9 fixtures: non-empty so both tools' second form/action
    // becomes reachable (see comment above).
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
    return Promise.resolve([]);
  });
});

const PRIMARY_SELECTOR = '[data-ui-button-variant="primary"], [data-ux-primary-action]';

// These vitest component tests never load the app's CSS (no `.css` import
// anywhere in `src/test/setup.ts`), so a container-query-gated disclosure —
// `display: none` unconditionally by default, only shown by an S/short-tier
// `@container` rule (`src/styles/claims.css` `.claims-add-toggle`, default
// rule right above the tier block) — renders in jsdom with BOTH the toggle
// and its (in a real browser, hidden) target simultaneously visible. That is
// a test-environment gap, not a production two-primary bug: exclude the one
// known toggle by class so this sweep measures what a user actually sees.
const JSDOM_CSS_GAP_TOGGLE_CLASSES = ["claims-add-toggle"];

describe("Spółka workshop tools: at most one primary action", () => {
  it.each(TOOL_KINDS.map((kind) => ({ kind })))(
    "tool $kind renders at most one primary action",
    async ({ kind }) => {
      render(
        <ToastProvider>
          <ResearchProvider value={researchViewModelStub}>
            {renderTool(toolFor(kind), ctx)}
          </ResearchProvider>
        </ToastProvider>,
      );
      const frame = await screen.findByRole("group", { name: "Workshop tool" });

      // Let every mocked fetch settle before counting: a single macrotask
      // tick drains the whole (however deep) chain of already-resolved
      // mock promises, since microtasks fully empty before a timer fires.
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      const primaries = [...frame.querySelectorAll(PRIMARY_SELECTOR)].filter(
        (el) => !JSDOM_CSS_GAP_TOGGLE_CLASSES.some((cls) => el.classList.contains(cls)),
      );

      expect(primaries.length).toBeLessThanOrEqual(1);
    },
  );
});
