import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";

const baseAutopilotRun = {
  id: "run_drift_1",
  companyId: "company_does_not_matter",
  reportDocumentId: "doc_1",
  trigger: "scheduled",
  mode: "autopilot",
  status: "completed",
  stage: "notify",
  summaryText: "New report processed.",
  kpiDeltaJson: null,
  reportDiffRef: null,
  crossRefsJson: null,
  producedFactIds: [],
  notificationState: "unread",
  lastError: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

const baseFinancialFact = {
  id: "fact_run_1",
  companyId: "company_does_not_matter",
  periodId: "period_1",
  definitionId: "kpi_revenue",
  valueNumeric: "100",
  currency: "USD",
  statementBasis: "consolidated",
  attribution: "as_reported",
  variant: "reported",
  measureWindow: "quarter",
  dataQuality: "high",
  asReportedValue: null,
  asReportedScale: null,
  reportingStandard: null,
  extractionMethod: "ai",
  confidence: null,
  confirmationState: "auto_unreviewed",
  supersedesId: null,
  sourceDocumentRef: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

describe("Today/Pulse attention home (ADR 0054)", () => {
  it("renders the attention sections and the watchlist conviction rollup", async () => {
    renderApp({ section: "Today" });

    expect(await screen.findByRole("heading", { name: "Today" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "What changed" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "To verify" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Upcoming reports" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Conviction" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Recent activity" })).toBeInTheDocument();

    // The sample scenario pins one company; the rollup reports the tracked count.
    expect(screen.getByText(/Tracking 1 pinned of/)).toBeInTheDocument();
  });

  it("opens the Inbox from the secondary feed peek", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Today" });

    const activity = await screen.findByLabelText("Recent activity");
    await user.click(within(activity).getByRole("button", { name: "Open Inbox" }));

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
  });

  it("shows the 'Structure changed' drift for an autopilot run whose structured extraction drifted (ADR 0061 wave 2)", async () => {
    appTestState.autopilotRunsResponse = [
      {
        ...baseAutopilotRun,
        kpiDeltaJson: JSON.stringify({
          extractionAvailable: true,
          structured: true,
          structureChanged: true,
          driftJson: JSON.stringify({
            added_labels: ["Net profit from continuing operations"],
            removed_labels: ["Net profit"],
            unit_changed: null,
          }),
        }),
      },
    ];

    renderApp({ section: "Today" });

    const drift = await screen.findByRole("region", { name: "Structure changed" });
    expect(within(drift).getByText("Net profit from continuing operations")).toBeInTheDocument();
    expect(within(drift).getByText("Net profit", { exact: true })).toBeInTheDocument();
  });

  it("renders no drift section for a run whose delta never drifted (legacy shape tolerated)", async () => {
    appTestState.autopilotRunsResponse = [
      {
        ...baseAutopilotRun,
        id: "run_clean_1",
        kpiDeltaJson: JSON.stringify({ extractionAvailable: true, structured: true }),
      },
    ];

    renderApp({ section: "Today" });

    expect(await screen.findByText("New report processed.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Structure changed" })).not.toBeInTheDocument();
  });

  it("tolerates a malformed kpiDeltaJson blob without crashing or showing a drift section", async () => {
    appTestState.autopilotRunsResponse = [
      {
        ...baseAutopilotRun,
        id: "run_malformed_1",
        kpiDeltaJson: "{not valid json",
      },
    ];

    renderApp({ section: "Today" });

    expect(await screen.findByText("New report processed.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Structure changed" })).not.toBeInTheDocument();
  });

  it("renders no drift section for a legacy run with a null kpiDeltaJson", async () => {
    appTestState.autopilotRunsResponse = [
      {
        ...baseAutopilotRun,
        id: "run_null_delta_1",
        kpiDeltaJson: null,
      },
    ];

    renderApp({ section: "Today" });

    expect(await screen.findByText("New report processed.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Structure changed" })).not.toBeInTheDocument();
  });

  // Run-level undo (ADR 0055 §4, contracts.md `undo_autopilot_run`) — the
  // reversibility promise of auto-committed (`auto_unreviewed`) facts.
  describe("autopilot run undo (ADR 0055 §4)", () => {
    it("shows Undo for an autopilot run with committed facts, confirms two-step, calls the command, and shows the reverted state", async () => {
      const user = userEvent.setup();
      appTestState.financialFactsResponse = [baseFinancialFact];
      appTestState.autopilotRunsResponse = [
        { ...baseAutopilotRun, id: "run_undoable_1", producedFactIds: ["fact_run_1"] },
      ];

      renderApp({ section: "Today" });
      await screen.findByText("New report processed.");

      // The action is not a single click — InlineConfirm two-step.
      await user.click(screen.getByRole("button", { name: "Undo" }));
      expect(screen.getByText("Undo this run and revert its facts?")).toBeInTheDocument();
      await user.click(screen.getByRole("button", { name: "Undo" }));

      expect(await screen.findByText(/Reverted 1 fact/)).toBeInTheDocument();
      // The undo button disappears once the run has nothing left to revert.
      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    });

    it("lets Cancel back out of the two-step confirm without undoing anything", async () => {
      const user = userEvent.setup();
      appTestState.financialFactsResponse = [baseFinancialFact];
      appTestState.autopilotRunsResponse = [
        { ...baseAutopilotRun, id: "run_cancelled_1", producedFactIds: ["fact_run_1"] },
      ];

      renderApp({ section: "Today" });
      await screen.findByText("New report processed.");

      await user.click(screen.getByRole("button", { name: "Undo" }));
      await user.click(screen.getByRole("button", { name: "Cancel" }));

      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
      expect(screen.queryByText(/Reverted/)).not.toBeInTheDocument();
    });

    it("hides Undo for a run that has not produced any facts yet", async () => {
      appTestState.autopilotRunsResponse = [
        { ...baseAutopilotRun, id: "run_no_facts_1", producedFactIds: [] },
      ];

      renderApp({ section: "Today" });
      await screen.findByText("New report processed.");

      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    });

    it("hides Undo for an assist-mode run — its facts land pending, not auto-committed, so they go through the existing confirm/reject review instead", async () => {
      appTestState.financialFactsResponse = [baseFinancialFact];
      appTestState.autopilotRunsResponse = [
        { ...baseAutopilotRun, id: "run_assist_1", mode: "assist", producedFactIds: ["fact_run_1"] },
      ];

      renderApp({ section: "Today" });
      await screen.findByText("New report processed.");

      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    });
  });
});
