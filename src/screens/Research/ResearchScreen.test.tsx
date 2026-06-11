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

  it("creates a research question and links selected evidence", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Research" }));
    await waitFor(() => {
      expect(screen.getAllByText("Will margins recover?").length).toBeGreaterThan(0);
    });

    await user.type(screen.getByLabelText("Question title"), "What changed in the report?");
    await user.type(screen.getByLabelText("Question context"), "Track the source report and notes.");
    await user.click(screen.getByRole("button", { name: "Add question" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_research_question", {
        input: {
          scopeType: "company",
          scopeId: "company_gpw_cdr",
          title: "What changed in the report?",
          body: "Track the source report and notes.",
        },
      });
    });
    await waitFor(() => {
      expect(screen.getAllByText("What changed in the report?").length).toBeGreaterThan(0);
    });

    const researchRegion = await screen.findByLabelText("Evidence timeline");
    const feedEvidenceTitle = within(researchRegion).getByText(
      "Current report placeholder for watchlist company",
    );
    const feedEvidenceRow = feedEvidenceTitle.closest("article");
    expect(feedEvidenceRow).not.toBeNull();
    await user.click(within(feedEvidenceRow as HTMLElement).getByTitle("Link evidence"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_evidence_link", {
        input: {
          fromType: "research_question",
          fromId: expect.stringContaining("research_question_company_"),
          toType: "feed_item",
          toId: "feed_sample_cdr_report",
          relationType: "related",
        },
      });
    });
    expect(screen.getByText("Linked evidence")).toBeInTheDocument();
  });

  it("opens research question evidence inside Research instead of Notebooks", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Research" }));
    await user.type(screen.getByLabelText("Question title"), "Should backlog normalize?");
    await user.click(screen.getByRole("button", { name: "Add question" }));

    await waitFor(() => {
      expect(screen.getAllByText("Should backlog normalize?").length).toBeGreaterThan(0);
    });

    const researchRegion = await screen.findByLabelText("Evidence timeline");
    const questionEvidenceTitle = within(researchRegion).getByText("Should backlog normalize?");
    const questionEvidenceRow = questionEvidenceTitle.closest("article");
    expect(questionEvidenceRow).not.toBeNull();

    await user.click(within(questionEvidenceRow as HTMLElement).getByTitle("Open evidence"));

    expect(screen.getByRole("heading", { name: "Research" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Notebooks" })).not.toBeInTheDocument();
    expect(screen.getAllByText("Should backlog normalize?").length).toBeGreaterThan(0);
    expect(
      within(researchRegion).getByText("Current report placeholder for watchlist company"),
    ).toBeInTheDocument();
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
