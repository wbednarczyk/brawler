import { describe, it } from "vitest";
import {
  appTestState,
  expect,
  initialCompanies,
  renderApp,
  screen,
  userEvent,
  within,
} from "../../test/appWorkflowHarness";
import type { FeedItem } from "../../api/types";

// The pinned company (minimal scenario pins companies[0]); autopilot runs scoped
// to it render a Review that opens its workspace.
const pinnedCompanyId = initialCompanies[0].id;

const baseAutopilotRun = {
  id: "run_drift_1",
  companyId: pinnedCompanyId,
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
  companyId: pinnedCompanyId,
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

/** A minimal report-type feed item (matches the "what changed" filter). */
function reportFeedItem(id: string, index: number): FeedItem {
  const ts = `2026-06-${String(10 + index).padStart(2, "0")}T09:00:00Z`;
  return {
    id,
    company: "GPW:ZZZ", // not in the registry → its Review falls back to the Inbox
    title: `Quarterly report ${index}`,
    type: "Official report",
    source: "GPW ESPI/EBI",
    sourceUrl: "https://example.test/report",
    attribution: "GPW",
    language: "pl",
    summary: "",
    bodyText: "",
    time: ts,
    publishedAt: ts,
    fetchedAt: ts,
    unread: true,
    saved: false,
    attachments: [],
  };
}

/** Categories present in the stream, in DOM order. */
function streamCategories(container: HTMLElement): string[] {
  return [...container.querySelectorAll("li[data-category]")].map(
    (el) => (el as HTMLElement).dataset.category ?? "",
  );
}

const PRIORITY: Record<string, number> = { autopilot: 0, verify: 1, changed: 2, upcoming: 3 };

describe("Today/Pulse — prioritized attention stream (J1, ADR 0076 U-Rb)", () => {
  it("merges all four categories into one stream in the fixed priority order", async () => {
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    const { container } = renderApp({ section: "Today" });

    // Wait for the async claim/feed loads to settle into stream rows.
    await screen.findByRole("button", { name: "Details" });
    await within(container as HTMLElement)
      .findAllByText(/report/i)
      .catch(() => []);

    const cats = streamCategories(container as HTMLElement);
    // All four attention categories are represented.
    expect(new Set(cats)).toEqual(new Set(["autopilot", "verify", "changed", "upcoming"]));
    // …and each category's rows are contiguous, in the fixed priority order.
    const idxs = cats.map((c) => PRIORITY[c]);
    expect(idxs).toEqual([...idxs].sort((a, b) => a - b));
  });

  it("gives every stream row a ticker, a type badge, a date and exactly one action button", async () => {
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    const { container } = renderApp({ section: "Today" });

    await screen.findByRole("button", { name: "Details" });

    const rows = [...container.querySelectorAll("li[data-category]")] as HTMLElement[];
    expect(rows.length).toBeGreaterThan(0);
    for (const row of rows) {
      // Exactly one primary (roving) action button per row.
      expect(row.querySelectorAll('[data-today-row="true"]').length).toBe(1);
      // A type badge (StatusChip) and a full date.
      expect(row.querySelector(".ui-status-chip")).not.toBeNull();
      expect(row.querySelector(".today-row-date")).not.toBeNull();
    }
  });

  it("keeps the App Today home reachable (heading stays 'Today')", async () => {
    renderApp({ section: "Today" });
    expect(await screen.findByRole("heading", { name: "Today" })).toBeInTheDocument();
  });
});

describe("Today counters column (ADR 0076 U-Rb D5)", () => {
  it("shows live counts for autopilot / to-verify / upcoming", async () => {
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    renderApp({ section: "Today" });

    const autopilotTile = await screen.findByRole("button", { name: /Autopilot/ });
    expect(within(autopilotTile).getByText("1")).toBeInTheDocument();
    // Minimal scenario seeds 2 claims to verify (1 due + 1 overdue) and 3 upcoming;
    // both load asynchronously, so the counts settle after the initial render.
    const verifyTile = screen.getByRole("button", { name: /To verify/ });
    const upcomingTile = screen.getByRole("button", { name: /Upcoming reports/ });
    expect(await within(verifyTile).findByText("2")).toBeInTheDocument();
    expect(await within(upcomingTile).findByText("3")).toBeInTheDocument();
  });

  it("filters the stream to a single category and restores it when toggled off", async () => {
    const user = userEvent.setup();
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    const { container } = renderApp({ section: "Today" });

    await screen.findByRole("button", { name: "Details" });
    expect(new Set(streamCategories(container as HTMLElement)).size).toBeGreaterThan(1);

    const verifyTile = screen.getByRole("button", { name: /To verify/ });
    await user.click(verifyTile);
    expect(verifyTile).toHaveAttribute("aria-pressed", "true");
    expect(new Set(streamCategories(container as HTMLElement))).toEqual(new Set(["verify"]));

    // Toggling the same tile off restores every category.
    await user.click(verifyTile);
    expect(verifyTile).toHaveAttribute("aria-pressed", "false");
    expect(new Set(streamCategories(container as HTMLElement)).size).toBeGreaterThan(1);
  });
});

describe("Today quiet state (ADR 0076 U-Rb D6)", () => {
  it("renders a single calm empty state when nothing needs attention", async () => {
    appTestState.autopilotRunsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];

    renderApp({ section: "Today" });

    expect(await screen.findByText("Nothing needs your attention.")).toBeInTheDocument();
  });

  it("still lists upcoming reports under the quiet state", async () => {
    appTestState.autopilotRunsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [
      {
        companyId: pinnedCompanyId,
        qualifiedTicker: "GPW:CDR",
        displayName: "CD PROJEKT",
        eventKey: "q1-2026",
        eventDate: "2026-06-30",
        eventTime: null,
        title: "Q1 2026 report",
        preparationStatus: "upcoming",
      },
    ];

    const { container } = renderApp({ section: "Today" });

    expect(await screen.findByText("Nothing needs your attention.")).toBeInTheDocument();
    await screen.findByText("CD PROJEKT");
    expect(streamCategories(container as HTMLElement)).toEqual(["upcoming"]);
  });
});

describe("Today caps and 'show all' links (ADR 0076 U-Rb D2)", () => {
  it("caps the what-changed category at 8 and offers a Show-all link into the Inbox", async () => {
    const user = userEvent.setup();
    appTestState.autopilotRunsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
    appTestState.feedItemsResponse = Array.from({ length: 10 }, (_, i) =>
      reportFeedItem(`feed_${i}`, i),
    );

    const { container } = renderApp({ section: "Today" });

    await screen.findByRole("button", { name: /Show all in Inbox/ });
    const changedRows = [...container.querySelectorAll('li[data-category="changed"]')];
    expect(changedRows.length).toBe(8);

    await user.click(screen.getByRole("button", { name: /Show all in Inbox/ }));
    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
  });
});

describe("Today stream keyboard navigation (ADR 0076 U-Rb D4)", () => {
  it("moves roving focus with j/k and triggers Review with Enter", async () => {
    const user = userEvent.setup();
    appTestState.autopilotRunsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
    appTestState.feedItemsResponse = [
      reportFeedItem("feed_a", 0),
      reportFeedItem("feed_b", 1),
      reportFeedItem("feed_c", 2),
    ];

    const { container } = renderApp({ section: "Today" });

    await screen.findByRole("button", { name: /Show all in Inbox/ }).catch(() => null);
    const buttons = [...container.querySelectorAll('[data-today-row="true"]')] as HTMLElement[];
    expect(buttons.length).toBeGreaterThanOrEqual(3);

    buttons[0].focus();
    expect(buttons[0]).toHaveFocus();
    await user.keyboard("j");
    expect(buttons[1]).toHaveFocus();
    await user.keyboard("k");
    expect(buttons[0]).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(buttons[1]).toHaveFocus();

    // Enter on the focused row triggers its Review — these feed items resolve to
    // no registered company, so Review opens the Inbox.
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
  });
});

describe("Today autopilot detail — undo / dismiss / drift behind the expandable (ADR 0055 §4, U-Rb D3)", () => {
  it("shows the 'Structure changed' drift once the run's detail is expanded (ADR 0061 wave 2)", async () => {
    const user = userEvent.setup();
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

    // The drift lives in the collapsed detail — expand the row first.
    await user.click(await screen.findByRole("button", { name: "Details" }));
    const drift = await screen.findByRole("region", { name: "Structure changed" });
    expect(within(drift).getByText("Net profit from continuing operations")).toBeInTheDocument();
    expect(within(drift).getByText("Net profit", { exact: true })).toBeInTheDocument();
  });

  it("renders no drift for a run whose delta never drifted (legacy shape tolerated)", async () => {
    const user = userEvent.setup();
    appTestState.autopilotRunsResponse = [
      {
        ...baseAutopilotRun,
        id: "run_clean_1",
        kpiDeltaJson: JSON.stringify({ extractionAvailable: true, structured: true }),
      },
    ];

    renderApp({ section: "Today" });

    await user.click(await screen.findByRole("button", { name: "Details" }));
    expect(await screen.findByText("New report processed.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Structure changed" })).not.toBeInTheDocument();
  });

  it("tolerates a malformed kpiDeltaJson blob without crashing or showing a drift section", async () => {
    const user = userEvent.setup();
    appTestState.autopilotRunsResponse = [
      { ...baseAutopilotRun, id: "run_malformed_1", kpiDeltaJson: "{not valid json" },
    ];

    renderApp({ section: "Today" });

    await user.click(await screen.findByRole("button", { name: "Details" }));
    expect(await screen.findByText("New report processed.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Structure changed" })).not.toBeInTheDocument();
  });

  describe("autopilot run undo (ADR 0055 §4)", () => {
    it("shows Undo behind the detail, confirms two-step, calls the command, and shows the reverted state", async () => {
      const user = userEvent.setup();
      appTestState.financialFactsResponse = [baseFinancialFact];
      appTestState.autopilotRunsResponse = [
        { ...baseAutopilotRun, id: "run_undoable_1", producedFactIds: ["fact_run_1"] },
      ];

      renderApp({ section: "Today" });
      await user.click(await screen.findByRole("button", { name: "Details" }));

      await user.click(screen.getByRole("button", { name: "Undo" }));
      expect(screen.getByText("Undo this run and revert its facts?")).toBeInTheDocument();
      await user.click(screen.getByRole("button", { name: "Undo" }));

      expect(await screen.findByText(/Reverted 1 fact/)).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    });

    it("lets Cancel back out of the two-step confirm without undoing anything", async () => {
      const user = userEvent.setup();
      appTestState.financialFactsResponse = [baseFinancialFact];
      appTestState.autopilotRunsResponse = [
        { ...baseAutopilotRun, id: "run_cancelled_1", producedFactIds: ["fact_run_1"] },
      ];

      renderApp({ section: "Today" });
      await user.click(await screen.findByRole("button", { name: "Details" }));

      await user.click(screen.getByRole("button", { name: "Undo" }));
      await user.click(screen.getByRole("button", { name: "Cancel" }));

      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
      expect(screen.queryByText(/Reverted/)).not.toBeInTheDocument();
    });

    it("hides Undo for an assist-mode run — its facts go through the confirm/reject review instead", async () => {
      const user = userEvent.setup();
      appTestState.financialFactsResponse = [baseFinancialFact];
      appTestState.autopilotRunsResponse = [
        { ...baseAutopilotRun, id: "run_assist_1", mode: "assist", producedFactIds: ["fact_run_1"] },
      ];

      renderApp({ section: "Today" });
      await user.click(await screen.findByRole("button", { name: "Details" }));

      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
    });
  });
});
