import { describe, it, vi } from "vitest";
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

    renderApp({ section: "Research" });

    const researchRegion = await screen.findByLabelText("Evidence timeline");
    const firstEvidenceRow = (await within(researchRegion).findAllByRole("article"))[0];

    await user.click(within(firstEvidenceRow).getByTitle("Open evidence"));

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getAllByText("Current report placeholder for watchlist company").length).toBeGreaterThan(0);
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:CDR");
  });

  it("loads company evidence through backend-owned timeline filters", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });


    const researchRegion = await screen.findByLabelText("Evidence timeline");
    expect(screen.getByRole("heading", { name: "Research" })).toBeInTheDocument();
    expect(
      await within(researchRegion).findByText("Current report placeholder for watchlist company"),
    ).toBeInTheDocument();
    expect(screen.getByText("CDR follow-up note")).toBeInTheDocument();
    expect(screen.getByText("Shareholder Meeting")).toBeInTheDocument();
    expect(screen.getByText("Gemini")).toBeInTheDocument();
    expect(screen.queryByText("shareholder_meeting")).not.toBeInTheDocument();
    expect(screen.queryByText("provider_gemini")).not.toBeInTheDocument();
    const reviewSummary = screen.getByLabelText("Research review summary");
    expect(within(reviewSummary).getByText("Evidence")).toBeInTheDocument();
    expect(within(reviewSummary).getByText("Last reviewed")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Evidence" })).toBeInTheDocument();
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

    renderApp({ section: "Research" });

    await waitFor(() => {
      expect(screen.getAllByText("Will margins recover?").length).toBeGreaterThan(0);
    });

    await user.click(screen.getByRole("button", { name: "Add question" }));
    await user.type(screen.getByLabelText("Question title"), "What changed in the report?");
    await user.type(screen.getByLabelText("Question context"), "Track the source report and notes.");
    await user.click(screen.getByRole("button", { name: "Save question" }));

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

    renderApp({ section: "Research" });

    await user.click(screen.getByRole("button", { name: "Add question" }));
    await user.type(screen.getByLabelText("Question title"), "Should backlog normalize?");
    await user.click(screen.getByRole("button", { name: "Save question" }));

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

  it("confirms and deletes a selected research question", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderApp({ section: "Research" });

    await waitFor(() => {
      expect(screen.getAllByText("Will margins recover?").length).toBeGreaterThan(0);
    });

    await user.click(screen.getByRole("button", { name: "Delete research question" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("delete_research_question", {
        id: "research_question_company_gpw_cdr_margin",
      });
    });
    expect(confirm).toHaveBeenCalledWith('Delete research question "Will margins recover?"?');

    confirm.mockRestore();
  });

  it("loads watchlist evidence and can explicitly cascade review to member companies", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

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

  it("generates and displays a cited AI research brief", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

    expect(screen.getByText("No research brief generated yet.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "AI research" })).toBeInTheDocument();
    expect(screen.getAllByText("What needs attention").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Research summary").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "Generate brief" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("start_research_brief", {
        input: {
          scopeType: "company",
          scopeId: "company_gpw_cdr",
        },
      });
    });

    expect(await screen.findByText("Investor research brief: CD PROJEKT S.A.")).toBeInTheDocument();
    expect(screen.queryByText(/company_gpw_cdr/i)).not.toBeInTheDocument();
    expect(screen.getByText("Source-grounded brief summary.")).toBeInTheDocument();
    expect(screen.getByText("Citations (1)")).toBeInTheDocument();
    expect(screen.getAllByText("Current report placeholder for watchlist company").length).toBeGreaterThan(0);
  });

  it("shows open research reminders and completes one", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });


    expect(await screen.findByText("Review open claim follow-up")).toBeInTheDocument();
    await user.click(screen.getByTitle("Snooze reminder"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_research_reminder", {
        input: expect.objectContaining({
          id: "research_reminder_claim_follow_up",
          status: "open",
        }),
      });
    });

    await user.click(screen.getByTitle("Complete reminder"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_research_reminder", {
        input: {
          id: "research_reminder_claim_follow_up",
          status: "completed",
        },
      });
    });

    await user.click(await screen.findByTitle("Reopen reminder"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_research_reminder", {
        input: {
          id: "research_reminder_claim_follow_up",
          status: "open",
          snoozedUntil: null,
        },
      });
    });
  });

  it("creates a manual research reminder", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

    await user.click(screen.getByRole("button", { name: "Add reminder" }));
    await user.type(screen.getByLabelText("Reminder title"), "Check next report");
    await user.type(screen.getByLabelText("Reminder notes"), "Look for margin commentary.");
    await user.click(screen.getByRole("button", { name: "Save reminder" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_research_reminder", {
        input: {
          scopeType: "company",
          scopeId: "company_gpw_cdr",
          companyId: "company_gpw_cdr",
          reminderKind: "manual_research",
          sourceType: null,
          sourceId: null,
          title: "Check next report",
          body: "Look for margin commentary.",
          dueAt: null,
        },
      });
    });
  });

  it("generates and displays an event-aware research digest", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

    expect(screen.getByText("No research digest generated yet.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Generate digest" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("start_research_digest", {
        input: {
          scopeType: "company",
          scopeId: "company_gpw_cdr",
        },
      });
    });

    expect(await screen.findByText("Open reminders and changed evidence to review.")).toBeInTheDocument();
    expect(screen.getAllByText("What needs attention").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Citations (1)").length).toBeGreaterThan(0);
  });

  it("opens cited evidence from an AI research brief", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

    await user.click(screen.getByRole("button", { name: "Generate brief" }));

    await user.click(await screen.findByText("Citations (1)"));
    await user.click(screen.getByRole("button", { name: /E1 Current report placeholder/ }));

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:CDR");
  });

  it("resizes research split panels with keyboard controls", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

    const briefResizer = screen.getByRole("separator", { name: "Resize AI research brief panel" });
    expect(briefResizer).toHaveAttribute("aria-valuenow", "520");

    briefResizer.focus();
    await user.keyboard("{ArrowLeft}");
    expect(briefResizer).toHaveAttribute("aria-valuenow", "544");

    await user.click(screen.getByRole("button", { name: "Watchlist" }));
    const queueResizer = screen.getByRole("separator", { name: "Resize watchlist company list" });
    expect(queueResizer).toHaveAttribute("aria-valuenow", "220");

    queueResizer.focus();
    await user.keyboard("{ArrowRight}");
    expect(queueResizer).toHaveAttribute("aria-valuenow", "244");
  });
});
