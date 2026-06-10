import { describe, it } from "vitest";
import {
  expect,
  invoke,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

describe("Research screen workflows", () => {
  it("opens a feed evidence item in Inbox without hiding the selected item", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Research" }));
    const researchRegion = await screen.findByLabelText("Evidence timeline");
    const firstEvidenceRow = within(researchRegion).getAllByRole("article")[0];

    await user.click(within(firstEvidenceRow).getByTitle("Open evidence"));

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getAllByText("Current report placeholder for watchlist company").length).toBeGreaterThan(0);
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:CDR");
  });

  it("loads company evidence through backend-owned timeline filters", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Research" }));

    const researchRegion = await screen.findByLabelText("Evidence timeline");
    expect(screen.getByRole("heading", { name: "Research" })).toBeInTheDocument();
    expect(screen.getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(screen.getByText("CDR follow-up note")).toBeInTheDocument();
    expect(screen.getByText("Shareholder Meeting")).toBeInTheDocument();
    expect(screen.getByText("Gemini")).toBeInTheDocument();
    expect(screen.queryByText("shareholder_meeting")).not.toBeInTheDocument();
    expect(screen.queryByText("provider_gemini")).not.toBeInTheDocument();
    expect(screen.getByText("Evidence")).toBeInTheDocument();
    expect(screen.getByText("Last reviewed")).toBeInTheDocument();
    expect(within(researchRegion).getAllByRole("article")).toHaveLength(4);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_research_evidence", {
        input: {
          companyId: "company_gpw_cdr",
          watchlistId: null,
          evidenceTypes: null,
          changedSinceReviewOnly: false,
          limit: 100,
        },
      });
    });

    await user.click(screen.getByRole("button", { name: "Notes" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_research_evidence", {
        input: {
          companyId: "company_gpw_cdr",
          watchlistId: null,
          evidenceTypes: ["notebook_entry"],
          changedSinceReviewOnly: false,
          limit: 100,
        },
      });
    });
    expect(screen.queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
    expect(screen.getByText("CDR follow-up note")).toBeInTheDocument();

    await user.click(screen.getByLabelText("Changed since review"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_research_evidence", {
        input: {
          companyId: "company_gpw_cdr",
          watchlistId: null,
          evidenceTypes: ["notebook_entry"],
          changedSinceReviewOnly: true,
          limit: 100,
        },
      });
    });
    expect(screen.getByText("No evidence for selected filters.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Mark reviewed" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("mark_research_scope_reviewed", {
        input: expect.objectContaining({
          scopeType: "company",
          scopeId: "company_gpw_cdr",
        }),
      });
    });
  });

  it("loads watchlist evidence and can explicitly cascade review to member companies", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Research" }));
    await user.click(screen.getByRole("button", { name: "Watchlist" }));

    const reviewSummary = screen.getByLabelText("Research review summary");
    expect(screen.getByLabelText("Watchlist company review queue")).toBeInTheDocument();
    expect(within(reviewSummary).getByText("Companies")).toBeInTheDocument();
    expect(within(reviewSummary).getByText("Need review")).toBeInTheDocument();

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_research_evidence", {
        input: {
          companyId: null,
          watchlistId: "watchlist_main_gpw",
          evidenceTypes: null,
          changedSinceReviewOnly: false,
          limit: 100,
        },
      });
    });

    await user.click(screen.getByLabelText("Also mark member companies reviewed"));
    await user.click(screen.getByRole("button", { name: "Mark reviewed" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("mark_research_scope_reviewed", {
        input: expect.objectContaining({
          scopeType: "watchlist",
          scopeId: "watchlist_main_gpw",
          cascadeToCompanies: true,
        }),
      });
    });
  });
});
