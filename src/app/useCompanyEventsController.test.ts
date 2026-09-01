import { act, renderHook } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as eventsApi from "../api/events";
import type { Company, CompanyEvent } from "../api/types";
import { emptyCompanyEventForm } from "./eventForms";
import { useCompanyEventsController } from "./useCompanyEventsController";

// useCompanyEventsController owns the Events screen's read/write side (F4b S3
// contract § Events points 4 and 7): the empty-week jump lookup
// (`findNextWeekWithEvents`, one read with every active filter retained) and
// the request-sequence guard behind `refreshCompanyEvents`/`companyEventsLoading`.
// Exercised directly via renderHook (a mocked api/events module), mirroring
// useFundamentalsController.test.ts.

vi.mock("../api/events");

const COMPANIES: Company[] = [
  { id: "company_1", ticker: "CDR", exchange: "GPW", qualifiedTicker: "GPW:CDR", displayName: "CD Projekt", isin: "PLOPTTC00011" } as Company,
];

const EVENT: CompanyEvent = {
  id: "event_1",
  companyId: "company_1",
  company: "GPW:CDR",
  companyName: "CD Projekt",
  eventType: "dividend",
  title: "Next dividend",
  eventDate: "2026-09-30",
  eventTime: null,
  status: "confirmed",
  sourceType: "official_calendar",
  sourceAdapterId: "adapter_1",
  sourceEventKey: "key",
  sourceUrl: null,
  attribution: "GPW",
  fetchedAt: "2026-06-01T08:00:00Z",
  manual: false,
  createdAt: "2026-06-01T08:00:00Z",
  updatedAt: "2026-06-01T08:00:00Z",
};

function useHarness() {
  const [companyEventCompanyFilter, setCompanyEventCompanyFilter] = useState("all");
  const [companyEventDateFrom, setCompanyEventDateFrom] = useState("");
  const [companyEventDateTo, setCompanyEventDateTo] = useState("");
  const [companyEventForm, setCompanyEventForm] = useState(emptyCompanyEventForm());
  const [companyEventMode] = useState<"upcoming" | "historical" | "all">("upcoming");
  const [companyEventStatusFilter, setCompanyEventStatusFilter] = useState("all");
  const [companyEventTypeFilter, setCompanyEventTypeFilter] = useState("all");
  const [companyEventViewMode] = useState<"week" | "list">("week");
  const [companyEventWatchlistFilter, setCompanyEventWatchlistFilter] = useState("all");
  const [companyEvents, setCompanyEvents] = useState<CompanyEvent[]>([]);
  const [companyEventsError, setCompanyEventsError] = useState<string | null>(null);
  const [, setComposerOpen] = useState(false);
  const [, setCreateError] = useState<string | null>(null);
  const [, setSelectedCompanyEventId] = useState<string | null>(null);

  const controller = useCompanyEventsController({
    companies: COMPANIES,
    companyEventCompanyFilter,
    companyEventDateFrom,
    companyEventDateTo,
    companyEventForm,
    companyEventMode,
    companyEventStatusFilter,
    companyEventTypeFilter,
    companyEventViewMode,
    companyEventWatchlistFilter,
    companyEventWeekAnchorDate: "2026-08-24",
    companyEventWeekRange: { start: "2026-08-24", end: "2026-08-30" },
    setCompanyEventCompanyFilter,
    setCompanyEventComposerOpen: setComposerOpen,
    setCompanyEventCreateError: setCreateError,
    setCompanyEventDateFrom,
    setCompanyEventDateTo,
    setCompanyEventForm,
    setCompanyEventStatusFilter,
    setCompanyEventTypeFilter,
    setCompanyEvents,
    setCompanyEventsError,
    setCompanyEventWatchlistFilter,
    setSelectedCompanyEventId,
  });

  return {
    ...controller,
    companyEventCompanyFilter,
    companyEventStatusFilter,
    companyEventTypeFilter,
    companyEventWatchlistFilter,
    companyEvents,
    companyEventsError,
    setCompanyEventCompanyFilter,
    setCompanyEventStatusFilter,
    setCompanyEventTypeFilter,
    setCompanyEventWatchlistFilter,
  };
}

describe("useCompanyEventsController.findNextWeekWithEvents (F4b contract § Events point 4 / decision 4)", () => {
  beforeEach(() => {
    vi.mocked(eventsApi.listCompanyEvents).mockReset();
    vi.mocked(eventsApi.listCompanyEvents).mockResolvedValue([EVENT]);
  });

  it("reads mode: upcoming, dateFrom the day after the displayed week, no filters", async () => {
    const { result } = renderHook(() => useHarness());
    const match = await act(() => result.current.findNextWeekWithEvents());
    expect(eventsApi.listCompanyEvents).toHaveBeenCalledWith({
      mode: "upcoming",
      companyId: null,
      watchlistId: null,
      eventType: null,
      status: null,
      dateFrom: "2026-08-31",
      dateTo: null,
    });
    expect(match).toEqual(EVENT);
  });

  it("retains the watchlist filter alone", async () => {
    const { result } = renderHook(() => useHarness());
    act(() => result.current.setCompanyEventWatchlistFilter("watchlist_1"));
    await act(() => result.current.findNextWeekWithEvents());
    expect(eventsApi.listCompanyEvents).toHaveBeenCalledWith(
      expect.objectContaining({ watchlistId: "watchlist_1", companyId: null, eventType: null, status: null }),
    );
  });

  it("retains the company filter alone", async () => {
    const { result } = renderHook(() => useHarness());
    act(() => result.current.setCompanyEventCompanyFilter("company_1"));
    await act(() => result.current.findNextWeekWithEvents());
    expect(eventsApi.listCompanyEvents).toHaveBeenCalledWith(
      expect.objectContaining({ companyId: "company_1", watchlistId: null, eventType: null, status: null }),
    );
  });

  it("retains the type filter alone", async () => {
    const { result } = renderHook(() => useHarness());
    act(() => result.current.setCompanyEventTypeFilter("dividend"));
    await act(() => result.current.findNextWeekWithEvents());
    expect(eventsApi.listCompanyEvents).toHaveBeenCalledWith(
      expect.objectContaining({ eventType: "dividend", companyId: null, watchlistId: null, status: null }),
    );
  });

  it("retains the status filter alone", async () => {
    const { result } = renderHook(() => useHarness());
    act(() => result.current.setCompanyEventStatusFilter("proposed"));
    await act(() => result.current.findNextWeekWithEvents());
    expect(eventsApi.listCompanyEvents).toHaveBeenCalledWith(
      expect.objectContaining({ status: "proposed", companyId: null, watchlistId: null, eventType: null }),
    );
  });

  it("retains every active filter combined", async () => {
    const { result } = renderHook(() => useHarness());
    act(() => {
      result.current.setCompanyEventWatchlistFilter("watchlist_1");
      result.current.setCompanyEventCompanyFilter("company_1");
      result.current.setCompanyEventTypeFilter("dividend");
      result.current.setCompanyEventStatusFilter("proposed");
    });
    await act(() => result.current.findNextWeekWithEvents());
    expect(eventsApi.listCompanyEvents).toHaveBeenCalledWith(
      expect.objectContaining({
        watchlistId: "watchlist_1",
        companyId: "company_1",
        eventType: "dividend",
        status: "proposed",
      }),
    );
  });

  it("returns null when nothing matches", async () => {
    vi.mocked(eventsApi.listCompanyEvents).mockResolvedValue([]);
    const { result } = renderHook(() => useHarness());
    const match = await act(() => result.current.findNextWeekWithEvents());
    expect(match).toBeNull();
  });

  it("rejects on a read failure — never folds it into a false 'no match' (F4b sol R1)", async () => {
    vi.mocked(eventsApi.listCompanyEvents).mockRejectedValue(new Error("network"));
    const { result } = renderHook(() => useHarness());
    await expect(act(() => result.current.findNextWeekWithEvents())).rejects.toThrow("network");
  });
});

describe("useCompanyEventsController.refreshCompanyEvents (F4b contract § Events point 7)", () => {
  beforeEach(() => {
    vi.mocked(eventsApi.listCompanyEvents).mockReset();
  });

  it("sets companyEventsLoading while in flight and clears it on success", async () => {
    let resolveFirst!: (value: CompanyEvent[]) => void;
    vi.mocked(eventsApi.listCompanyEvents).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveFirst = resolve;
      }),
    );
    const { result } = renderHook(() => useHarness());

    act(() => {
      void result.current.refreshCompanyEvents();
    });
    expect(result.current.companyEventsLoading).toBe(true);

    await act(async () => {
      resolveFirst([EVENT]);
      await Promise.resolve();
    });
    expect(result.current.companyEventsLoading).toBe(false);
    expect(result.current.companyEvents).toEqual([EVENT]);
  });

  it("request-sequence guard: an older read resolving after a newer one is ignored", async () => {
    let resolveFirst!: (value: CompanyEvent[]) => void;
    let resolveSecond!: (value: CompanyEvent[]) => void;
    vi.mocked(eventsApi.listCompanyEvents)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveSecond = resolve;
        }),
      );
    const { result } = renderHook(() => useHarness());

    act(() => {
      void result.current.refreshCompanyEvents();
    });
    act(() => {
      void result.current.refreshCompanyEvents();
    });

    // The newer (second) request resolves first with its real data...
    await act(async () => {
      resolveSecond([EVENT]);
      await Promise.resolve();
    });
    expect(result.current.companyEvents).toEqual([EVENT]);

    // ...then the stale first request resolves with different data: ignored.
    await act(async () => {
      resolveFirst([]);
      await Promise.resolve();
    });
    expect(result.current.companyEvents).toEqual([EVENT]);
    expect(result.current.companyEventsLoading).toBe(false);
  });
});
