import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SpolkaScreen, type SpolkaScreenProps } from "./SpolkaScreen";
import { getCompanyView } from "../../api/companyView";
import type { CompanyView } from "../../api/generated/CompanyView";
import type { Tool } from "./route";
import { TOOL_KINDS } from "./route";
import { COMPANY_SPECS, makeCompany } from "../../test/scenarios/entities";

vi.mock("../../api/companyView", () => ({
  getCompanyView: vi.fn(),
}));

const getCompanyViewMock = vi.mocked(getCompanyView);

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
    onOpenTool: vi.fn(),
    onOpenDocument: vi.fn(),
    onOpenFeedItem: vi.fn(),
    refreshCompletionCount: 0,
    ...overrides,
  };
}

beforeEach(() => {
  getCompanyViewMock.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("SpolkaScreen", () => {
  it("renders from a single get_company_view call", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    render(<SpolkaScreen {...baseProps()} />);

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
    render(<SpolkaScreen {...baseProps()} />);
    const group = await screen.findByRole("group", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it.each(sections)("no core section ever renders blank: $name (empty)", async ({ name, empty }) => {
    getCompanyViewMock.mockResolvedValue(emptyView(empty));
    render(<SpolkaScreen {...baseProps()} />);
    const group = await screen.findByRole("group", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it.each(sections)("no core section ever renders blank: $name (error)", async ({ name, errorKey }) => {
    getCompanyViewMock.mockResolvedValue(
      emptyView({ sectionErrors: { [errorKey]: "unavailable" } }),
    );
    render(<SpolkaScreen {...baseProps()} />);
    const group = await screen.findByRole("group", { name });
    expect(group.textContent?.trim().length).toBeGreaterThan(0);
  });

  it("every workshop tool opens in one click", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    const onOpenTool = vi.fn();
    render(<SpolkaScreen {...baseProps({ onOpenTool })} />);
    await screen.findByText("CD Projekt");

    const expectations: Array<{ label: string; tool: Tool }> = [
      { label: "Signals counter", tool: { t: "sygnaly" } },
      { label: "Claims counter", tool: { t: "tezy" } },
      { label: "Shorts counter", tool: { t: "akcjonariat" } },
      { label: "Events counter", tool: { t: "wydarzenia" } },
      { label: "Open fundamentals", tool: { t: "fundamenty" } },
      { label: "Open feed", tool: { t: "feed" } },
      { label: "Open coverage", tool: { t: "pokrycie" } },
      { label: "Open recommendations", tool: { t: "rekomendacje" } },
      { label: "Open claims", tool: { t: "tezy" } },
      { label: "Open notebook", tool: { t: "notatnik" } },
      { label: "Open decision journal", tool: { t: "dziennik" } },
      { label: "Open quality", tool: { t: "jakosc" } },
      { label: "Open report diff", tool: { t: "diff" } },
      { label: "Open research", tool: { t: "research" } },
      { label: "Open ownership", tool: { t: "akcjonariat" } },
      { label: "Open signals", tool: { t: "sygnaly" } },
      { label: "Open documents", tool: { t: "dokumenty" } },
    ];

    for (const { label, tool } of expectations) {
      onOpenTool.mockClear();
      await user.click(screen.getByRole("button", { name: label }));
      expect(onOpenTool).toHaveBeenCalledTimes(1);
      expect(onOpenTool).toHaveBeenCalledWith(tool);
    }

    // feedItem: click a feed row.
    const onOpenFeedItem = vi.fn();
    const { container: feedContainer } = render(<SpolkaScreen {...baseProps({ onOpenFeedItem })} />);
    const feedButtons = await within(feedContainer).findAllByRole("button", { name: /Report \d/ });
    await user.click(feedButtons[0]);
    expect(onOpenFeedItem).toHaveBeenCalledWith("feed_0");

    // Every tool kind is reachable from this screen (15).
    const reached = new Set(expectations.map((e) => e.tool.t));
    reached.add("feedItem");
    expect([...reached].sort()).toEqual([...TOOL_KINDS].sort());
  });

  it("KPI ticket navigates to its document", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockResolvedValue(fullView());
    const onOpenDocument = vi.fn();
    render(<SpolkaScreen {...baseProps({ onOpenDocument })} />);
    await user.click(await screen.findByRole("button", { name: "Open source document" }));
    expect(onOpenDocument).toHaveBeenCalledWith("Raport roczny 2026 · s. 44");
  });

  it("kurs and coverage carry as-of dates", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    render(<SpolkaScreen {...baseProps()} />);
    const priceGroup = await screen.findByRole("group", { name: "Price chart" });
    expect(within(priceGroup).getByText(/18\.08\.2026/)).toBeInTheDocument();
    const coverageGroup = screen.getByRole("group", { name: "Report coverage" });
    expect(within(coverageGroup).getByText(/FY ?2025/)).toBeInTheDocument();
    expect(within(coverageGroup).getByText(/Q3 ?2026/)).toBeInTheDocument();
  });

  it("whole-read error shows the error card with Refresh and refetches", async () => {
    const user = userEvent.setup();
    getCompanyViewMock.mockRejectedValueOnce(new Error("boom"));
    render(<SpolkaScreen {...baseProps()} />);
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

    const { rerender } = render(<SpolkaScreen {...baseProps()} />);
    rerender(<SpolkaScreen {...baseProps({ companyId: companyB.id, company: companyB })} />);

    await screen.findByText("PKN Orlen");
    resolveA(fullView());
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.getByText("PKN Orlen")).toBeInTheDocument();
    expect(screen.queryByText("CD Projekt")).not.toBeInTheDocument();
  });

  it("no primary action at rest", async () => {
    getCompanyViewMock.mockResolvedValue(fullView());
    const { container } = render(<SpolkaScreen {...baseProps()} />);
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
    render(<SpolkaScreen {...baseProps()} />);
    await screen.findByText("CD Projekt");
    expect(screen.getByRole("button", { name: /Signals counter/ }).textContent).toContain("99+");
    const feedGroup = screen.getByRole("group", { name: "Company feed" });
    expect(within(feedGroup).getAllByRole("button", { name: /Report \d/ })).toHaveLength(6);
  });
});
