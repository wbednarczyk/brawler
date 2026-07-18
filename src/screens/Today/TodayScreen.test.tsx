import { describe, it, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import {
  appTestState,
  expect,
  handleAppCommand,
  initialCompanies,
  invoke,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";
import type { AttentionEvent } from "../../api/attention";
import type { MorningBriefing } from "../../api/briefing";
import type { MorningBriefingItem } from "../../api/generated/MorningBriefingItem";
import type { FeedItem } from "../../api/types";

// The pinned company (minimal scenario pins companies[0]); autopilot runs scoped
// to it render a Review that opens its workspace.
const pinnedCompanyId = initialCompanies[0].id;

const baseAutopilotRun = {
  id: "run_drift_1",
  companyId: pinnedCompanyId,
  reportDocumentId: "doc_1",
  trigger: "scheduled",
  sweepId: null,
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

/** A minimal composed briefing item (ADR 0068 T5), overridable per test. */
function morningBriefingItem(overrides: Partial<MorningBriefingItem> = {}): MorningBriefingItem {
  return {
    id: "briefing_item_1",
    briefingId: "briefing_1",
    position: 0,
    itemType: "signal",
    companyId: pinnedCompanyId,
    domainDate: "2026-07-10",
    citationKey: "b1",
    evidenceType: "company_signal",
    evidenceRef: "sig_briefing_1",
    title: "Profit warning issued",
    detail: "Profit warning",
    createdAt: "2026-07-10T09:00:00Z",
    ...overrides,
  };
}

/** A minimal composed morning briefing (ADR 0068 T5), overridable per test. */
function morningBriefing(overrides: Partial<MorningBriefing> = {}): MorningBriefing {
  return {
    id: "briefing_1",
    composedAt: "2026-07-10T09:00:00Z",
    since: "2026-07-01",
    narrativeMarkdown: null,
    narrativeProviderId: null,
    narrativeModel: null,
    language: null,
    createdAt: "2026-07-10T09:00:00Z",
    items: [morningBriefingItem()],
    ...overrides,
  };
}

/** A minimal fired attention event (ADR 0068 T4), overridable per test. */
function attentionEvent(overrides: Partial<AttentionEvent> = {}): AttentionEvent {
  return {
    id: "attn_1",
    ruleId: "alert_rule_1",
    triggerType: "signal_category",
    companyId: pinnedCompanyId,
    evidenceType: "company_signal",
    evidenceRef: "signal_1",
    firedAt: "2026-06-10T09:00:00Z",
    seen: false,
    dismissed: false,
    ...overrides,
  };
}

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

/**
 * Await one stream category's rows to land. The verify (claims) and upcoming
 * (report-season) categories load on their own async effects — independent of
 * the autopilot "Details" row — so a test that asserts a category is present
 * must wait for THAT category, not just the first row to render, or it races
 * the slower load and intermittently sees the category dropped.
 */
async function findCategoryRow(container: HTMLElement, category: string): Promise<void> {
  await waitFor(() =>
    expect(container.querySelector(`li[data-category="${category}"]`)).not.toBeNull(),
  );
}

const PRIORITY: Record<string, number> = {
  autopilot: 0,
  attention: 1,
  verify: 2,
  changed: 3,
  upcoming: 4,
};

describe("Today/Pulse — prioritized attention stream (J1, ADR 0076 U-Rb)", () => {
  it("merges all four categories into one stream in the fixed priority order", async () => {
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    // Fired alerts (ADR 0068 T4) are their own describe block below; keep this
    // test to the four legacy categories it names.
    appTestState.attentionEventsResponse = [];
    const { container } = renderApp({ section: "Today" });

    // Wait for the async claim/feed loads to settle into stream rows. Each
    // category loads on its own effect, so wait for the two slower async ones
    // (verify = claims, upcoming = report season) explicitly — not just the
    // autopilot "Details" row — or the assertion races the verify load.
    await screen.findByRole("button", { name: "Details" });
    await findCategoryRow(container as HTMLElement, "verify");
    await findCategoryRow(container as HTMLElement, "upcoming");

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
    // Filtering TO verify only makes sense once the claims have loaded — await
    // the verify rows, not just the autopilot "Details" row (same race the
    // all-four-categories test guards against).
    await findCategoryRow(container as HTMLElement, "verify");
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
    appTestState.attentionEventsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];

    renderApp({ section: "Today" });

    expect(await screen.findByText("Nothing needs your attention.")).toBeInTheDocument();
  });

  it("still lists upcoming reports under the quiet state", async () => {
    appTestState.autopilotRunsResponse = [];
    appTestState.attentionEventsResponse = [];
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

/** The single `AlertRule` the attention-list tests join `attentionEvent`s against. */
const signalRule = {
  id: "alert_rule_1",
  triggerType: "signal_category" as const,
  signalCategory: "profit_warning",
  priceMin: null,
  priceMax: null,
  scopeType: "company" as const,
  scopeRef: pinnedCompanyId,
  enabled: true,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

/** Reads the qualified-ticker `aria-label` off a stream row's `TickerLabel`. */
function rowTicker(row: HTMLElement): string | null {
  return row.querySelector(".ticker-label")?.getAttribute("aria-label") ?? null;
}

describe("Today attention list — fired alerts (ADR 0068 T4)", () => {
  const otherCompanyId = initialCompanies[1].id;
  const pinnedTicker = initialCompanies[0].qualifiedTicker;
  const otherTicker = initialCompanies[1].qualifiedTicker;

  function seedOnlyAttention(events: ReturnType<typeof attentionEvent>[]) {
    appTestState.autopilotRunsResponse = [];
    appTestState.attentionEventsResponse = events;
    appTestState.alertRulesResponse = [signalRule];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
  }

  it("renders open attention events grouped by company with their rule's trigger context", async () => {
    seedOnlyAttention([
      attentionEvent({ id: "attn_other", companyId: otherCompanyId, evidenceRef: "signal_other" }),
      attentionEvent({ id: "attn_pinned_1", firedAt: "2026-06-09T09:00:00Z" }),
      attentionEvent({ id: "attn_pinned_2", firedAt: "2026-06-10T09:00:00Z" }),
    ]);

    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    const rows = [...container.querySelectorAll('li[data-category="attention"]')] as HTMLElement[];
    expect(rows).toHaveLength(3);
    // Grouped by company: the two `pinnedTicker` rows sit together, not split
    // by the `otherTicker` row landing between them.
    const tickers = rows.map(rowTicker);
    const pinnedIndexes = tickers.flatMap((ticker, i) => (ticker === pinnedTicker ? [i] : []));
    expect(pinnedIndexes).toEqual([pinnedIndexes[0], pinnedIndexes[0] + 1]);
    expect(tickers).toContain(otherTicker);
    // The rule's category is the "what fired" context, not just a bare
    // trigger-type label — and its human display name (D3 fix), never the
    // raw enum code the backend/rule actually stores.
    expect(within(rows[0]).getByText("Profit warning / estimate")).toBeInTheDocument();
    expect(within(rows[0]).queryByText(/^profit_warning$/)).not.toBeInTheDocument();
  });

  it("dismisses a fired event: the row disappears and the domain command fires", async () => {
    const user = userEvent.setup();
    // A distinct id per test (module-level toast dedup, see below, spans the
    // whole file — a shared id would be silently pre-toasted by an earlier test).
    seedOnlyAttention([attentionEvent({ id: "attn_dismiss_1" })]);
    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    // Scoped to the row itself — the same event also raised a persistent
    // toast with its own "Dismiss" control by now.
    const row = container.querySelector('li[data-category="attention"]') as HTMLElement;
    await user.click(within(row).getByRole("button", { name: "Dismiss" }));

    expect(container.querySelector('li[data-category="attention"]')).toBeNull();
    expect(invoke).toHaveBeenCalledWith(
      "dismiss_attention_event",
      expect.objectContaining({ input: expect.objectContaining({ id: "attn_dismiss_1" }) }),
    );
  });

  it("marks the event seen and opens its evidence on Review (company_signal → the company workspace)", async () => {
    seedOnlyAttention([attentionEvent({ id: "attn_seen_1" })]);
    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    // Scoped to the row itself — a persistent toast for this same event is
    // also on screen by now, carrying the same "profit_warning" text.
    const row = container.querySelector('li[data-category="attention"]') as HTMLElement;
    const reviewButton = within(row).getByRole("button", { name: "Review" });
    // `fireEvent`, not `userEvent`: no Today "Review" row action was ever
    // mouse-clicked in this suite before (only Enter-triggered via the roving
    // -focus keyboard tests) — userEvent's synthetic pointer sequence doesn't
    // reach this handler in jsdom even though the native click event does
    // (confirmed with a manual listener), a pre-existing gap unrelated to this
    // feature. `fireEvent.click` exercises the exact same onClick handler.
    fireEvent.click(reviewButton);

    expect(invoke).toHaveBeenCalledWith(
      "mark_attention_event_seen",
      expect.objectContaining({ input: expect.objectContaining({ id: "attn_seen_1" }) }),
    );
    // company_signal evidence opens the company's workspace (the cockpit
    // scoped to it) — the navigation target this Review actually invoked.
    expect(await screen.findByLabelText("Research cockpit")).toBeInTheDocument();
  });

  it("raises a persistent toast for a new unseen event, once per session (no duplicate on Today re-entry)", async () => {
    const user = userEvent.setup();
    seedOnlyAttention([attentionEvent({ id: "attn_toast_1" })]);
    renderApp({ section: "Today" });
    await screen.findByRole("alert");

    expect(screen.getAllByRole("alert")).toHaveLength(1);
    // The category's human display name (D3 fix), never the raw enum code.
    expect(
      screen.getByText(/Profit warning \/ estimate/, { selector: ".ui-toast-message" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/\bprofit_warning\b/, { selector: ".ui-toast-message" }),
    ).not.toBeInTheDocument();

    // Today unmounts on navigating away (`AppStateRoot` only renders it while
    // `activeSection === "Today"`) and remounts on return — the real "refresh"
    // this list has today. The already-shown event must not re-toast.
    await user.click(screen.getByRole("button", { name: "Inbox" }));
    await screen.findByRole("heading", { name: "Inbox" });
    await user.click(screen.getByRole("button", { name: "Today" }));
    await screen.findByRole("heading", { name: "Today" });

    expect(screen.getAllByRole("alert")).toHaveLength(1);
  });

  it("resolves the company id to its ticker even when the attention list loads before the company list (live defect: raw 'company_gpw_scw' id shown)", async () => {
    // Live-defect fix (v0.57 fix wave 2, D2, owner screenshot 27-toast-stack.png):
    // `companies` and the attention-events/alert-rules pair are two
    // independent fetches (AppStateRoot's own company load vs
    // `useTodayPulse`'s). If the attention fetch settles first, the
    // toast-raising effect built its `companyById` map off the still-empty
    // `companies` array and permanently baked the raw company id into the
    // toast message — the module-scoped `toastedAttentionEventIds` dedup Set
    // means it is never retried once the real company list arrives. Force
    // that exact ordering: hold `list_companies` open until the attention
    // event has already had a chance to fire, then release it.
    seedOnlyAttention([attentionEvent({ id: "attn_race_company_1" })]);

    let releaseCompanies: (() => void) | undefined;
    const companiesGate = new Promise<void>((resolve) => {
      releaseCompanies = resolve;
    });
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === "list_companies") {
        await companiesGate;
      }
      return handleAppCommand(command as string, args as Record<string, unknown> | undefined);
    });

    const { container } = renderApp({ section: "Today" });

    // Attention data can settle first, but the toast must not fire — and must
    // not mark the event toasted — until the company list is actually usable.
    await findCategoryRow(container as HTMLElement, "attention");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();

    releaseCompanies?.();

    // The toast must show the resolved ticker, never the raw internal id.
    expect(
      await screen.findByText(new RegExp(`^${pinnedTicker} —`), { selector: ".ui-toast-message" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(new RegExp(pinnedCompanyId), { selector: ".ui-toast-message" }),
    ).not.toBeInTheDocument();
  });

  it("renders no attention rows when there are nothing fired (same stream, no special sub-state)", async () => {
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    appTestState.attentionEventsResponse = [];

    const { container } = renderApp({ section: "Today" });
    await screen.findByRole("button", { name: "Details" });

    expect(container.querySelector('li[data-category="attention"]')).toBeNull();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});

// D3 fix (v0.57 fix wave 2, owner screenshot 27-toast-stack.png): the toast's
// reconciliation message and its Review/Dismiss actions are `text()` literals
// with real `plText` entries already — but nothing exercised them in the
// Polish locale end to end, so a regression here (or in how `locale` reaches
// this screen) would have slipped through unnoticed exactly like D1/D2 did.
describe("Today attention toast — Polish locale content (D3 fix)", () => {
  it("renders the reconciliation message and Review/Dismiss actions in Polish, not English", async () => {
    appTestState.settingsResponse = { ...appTestState.settingsResponse, locale: "pl" };
    appTestState.autopilotRunsResponse = [];
    appTestState.attentionEventsResponse = [
      attentionEvent({
        id: "attn_reconciliation_pl_1",
        triggerType: "source_reconciliation",
        evidenceType: "source_reconciliation",
        ruleId: null,
      }),
    ];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];

    renderApp({ section: "Today" });

    const alert = await screen.findByRole("alert");
    expect(
      within(alert).getByText(/Raport oficjalny pominięty/, { selector: ".ui-toast-message" }),
    ).toBeInTheDocument();
    expect(within(alert).getByRole("button", { name: "Przejrzyj" })).toBeInTheDocument();
    expect(within(alert).getByRole("button", { name: "Odrzuć" })).toBeInTheDocument();
    expect(screen.queryByText(/Official report missed/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Review" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Dismiss" })).not.toBeInTheDocument();
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

describe("Today morning-briefing card (ADR 0068 decision 4, v0.54 §T5)", () => {
  it("shows an empty state with no briefing yet, and Generate enqueues + refetches the result", async () => {
    const user = userEvent.setup();
    appTestState.morningBriefingResponse = null;
    const { container } = renderApp({ section: "Today" });

    expect(
      await screen.findByText("No briefing yet. Generate one to see what's changed."),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Generate briefing" }));

    expect(invoke).toHaveBeenCalledWith("generate_morning_briefing");
    await waitFor(() => {
      expect(container.querySelector(".today-briefing-items")).not.toBeNull();
    });
    expect(
      screen.queryByText("No briefing yet. Generate one to see what's changed."),
    ).not.toBeInTheDocument();
  });

  it("renders the structured item list when no narrative was phrased, and Review opens the item's evidence", async () => {
    appTestState.morningBriefingResponse = morningBriefing({
      narrativeMarkdown: null,
      items: [morningBriefingItem({ title: "Profit warning issued", detail: "Profit warning" })],
    });
    renderApp({ section: "Today" });

    const row = (await screen.findByText("Profit warning issued — Profit warning")).closest("li");
    expect(row).not.toBeNull();

    fireEvent.click(within(row as HTMLElement).getByRole("button", { name: "Review" }));

    // `signal` items open the evidence company's Feed (the company workspace).
    expect(await screen.findByLabelText("Research cockpit")).toBeInTheDocument();
  });

  it("renders the narrative variant with a citation source that click-throughs to its evidence", async () => {
    appTestState.morningBriefingResponse = morningBriefing({
      narrativeMarkdown: "## Signals\n\nCD PROJEKT reported a profit warning [b1]",
      items: [morningBriefingItem({ title: "Profit warning issued", citationKey: "b1" })],
    });
    renderApp({ section: "Today" });

    expect(await screen.findByText(/CD PROJEKT reported a profit warning/)).toBeInTheDocument();
    const sourceButton = await screen.findByRole("button", { name: "Profit warning issued" });

    fireEvent.click(sourceButton);

    expect(await screen.findByLabelText("Research cockpit")).toBeInTheDocument();
  });
});
