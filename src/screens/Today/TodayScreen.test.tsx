import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TodayScreen, type TodayScreenProps } from "./TodayScreen";
import { getTodayView, markTodayVisited, type TodayView } from "../../api/today";
import { setAutopilotRunNotificationState } from "../../api/autopilot";
import { listAttentionEvents } from "../../api/attention";
import type { TodayItem } from "../../api/generated/TodayItem";
import type { AttentionController } from "../../app/useAttentionController";
import { COMPANY_SPECS, makeAttentionEvent, makeCompany } from "../../test/scenarios/entities";

vi.mock("../../api/today", () => ({
  getTodayView: vi.fn(),
  markTodayVisited: vi.fn(),
}));
vi.mock("../../api/attention", () => ({
  listAttentionEvents: vi.fn(),
}));
vi.mock("../../api/autopilot", () => ({
  setAutopilotRunNotificationState: vi.fn(),
  undoAutopilotRun: vi.fn(),
}));

const getTodayViewMock = vi.mocked(getTodayView);
const markTodayVisitedMock = vi.mocked(markTodayVisited);
const listAttentionEventsMock = vi.mocked(listAttentionEvents);
const setAutopilotRunNotificationStateMock = vi.mocked(setAutopilotRunNotificationState);

// The fixed "now" every test bucketing (`dayQueueModel`) resolves against —
// TodayBody reads the real `Date` at mount, so the system clock must be
// pinned for deterministic Today/Yesterday day-section labels.
const NOW = "2026-08-21T09:00:00Z";
const TODAY = "2026-08-21";
const YESTERDAY = "2026-08-20";

const company = makeCompany(COMPANY_SPECS.find((spec) => spec.key === "pzu")!);

function emptyView(overrides: Partial<TodayView> = {}): TodayView {
  return {
    items: [],
    toVerify: [],
    deltaSummary: { reportCount: 0, filingCount: 0, mediaCount: 0 },
    previousVisitAt: null,
    sectionErrors: {},
    ...overrides,
  };
}

function fakeAttention(overrides: Partial<AttentionController> = {}): AttentionController {
  return {
    events: [],
    rules: [],
    rulesById: new Map(),
    loading: false,
    hydrated: true,
    error: null,
    unseenCount: 0,
    refresh: vi.fn(),
    markSeen: vi.fn(() => Promise.resolve()),
    markManySeen: vi.fn(() => Promise.resolve()),
    dismiss: vi.fn(() => Promise.resolve()),
    ...overrides,
  };
}

function baseProps(overrides: Partial<TodayScreenProps> = {}): TodayScreenProps {
  return {
    attention: fakeAttention(),
    companies: [company],
    openCompanyWorkspace: vi.fn(),
    openInboxItem: vi.fn(),
    openCompanyInbox: vi.fn(),
    openInbox: vi.fn(),
    openCompanyClaims: vi.fn(),
    openExternalUrl: vi.fn(),
    sourceAdapters: [],
    openSources: vi.fn(),
    refreshSources: vi.fn(() => Promise.resolve()),
    todayReviewedDays: [],
    updateTodayReviewedDays: vi.fn(),
    refreshCompletionCount: 0,
    ...overrides,
  };
}

function unreadReportItem(): TodayItem {
  return {
    kind: "filing",
    feedItemId: "feed_today_1",
    companyId: company.id,
    qualifiedTicker: company.qualifiedTicker,
    title: "Wyniki finansowe PSr /2026",
    publishedAt: `${TODAY}T08:00:00Z`,
    read: false,
    presentationKind: "report",
  };
}

function readYesterdayFilingItem(): TodayItem {
  return {
    kind: "filing",
    feedItemId: "feed_yesterday_1",
    companyId: company.id,
    qualifiedTicker: company.qualifiedTicker,
    title: "Zawiadomienie o transakcji",
    publishedAt: `${YESTERDAY}T15:00:00Z`,
    read: true,
    presentationKind: "filing",
  };
}

function olderFilingItem(day: string, feedItemId: string, title: string): TodayItem {
  return {
    kind: "filing",
    feedItemId,
    companyId: company.id,
    qualifiedTicker: company.qualifiedTicker,
    title,
    publishedAt: `${day}T08:00:00Z`,
    read: false,
    presentationKind: "filing",
  };
}

describe("TodayScreen", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date(NOW));
    listAttentionEventsMock.mockResolvedValue([]);
    markTodayVisitedMock.mockResolvedValue(NOW);
  });

  afterEach(async () => {
    // Drain the previous test's pending microtasks BEFORE clearing mocks: the
    // fire-once `mark_today_visited` effect can still be in flight when a test
    // ends, and landing on an already-cleared spy makes the NEXT test's
    // negative assertion flaky (seen once in a full-suite run, 2026-08-23).
    await vi.advanceTimersByTimeAsync(0);
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("Loading: shows a skeleton before the read resolves", () => {
    getTodayViewMock.mockReturnValue(new Promise(() => {}));
    render(<TodayScreen {...baseProps()} />);
    expect(screen.getByLabelText("Checking what's new since your last visit…")).toBeInTheDocument();
  });

  it("Error: shows a typed error with Retry, and never stamps the visit anchor", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    getTodayViewMock.mockRejectedValue(new Error("boom"));
    render(<TodayScreen {...baseProps()} />);

    expect(await screen.findByText("Couldn't load your Today view.")).toBeInTheDocument();
    expect(screen.queryByText("boom")).not.toBeInTheDocument();
    expect(markTodayVisitedMock).not.toHaveBeenCalled();

    getTodayViewMock.mockResolvedValueOnce(emptyView());
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Nothing new since your last visit")).toBeInTheDocument();
  });

  it("Empty: renders the 3-beat clean-morning state with a quiet refresh action", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    getTodayViewMock.mockResolvedValue(emptyView());
    const refreshSources = vi.fn(() => Promise.resolve());
    render(<TodayScreen {...baseProps({ refreshSources })} />);

    expect(await screen.findByText("Nothing new since your last visit")).toBeInTheDocument();
    expect(
      screen.getByText("Sources are connected and the calendar names nothing due today."),
    ).toBeInTheDocument();
    // Clean morning: no primary action (deliberate — contract §6).
    expect(document.querySelectorAll('[data-ux-primary-action="true"]')).toHaveLength(0);

    await user.click(screen.getByRole("button", { name: "Refresh sources" }));
    expect(refreshSources).toHaveBeenCalledWith("manual");
  });

  it("Success: renders day sections with exactly one screen-wide primary action, and stamps the visit anchor once", async () => {
    getTodayViewMock.mockResolvedValue(
      emptyView({
        items: [unreadReportItem(), readYesterdayFilingItem()],
        deltaSummary: { reportCount: 1, filingCount: 1, mediaCount: 0 },
        previousVisitAt: `${YESTERDAY}T10:00:00Z`,
      }),
    );
    render(<TodayScreen {...baseProps()} />);

    expect(await screen.findByText("Wyniki finansowe PSr /2026")).toBeInTheDocument();
    // `.dayq-day-label` scopes the day-section headers away from the "Today"
    // panel heading (PanelHeader) — both would otherwise match `getByText`.
    const dayLabels = [...document.querySelectorAll(".dayq-day-label")].map((node) => node.textContent);
    expect(dayLabels).toContain("Today");
    expect(dayLabels).toContain("Yesterday");

    // Exactly one screen-wide primary action (the header CTA) — the report is
    // the only unread report, so it wins `pickPrimary`.
    const primaries = document.querySelectorAll('[data-ux-primary-action="true"]');
    expect(primaries).toHaveLength(1);
    expect(primaries[0]?.textContent).toContain("Read report");

    await waitFor(() => expect(markTodayVisitedMock).toHaveBeenCalledTimes(1));
  });

  it("Partial: a section error renders inline while the rest of the screen stays alive", async () => {
    getTodayViewMock.mockResolvedValue(
      emptyView({
        items: [unreadReportItem()],
        sectionErrors: { claims: "unavailable" },
      }),
    );
    render(<TodayScreen {...baseProps()} />);

    expect(await screen.findByText("Couldn't load claims to verify.")).toBeInTheDocument();
    // The feed section still rendered despite the claims-section failure.
    expect(screen.getByText("Wyniki finansowe PSr /2026")).toBeInTheDocument();
  });

  it("Collapse: an all-seen day renders collapsed, and Open day expands it", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    getTodayViewMock.mockResolvedValue(emptyView({ items: [readYesterdayFilingItem()] }));
    render(<TodayScreen {...baseProps()} />);

    await screen.findByText("Yesterday", { exact: true });
    // Collapsed: the row itself is not rendered yet.
    expect(screen.queryByText("Zawiadomienie o transakcji")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Open day" }));
    expect(await screen.findByText("Zawiadomienie o transakcji")).toBeInTheDocument();
  });

  it("Wcześniej rollup: buckets beyond Today/Yesterday collapse into one line, and Open days expands them", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    const dayA = olderFilingItem("2026-08-19", "feed_older_a", "Starszy komunikat A");
    const dayB = olderFilingItem("2026-08-18", "feed_older_b", "Starszy komunikat B");
    getTodayViewMock.mockResolvedValue(
      emptyView({ items: [unreadReportItem(), readYesterdayFilingItem(), dayA, dayB] }),
    );
    render(<TodayScreen {...baseProps()} />);

    await screen.findByText("Today", { exact: true });
    expect(screen.getByText("Yesterday", { exact: true })).toBeInTheDocument();

    // The two older days (2026-08-19, 2026-08-18) collapse into ONE rollup
    // line — neither renders its own day section yet.
    expect(screen.queryByText("Starszy komunikat A")).not.toBeInTheDocument();
    expect(screen.queryByText("Starszy komunikat B")).not.toBeInTheDocument();
    expect(screen.getByText("Earlier", { exact: true })).toBeInTheDocument();

    // "Open days" expands the rollup into individual day sections.
    await user.click(screen.getByRole("button", { name: "Open days" }));
    expect(await screen.findByText("Starszy komunikat A")).toBeInTheDocument();
    expect(screen.getByText("Starszy komunikat B")).toBeInTheDocument();
    expect(screen.queryByText("Earlier", { exact: true })).not.toBeInTheDocument();
  });

  it("mark_today_visited fires exactly once across a refetch, never on error", async () => {
    getTodayViewMock.mockResolvedValue(emptyView());
    const { rerender } = render(<TodayScreen {...baseProps()} />);
    await screen.findByText("Nothing new since your last visit");
    await waitFor(() => expect(markTodayVisitedMock).toHaveBeenCalledTimes(1));

    // A later render bumping `refreshCompletionCount` (fix wave B finding 1a's
    // refresh-coordination key) re-keys `useCommandQuery` and refetches — the
    // ref guard keeps the visit stamp to exactly once regardless.
    getTodayViewMock.mockResolvedValue(emptyView({ deltaSummary: { reportCount: 0, filingCount: 1, mediaCount: 0 } }));
    rerender(<TodayScreen {...baseProps({ refreshCompletionCount: 1 })} />);
    await waitFor(() => expect(getTodayViewMock.mock.calls.length).toBeGreaterThan(1));
    expect(markTodayVisitedMock).toHaveBeenCalledTimes(1);
  });

  it("sol R3 guard: a successful run dismiss (Mark as read) refetches the composed view — the row outcome is screen-visible", async () => {
    const run: Extract<TodayItem, { kind: "autopilotRun" }> = {
      kind: "autopilotRun",
      run: {
        id: "run_g1",
        companyId: company.id,
        reportDocumentId: "doc_g1",
        trigger: "detection",
        mode: "assist",
        sweepId: null,
        status: "succeeded",
        stage: "done",
        summaryText: null,
        kpiDeltaJson: null,
        reportDiffRef: null,
        crossRefsJson: null,
        producedFactIds: [],
        notificationState: "unread",
        lastError: null,
        createdAt: NOW,
        updatedAt: NOW,
        severity: "routine",
        reportDocumentTitle: "Raport g1",
      },
    };
    getTodayViewMock.mockResolvedValue(emptyView({ items: [run] }));
    setAutopilotRunNotificationStateMock.mockResolvedValue({ ...run.run, notificationState: "read" });
    render(<TodayScreen {...baseProps()} />);
    const dismiss = await screen.findByRole("button", { name: "Mark as read" });
    expect(getTodayViewMock.mock.calls.length).toBe(1);
    dismiss.click();
    await waitFor(() => expect(getTodayViewMock.mock.calls.length).toBe(2));
  });

  it("fix wave B finding 1a: bumping refreshCompletionCount refetches even when attention.events is unchanged", async () => {
    getTodayViewMock.mockResolvedValue(emptyView());
    const attention = fakeAttention();
    const { rerender } = render(<TodayScreen {...baseProps({ attention })} />);
    await screen.findByText("Nothing new since your last visit");
    expect(getTodayViewMock.mock.calls.length).toBe(1);

    // Same `attention` object (its own reference AND its `.events` array are
    // untouched — the old, fragile key) — only the completion count moves.
    rerender(<TodayScreen {...baseProps({ attention, refreshCompletionCount: 1 })} />);
    await waitFor(() => expect(getTodayViewMock.mock.calls.length).toBe(2));
  });

  it("fix wave B finding 1b: a nonArrival captured by a fired report_delay attention event renders EXACTLY one row, never zero and never two", async () => {
    const missed: TodayItem = {
      kind: "nonArrival",
      eventKey: "event_missed",
      companyId: company.id,
      qualifiedTicker: company.qualifiedTicker,
      eventDate: TODAY,
      title: "Raport roczny zapowiedziany",
    };
    getTodayViewMock.mockResolvedValue(emptyView({ items: [missed] }));
    const reportDelayRule = { id: "rule_rd", triggerType: "signal_category" as const, signalCategory: "report_delay", priceMin: null, priceMax: null, scopeType: "company" as const, scopeRef: company.id, enabled: true, createdAt: NOW, updatedAt: NOW };
    // The flag fires AFTER the event it is about (real detector timing) — the
    // date-bound suppression key requires eventDate <= day(firedAt).
    const rdEvent = { ...makeAttentionEvent("attn_rd", reportDelayRule.id, company.id), firedAt: NOW };
    const attention = fakeAttention({ events: [rdEvent], rulesById: new Map([[reportDelayRule.id, reportDelayRule]]) });
    render(<TodayScreen {...baseProps({ attention })} />);

    // The attention row (the flag that captured the non-arrival) is on screen…
    await screen.findByText("Signal");
    // …and the non-arrival row is NOT — exactly one row for this event, not two.
    expect(screen.queryByText("Raport roczny zapowiedziany")).not.toBeInTheDocument();
  });

  it("fix wave B finding 1b: a nonArrival with NO matching report_delay event still renders (no false suppression)", async () => {
    const missed: TodayItem = {
      kind: "nonArrival",
      eventKey: "event_missed",
      companyId: company.id,
      qualifiedTicker: company.qualifiedTicker,
      eventDate: TODAY,
      title: "Raport roczny zapowiedziany",
    };
    getTodayViewMock.mockResolvedValue(emptyView({ items: [missed] }));
    render(<TodayScreen {...baseProps()} />);
    expect(await screen.findByText("Raport roczny zapowiedziany")).toBeInTheDocument();
  });

  it("fix wave B finding 4: batch-mark-seen fires only after the query's FIRST success, never while loading or on error", async () => {
    getTodayViewMock.mockReturnValue(new Promise(() => {}));
    const event = makeAttentionEvent("attn_unseen", "rule1", company.id, "notable");
    const markManySeen = vi.fn(() => Promise.resolve());
    const attention = fakeAttention({ events: [event], markManySeen });
    render(<TodayScreen {...baseProps({ attention })} />);
    expect(markManySeen).not.toHaveBeenCalled();
  });

  it("fix wave B finding 4: an errored attention state renders an error strip and suppresses the clean-morning empty state", async () => {
    getTodayViewMock.mockResolvedValue(emptyView());
    const attention = fakeAttention({ error: "network down" });
    render(<TodayScreen {...baseProps({ attention })} />);

    expect(await screen.findByText("Couldn't load attention signals.")).toBeInTheDocument();
    // No false quiet: the "clean morning" empty-state copy must not appear.
    expect(
      screen.queryByText("Sources are connected and the calendar names nothing due today."),
    ).not.toBeInTheDocument();
  });

  it("fix wave B finding 5: mark_today_visited does NOT fire when sectionErrors.feed is set", async () => {
    getTodayViewMock.mockResolvedValue(emptyView({ sectionErrors: { feed: "unavailable" } }));
    render(<TodayScreen {...baseProps()} />);
    await screen.findByText("Couldn't load new filings/media.");
    // Passive effects (the `mark_today_visited` guard) are scheduled after
    // paint — `findByText`'s MutationObserver-driven resolution can race
    // ahead of them under fake timers, so force a flush before the negative
    // assertion (same idiom as `CompanyCoveragePanel.test.tsx`).
    await vi.advanceTimersByTimeAsync(0);
    expect(markTodayVisitedMock).not.toHaveBeenCalled();
  });

  it("fix wave B finding 5: mark_today_visited does NOT fire when sectionErrors.anchor is set — and the failure is VISIBLE, never quiet", async () => {
    getTodayViewMock.mockResolvedValue(emptyView({ sectionErrors: { anchor: "unavailable" } }));
    render(<TodayScreen {...baseProps()} />);
    await screen.findByText("Couldn't read your last-visit anchor — the delta may be incomplete.");
    await vi.advanceTimersByTimeAsync(0);
    expect(markTodayVisitedMock).not.toHaveBeenCalled();
  });
});
