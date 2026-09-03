import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
    onOpenExternalUrl: vi.fn(),
    onOpenFeedItem: vi.fn(),
    onSwitchCompany: vi.fn(),
    refreshCompletionCount: 0,
    ...overrides,
  };
}

// A real, running tool-host instance (not a mock) so the dirty-guard seam
// behaves exactly as it does in the app — `renderScreen`'s overrides can be
// re-applied via `rerender` while the SAME host instance keeps its state.
// `onHost` (F3c S1) exposes the live instance for tests that drive
// `commitTool`/`openTool` directly (payload-only re-commits have no UI path
// in this isolated harness).
function Harness(overrides: Partial<SpolkaScreenProps> & { onHost?: (host: SpolkaScreenProps["spolkaTool"]) => void }) {
  const spolkaTool = useSpolkaToolHost();
  overrides.onHost?.(spolkaTool);
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
    const group = await screen.findByRole("article", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it.each(sections)("no core section ever renders blank: $name (empty)", async ({ name, empty }) => {
    getCompanyViewMock.mockResolvedValue(emptyView(empty));
    renderScreen();
    const group = await screen.findByRole("article", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it.each(sections)("no core section ever renders blank: $name (error)", async ({ name, errorKey }) => {
    getCompanyViewMock.mockResolvedValue(
      emptyView({ sectionErrors: { [errorKey]: "unavailable" } }),
    );
    renderScreen();
    const group = await screen.findByRole("article", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it("every workshop tool opens in one click", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");
    // The workshop bar now ALSO lists fundamenty/feed/pokrycie/rekomendacje
    // (wave 2, item 1) — the same noun label as those tools' card buttons, so
    // those four are scoped to the workshop bar to disambiguate from the
    // card's own copy of the button.
    const workshopBar = screen.getByRole("toolbar", { name: "Workshop" });

    const expectations: Array<{ label: string; tool: Tool; scopeToWorkshopBar?: boolean }> = [
      { label: "Signals counter", tool: { t: "sygnaly" } },
      { label: "Claims counter", tool: { t: "tezy" } },
      { label: "Shorts counter", tool: { t: "akcjonariat" } },
      { label: "Events counter", tool: { t: "wydarzenia" } },
      // Destination buttons are nouns (owner dogfooding v0.74 item 4; ADR
      // 0104 dec. 3 amendment) — only the ⌘K palette keeps "Open X".
      { label: "Fundamentals", tool: { t: "fundamenty" }, scopeToWorkshopBar: true },
      { label: "Feed", tool: { t: "feed" }, scopeToWorkshopBar: true },
      { label: "Coverage", tool: { t: "pokrycie" }, scopeToWorkshopBar: true },
      { label: "Recommendations", tool: { t: "rekomendacje" }, scopeToWorkshopBar: true },
      { label: "Claims", tool: { t: "tezy" } },
      { label: "Notebook", tool: { t: "notatnik" } },
      { label: "Decision journal", tool: { t: "dziennik" } },
      { label: "Quality", tool: { t: "jakosc" } },
      { label: "Report diff", tool: { t: "diff" } },
      { label: "Research", tool: { t: "research" } },
      { label: "Ownership", tool: { t: "akcjonariat" } },
      { label: "Signals", tool: { t: "sygnaly" } },
      { label: "Documents", tool: { t: "dokumenty" } },
      { label: "Events", tool: { t: "wydarzenia" } },
    ];

    for (const { label, tool, scopeToWorkshopBar } of expectations) {
      const button = scopeToWorkshopBar
        ? within(workshopBar).getByRole("button", { name: label })
        : screen.getByRole("button", { name: label });
      await user.click(button);
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

  // Owner dogfooding v0.74 wave 2, item 1: Przegląd/Overview leads the bar
  // (the main, no-tool view), every workshop tool is listed (not just the
  // ones without their own card button), and the active entry is marked.
  it("workshop bar lists Overview first and every tool, marking the active one", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");

    const workshopBar = screen.getByRole("toolbar", { name: "Workshop" });
    const labels = within(workshopBar).getAllByRole("button").map((b) => b.textContent);
    expect(labels).toEqual([
      "Overview",
      "Fundamentals",
      "Feed",
      "Coverage",
      "Recommendations",
      "Claims",
      "Notebook",
      "Decision journal",
      "Quality",
      "Report diff",
      "Research",
      "Ownership",
      "Signals",
      "Documents",
      "Events",
    ]);

    expect(within(workshopBar).getByRole("button", { name: "Overview" })).toHaveAttribute("aria-pressed", "true");

    await user.click(within(workshopBar).getByRole("button", { name: "Notebook" }));
    await screen.findByRole("group", { name: "Workshop tool" });
    expect(within(workshopBar).getByRole("button", { name: "Notebook" })).toHaveAttribute("aria-pressed", "true");
    expect(within(workshopBar).getByRole("button", { name: "Overview" })).toHaveAttribute("aria-pressed", "false");
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
    const priceGroup = await screen.findByRole("article", { name: "Price chart" });
    expect(within(priceGroup).getByText(/18\.08\.2026/)).toBeInTheDocument();
    const coverageGroup = screen.getByRole("article", { name: "Report coverage" });
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
    const feedGroup = screen.getByRole("article", { name: "Company feed" });
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
    const coverageGroup = screen.getByRole("article", { name: "Report coverage" });
    // A compact table now (wave 2, item 2), not a bare list — one row per
    // period, excluding the header row.
    expect(within(coverageGroup).getAllByRole("row")).toHaveLength(9);
  });

  // Owner dogfooding v0.74 wave 2, item 2: the coverage card is a compact
  // table — period as a mono id, status as a StatusChip, facts count as a
  // figure — not a bare list.
  it("coverage card renders period, status chip and fact count", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");
    const coverageGroup = screen.getByRole("article", { name: "Report coverage" });

    const readRow = within(coverageGroup).getByText(/FY ?2025/).closest("tr")!;
    expect(within(readRow).getByText("read")).toBeInTheDocument();
    expect(within(readRow).getByText("128")).toBeInTheDocument();

    const expectedRow = within(coverageGroup).getByText(/Q3 ?2026/).closest("tr")!;
    expect(within(expectedRow).getByText("expected")).toBeInTheDocument();
  });

  // Owner dogfooding v0.74 (item 5, then wave 3): the workshop bar's Overview
  // tab is THE way back to the untouched core — the tool header carries no
  // leading back button of its own.
  it("Overview tab returns from a tool to the untouched core", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    const { container } = renderScreen();
    await screen.findByText("CD Projekt");

    const layout = container.querySelector(".spolka-body-scroll") as HTMLElement;
    layout.scrollTop = 240;
    await user.click(screen.getAllByRole("button", { name: /Report 0/ })[0]);
    const frame = await screen.findByRole("group", { name: "Workshop tool" });

    expect(within(frame).queryByRole("button", { name: "Overview" })).not.toBeInTheDocument();
    await user.click(within(screen.getByRole("toolbar", { name: "Workshop" })).getByRole("button", { name: "Overview" }));
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
    // The `feedItem` tool reads the CompanyFeedPanel's OWN global
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

  // sol fix1 item 3: the F3a "cross-screen actions omitted" comment no
  // longer holds — the feed panel already knows its OWN company, so a note
  // draft lands in the SAME company's `notatnik` tool (no cross-screen jump).
  it("a feed item's Note action opens the notatnik tool prefilled with its draft", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
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
    const feedItemFrame = await screen.findByRole("group", { name: "Workshop tool" });
    expect(feedItemFrame.getAttribute("data-tool")).toBe("feedItem");

    await user.click(within(feedItemFrame).getByRole("button", { name: "Note" }));

    const notatnikFrame = await screen.findByRole("group", { name: "Workshop tool" });
    expect(notatnikFrame.getAttribute("data-tool")).toBe("notatnik");
    expect(
      within(notatnikFrame).getByLabelText<HTMLInputElement>("Notebook note title").value,
    ).toBe("Report 0");
  });
});

// F3c S1 (plan § Design 1–3): the workshop bar as an APG toolbar (roving
// tabindex), tool-frame focus on open/close, and the Escape → Overview
// contract.
describe("SpolkaScreen keyboard model (F3c S1)", () => {
  it("is an APG toolbar with exactly one tabindex=0 among the 15 entries", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");

    const bar = screen.getByRole("toolbar", { name: "Workshop" });
    expect(bar).toHaveAttribute("aria-orientation", "horizontal");
    const entries = within(bar).getAllByRole("button");
    expect(entries).toHaveLength(15);
    const zeroStops = entries.filter((entry) => entry.getAttribute("tabindex") === "0");
    expect(zeroStops).toHaveLength(1);
    expect(zeroStops[0]).toHaveAccessibleName("Overview");
  });

  it("ArrowRight/ArrowLeft move the tab stop with wraparound; Home/End jump to the ends", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");
    const bar = screen.getByRole("toolbar", { name: "Workshop" });
    const overview = within(bar).getByRole("button", { name: "Overview" });
    const fundamentals = within(bar).getByRole("button", { name: "Fundamentals" });
    const events = within(bar).getByRole("button", { name: "Events" });

    overview.focus();
    await user.keyboard("{ArrowLeft}");
    expect(events).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(overview).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(fundamentals).toHaveFocus();
    await user.keyboard("{End}");
    expect(events).toHaveFocus();
    await user.keyboard("{Home}");
    expect(overview).toHaveFocus();
  });

  it("Enter opens the focused entry's tool", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");
    within(screen.getByRole("toolbar", { name: "Workshop" })).getByRole("button", { name: "Claims" }).focus();
    await user.keyboard("{Enter}");
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    expect(frame).toHaveAttribute("data-tool", "tezy");
  });

  it("focuses the tool heading on open, and again on a same-kind payload change", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    let host: SpolkaScreenProps["spolkaTool"] | undefined;
    render(<Harness onHost={(h) => { host = h; }} />);
    await screen.findByText("CD Projekt");

    host!.openTool(company.id, { t: "dokumenty", documentId: "doc_1" });
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    const heading = within(frame).getByRole("heading", { level: 2, name: "Documents" });
    await waitFor(() => expect(heading).toHaveFocus());

    // Move focus away, then re-commit the SAME kind with a DIFFERENT payload
    // — the heading must be re-focused (keyed on `openSeq`, not tool kind).
    heading.blur();
    document.body.focus();
    host!.openTool(company.id, { t: "dokumenty", documentId: "doc_2" });
    await waitFor(() => expect(within(frame).getByRole("heading", { level: 2, name: "Documents" })).toHaveFocus());
  });

  it("Escape closes the tool and focuses the closed entry", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");

    await user.click(within(screen.getByRole("toolbar", { name: "Workshop" })).getByRole("button", { name: "Notebook" }));
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    await waitFor(() => expect(within(frame).getByRole("heading", { level: 2, name: "Notebook" })).toHaveFocus());

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    expect(within(screen.getByRole("toolbar", { name: "Workshop" })).getByRole("button", { name: "Notebook" })).toHaveFocus();
  });

  it("Overview tab click focuses the Overview entry (the 'overview' intent)", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");

    const bar = screen.getByRole("toolbar", { name: "Workshop" });
    await user.click(within(bar).getByRole("button", { name: "Claims" }));
    await screen.findByRole("group", { name: "Workshop tool" });
    await user.click(within(bar).getByRole("button", { name: "Overview" }));
    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    expect(within(bar).getByRole("button", { name: "Overview" })).toHaveFocus();
  });

  it("feedItem open marks and, on close, focuses the Feed entry (no bar entry of its own)", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");

    await user.click(screen.getAllByRole("button", { name: /Report \d/ })[0]);
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    expect(frame).toHaveAttribute("data-tool", "feedItem");
    const bar = screen.getByRole("toolbar", { name: "Workshop" });
    expect(within(bar).getByRole("button", { name: "Feed" })).toHaveAttribute("aria-pressed", "true");

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    expect(within(bar).getByRole("button", { name: "Feed" })).toHaveFocus();
  });

  it("Escape on a hosted native <select> leaves the tool open", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");

    await user.click(within(screen.getByRole("toolbar", { name: "Workshop" })).getByRole("button", { name: "Decision journal" }));
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    await user.click(within(frame).getByRole("button", { name: "New entry" }));
    const decisionKind = within(frame).getByRole("combobox", { name: "Decision kind" });
    decisionKind.focus();

    fireEvent.keyDown(decisionKind, { key: "Escape" });
    expect(screen.getByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "dziennik");
  });

  it("Escape from a dirty notebook composer opens stay/discard; Discard lands focus on the closed entry", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    renderScreen();
    await screen.findByText("CD Projekt");

    const bar = screen.getByRole("toolbar", { name: "Workshop" });
    await user.click(within(bar).getByRole("button", { name: "Notebook" }));
    const frame = await screen.findByRole("group", { name: "Workshop tool" });
    await user.click(within(frame).getByRole("button", { name: "New note" }));
    const titleField = within(frame).getByRole("textbox", { name: "Notebook note title" });
    await user.type(titleField, "Draft in progress");

    await user.keyboard("{Escape}");
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("button", { name: "Stay" })).toBeInTheDocument();
    // The tool is still open behind the dialog — Escape did NOT close it.
    expect(screen.getByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "notatnik");

    // A second Escape (dialog open) closes only the dialog (Modal's own
    // Escape → Stay), never the tool.
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByRole("group", { name: "Workshop tool" })).toHaveAttribute("data-tool", "notatnik");

    // Re-open the dialog and Discard this time — focus lands on the closed
    // entry (Notebook), the transition Escape originally intended.
    await user.keyboard("{Escape}");
    const dialogAgain = await screen.findByRole("dialog");
    await user.click(within(dialogAgain).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(screen.queryByRole("group", { name: "Workshop tool" })).not.toBeInTheDocument());
    expect(within(bar).getByRole("button", { name: "Notebook" })).toHaveFocus();
  });
});
