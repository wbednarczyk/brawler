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
import { COMPANY_SPECS, makeClaimToVerify, makeSourceAdapter } from "../../test/scenarios/entities";
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
  // Typed severity (ADR 0087 dec. 2) now lands on the payload from D3a; the Today
  // frontend still routes via the placeholder adapter until D3a swaps the call
  // sites, so this fixture value is inert for these tests.
  severity: "routine" as const,
  // Null by default (legacy/tolerant fallback: the summary stays the statement);
  // the D6 tests set a title to prove the document identity leads the row.
  reportDocumentTitle: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

const baseFinancialFact = {
  id: "fact_run_1",
  companyId: pinnedCompanyId,
  periodId: "period_1",
  definitionId: "kpi_revenue",
  metricKey: "revenue",
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
  annotation: null,
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
    // Typed severity now on the payload (ADR 0087 dec. 2); inert here — the Today
    // frontend routes via the placeholder adapter until D3a swaps the call sites.
    severity: "notable",
    // Evidence specifics (v0.60 D6): null by default so the category-display
    // fallback still applies; the D6 tests set a title to prove the concrete
    // statement leads the row.
    evidenceTitle: null,
    evidenceDetail: null,
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

/** Severity rank (urgent leads), read from each row's `data-severity` attribute. */
const SEVERITY_RANK: Record<string, number> = { urgent: 0, notable: 1, routine: 2 };

/** The `data-severity` of each top-level stream row, in DOM order. */
function streamSeverities(container: HTMLElement): string[] {
  return [...container.querySelectorAll("li[data-category]")].map(
    (el) => (el as HTMLElement).dataset.severity ?? "routine",
  );
}

describe("Today/Pulse — severity-ranked attention stream (J1, ADR 0087)", () => {
  it("merges every category into one severity-ranked stream (notable leads routine)", async () => {
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    // A fired alert is `notable` under the placeholder adapter; every other
    // category is `routine` — so the attention row must lead the routine rows
    // regardless of recency (ADR 0087 dec. 1c severity-first ranking).
    appTestState.attentionEventsResponse = [attentionEvent({ id: "attn_merge_1" })];
    appTestState.alertRulesResponse = [signalRule];
    const { container } = renderApp({ section: "Today" });

    // Each category loads on its own effect; wait for the slower async ones.
    await screen.findByRole("button", { name: "Details" });
    await findCategoryRow(container as HTMLElement, "verify");
    await findCategoryRow(container as HTMLElement, "upcoming");
    await findCategoryRow(container as HTMLElement, "attention");

    const cats = streamCategories(container as HTMLElement);
    expect(new Set(cats)).toEqual(
      new Set(["autopilot", "attention", "verify", "changed", "upcoming"]),
    );
    // Severity rank is monotonic non-decreasing down the stream.
    const ranks = streamSeverities(container as HTMLElement).map((s) => SEVERITY_RANK[s]);
    expect(ranks).toEqual([...ranks].sort((a, b) => a - b));
    // The single notable (attention) row leads every routine row.
    expect(ranks.lastIndexOf(1)).toBeLessThan(ranks.indexOf(2));
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

// The single claim fixture the overdue-escalation tests seed. Its bucket is set
// by which slot (`due`/`overdue`) it lands in — the frontend derives severity
// from that (verifyClaimSeverity), not from anything on the claim itself.
const sampleClaimToVerify = makeClaimToVerify(COMPANY_SPECS[0]);

/** The `data-severity` of the single top-level verify row, or null if absent. */
function verifyRowSeverity(container: HTMLElement): string | null {
  const row = container.querySelector('li[data-category="verify"]') as HTMLElement | null;
  return row?.dataset.severity ?? null;
}

describe("Today verify severity — the one FE-side severity entry (ADR 0087 dec. 2, product-spec §Attention Routing)", () => {
  it("escalates an OVERDUE claim's verify row to notable, ranked above routine and below urgent", async () => {
    // Routine autopilot + an URGENT attention row bracket the overdue claim.
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    appTestState.attentionEventsResponse = [
      attentionEvent({ id: "attn_overdue_rank", severity: "urgent" }),
    ];
    appTestState.alertRulesResponse = [signalRule];
    appTestState.claimsToVerifyResponse = {
      due: [],
      overdue: [sampleClaimToVerify],
      upcoming: [],
    };
    const { container } = renderApp({ section: "Today" });

    await screen.findByRole("button", { name: "Details" });
    await findCategoryRow(container as HTMLElement, "verify");
    await findCategoryRow(container as HTMLElement, "attention");

    // The overdue claim escalated to notable (was routine before ADR 0087's
    // FE-side entry).
    expect(verifyRowSeverity(container as HTMLElement)).toBe("notable");

    // Ordering: the urgent attention row precedes the notable verify row, which
    // precedes the routine autopilot row.
    const cats = streamCategories(container as HTMLElement);
    const sevs = streamSeverities(container as HTMLElement);
    const urgentIndex = sevs.indexOf("urgent");
    const verifyIndex = cats.indexOf("verify");
    const routineIndex = sevs.lastIndexOf("routine");
    expect(urgentIndex).toBeGreaterThanOrEqual(0);
    expect(urgentIndex).toBeLessThan(verifyIndex);
    expect(verifyIndex).toBeLessThan(routineIndex);
  });

  it("keeps a DUE-but-not-yet-overdue claim's verify row routine", async () => {
    appTestState.autopilotRunsResponse = [];
    appTestState.attentionEventsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.reportSeasonUpcomingResponse = [];
    appTestState.claimsToVerifyResponse = {
      due: [sampleClaimToVerify],
      overdue: [],
      upcoming: [],
    };
    const { container } = renderApp({ section: "Today" });

    await findCategoryRow(container as HTMLElement, "verify");
    expect(verifyRowSeverity(container as HTMLElement)).toBe("routine");
  });

  it("never folds the escalated (notable) overdue claim into a routine aggregate, and raises no toast for it", async () => {
    // >3 routine autopilot runs across distinct companies collapse into an
    // aggregate; the notable overdue claim must survive as its own verify row.
    appTestState.autopilotRunsResponse = Array.from({ length: 4 }, (_, i) => ({
      ...baseAutopilotRun,
      id: `run_agg_${i}`,
      companyId: `company_agg_${i}`,
    }));
    appTestState.attentionEventsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.reportSeasonUpcomingResponse = [];
    appTestState.claimsToVerifyResponse = {
      due: [],
      overdue: [sampleClaimToVerify],
      upcoming: [],
    };
    const { container } = renderApp({ section: "Today" });

    await findCategoryRow(container as HTMLElement, "verify");

    // Exactly one verify row, escalated, standing on its own — a lone notable
    // claim is below the notable-wall threshold, so it never folds into an
    // aggregate (and a notable row never joins the routine aggregate anyway).
    const verifyRows = container.querySelectorAll('li[data-category="verify"]');
    expect(verifyRows).toHaveLength(1);
    expect((verifyRows[0] as HTMLElement).dataset.severity).toBe("notable");
    // The routine autopilot rows DID collapse (proof the aggregate stage ran).
    await waitFor(() =>
      expect(container.querySelector(".today-group-chip")).not.toBeNull(),
    );
    // A claim is not an attention event — it raises no toast (persistent or not).
    expect(screen.queryByRole("alert")).toBeNull();
    expect(container.querySelector(".ui-toast-message")).toBeNull();
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

  function seedOnlyAttention(events: ReturnType<typeof attentionEvent>[]) {
    appTestState.autopilotRunsResponse = [];
    appTestState.attentionEventsResponse = events;
    appTestState.alertRulesResponse = [signalRule];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
  }

  it("collapses multiple fired events for one company into a ×N group row, expandable in place (ADR 0087 dec. 1b)", async () => {
    const user = userEvent.setup();
    seedOnlyAttention([
      attentionEvent({ id: "attn_other", companyId: otherCompanyId, evidenceRef: "signal_other", firedAt: "2026-06-08T09:00:00Z" }),
      attentionEvent({ id: "attn_pinned_1", firedAt: "2026-06-09T09:00:00Z" }),
      attentionEvent({ id: "attn_pinned_2", firedAt: "2026-06-10T09:00:00Z" }),
    ]);

    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    // Two top-level rows: the other company (×1) and the pinned company's ×2 group.
    const rows = [...container.querySelectorAll('li[data-category="attention"]')] as HTMLElement[];
    expect(rows).toHaveLength(2);
    // The per-company count chip carries its unit so "×2" is not opaque (owner
    // dogfooding 2026-07-23): attention groups count fired events.
    const group = rows.find((row) => within(row).queryByText("×2 events")) as HTMLElement;
    expect(group).toBeTruthy();
    expect(within(group).queryByText("×2")).toBeNull();
    expect(rowTicker(group)).toBe(pinnedTicker);
    // The group header shows the newest member's rule context — its human display
    // name, never the raw enum code.
    expect(within(group).getByText("Profit warning / estimate")).toBeInTheDocument();
    expect(within(group).queryByText(/^profit_warning$/)).not.toBeInTheDocument();
    expect([...container.querySelectorAll('[data-member-category]')].length).toBe(0);

    // Members expand in place — one compact member row per event.
    await user.click(within(group).getByRole("button", { name: "Details" }));
    expect(group.querySelectorAll('[data-member-category="attention"]')).toHaveLength(2);
    // Each member keeps its own single Review action (so j/k traverses members).
    expect(group.querySelectorAll('[data-today-row="true"]').length).toBe(3);
    expect(container.querySelector('[data-counter="urgent"]')).not.toBeNull();
  });

  it("labels a per-company autopilot group chip with its 'runs' unit", async () => {
    // Same opaque-×N fix, autopilot side: a per-company group of runs counts runs,
    // not events (owner dogfooding 2026-07-23).
    appTestState.autopilotRunsResponse = [
      { ...baseAutopilotRun, id: "run_grp_0", companyId: pinnedCompanyId },
      { ...baseAutopilotRun, id: "run_grp_1", companyId: pinnedCompanyId },
    ];
    appTestState.attentionEventsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];

    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "autopilot");

    const rows = [...container.querySelectorAll('li[data-category="autopilot"]')] as HTMLElement[];
    expect(rows).toHaveLength(1);
    expect(within(rows[0]).getByText("×2 runs")).toBeInTheDocument();
  });

  it("dismisses a fired event: the row disappears and the domain command fires", async () => {
    const user = userEvent.setup();
    // A distinct id per test (module-level toast dedup, see below, spans the
    // whole file — a shared id would be silently pre-toasted by an earlier test).
    seedOnlyAttention([attentionEvent({ id: "attn_dismiss_1" })]);
    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    // Scoped to the row itself — the default event is `notable`, so it also raised
    // a transient toast (no Dismiss control of its own) by now.
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

    // Scoped to the row itself — a transient toast for this same (notable) event
    // is also on screen by now, carrying the same "profit_warning" text.
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

  it("raises a persistent toast for a new unseen URGENT event, once per session (no duplicate on Today re-entry)", async () => {
    const user = userEvent.setup();
    // Persistent toasts are reserved for `urgent` events (ADR 0087 dec. 3).
    seedOnlyAttention([attentionEvent({ id: "attn_toast_1", severity: "urgent" })]);
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

  it("raises a TRANSIENT (auto-dismissing) toast for a NOTABLE event, never a persistent one (ADR 0087 dec. 3)", async () => {
    seedOnlyAttention([attentionEvent({ id: "attn_notable_1", severity: "notable" })]);
    renderApp({ section: "Today" });

    // Notable → a transient toast (`role="status"`, `caution` tone), announcing
    // the landing; it is NOT the persistent (`role="alert"`) variant, and the
    // toast itself carries no Dismiss control (the stream row stays the place to act).
    const message = await screen.findByText(/Profit warning \/ estimate/, {
      selector: ".ui-toast-message",
    });
    const toast = message.closest(".ui-toast") as HTMLElement;
    expect(toast).not.toBeNull();
    expect(toast.className).toContain("ui-toast-caution");
    expect(toast.className).not.toContain("ui-toast-persistent");
    expect(toast.querySelector('[role="status"]')).not.toBeNull();
    expect(within(toast).queryByRole("alert")).toBeNull();
    // The transient toast has no Dismiss of its own (only persistent toasts do).
    expect(within(toast).queryByRole("button", { name: "Dismiss" })).toBeNull();
  });

  it("raises NO toast for a ROUTINE event — stream only (ADR 0087 dec. 3)", async () => {
    seedOnlyAttention([attentionEvent({ id: "attn_routine_1", severity: "routine" })]);
    const { container } = renderApp({ section: "Today" });
    // The routine event still renders its stream row…
    await findCategoryRow(container as HTMLElement, "attention");
    // …but never surfaces as a toast (persistent or transient).
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(container.ownerDocument.querySelector(".ui-toast-message")).toBeNull();
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
        // Missed-report reconciliation → urgent, so it raises the persistent
        // (role="alert") toast this test asserts (ADR 0087 dec. 3).
        severity: "urgent",
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
    // Distinct companies so grouping (ADR 0087) does not collapse them — this
    // proves the pre-group item cap, not the grouping.
    appTestState.feedItemsResponse = Array.from({ length: 10 }, (_, i) => ({
      ...reportFeedItem(`feed_${i}`, i),
      company: `GPW:Z${i}`,
    }));

    const { container } = renderApp({ section: "Today" });

    await screen.findByRole("button", { name: /Show all in Inbox/ });
    // All routine, distinct companies, >3 → they collapse into one cross-company
    // aggregate; the pre-group cap still bounds it to 8 members (10 total > cap).
    const changedRows = [...container.querySelectorAll('li[data-category="changed"]')] as HTMLElement[];
    expect(changedRows).toHaveLength(1);
    const aggregate = changedRows[0];
    expect(within(aggregate).getByText(/×8/)).toBeInTheDocument();
    await user.click(within(aggregate).getByRole("button", { name: "Details" }));
    expect(aggregate.querySelectorAll('[data-member-category="changed"]')).toHaveLength(8);

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
    // Distinct companies so grouping keeps three separate rows to rove across.
    appTestState.feedItemsResponse = [
      { ...reportFeedItem("feed_a", 0), company: "GPW:D0" },
      { ...reportFeedItem("feed_b", 1), company: "GPW:D1" },
      { ...reportFeedItem("feed_c", 2), company: "GPW:D2" },
    ];

    const { container } = renderApp({ section: "Today" });

    await findCategoryRow(container as HTMLElement, "changed");
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

  describe("rows state WHAT concretely happened (v0.60 D6, owner dogfooding 2026-07-23)", () => {
    it("an autopilot run leads with its report document title; the summary drops to the sub-line", async () => {
      appTestState.autopilotRunsResponse = [
        {
          ...baseAutopilotRun,
          id: "run_titled_1",
          reportDocumentTitle: "Skonsolidowany raport kwartalny Q2 2026",
        },
      ];

      const { container } = renderApp({ section: "Today" });

      const title = await screen.findByText("Skonsolidowany raport kwartalny Q2 2026");
      expect(title).toHaveClass("today-row-title");
      // The token summary is no longer the bare statement — it moves to the sub-line.
      const sub = container.querySelector(".today-row-sub");
      expect(sub?.textContent).toBe("New report processed.");
    });

    it("splits a filename glued onto an autopilot document title: statement leads, filename is a secondary link", async () => {
      appTestState.autopilotRunsResponse = [
        {
          ...baseAutopilotRun,
          id: "run_glued_1",
          reportDocumentTitle:
            "Y24_25_Sprawozdanie jednostkowe.xhtmlJednostkowe Sprawozdanie Finansowe AB S.A.",
        },
      ];

      const { container } = renderApp({ section: "Today" });

      // The human statement is the row title — no filename glued into it.
      const title = await screen.findByText("Jednostkowe Sprawozdanie Finansowe AB S.A.");
      expect(title).toHaveClass("today-row-title");
      expect(container.querySelector(".today-row-title")?.textContent).not.toContain(".xhtml");
      // The filename moves to a keyboard-reachable secondary link line.
      const link = screen.getByRole("button", {
        name: /Y24_25_Sprawozdanie jednostkowe\.xhtml/,
      });
      expect(link).toHaveClass("today-row-doc-link");
    });

    it("renders a filename-only autopilot title as generic statement + filename link", async () => {
      appTestState.autopilotRunsResponse = [
        {
          ...baseAutopilotRun,
          id: "run_fileonly_1",
          summaryText: "New report processed.",
          reportDocumentTitle: "2410_Passus_2023_PSSF_MSSF_skrócone_PL-sig.pdf",
        },
      ];

      const { container } = renderApp({ section: "Today" });

      // Filename-only → the generic summary is the statement; the filename links out.
      const title = await screen.findByText("New report processed.");
      expect(title).toHaveClass("today-row-title");
      expect(container.querySelector(".today-row-title")?.textContent).not.toContain(".pdf");
      expect(
        screen.getByRole("button", { name: /2410_Passus_2023_PSSF_MSSF_skrócone_PL-sig\.pdf/ }),
      ).toBeInTheDocument();
    });

    it("a signal attention row states its own filing title, never the bare category", async () => {
      appTestState.attentionEventsResponse = [
        attentionEvent({
          id: "attn_titled_1",
          evidenceTitle: "Wstępne wyniki produkcyjne i sprzedażowe za czerwiec 2026",
        }),
      ];
      appTestState.alertRulesResponse = [signalRule];

      renderApp({ section: "Today" });

      expect(
        await screen.findByText("Wstępne wyniki produkcyjne i sprzedażowe za czerwiec 2026"),
      ).toBeInTheDocument();
    });

    it("splits a filename off an attention row's evidence title (ABE-style completion row)", async () => {
      appTestState.attentionEventsResponse = [
        attentionEvent({
          id: "attn_glued_1",
          evidenceTitle:
            "Y24_25_Sprawozdanie jednostkowe.xhtmlJednostkowe Sprawozdanie Finansowe AB S.A.",
        }),
      ];
      appTestState.alertRulesResponse = [signalRule];

      const { container } = renderApp({ section: "Today" });

      const title = await screen.findByText("Jednostkowe Sprawozdanie Finansowe AB S.A.");
      expect(title).toHaveClass("today-row-title");
      expect(
        screen.getByRole("button", { name: /Y24_25_Sprawozdanie jednostkowe\.xhtml/ }),
      ).toHaveClass("today-row-doc-link");
      expect(container.querySelector(".today-row-title")?.textContent).not.toContain(".xhtml");
    });

    it("a reconciliation row names the missed report and the registry that caught it", async () => {
      appTestState.attentionEventsResponse = [
        attentionEvent({
          id: "attn_recon_1",
          ruleId: null,
          triggerType: "source_reconciliation",
          evidenceType: "source_reconciliation",
          evidenceRef: "recon_1",
          severity: "urgent",
          evidenceTitle: "Raport bieżący 15/2026 — zawarcie znaczącej umowy",
          evidenceDetail: "GPW ESPI/EBI",
        }),
      ];

      renderApp({ section: "Today" });

      expect(
        await screen.findByText(
          "Raport bieżący 15/2026 — zawarcie znaczącej umowy — missed by the primary source, backfilled from GPW ESPI/EBI",
        ),
      ).toBeInTheDocument();
    });
  });

  describe("alert-rule origin indicator (owner dogfooding 2026-07-23)", () => {
    it("marks a rule-fired event row and leaves a system reconciliation row unmarked", async () => {
      appTestState.attentionEventsResponse = [
        attentionEvent({ id: "attn_from_rule", evidenceTitle: "Dywidenda 2026" }),
        attentionEvent({
          id: "attn_system",
          companyId: initialCompanies[1].id,
          ruleId: null,
          triggerType: "source_reconciliation",
          evidenceType: "source_reconciliation",
          evidenceRef: "recon_sys",
          evidenceTitle: "Raport bieżący 9/2026",
          evidenceDetail: "GPW ESPI/EBI",
        }),
      ];
      appTestState.alertRulesResponse = [signalRule];

      const { container } = renderApp({ section: "Today" });
      await findCategoryRow(container as HTMLElement, "attention");

      const rows = [...container.querySelectorAll('li[data-category="attention"]')] as HTMLElement[];
      const ruleRow = rows.find((row) => within(row).queryByText("Dywidenda 2026")) as HTMLElement;
      const systemRow = rows.find((row) =>
        within(row).queryByText(/Raport bieżący 9\/2026/),
      ) as HTMLElement;

      expect(within(ruleRow).getByRole("img", { name: "From your alert rule" })).toBeInTheDocument();
      expect(within(systemRow).queryByRole("img", { name: "From your alert rule" })).toBeNull();
    });

    it("hides the indicator on a mixed group header while each member keeps its own", async () => {
      const user = userEvent.setup();
      appTestState.attentionEventsResponse = [
        attentionEvent({
          id: "attn_mix_rule",
          evidenceTitle: "Wezwanie do zapisywania akcji",
          firedAt: "2026-06-11T09:00:00Z",
        }),
        attentionEvent({
          id: "attn_mix_system",
          ruleId: null,
          triggerType: "source_reconciliation",
          evidenceType: "source_reconciliation",
          evidenceRef: "recon_mix",
          evidenceTitle: "Raport bieżący 12/2026",
          evidenceDetail: "GPW ESPI/EBI",
          firedAt: "2026-06-10T09:00:00Z",
        }),
      ];
      appTestState.alertRulesResponse = [signalRule];

      const { container } = renderApp({ section: "Today" });
      await findCategoryRow(container as HTMLElement, "attention");

      const rows = [...container.querySelectorAll('li[data-category="attention"]')] as HTMLElement[];
      expect(rows).toHaveLength(1);
      const group = rows[0];
      // Header: not every member is rule-fired → no indicator on the collapsed head.
      const head = group.querySelector(".today-row-head") as HTMLElement;
      expect(within(head).queryByRole("img", { name: "From your alert rule" })).toBeNull();

      // Expanded: the rule-fired member carries its own indicator; the system one does not.
      await user.click(within(group).getByRole("button", { name: "Details" }));
      const members = [
        ...group.querySelectorAll('[data-member-category="attention"]'),
      ] as HTMLElement[];
      const ruleMember = members.find((m) =>
        within(m).queryByText("Wezwanie do zapisywania akcji"),
      ) as HTMLElement;
      const systemMember = members.find((m) =>
        within(m).queryByText(/Raport bieżący 12\/2026/),
      ) as HTMLElement;
      expect(
        within(ruleMember).getByRole("img", { name: "From your alert rule" }),
      ).toBeInTheDocument();
      expect(
        within(systemMember).queryByRole("img", { name: "From your alert rule" }),
      ).toBeNull();
    });
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

    it("shows Undo for an assist-mode run too — facts are review-free, so both modes commit and undo the same way (ADR 0086 dec. 5)", async () => {
      const user = userEvent.setup();
      appTestState.financialFactsResponse = [baseFinancialFact];
      appTestState.autopilotRunsResponse = [
        { ...baseAutopilotRun, id: "run_assist_1", mode: "assist", producedFactIds: ["fact_run_1"] },
      ];

      renderApp({ section: "Today" });
      await user.click(await screen.findByRole("button", { name: "Details" }));

      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
    });
  });
});

describe("Today v0.60 — four counter tiles + per-category error strips (ADR 0087)", () => {
  it("renders four tiles led by Pilne; a seeded urgent event lights the tile and the Pilne filter shows only urgent rows", async () => {
    const user = userEvent.setup();
    // A routine autopilot run plus one URGENT attention event (real payload
    // severity, ADR 0087 dec. 2) — so the Pilne tile counts 1 and its filter keeps
    // only the urgent attention row while hiding the routine autopilot row.
    appTestState.autopilotRunsResponse = [{ ...baseAutopilotRun }];
    appTestState.attentionEventsResponse = [
      attentionEvent({ id: "attn_pilne_urgent_1", severity: "urgent" }),
    ];
    appTestState.alertRulesResponse = [signalRule];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
    const { container } = renderApp({ section: "Today" });
    await screen.findByRole("button", { name: "Details" });

    const tiles = [...container.querySelectorAll("[data-counter]")].map(
      (el) => (el as HTMLElement).dataset.counter,
    );
    expect(tiles).toEqual(["urgent", "autopilot", "verify", "upcoming"]);

    // The Pilne tile reflects the one urgent row.
    const pilne = screen.getByRole("button", { name: /Urgent/ });
    expect(within(pilne).getByText("1")).toBeInTheDocument();

    await user.click(pilne);
    expect(pilne).toHaveAttribute("aria-pressed", "true");
    // Filtered to urgent: only the urgent attention row remains, the routine
    // autopilot row is filtered out.
    const urgentRows = [...container.querySelectorAll("li[data-category]")] as HTMLElement[];
    expect(urgentRows).toHaveLength(1);
    expect(urgentRows[0].dataset.category).toBe("attention");
    expect(urgentRows[0].dataset.severity).toBe("urgent");

    // Toggling off restores the full stream (both rows).
    await user.click(pilne);
    expect(container.querySelectorAll("li[data-category]").length).toBeGreaterThan(1);
  });

  it("shows a per-category error strip with retry and blocks the quiet state while errored (no false-quiet, ADR 0081 Q9)", async () => {
    const user = userEvent.setup();
    appTestState.autopilotRunsResponse = [];
    appTestState.attentionEventsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.reportSeasonUpcomingResponse = [];

    let failClaims = true;
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === "list_claims_to_verify" && failClaims) {
        throw new Error("boom");
      }
      return handleAppCommand(command as string, args as Record<string, unknown> | undefined);
    });

    renderApp({ section: "Today" });

    // The errored category is visibly errored (typed, translated — never a raw
    // `.message`), and the quiet state must not render alongside it.
    expect(await screen.findByText("Couldn't load claims to verify.")).toBeInTheDocument();
    expect(screen.queryByText("Nothing needs your attention.")).not.toBeInTheDocument();

    // Retry refetches only that category; on success the strip clears.
    failClaims = false;
    await user.click(screen.getByRole("button", { name: /Try again/ }));
    await waitFor(() =>
      expect(screen.queryByText("Couldn't load claims to verify.")).not.toBeInTheDocument(),
    );
  });
});

describe("Today v0.60 — config banner wired to source health (ADR 0087 dec. 5)", () => {
  it("raises a banner for a failing (enabled + attention) source and its Diagnostics action opens Sources", async () => {
    const user = userEvent.setup();
    // Only the failing adapter, so exactly one banner (the minimal scenario's other
    // adapters are healthy/off and must not raise one).
    appTestState.sourceAdaptersResponse = [
      makeSourceAdapter({
        id: "bankier-market-rss",
        displayName: "Bankier Giełda RSS",
        sourceType: "public_media",
        fetchMode: "rss",
        visibility: "optional",
        userConfigurable: true,
        healthStatus: "attention",
        enabled: true,
        sourceUrl: "https://www.bankier.pl/rss/gielda.xml",
        markets: ["GPW"],
      }),
    ];

    renderApp({ section: "Today" });

    // One banner, naming the source, with the translated "may be delayed" clause.
    const banner = await screen.findByText(
      /Bankier Giełda RSS.*isn't responding — signals may be delayed/,
    );
    expect(banner).toBeInTheDocument();

    // Diagnostics jumps to the Sources surface.
    await user.click(screen.getByRole("button", { name: "Diagnostics" }));
    expect(await screen.findByRole("heading", { name: "Sources" })).toBeInTheDocument();
  });

  it("raises no banner when every source is healthy, off, or merely not-refreshed", async () => {
    appTestState.sourceAdaptersResponse = [
      makeSourceAdapter({
        id: "bankier-company-komunikaty",
        displayName: "Bankier Company Komunikaty",
        sourceType: "official_report",
        fetchMode: "public_json",
        visibility: "required",
        userConfigurable: true,
        healthStatus: "healthy",
        enabled: true,
        sourceUrl: "https://www.bankier.pl/x",
        markets: ["GPW"],
      }),
      makeSourceAdapter({
        id: "portal-analiz",
        displayName: "Portal Analiz",
        sourceType: "authenticated_research",
        fetchMode: "authenticated",
        visibility: "developer",
        userConfigurable: false,
        healthStatus: "off",
        enabled: false,
        sourceUrl: "https://portalanaliz.pl/",
        markets: ["GPW"],
      }),
    ];

    renderApp({ section: "Today" });
    await screen.findByRole("button", { name: "Details" });
    expect(screen.queryByText(/isn't responding/)).not.toBeInTheDocument();
  });
});

describe("Today v0.60 — cross-company routine aggregate + upcoming show-all (ADR 0087)", () => {
  it("collapses >3 routine autopilot runs across companies into one ×K aggregate, expandable in place", async () => {
    const user = userEvent.setup();
    appTestState.autopilotRunsResponse = Array.from({ length: 5 }, (_, i) => ({
      ...baseAutopilotRun,
      id: `run_agg_${i}`,
      companyId: `co_agg_${i}`,
    }));
    appTestState.attentionEventsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
    const { container } = renderApp({ section: "Today" });

    await screen.findByRole("button", { name: "Details" });
    // The five routine runs collapse into ONE autopilot row.
    const rows = [...container.querySelectorAll('li[data-category="autopilot"]')] as HTMLElement[];
    expect(rows).toHaveLength(1);
    const aggregate = rows[0];
    expect(within(aggregate).getByText(/×5/)).toBeInTheDocument();
    // The counter still counts the underlying rows, not the collapsed 1.
    const autopilotTile = screen.getByRole("button", { name: /Autopilot/ });
    expect(within(autopilotTile).getByText("5")).toBeInTheDocument();
    // Members are hidden until expanded — then one per company, in place.
    expect(aggregate.querySelectorAll('[data-member-category="autopilot"]')).toHaveLength(0);
    await user.click(within(aggregate).getByRole("button", { name: "Details" }));
    expect(aggregate.querySelectorAll('[data-member-category="autopilot"]')).toHaveLength(5);
    // Header + five members each expose one Review, so j/k traverses all six.
    expect(aggregate.querySelectorAll('[data-today-row="true"]').length).toBe(6);
  });

  it("caps upcoming at 6 and links to the Report-season surface with the full count", async () => {
    const user = userEvent.setup();
    appTestState.autopilotRunsResponse = [];
    appTestState.attentionEventsResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = Array.from({ length: 8 }, (_, i) => ({
      companyId: `co_up_${i}`,
      qualifiedTicker: `GPW:U${i}`,
      displayName: `Company ${i}`,
      eventKey: `q1-${i}`,
      eventDate: `2026-08-${String(10 + i).padStart(2, "0")}`,
      eventTime: null,
      title: `Q1 report ${i}`,
      preparationStatus: "upcoming",
    }));
    const { container } = renderApp({ section: "Today" });

    const showAll = await screen.findByRole("button", {
      name: /Show all upcoming reports \(8\)/,
    });
    // Cap 6 (of 8) → one cross-company routine aggregate; expanding shows the 6
    // capped members, and the "Show all (8)" link carries the full count.
    await findCategoryRow(container as HTMLElement, "upcoming");
    const upcomingRows = [...container.querySelectorAll('li[data-category="upcoming"]')] as HTMLElement[];
    expect(upcomingRows).toHaveLength(1);
    await user.click(within(upcomingRows[0]).getByRole("button", { name: "Details" }));
    expect(upcomingRows[0].querySelectorAll('[data-member-category="upcoming"]')).toHaveLength(6);

    await user.click(showAll);
    expect(await screen.findByRole("heading", { name: "Report Season" })).toBeInTheDocument();
  });
});

describe("Today morning-briefing strip (ADR 0087 dec. 5 mockup amendment)", () => {
  it("shows a no-briefing summary, and Generate enqueues + reveals the composed strip", async () => {
    const user = userEvent.setup();
    appTestState.morningBriefingResponse = null;
    renderApp({ section: "Today" });

    expect(
      await screen.findByText("No briefing yet. Generate one to see what's changed."),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Generate" }));

    expect(invoke).toHaveBeenCalledWith("generate_morning_briefing");
    // The empty summary is replaced by the composed strip's Expand affordance.
    await screen.findByRole("button", { name: /Expand/ });
    expect(
      screen.queryByText("No briefing yet. Generate one to see what's changed."),
    ).not.toBeInTheDocument();
  });

  it("expands the strip in place to the deterministic item list, and Review opens the item's evidence", async () => {
    const user = userEvent.setup();
    appTestState.morningBriefingResponse = morningBriefing({
      items: [morningBriefingItem({ title: "Profit warning issued", detail: "Profit warning" })],
    });
    const { container } = renderApp({ section: "Today" });

    // The list is hidden until the strip is expanded (ADR 0087 hierarchy tier 3).
    expect(container.querySelector(".today-briefing-items")).toBeNull();
    await user.click(await screen.findByRole("button", { name: /Expand/ }));

    const row = (await screen.findByText("Profit warning issued — Profit warning")).closest("li");
    expect(row).not.toBeNull();

    fireEvent.click(within(row as HTMLElement).getByRole("button", { name: "Review" }));

    // `signal` items open the evidence company's Feed (the company workspace).
    expect(await screen.findByLabelText("Research cockpit")).toBeInTheDocument();
  });
});

// ADR 0081 mid-milestone live checkpoint (owner's real DB, 2026-07-23): a
// systemic cause (many companies each missing an official-report reconciliation)
// rendered as a wall of separate PILNE rows. While still urgent, that one cause
// must read as ONE alarm — the urgent same-cause aggregate + a group-level
// Dismiss all. (Aging separately demotes the events that go unacted past 72h;
// that is the backend severity path, covered in storage::tests::severity.)
describe("Today v0.60 fix — urgent systemic-cause aggregate + Dismiss all (ADR 0087 amendment 2026-07-23)", () => {
  function reconciliationEvents(n: number, idPrefix: string) {
    return Array.from({ length: n }, (_, i) =>
      attentionEvent({
        id: `${idPrefix}_${i}`,
        companyId: `co_${idPrefix}_${i}`,
        triggerType: "source_reconciliation",
        evidenceType: "source_reconciliation",
        ruleId: null,
        severity: "urgent",
        firedAt: `2026-06-${String(10 + i).padStart(2, "0")}T09:00:00Z`,
      }),
    );
  }

  function seedReconciliations(n: number, idPrefix: string) {
    appTestState.autopilotRunsResponse = [];
    appTestState.alertRulesResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
    appTestState.attentionEventsResponse = reconciliationEvents(n, idPrefix);
  }

  it("collapses 14 same-cause urgent reconciliations across companies into one leading PILNE aggregate", async () => {
    const user = userEvent.setup();
    seedReconciliations(14, "attn_rec");
    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    // ONE leading urgent aggregate row, not 14 separate PILNE rows.
    const rows = [...container.querySelectorAll('li[data-category="attention"]')] as HTMLElement[];
    expect(rows).toHaveLength(1);
    const aggregate = rows[0];
    expect(aggregate.dataset.severity).toBe("urgent");
    expect(within(aggregate).getByText(/×14/)).toBeInTheDocument();
    // It keeps the PILNE severity chip (urgent aggregates lead + still shout).
    expect(within(aggregate).getByText("Urgent")).toBeInTheDocument();

    // Members expand in place — one per company, each with its own Review.
    expect(aggregate.querySelectorAll('[data-member-category="attention"]')).toHaveLength(0);
    await user.click(within(aggregate).getByRole("button", { name: "Details" }));
    expect(aggregate.querySelectorAll('[data-member-category="attention"]')).toHaveLength(14);
  });

  it("keeps a different urgent cause (an insider signal) as its own row, never merged", async () => {
    seedReconciliations(2, "attn_mix_rec");
    // Add two insider (signal_category) urgent events across two more companies:
    // a DIFFERENT cause, so it must NOT fold into the reconciliation aggregate.
    appTestState.alertRulesResponse = [signalRule];
    appTestState.attentionEventsResponse = [
      ...reconciliationEvents(2, "attn_mix_rec"),
      attentionEvent({
        id: "attn_mix_ins_0",
        companyId: "co_mix_ins_0",
        triggerType: "signal_category",
        ruleId: signalRule.id,
        severity: "urgent",
        firedAt: "2026-06-20T09:00:00Z",
      }),
    ];
    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    const rows = [...container.querySelectorAll('li[data-category="attention"]')] as HTMLElement[];
    // The reconciliation pair collapses (×2 aggregate); the lone insider stays a row.
    const aggregate = rows.find((row) => within(row).queryByText(/×2/));
    const insider = rows.find((row) => within(row).queryByText(/×2/) === null);
    expect(aggregate).toBeTruthy();
    expect(insider).toBeTruthy();
    expect(rows).toHaveLength(2);
  });

  it("Dismiss all on an attention aggregate two-step-confirms and dismisses every member event", async () => {
    const user = userEvent.setup();
    seedReconciliations(3, "attn_da");
    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    const aggregate = container.querySelector('li[data-category="attention"]') as HTMLElement;
    await user.click(within(aggregate).getByRole("button", { name: "Details" }));

    // Two-step (like Undo): the first click reveals the confirm, the second fires.
    await user.click(within(aggregate).getByRole("button", { name: "Dismiss all" }));
    await user.click(within(aggregate).getByRole("button", { name: "Dismiss all" }));

    for (let i = 0; i < 3; i += 1) {
      expect(invoke).toHaveBeenCalledWith(
        "dismiss_attention_event",
        expect.objectContaining({ input: expect.objectContaining({ id: `attn_da_${i}` }) }),
      );
    }
    // Every member dismissed → the aggregate empties out of the stream.
    await waitFor(() =>
      expect(container.querySelector('li[data-category="attention"]')).toBeNull(),
    );
  });

  it("folds an AGED (notable) same-cause wall into one Uwaga aggregate carrying Dismiss all", async () => {
    const user = userEvent.setup();
    // 14 reconciliations that aged out of urgent (backend demoted them to notable):
    // a wall that merely changed color must still collapse to ONE notable line.
    appTestState.autopilotRunsResponse = [];
    appTestState.alertRulesResponse = [];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
    appTestState.attentionEventsResponse = reconciliationEvents(14, "attn_aged").map((event) => ({
      ...event,
      severity: "notable" as const,
    }));
    const { container } = renderApp({ section: "Today" });
    await findCategoryRow(container as HTMLElement, "attention");

    const rows = [...container.querySelectorAll('li[data-category="attention"]')] as HTMLElement[];
    expect(rows).toHaveLength(1);
    const aggregate = rows[0];
    expect(aggregate.dataset.severity).toBe("notable");
    expect(within(aggregate).getByText(/×14/)).toBeInTheDocument();
    // Notable → the "Notable" chip (Polish: Uwaga), never the urgent PILNE chip.
    expect(within(aggregate).getByText("Notable")).toBeInTheDocument();
    expect(within(aggregate).queryByText("Urgent")).toBeNull();

    // The attention aggregate still carries the two-step Dismiss all.
    await user.click(within(aggregate).getByRole("button", { name: "Details" }));
    await user.click(within(aggregate).getByRole("button", { name: "Dismiss all" }));
    await user.click(within(aggregate).getByRole("button", { name: "Dismiss all" }));
    expect(invoke).toHaveBeenCalledWith(
      "dismiss_attention_event",
      expect.objectContaining({ input: expect.objectContaining({ id: "attn_aged_0" }) }),
    );
    await waitFor(() =>
      expect(container.querySelector('li[data-category="attention"]')).toBeNull(),
    );
  });
});

// Archive view (owner 2026-07-23: "dwa widoki? ten co teraz i drugi Archiwum").
// Dismiss stays the acknowledgement — nothing is deleted; the Archive is the
// read-only history of DISMISSED attention events.
describe("Today Archive view (owner 2026-07-23)", () => {
  function seedArchive(
    dismissed: ReturnType<typeof attentionEvent>[],
    active: ReturnType<typeof attentionEvent>[] = [],
  ) {
    appTestState.autopilotRunsResponse = [];
    appTestState.attentionEventsResponse = [...active, ...dismissed];
    appTestState.alertRulesResponse = [signalRule];
    appTestState.feedItemsResponse = [];
    appTestState.claimsToVerifyResponse = { due: [], overdue: [], upcoming: [] };
    appTestState.reportSeasonUpcomingResponse = [];
  }

  it("renders a two-state Active | Archive switch that defaults to Active (aria-pressed toggles)", async () => {
    seedArchive([]);
    const user = userEvent.setup();
    renderApp({ section: "Today" });

    const activeSeg = await screen.findByRole("button", { name: "Active" });
    const archiveSeg = screen.getByRole("button", { name: "Archive" });
    expect(activeSeg).toHaveAttribute("aria-pressed", "true");
    expect(archiveSeg).toHaveAttribute("aria-pressed", "false");

    await user.click(archiveSeg);
    expect(archiveSeg).toHaveAttribute("aria-pressed", "true");
    expect(activeSeg).toHaveAttribute("aria-pressed", "false");

    await user.click(activeSeg);
    expect(activeSeg).toHaveAttribute("aria-pressed", "true");
  });

  it("Archive lists dismissed events (with their snapshot title) and hides the active-stream chrome", async () => {
    seedArchive(
      [
        attentionEvent({
          id: "attn_archived_1",
          dismissed: true,
          seen: true,
          evidenceTitle: "Zarchiwizowany komunikat spółki",
        }),
      ],
      [attentionEvent({ id: "attn_active_1", evidenceTitle: "Aktywny komunikat spółki" })],
    );
    const user = userEvent.setup();
    renderApp({ section: "Today" });

    await user.click(await screen.findByRole("button", { name: "Archive" }));

    // The dismissed event's snapshot title renders in the read-only archive…
    expect(await screen.findByText("Zarchiwizowany komunikat spółki")).toBeInTheDocument();
    // …the archive is honestly scoped to attention events…
    expect(screen.getByText("Dismissed attention events")).toBeInTheDocument();
    // …the counters column (an active-stream concern) is gone…
    expect(screen.queryByRole("group", { name: "Filter the stream" })).toBeNull();
    // …and an active (non-dismissed) event never appears in the archive.
    expect(screen.queryByText("Aktywny komunikat spółki")).toBeNull();
    // The archive row carries no Dismiss control (already dismissed).
    const archiveRow = document.querySelector('li[data-category="attention"]') as HTMLElement;
    expect(within(archiveRow).queryByRole("button", { name: "Dismiss" })).toBeNull();
  });

  it("the active stream is unaffected — a dismissed event never appears in Active", async () => {
    seedArchive(
      [attentionEvent({ id: "attn_archived_2", dismissed: true, seen: true, evidenceTitle: "Odrzucone" })],
      [attentionEvent({ id: "attn_active_2", evidenceTitle: "Aktywne" })],
    );
    renderApp({ section: "Today" });

    expect(await screen.findByText("Aktywne")).toBeInTheDocument();
    expect(screen.queryByText("Odrzucone")).toBeNull();
  });

  it("raises no toast from archive data (dismissed events never reach the toast wiring)", async () => {
    // A dismissed URGENT event — would toast if it leaked into the active toast set.
    seedArchive([
      attentionEvent({
        id: "attn_archived_urgent",
        severity: "urgent",
        dismissed: true,
        seen: true,
        evidenceTitle: "Odrzucony pilny alert",
      }),
    ]);
    const user = userEvent.setup();
    const { container } = renderApp({ section: "Today" });

    await user.click(await screen.findByRole("button", { name: "Archive" }));
    await screen.findByText("Odrzucony pilny alert");
    // No toast fired — not in active, not from opening the archive.
    expect(container.querySelector(".ui-toast-message")).toBeNull();
  });

  it("shows a quiet empty state when the archive is empty", async () => {
    seedArchive([], [attentionEvent({ id: "attn_active_3", evidenceTitle: "Aktywne" })]);
    const user = userEvent.setup();
    renderApp({ section: "Today" });

    await user.click(await screen.findByRole("button", { name: "Archive" }));
    expect(await screen.findByText("Archive is empty.")).toBeInTheDocument();
  });
});
