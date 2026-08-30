import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi, beforeEach } from "vitest";
import * as reportSeasonApi from "../../api/reportSeason";
import * as expectationsApi from "../../api/reportExpectations";
import { CommandInvocationError } from "../../api/tauri";
import type { Watchlist } from "../../api/types";
import { ReportSeasonScreen } from "./ReportSeasonScreen";
import { ReportSeasonProvider } from "../../app/state/screenViewModels";

vi.mock("../../api/reportSeason");
vi.mock("../../api/reportExpectations");

// F4b S4: KPI values render as `Figure kind="money"` (a formatted span) plus
// a separate unit text node — RTL's default string matcher can't join text
// split across sibling elements, so this asserts on normalized textContent.
function exactJoinedText(expected: string) {
  return (_content: string, node: Element | null) =>
    (node?.textContent ?? "").replace(/\s+/g, " ").trim() === expected;
}

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

// --- Pre-report expectations (J4, ADR 0071) ---------------------------------

function sampleExpectation(
  overrides: Partial<expectationsApi.ReportExpectation> = {},
): expectationsApi.ReportExpectation {
  return {
    id: "report_expectation_1",
    companyId: "company_gpw_cdr",
    eventKey: "evt-upcoming",
    fiscalYear: 2099,
    periodType: "H1",
    stanceMd: "Old stance",
    frozenAt: null,
    resolutionNoteMd: null,
    resolvedAt: null,
    createdAt: "2099-08-01T00:00:00Z",
    updatedAt: "2099-08-01T00:00:00Z",
    metrics: [],
    ...overrides,
  };
}

function sampleReview(
  overrides: Partial<expectationsApi.ExpectationReview> = {},
): expectationsApi.ExpectationReview {
  return {
    companyId: "company_gpw_cdr",
    eventKey: "evt-upcoming",
    fiscalYear: 2099,
    periodType: "H1",
    stanceMd: "Old stance",
    frozenAt: null,
    factsAvailable: false,
    resolutionNoteMd: null,
    resolvedAt: null,
    metrics: [],
    ...overrides,
  };
}

beforeEach(() => {
  // Default: the occurrence has no recorded expectation yet.
  vi.mocked(expectationsApi.listReportExpectations).mockResolvedValue([]);
  vi.mocked(expectationsApi.expectationReview).mockResolvedValue(sampleReview());
  vi.mocked(expectationsApi.createReportExpectation).mockResolvedValue(sampleExpectation());
  vi.mocked(expectationsApi.updateReportExpectation).mockResolvedValue(sampleExpectation());
  vi.mocked(expectationsApi.recordExpectationResolution).mockResolvedValue(
    sampleExpectation({ resolutionNoteMd: "done", resolvedAt: "2099-08-31T00:00:00Z" }),
  );
});

describe("Report-season expectations (J4)", () => {
  it("offers writing expectations when none exist yet", async () => {
    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    expect(
      await screen.findByRole("button", { name: /Add expectations/ }),
    ).toBeInTheDocument();
  });

  // F4b S4 (contract § Report Season, decision 5-style card choreography):
  // one filled action per expanded card, coordinated across the prep
  // checklist AND the expectations section.
  it("card primary choreography: no expectation → `Add expectations`; an expectation exists → `Mark as prepared`; composer open → `Save`", async () => {
    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    await screen.findByRole("button", { name: /Add expectations/ });
    let primaries = document.querySelectorAll('[data-ux-primary-action="true"]');
    expect(primaries).toHaveLength(1);
    expect(primaries[0]).toHaveTextContent("Add expectations");

    // Opening the composer demotes it: `Save` becomes the sole primary.
    await user.click(screen.getByRole("button", { name: /Add expectations/ }));
    await screen.findByRole("button", { name: "Save" });
    primaries = document.querySelectorAll('[data-ux-primary-action="true"]');
    expect(primaries).toHaveLength(1);
    expect(primaries[0]).toHaveTextContent("Save");
  });

  it("card primary choreography: an expectation exists → `Mark as prepared` is the one filled action", async () => {
    vi.mocked(expectationsApi.listReportExpectations).mockResolvedValue([sampleExpectation()]);

    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    await screen.findByRole("button", { name: /Edit expectations/ });

    const primaries = document.querySelectorAll('[data-ux-primary-action="true"]');
    expect(primaries).toHaveLength(1);
    expect(primaries[0]).toHaveTextContent("Mark as prepared");
  });

  it("writes a stance-only expectation for the occurrence", async () => {
    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    await user.click(await screen.findByRole("button", { name: /Add expectations/ }));
    await user.selectOptions(await screen.findByLabelText("Period type"), "H1");
    await user.type(screen.getByLabelText("Your stance"), "Margin recovery on the launch.");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(expectationsApi.createReportExpectation).toHaveBeenCalledWith(
        expect.objectContaining({
          companyId: "company_gpw_cdr",
          eventKey: "evt-upcoming",
          fiscalYear: 2099,
          periodType: "H1",
          stanceMd: "Margin recovery on the launch.",
          metrics: [],
        }),
      ),
    );
  });

  it("adds a metric expectation via the KPI picker", async () => {
    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    await user.click(await screen.findByRole("button", { name: /Add expectations/ }));
    await user.selectOptions(await screen.findByLabelText("Period type"), "H1");
    await user.type(screen.getByLabelText("Your stance"), "Revenue above a billion.");
    await user.click(screen.getByRole("button", { name: /Add metric/ }));
    await user.selectOptions(screen.getByLabelText("Metric"), "net_revenue");
    await user.selectOptions(screen.getByLabelText("Direction"), "gte");
    await user.type(screen.getByLabelText("Expected value"), "1000000000");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(expectationsApi.createReportExpectation).toHaveBeenCalledWith(
        expect.objectContaining({
          companyId: "company_gpw_cdr",
          eventKey: "evt-upcoming",
          metrics: [
            {
              metricKey: "net_revenue",
              comparator: "gte",
              expectedValue: "1000000000",
              unit: "PLN",
            },
          ],
        }),
      ),
    );
  });

  it("shows expectation-vs-actual and records a resolution note once facts land", async () => {
    vi.mocked(expectationsApi.listReportExpectations).mockResolvedValue([
      sampleExpectation({ frozenAt: "2099-08-30T00:00:00Z" }),
    ]);
    vi.mocked(expectationsApi.expectationReview).mockResolvedValue(
      sampleReview({
        frozenAt: "2099-08-30T00:00:00Z",
        factsAvailable: true,
        metrics: [
          {
            metricKey: "net_revenue",
            comparator: "gte",
            expectedValue: "1000000000",
            unit: "PLN",
            actualValue: "1200000000",
            outcome: "met",
          },
        ],
      }),
    );

    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));

    // Frozen → read-only review, no composer trigger.
    expect(await screen.findByText(/Expectations vs actuals/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Add expectations/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Edit expectations/ })).toBeNull();
    // Expected vs actual side by side, plus the factual outcome.
    expect(screen.getByText(/1200000000/)).toBeInTheDocument();
    expect(screen.getByText("Met")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Your verdict"), "Revenue beat my bar.");
    await user.click(screen.getByRole("button", { name: /Save verdict/ }));

    await waitFor(() =>
      expect(expectationsApi.recordExpectationResolution).toHaveBeenCalledWith(
        expect.objectContaining({
          companyId: "company_gpw_cdr",
          eventKey: "evt-upcoming",
          resolutionNoteMd: "Revenue beat my bar.",
        }),
      ),
    );
  });

  it("flips an editable expectation to read-only when the update hits the freeze conflict", async () => {
    vi.mocked(expectationsApi.listReportExpectations).mockResolvedValue([sampleExpectation()]);
    // Editable on expand; frozen after the conflict reload.
    vi.mocked(expectationsApi.expectationReview)
      .mockResolvedValueOnce(sampleReview({ factsAvailable: false }))
      .mockResolvedValue(sampleReview({ factsAvailable: true, frozenAt: "2099-08-30T00:00:00Z" }));
    vi.mocked(expectationsApi.updateReportExpectation).mockRejectedValue(
      new CommandInvocationError({ code: "conflict", message: "expectation is frozen" }),
    );

    const user = userEvent.setup();
    render(
      <ReportSeasonProvider value={{ watchlists, openCompanyWorkspace: vi.fn() }}>
        <ReportSeasonScreen />
      </ReportSeasonProvider>,
    );

    await user.click(await screen.findByText("CD Projekt"));
    await user.click(await screen.findByRole("button", { name: /Edit expectations/ }));
    await user.clear(screen.getByLabelText("Your stance"));
    await user.type(screen.getByLabelText("Your stance"), "Hindsight rewrite");
    await user.click(screen.getByRole("button", { name: "Save" }));

    // The conflict envelope reloads the review, which is now frozen → read-only:
    // the composer's save action is gone.
    expect(await screen.findByText(/Expectations vs actuals/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save" })).toBeNull();
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
    expect(screen.getByText(exactJoinedText("950 M PLN"))).toBeInTheDocument();
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
    await user.click(await screen.findByRole("button", { name: /Mark as prepared/ }));

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
    await user.click(await screen.findByRole("button", { name: "Company" }));
    expect(openCompanyWorkspace).toHaveBeenCalledWith("company_gpw_cdr", "Feed");

    await user.click(screen.getByRole("button", { name: "Claims" }));
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
