import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CompanyContextSection } from "./CompanyContextSection";
import { getCompanyContext } from "../../../api/companyContext";
import type { CompanyContext } from "../../../api/companyContext";

vi.mock("../../../api/companyContext", () => ({
  getCompanyContext: vi.fn(),
}));

const getCompanyContextMock = vi.mocked(getCompanyContext);

function context(overrides: Partial<CompanyContext> = {}): CompanyContext {
  return {
    companyId: "company_gpw_pzu",
    latestPeriodFacts: {
      periodLabel: "H1 2026",
      facts: [
        {
          metricKey: "eps_basic",
          valueNumeric: "3.49",
          currency: "PLN",
          sourceDocumentRef: "doc_evidence_1",
          createdAt: "2026-08-19T00:00:00Z",
        },
      ],
    },
    upcomingEvents: [{ title: "Publikacja raportu H1 2026", eventDate: "2026-08-20", eventType: "report" }],
    notebook: { count: 1, latestAt: "2026-05-29T00:00:00Z" },
    claimsDue: { due: 0, overdue: 0 },
    ...overrides,
  };
}

describe("CompanyContextSection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows a loading skeleton before the read resolves", () => {
    getCompanyContextMock.mockReturnValue(new Promise(() => {}));
    render(<CompanyContextSection companyId="company_gpw_pzu" />);
    expect(screen.getByLabelText("Loading company context…")).toBeInTheDocument();
  });

  it("renders facts, events, notebook, and claims once the read resolves", async () => {
    getCompanyContextMock.mockResolvedValue(context());
    render(<CompanyContextSection companyId="company_gpw_pzu" />);

    expect(await screen.findByText("Publikacja raportu H1 2026")).toBeInTheDocument();
    expect(screen.getByText("3.49 PLN")).toBeInTheDocument();
    expect(screen.getAllByText(/H1 2026/).length).toBeGreaterThan(0);
    expect(screen.getByText("No management promises awaiting verification.")).toBeInTheDocument();
  });

  it("shows each section's own empty state when its data is empty", async () => {
    getCompanyContextMock.mockResolvedValue(
      context({
        latestPeriodFacts: null,
        upcomingEvents: [],
        notebook: { count: 0, latestAt: null },
      }),
    );
    render(<CompanyContextSection companyId="company_gpw_pzu" />);

    expect(await screen.findByText("No recorded financial facts yet.")).toBeInTheDocument();
    expect(screen.getByText("No upcoming events tracked.")).toBeInTheDocument();
    expect(screen.getByText("No notebook entries for this company yet.")).toBeInTheDocument();
  });

  it("navigates to the report-documents view when a fact's provenance ticket is clicked", async () => {
    const user = userEvent.setup();
    const onOpenReportDocuments = vi.fn();
    getCompanyContextMock.mockResolvedValue(context());
    render(<CompanyContextSection companyId="company_gpw_pzu" onOpenReportDocuments={onOpenReportDocuments} />);

    const ticket = await screen.findByRole("button", { name: /H1 2026/ });
    await user.click(ticket);

    expect(onOpenReportDocuments).toHaveBeenCalledTimes(1);
  });

  it("renders the ticket as non-interactive text when the host has no navigation to give it", async () => {
    getCompanyContextMock.mockResolvedValue(context());
    render(<CompanyContextSection companyId="company_gpw_pzu" />);

    await screen.findByText(/H1 2026 · /);
    expect(screen.queryByRole("button", { name: /H1 2026/ })).not.toBeInTheDocument();
  });

  it("shows an inline error with a Refresh action that retries", async () => {
    const user = userEvent.setup();
    getCompanyContextMock.mockRejectedValueOnce(new Error("boom"));
    render(<CompanyContextSection companyId="company_gpw_pzu" />);

    expect(await screen.findByText("Could not load company context.")).toBeInTheDocument();

    getCompanyContextMock.mockResolvedValueOnce(context());
    await user.click(screen.getByRole("button", { name: "Refresh" }));

    expect(await screen.findByText("Publikacja raportu H1 2026")).toBeInTheDocument();
  });
});
