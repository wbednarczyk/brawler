import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { SpolkaScreen, type SpolkaScreenProps } from "./SpolkaScreen";
import { useSpolkaToolHost } from "./ToolHost";
import { ToastProvider } from "../../ui";
import { CommandPaletteProvider } from "../../app/commandPalette";
import { ResearchProvider } from "../../app/state/screenViewModels";
import type { ResearchScreenProps } from "../Research/ResearchScreen";
import { getCompanyView } from "../../api/companyView";
import type { CompanyView } from "../../api/generated/CompanyView";
import type { FeedItem } from "../../api/types";
import type { Tool } from "./route";
import { TOOL_KINDS } from "./route";
import { COMPANY_SPECS, makeCompany } from "../../test/scenarios/entities";

vi.mock("../../api/companyView", () => ({
  getCompanyView: vi.fn(),
}));

const getCompanyViewMock = vi.mocked(getCompanyView);

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

const company = makeCompany(COMPANY_SPECS.find((spec) => spec.key === "cdr")!);

function emptyView(overrides: Partial<CompanyView> = {}): CompanyView {
  return {
    companyId: company.id,
    qualifiedTicker: company.qualifiedTicker,
    displayName: company.displayName,
    isin: company.isin ?? undefined,
    counters: {
      signals: { unacked: 0, byCategory: [] },
      claims: { open: 0, nearestDue: undefined },
      shorts: { activeSumPct: 0, largestHolder: undefined },
      events: { upcoming: 0 },
    },
    kpi: undefined,
    feed: [],
    price: undefined,
    coverage: [],
    recommendations: [],
    sectionErrors: {},
    ...overrides,
  };
}

function fullView(overrides: Partial<CompanyView> = {}): CompanyView {
  return emptyView({
    counters: {
      signals: {
        unacked: 9,
        byCategory: [
          { category: "short_spike", count: 4 },
          { category: "recommendation_change", count: 4 },
          { category: "general_meeting", count: 1 },
        ],
      },
      claims: { open: 2, nearestDue: "FY 2026" },
      shorts: { activeSumPct: 0.6, largestHolder: "Qube" },
      events: { upcoming: 0 },
    },
    kpi: {
      currency: "PLN",
      years: [2023, 2024, 2025, 2026],
      rows: [
        {
          metricKey: "revenue",
          cells: [
            { fiscalYear: 2023, valueNumeric: "1000", sourceDocumentRef: undefined },
            { fiscalYear: 2024, valueNumeric: "1200", sourceDocumentRef: undefined },
            { fiscalYear: 2025, valueNumeric: "1300", sourceDocumentRef: undefined },
            { fiscalYear: 2026, valueNumeric: "1500", sourceDocumentRef: "Raport roczny 2026 · s. 44" },
          ],
          yoyPct: 15.38,
        },
        {
          metricKey: "operating_profit",
          cells: [
            { fiscalYear: 2023, valueNumeric: "200" },
            { fiscalYear: 2024, valueNumeric: "220" },
            { fiscalYear: 2025, valueNumeric: "240" },
            { fiscalYear: 2026, valueNumeric: "260" },
          ],
          yoyPct: 8.33,
        },
        {
          metricKey: "net_profit",
          cells: [
            { fiscalYear: 2023, valueNumeric: "150" },
            { fiscalYear: 2024, valueNumeric: "160" },
            { fiscalYear: 2025, valueNumeric: "170" },
            { fiscalYear: 2026, valueNumeric: "190" },
          ],
          yoyPct: 11.76,
        },
      ],
    },
    feed: Array.from({ length: 6 }, (_, i) => ({
      feedItemId: `feed_${i}`,
      title: `Report ${i}`,
      publishedAt: `2026-08-${20 - i}T09:00:00Z`,
      read: i > 0,
      itemType: "Official report",
      sourceName: "ESPI",
      presentationKind: "report" as const,
    })),
    price: {
      candles: [
        { date: "2026-08-17", open: 250, high: 255, low: 248, close: 252 },
        { date: "2026-08-18", open: 252, high: 260, low: 251, close: 259.9 },
      ],
      lastClose: 259.9,
      asOf: "2026-08-18",
      delta1mPct: 10.8,
      deltaYtdPct: 7.5,
      currency: "PLN",
    },
    coverage: [
      {
        fiscalYear: 2025,
        periodType: "FY",
        report: { documentId: "doc_1", docKind: "periodic_ssf", title: "Raport roczny 2025", structured: true, fetched: true },
        facts: { total: 128, validated: 120, unvalidated: 8, flagged: 0 },
        review: { flaggedFacts: 0 },
        skippedBudget: false,
      },
      {
        fiscalYear: 2026,
        periodType: "Q3",
        report: null,
        facts: { total: 0, validated: 0, unvalidated: 0, flagged: 0 },
        review: { flaggedFacts: 0 },
        skippedBudget: false,
      },
    ],
    recommendations: [
      {
        firm: "Noble",
        analyst: null,
        rating: "akumuluj",
        ratingPrev: null,
        direction: "reiterate",
        targetPrice: "250",
        targetCurrency: "PLN",
        targetPrev: null,
        priceAtIssue: "240",
        publishedAt: "2026-06-18",
        reportUrl: null,
        sourceUrl: "https://example.com",
      },
    ],
    ...overrides,
  });
}

function baseProps(overrides: Partial<SpolkaScreenProps> = {}): SpolkaScreenProps {
  return {
    companyId: company.id,
    company,
    companies: [company],
    spolkaTool: overrides.spolkaTool as SpolkaScreenProps["spolkaTool"],
    feedItems: [],
    rootHighlightClaimId: null,
    onOpenDocument: vi.fn(),
    onOpenFeedItem: vi.fn(),
    onSwitchCompany: vi.fn(),
    refreshCompletionCount: 0,
    ...overrides,
  };
}

// A real, running tool-host instance (not a mock) so the dirty-guard seam
// behaves exactly as it does in the app — `renderScreen`'s overrides can be
// re-applied via `rerender` while the SAME host instance keeps its state.
function Harness(overrides: Partial<SpolkaScreenProps>) {
  const spolkaTool = useSpolkaToolHost();
  return (
    <CommandPaletteProvider appCommands={[]} text={(s) => s}>
      <ToastProvider>
        <ResearchProvider value={researchViewModelStub}>
          <SpolkaScreen {...baseProps({ ...overrides, spolkaTool })} />
        </ResearchProvider>
      </ToastProvider>
    </CommandPaletteProvider>
  );
}

function renderScreen(overrides: Partial<SpolkaScreenProps> = {}) {
  return render(<Harness {...overrides} />);
}

beforeEach(() => {
  getCompanyViewMock.mockReset();
  // Generic fallback for every command a hosted tool might call — real
  // shapes are asserted by that panel's own tests, not here; this only keeps
  // the frame from crashing while it renders.
  invokeMock.mockReset();
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
    if (command === "get_company_context") {
      return Promise.resolve({
        companyId: company.id,
        latestPeriodFacts: null,
        upcomingEvents: [],
        notebook: { count: 0, latestAt: null },
        claimsDue: { due: 0, overdue: 0 },
      });
    }
    return Promise.resolve([]);
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("SpolkaScreen", () => {
  it("renders from a single get_company_view call", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();

    expect(screen.queryAllByRole("progressbar")).toHaveLength(0);

    await screen.findByText("CD Projekt");
    expect(getCompanyViewMock).toHaveBeenCalledTimes(1);
    expect(getCompanyViewMock).toHaveBeenCalledWith(company.id);
  });

  const sections: Array<{ name: string; data: Partial<CompanyView>; empty: Partial<CompanyView>; errorKey: keyof NonNullable<CompanyView["sectionErrors"]> }> = [
    { name: "Annual KPI table", data: fullView(), empty: { kpi: undefined }, errorKey: "kpi" },
    { name: "Company feed", data: fullView(), empty: { feed: [] }, errorKey: "feed" },
    { name: "Price chart", data: fullView(), empty: { price: undefined }, errorKey: "price" },
    { name: "Report coverage", data: fullView(), empty: { coverage: [] }, errorKey: "coverage" },
    { name: "Recommendations", data: fullView(), empty: { recommendations: [] }, errorKey: "recommendations" },
  ];

  it.each(sections)("no core section ever renders blank: $name (data)", async ({ name, data }) => {
    getCompanyViewMock.mockResolvedValue(data as CompanyView);
    renderScreen();
    const group = await screen.findByRole("group", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it.each(sections)("no core section ever renders blank: $name (empty)", async ({ name, empty }) => {
    getCompanyViewMock.mockResolvedValue(emptyView(empty));
    renderScreen();
    const group = await screen.findByRole("group", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it.each(sections)("no core section ever renders blank: $name (error)", async ({ name, errorKey }) => {
    getCompanyViewMock.mockResolvedValue(
      emptyView({ sectionErrors: { [errorKey]: "unavailable" } }),
    );
    renderScreen();
    const group = await screen.findByRole("group", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it("every workshop tool opens in one click", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");

    const expectations: Array<{ label: string; tool: Tool }> = [
      { label: "Signals counter", tool: { t: "sygnaly" } },
      { label: "Claims counter", tool: { t: "tezy" } },
      { label: "Shorts counter", tool: { t: "akcjonariat" } },
      { label: "Events counter", tool: { t: "wydarzenia" } },
      // Destination buttons are nouns (owner dogfooding v0.74 item 4; ADR
      // 0104 dec. 3 amendment) — only the ⌘K palette keeps "Open X".
      { label: "Fundamentals", tool: { t: "fundamenty" } },
      { label: "Feed", tool: { t: "feed" } },
      { label: "Coverage", tool: { t: "pokrycie" } },
      { label: "Recommendations", tool: { t: "rekomendacje" } },
      { label: "Claims", tool: { t: "tezy" } },
      { label: "Notebook", tool: { t: "notatnik" } },
      { label: "Decision journal", tool: { t: "dziennik" } },
      { label: "Quality", tool: { t: "jakosc" } },
      { label: "Report diff", tool: { t: "diff" } },
      { label: "Research", tool: { t: "research" } },
      { label: "Ownership", tool: { t: "akcjonariat" } },
      { label: "Signals", tool: { t: "sygnaly" } },
      { label: "Documents", tool: { t: "dokumenty" } },
    ];

    for (const { label, tool } of expectations) {
      await user.click(screen.getByRole("button", { name: label }));
      const frame = await screen.findByRole("group", { name: "Workshop tool" });
      expect(frame.getAttribute("data-tool")).toBe(tool.t);
      await user.click(within(frame).getByRole("button", { name: "Close tool" }));
      await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    }

    // feedItem: click a feed row.
    await user.click(screen.getAllByRole("button", { name: /Report \d/ })[0]);
    const feedItemFrame = await screen.findByRole("group", { name: "Workshop tool" });
    expect(feedItemFrame.getAttribute("data-tool")).toBe("feedItem");

    // Every tool kind is reachable from this screen (15).
    const reached = new Set(expectations.map((e) => e.tool.t));
    reached.add("feedItem");
    expect([...reached].sort()).toEqual([...TOOL_KINDS].sort());
  });

  it("KPI ticket navigates to its document", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    const onOpenDocument = vi.fn();
    renderScreen({ onOpenDocument });
    await user.click(await screen.findByRole("button", { name: "Open source document" }));
    expect(onOpenDocument).toHaveBeenCalledWith("Raport roczny 2026 · s. 44");
  });

  it("kurs and coverage carry as-of dates", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    const priceGroup = await screen.findByRole("group", { name: "Price chart" });
    expect(within(priceGroup).getByText(/18\.08\.2026/)).toBeInTheDocument();
    const coverageGroup = screen.getByRole("group", { name: "Report coverage" });
    expect(within(coverageGroup).getByText(/FY ?2025/)).toBeInTheDocument();
    expect(within(coverageGroup).getByText(/Q3 ?2026/)).toBeInTheDocument();
  });

  it("whole-read error shows the error card with Refresh and refetches", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockRejectedValueOnce(new Error("boom"));
    renderScreen();
    await screen.findByText("Couldn't read this company's data.");

    getCompanyViewMock.mockResolvedValueOnce(fullView());
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(getCompanyViewMock).toHaveBeenCalledTimes(2));
    await screen.findByText("CD Projekt");
  });

  it("a late response for company A after switching to B never renders A", async () => {
    const companyB = makeCompany(COMPANY_SPECS.find((spec) => spec.key === "pkn")!);
    let resolveA: (value: CompanyView) => void = () => {};
    getCompanyViewMock.mockImplementationOnce(
      () => new Promise((resolve) => { resolveA = resolve; }),
    );
    getCompanyViewMock.mockResolvedValueOnce(fullView({ companyId: companyB.id, qualifiedTicker: companyB.qualifiedTicker, displayName: companyB.displayName }));

    const { rerender } = renderScreen();
    rerender(<Harness companyId={companyB.id} company={companyB} />);

    await screen.findByText("PKN Orlen");
    resolveA(fullView());
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.getByText("PKN Orlen")).toBeInTheDocument();
    expect(screen.queryByText("CD Projekt")).not.toBeInTheDocument();
  });

  it("no primary action at rest", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    const { container } = renderScreen();
    await screen.findByText("CD Projekt");
    expect(container.querySelectorAll('[data-ui-button-variant="primary"]')).toHaveLength(0);
  });

  it("dense: feed caps at 6 and counters 99+", async () => {
    getCompanyViewMock.mockResolvedValue(
      fullView({
        counters: {
          signals: { unacked: 140, byCategory: [] },
          claims: { open: 2, nearestDue: "FY 2026" },
          shorts: { activeSumPct: 0.6, largestHolder: "Qube" },
          events: { upcoming: 0 },
        },
      }),
    );
    renderScreen();
    await screen.findByText("CD Projekt");
    expect(screen.getByRole("button", { name: /Signals counter/ }).textContent).toContain("99+");
    const feedGroup = screen.getByRole("group", { name: "Company feed" });
    expect(within(feedGroup).getAllByRole("button", { name: /Report \d/ })).toHaveLength(6);
  });

  // Owner dogfooding v0.74, item 2: the real base carries 30+ coverage
  // periods; the core card caps display to the 8 newest, the rest stays
  // behind the card's own "Coverage" button into the `pokrycie` tool.
  it("coverage card caps to 8 newest periods", async () => {
    const coverage = Array.from({ length: 30 }, (_, i) => ({
      fiscalYear: 2026 - i,
      periodType: "FY",
      report: null,
      facts: { total: 0, validated: 0, unvalidated: 0, flagged: 0 },
      review: { flaggedFacts: 0 },
      skippedBudget: false,
    }));
    getCompanyViewMock.mockResolvedValue(fullView({ coverage }));
    renderScreen();
    await screen.findByText("CD Projekt");
    const coverageGroup = screen.getByRole("group", { name: "Report coverage" });
    expect(within(coverageGroup).getAllByRole("listitem")).toHaveLength(8);
  });

  // Owner dogfooding v0.74, item 5: a tool has no obvious way back to the
  // overview short of finding the small ✕ — a leading "Overview" button next
  // to the tool title returns to the untouched core.
  it("Overview returns from a tool to the untouched core", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    const { container } = renderScreen();
    await screen.findByText("CD Projekt");

    const layout = container.querySelector(".spolka-body-scroll") as HTMLElement;
    layout.scrollTop = 240;
    await user.click(screen.getAllByRole("button", { name: /Report 0/ })[0]);
    await screen.findByRole("group", { name: "Workshop tool" });

    await user.click(screen.getByRole("button", { name: "Overview" }));
    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    expect(screen.getByRole("group", { name: "Company core" })).toBeVisible();
    expect(layout.scrollTop).toBe(240);
  });

  // Owner dogfooding v0.74, item 7: opened from the Inbox "Otwórz spółkę",
  // the `feedItem` tool used to bury the selected item in the whole feed
  // list — its detail now leads, the list stays reachable below it.
  it("feedItem tool leads with the item detail", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    // The `feedItem` tool reads the CockpitCompanyFeedPanel's OWN global
    // `feedItems` prop (filtered by `company.qualifiedTicker`), not
    // `data.feed` — a real detail (not the empty state) needs a matching row.
    const feedItem: FeedItem = {
      id: "feed_0",
      company: company.qualifiedTicker,
      type: "Official report",
      source: "GPW ESPI/EBI",
      time: "Today 09:12",
      title: "Report 0",
      unread: false,
      saved: false,
      sourceUrl: "https://example.test/feed/0",
      language: "pl",
      publishedAt: "2026-08-20T09:00:00Z",
      fetchedAt: "2026-08-20T09:00:00Z",
      attribution: "GPW",
      summary: "Sample feed item summary.",
      bodyText: "Body text.",
      attachments: [],
      presentationKind: "report",
    };
    renderScreen({ feedItems: [feedItem] });
    await screen.findByText("CD Projekt");

    await user.click(screen.getAllByRole("button", { name: /Report 0/ })[0]);
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    expect(frame.getAttribute("data-tool")).toBe("feedItem");

    const detail = within(frame).getByLabelText("Company feed item details");
    const list = within(frame).getByLabelText("Company feed");
    const firstRow = list.querySelector('[data-company-feed-row="true"]');
    expect(firstRow).not.toBeNull();
    // The detail <aside> must precede the first feed row in DOM order.
    expect(detail.compareDocumentPosition(firstRow!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

});
