import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import * as reportSeasonApi from "../../api/reportSeason";
import type { Watchlist } from "../../api/types";
import { ReportSeasonScreen } from "./ReportSeasonScreen";
import { ReportSeasonProvider } from "../../app/state/screenViewModels";

vi.mock("../../api/reportSeason");

const watchlists: Watchlist[] = [
  { id: "watchlist_main", name: "Main GPW", description: null, companyCount: 1 },
];

const season: reportSeasonApi.ReportSeasonResult = {
  upcoming: [
    {
      companyId: "company_gpw_cdr",
      qualifiedTicker: "GPW:CDR",
      displayName: "CD Projekt",
      eventKey: "evt-upcoming",
      eventDate: "2099-08-29",
      eventTime: null,
      title: "Raport za Q1 2099",
      preparationStatus: "upcoming",
    },
  ],
  past: [],
  calendarFreshness: { lastFetchedAt: "2099-08-01T08:00:00Z", stale: false },
};

const card: reportSeasonApi.PreReportCard = {
  companyId: "company_gpw_cdr",
  eventKey: "evt-upcoming",
  eventDate: "2099-08-29",
  preparationStatus: "upcoming",
  linkedReportDocumentId: null,
  openQuestions: [
    {
      id: "q_01",
      scopeType: "company",
      scopeId: "company_gpw_cdr",
      title: "Czy marża brutto się utrzyma?",
      body: "",
      status: "open",
      closedAt: null,
      createdAt: "2099-01-01T00:00:00Z",
      updatedAt: "2099-01-01T00:00:00Z",
    },
  ],
  unresolvedClaims: { due: [], overdue: [], upcoming: [] },
  lastPeriodKpis: [
    { periodId: "period_1", metricKey: "net_revenue", label: "Net Revenue", unit: "PLN", valueNumeric: "950000000" },
  ],
  recentEvidence: [],
};

beforeEach(() => {
  vi.mocked(reportSeasonApi.listReportSeason).mockResolvedValue(season);
  vi.mocked(reportSeasonApi.getPreReportCard).mockResolvedValue(card);
  vi.mocked(reportSeasonApi.markReportPrepared).mockResolvedValue({
    companyId: "company_gpw_cdr",
    eventKey: "evt-upcoming",
    status: "prepared",
    preparedAt: "2099-08-02T00:00:00Z",
    processedAt: null,
    linkedReportDocumentId: null,
  });
  vi.mocked(reportSeasonApi.markReportProcessed).mockResolvedValue({
    companyId: "company_gpw_cdr",
    eventKey: "evt-upcoming",
    status: "processed",
    preparedAt: "2099-08-02T00:00:00Z",
    processedAt: "2099-08-30T00:00:00Z",
    linkedReportDocumentId: null,
  });
});

describe("Report-season cockpit", () => {
  it("renders upcoming reports and expands a pre-report card", async () => {
    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    expect(await screen.findByText("CD Projekt")).toBeInTheDocument();

    await user.click(screen.getByText("CD Projekt"));

    expect(await screen.findByText("Czy marża brutto się utrzyma?")).toBeInTheDocument();
    expect(screen.getByText("Net Revenue")).toBeInTheDocument();
    expect(screen.getByText("950000000 PLN")).toBeInTheDocument();
    expect(reportSeasonApi.getPreReportCard).toHaveBeenCalledWith({
      companyId: "company_gpw_cdr",
      eventKey: "evt-upcoming",
    });
  });

  it("groups the pre-report card into prep + extended sections for tiered density", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    // U7-D density (ADR 0076 D6): the pre-report card splits into a prep checklist
    // (shown from M) and an extended context block (shown from L); container
    // queries fold each by tier. jsdom asserts the structure the CSS keys off.
    expect(await screen.findByText("Czy marża brutto się utrzyma?")).toBeInTheDocument();
    expect(container.querySelector(".report-season-card-prep")).not.toBeNull();
    expect(container.querySelector(".report-season-card-extended")).not.toBeNull();
  });

  it("marks a report prepared and refreshes the season", async () => {
    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    await user.click(await screen.findByRole("button", { name: /Mark prepared/ }));

    await waitFor(() =>
      expect(reportSeasonApi.markReportPrepared).toHaveBeenCalledWith({
        companyId: "company_gpw_cdr",
        eventKey: "evt-upcoming",
      }),
    );
    // The season is reloaded after the workflow action (initial load + post-action).
    const initialLoads = vi.mocked(reportSeasonApi.listReportSeason).mock.calls.length;
    await waitFor(() =>
      expect(
        vi.mocked(reportSeasonApi.listReportSeason).mock.calls.length,
      ).toBeGreaterThan(1),
    );
    expect(initialLoads).toBeGreaterThanOrEqual(1);
  });

  it("drills into the company workspace and its claims tab", async () => {
    const user = userEvent.setup();
    const openCompanyWorkspace = vi.fn();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    await user.click(await screen.findByRole("button", { name: "Open workspace" }));
    expect(openCompanyWorkspace).toHaveBeenCalledWith("company_gpw_cdr", "Feed");

    await user.click(screen.getByRole("button", { name: "Open claims" }));
    expect(openCompanyWorkspace).toHaveBeenCalledWith("company_gpw_cdr", "Claims");
  });

  it("shows an empty state when no upcoming reports are in scope", async () => {
    vi.mocked(reportSeasonApi.listReportSeason).mockResolvedValue({
      upcoming: [],
      past: [],
      calendarFreshness: { lastFetchedAt: null, stale: true },
    });
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    expect(
      await screen.findByText(/No upcoming reports in scope/),
    ).toBeInTheDocument();
    // A stale calendar is surfaced rather than silently empty.
    expect(screen.getByText("Calendar may be out of date")).toBeInTheDocument();
  });
});
