import { describe, it } from "vitest";
import { act, render } from "@testing-library/react";
import {
  expect,
  invoke,
  vi,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";
import { ResearchScreen, type ResearchScreenProps } from "./ResearchScreen";
import { ResearchProvider } from "../../app/state/screenViewModels";
import { ToolHostContext, type ToolHandle } from "../../shared/toolHost";
import { COMPANY_SPECS, makeCompany } from "../../test/scenarios/entities";

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

  // Regression for bug c80dabe: opening AI-analysis evidence set ONLY the Inbox
  // company filter, leaving every other filter stale — a leftover status filter
  // (or type/source/signal/search) silently hid the scoped feed (0 items on "All").
  it("opens AI-analysis evidence in the Inbox scoped to the company", async () => {
    const user = userEvent.setup();

    // Opening AI-analysis research evidence routes to the Inbox scoped to the
    // company via `scopeInboxToCompany`, which first clears any stale filters
    // (`clearInboxFilters`; the clear itself is covered by InboxScreen "clears
    // active inbox filters"). Reached here via the kept standalone Research
    // render (epic c793ca1).
    renderApp({ section: "Research" });

    const researchRegion = await screen.findByLabelText("Evidence timeline");
    const evidenceRows = await within(researchRegion).findAllByRole("article");
    const aiRow = evidenceRows.find((row) =>
      within(row).queryByText("AI-generated source-grounded summary."),
    );
    expect(aiRow).toBeDefined();
    await user.click(within(aiRow as HTMLElement).getByTitle("Open evidence"));

    // Lands in the Inbox scoped to CDR only — the report item (which a "Saved"
    // status filter would hide) is visible, i.e. no restrictive filter is active.
    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:CDR");
    expect(
      await screen.findByRole("button", {
        name: "Select feed item: Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();
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

    await user.click(await screen.findByRole("button", { name: "Add question" }));
    await user.type(
      await screen.findByLabelText("Question title", {}, { timeout: 5000 }),
      "What changed in the report?",
    );
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

  it("opens research question evidence in the Spółka research tool (not Notebooks)", async () => {
    const user = userEvent.setup();

    // Clicking a research_question evidence item opens the Spółka `research`
    // tool (F3a S3, ADR 0107 mapping "preset 'evidence'→research" — the
    // frozen cockpit's "evidence" preset is gone with the freeze, decision
    // 5), never Notebooks (epic c793ca1).
    renderApp({ section: "Research" });

    await user.click(await screen.findByRole("button", { name: "Add question" }));
    await user.type(
      await screen.findByLabelText("Question title", {}, { timeout: 5000 }),
      "Should backlog normalize?",
    );
    await user.click(screen.getByRole("button", { name: "Save question" }));

    await waitFor(() => {
      expect(screen.getAllByText("Should backlog normalize?").length).toBeGreaterThan(0);
    });

    const researchRegion = await screen.findByLabelText("Evidence timeline");
    // The new question's evidence row lands with the post-create refetch — a
    // later commit than the timeline region itself.
    const questionEvidenceTitle = await within(researchRegion).findByText(
      "Should backlog normalize?",
    );
    const questionEvidenceRow = questionEvidenceTitle.closest("article");
    expect(questionEvidenceRow).not.toBeNull();

    await user.click(within(questionEvidenceRow as HTMLElement).getByTitle("Open evidence"));

    // Landed on the Spółka screen with the research tool raised, not the
    // Notebooks screen and not the (now frozen) cockpit.
    const company = await screen.findByRole("region", { name: "Company view" });
    const tool = await within(company).findByRole("group", { name: "Workshop tool" });
    expect(tool).toHaveAttribute("data-tool", "research");
    expect(screen.queryByLabelText("Research cockpit")).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Notebooks" })).not.toBeInTheDocument();
  });

  it("confirms in place and deletes a selected research question", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

    await waitFor(() => {
      expect(screen.getAllByText("Will margins recover?").length).toBeGreaterThan(0);
    });

    // Cascading (ADR 0076 D5): confirm in place, then the delete fires.
    await user.click(screen.getByRole("button", { name: "Delete research question" }));
    expect(invoke).not.toHaveBeenCalledWith("delete_research_question", expect.anything());
    await user.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("delete_research_question", {
        id: "research_question_company_gpw_cdr_margin",
      });
    });
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

  // Owner decision 2026-08-26 (ADR 0107): the watchlist review queue's
  // company row gets an explicit "Open company" action that lands on that
  // company's Spółka screen through the guarded entry
  // (`openCompanyWorkspaceById`), never a direct state set — and it must
  // keep the row's existing select-into-the-queue behavior intact.
  it("Open company on a watchlist row lands on Spółka", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });
    await user.click(screen.getByRole("button", { name: "Watchlist" }));

    const reviewQueue = await screen.findByLabelText("Watchlist company review queue");
    await user.click(within(reviewQueue).getByRole("button", { name: "Open company" }));

    const spolka = await screen.findByRole("region", { name: "Company view" });
    expect(spolka).toHaveAttribute("data-company-id", "company_gpw_cdr");
  });

  // ADR 0084 decision 5 (clean cut): the research-brief/digest tables are
  // DROPPED, so the read-only archive the earlier slice kept has nothing to show
  // and is gone with them. Research keeps its deterministic surfaces.
  it("renders no saved-AI-research surface at all (ADR 0084 clean cut)", async () => {
    renderApp({ section: "Research" });

    // The deterministic research workspace still loads.
    await screen.findByLabelText("Evidence timeline");
    expect(screen.getByRole("heading", { name: "Research" })).toBeInTheDocument();

    // Every AI-research surface — archive included — is gone.
    expect(
      screen.queryByRole("heading", { name: "Saved AI research" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("No saved research brief.")).not.toBeInTheDocument();
    expect(screen.queryByText("No saved research digest.")).not.toBeInTheDocument();
    for (const name of ["Generate brief", "Generate digest"]) {
      expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    }

    // Nothing reached a retired research-AI command.
    const commands = vi.mocked(invoke).mock.calls.map(([command]) => command);
    for (const retired of [
      "list_research_briefs",
      "list_research_digests",
      "start_research_brief",
      "start_research_digest",
    ]) {
      expect(commands).not.toContain(retired);
    }
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

    await user.click(await screen.findByRole("button", { name: "Add reminder" }));
    await user.type(await screen.findByLabelText("Reminder title"), "Check next report");
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

  it("folds the review queue and questions behind count chips that expand in place (density)", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

    // U7-D density (ADR 0076 D6): below L / when short, secondary sections fold to
    // a clickable count chip (aria-expanded) that reveals the section in place; the
    // content stays in the DOM (CSS keys off the tier + data-expanded flag).
    const reviewChip = await screen.findByRole("button", { name: /Review queue/i });
    expect(reviewChip).toHaveAttribute("aria-expanded", "false");
    expect(reviewChip.getAttribute("aria-controls")).toBeTruthy();

    await user.click(reviewChip);
    expect(reviewChip).toHaveAttribute("aria-expanded", "true");

    const questionsChip = screen.getByRole("button", { name: /Research questions/i });
    expect(questionsChip).toHaveAttribute("aria-expanded", "false");
    await user.click(questionsChip);
    expect(questionsChip).toHaveAttribute("aria-expanded", "true");
  });

  it("resizes the watchlist queue with keyboard controls", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Research" });

    // The saved-AI-research panel and its resizer are gone (ADR 0084 clean cut);
    // the watchlist-queue resizer is the surviving split control.
    expect(
      screen.queryByRole("separator", { name: "Resize saved AI research panel" }),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Watchlist" }));
    const queueResizer = screen.getByRole("separator", { name: "Resize watchlist company list" });
    expect(queueResizer).toHaveAttribute("aria-valuenow", "220");

    queueResizer.focus();
    await user.keyboard("{ArrowRight}");
    expect(queueResizer).toHaveAttribute("aria-valuenow", "244");
  });
});

// R1 findings 1 (register the question/reminder drafts) and 10 (a hosted
// panel owns no landmark of its own — role="group" inside the Spółka
// workshop's ToolHost, ADR 0107). A lighter, isolated render (no `renderApp`
// scenario runtime needed) — `ResearchScreen` only reads `ResearchProvider`
// + `ToolHostContext`.
function researchStub(overrides: Partial<ResearchScreenProps> = {}): ResearchScreenProps {
  const company = makeCompany(COMPANY_SPECS.find((spec) => spec.key === "cdr")!);
  return {
    companies: [company],
    watchlists: [],
    watchlistMemberships: [],
    mode: "company",
    selectedCompanyId: company.id,
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
    ...overrides,
  };
}

describe("Research screen: hosted landmark + dirty draft (R1 findings 1, 10)", () => {
  it("renders its own landmark when NOT hosted in the Spółka workshop", async () => {
    render(
      <ResearchProvider value={researchStub()}>
        <ResearchScreen />
      </ResearchProvider>,
    );
    expect(await screen.findByRole("region", { name: "Research" })).toBeInTheDocument();
  });

  it("renders role=group, no landmark, when hosted inside the Spółka workshop's .spolka-tool", async () => {
    render(
      <div className="spolka-tool">
        <ResearchProvider value={researchStub()}>
          <ResearchScreen />
        </ResearchProvider>
      </div>,
    );
    expect(await screen.findByRole("group", { name: "Research" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Research" })).not.toBeInTheDocument();
  });

  it("registers a dirty draft with the tool host", async () => {
    const user = userEvent.setup();
    let handle: ToolHandle | null = null;
    const register = vi.fn((h: ToolHandle) => {
      handle = h;
      return () => {};
    });
    render(
      <ToolHostContext.Provider value={{ register }}>
        <ResearchProvider value={researchStub()}>
          <ResearchScreen />
        </ResearchProvider>
      </ToolHostContext.Provider>,
    );
    expect(register).toHaveBeenCalled();
    expect(handle!.isDirty()).toBe(false);

    await user.click(await screen.findByRole("button", { name: "Add reminder" }));
    await user.type(screen.getByLabelText("Reminder title"), "Check Q3 guidance");
    expect(handle!.isDirty()).toBe(true);

    act(() => {
      handle!.discard();
    });

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Add research reminder" })).not.toBeInTheDocument();
    });
  });
});
