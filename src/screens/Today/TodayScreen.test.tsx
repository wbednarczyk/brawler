import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TodayScreen, type TodayScreenProps } from "./TodayScreen";
import { getTodayView, markTodayVisited, type TodayView } from "../../api/today";
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

const getTodayViewMock = vi.mocked(getTodayView);
const markTodayVisitedMock = vi.mocked(markTodayVisited);
const listAttentionEventsMock = vi.mocked(listAttentionEvents);

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

  afterEach(() => {
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

    // A later render where `attention.events` genuinely changes (the refresh-
    // coordination key, plan decision 2) re-keys `useCommandQuery` and refetches
    // — the ref guard keeps the visit stamp to exactly once regardless.
    const newEvent = makeAttentionEvent("attn_refetch", "rule1", company.id, "notable");
    getTodayViewMock.mockResolvedValue(emptyView({ deltaSummary: { reportCount: 0, filingCount: 1, mediaCount: 0 } }));
    rerender(<TodayScreen {...baseProps({ attention: fakeAttention({ events: [newEvent] }) })} />);
    await waitFor(() => expect(getTodayViewMock.mock.calls.length).toBeGreaterThan(1));
    expect(markTodayVisitedMock).toHaveBeenCalledTimes(1);
  });
});
