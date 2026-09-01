import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { entryKey, useReportSeason } from "./useReportSeason";
import {
  getPreReportCard,
  listReportSeason,
  markReportPrepared,
  markReportProcessed,
} from "../../api/reportSeason";
import {
  createReportExpectation,
  expectationReview,
  listReportExpectations,
} from "../../api/reportExpectations";
import { CommandInvocationError } from "../../api/tauri";
import type { ReportSeasonResult } from "../../api/reportSeason";
import { COMPANY_SPECS, makePreReportCard, makeReportSeasonEntry } from "../../test/scenarios/entities";

vi.mock("../../api/reportSeason", () => ({
  listReportSeason: vi.fn(),
  getPreReportCard: vi.fn(),
  markReportPrepared: vi.fn(),
  markReportProcessed: vi.fn(),
}));
vi.mock("../../api/reportExpectations", () => ({
  createReportExpectation: vi.fn(),
  updateReportExpectation: vi.fn(),
  listReportExpectations: vi.fn(),
  expectationReview: vi.fn(),
  recordExpectationResolution: vi.fn(),
}));

const listReportSeasonMock = vi.mocked(listReportSeason);
const getPreReportCardMock = vi.mocked(getPreReportCard);
const markReportPreparedMock = vi.mocked(markReportPrepared);
const markReportProcessedMock = vi.mocked(markReportProcessed);
const listReportExpectationsMock = vi.mocked(listReportExpectations);
const expectationReviewMock = vi.mocked(expectationReview);
const createReportExpectationMock = vi.mocked(createReportExpectation);

const cdr = COMPANY_SPECS.find((spec) => spec.key === "cdr")!;
const entry = makeReportSeasonEntry(cdr, true);
const card = makePreReportCard(cdr);

function emptySeason() {
  return { upcoming: [], past: [], calendarFreshness: { lastFetchedAt: null, stale: false } };
}

describe("useReportSeason", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("loads the season via useCommandQuery and exposes it as `season`", async () => {
    listReportSeasonMock.mockResolvedValue({
      upcoming: [entry],
      past: [],
      calendarFreshness: { lastFetchedAt: null, stale: false },
    });

    const { result } = renderHook(() => useReportSeason(null));

    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.season?.upcoming).toEqual([entry]);
    expect(result.current.error).toBeNull();
    expect(listReportSeasonMock).toHaveBeenCalledWith({ watchlistId: null });
  });

  it.each([
    [
      "prepare",
      () => markReportPreparedMock.mockResolvedValue({
        companyId: entry.companyId,
        eventKey: entry.eventKey,
        status: "prepared",
        preparedAt: "2026-07-01T00:00:00Z",
        processedAt: null,
        linkedReportDocumentId: null,
      }),
      (hook: ReturnType<typeof useReportSeason>) => hook.prepare(entry),
    ],
    [
      "process",
      () => markReportProcessedMock.mockResolvedValue({
        companyId: entry.companyId,
        eventKey: entry.eventKey,
        status: "processed",
        preparedAt: null,
        processedAt: "2026-07-01T00:00:00Z",
        linkedReportDocumentId: null,
      }),
      (hook: ReturnType<typeof useReportSeason>) => hook.process(entry),
    ],
  ] as const)(
    "%s refetches the season afterward (F4b S4: `await refetch()` before reloading the expanded card)",
    async (_name, setupMock, runAction) => {
      listReportSeasonMock.mockResolvedValue(emptySeason());
      setupMock();

      const { result } = renderHook(() => useReportSeason(null));
      await waitFor(() => expect(result.current.loading).toBe(false));
      listReportSeasonMock.mockClear();
      listReportSeasonMock.mockResolvedValue(emptySeason());

      await act(async () => {
        runAction(result.current);
        await waitFor(() => expect(result.current.actionInFlightKey).toBeNull());
      });

      expect(listReportSeasonMock).toHaveBeenCalledTimes(1);
    },
  );

  it("reloads the expanded card after a mutation (today's order: mutate → refetch season → reload card)", async () => {
    listReportSeasonMock.mockResolvedValue({ upcoming: [entry], past: [], calendarFreshness: { lastFetchedAt: null, stale: false } });
    getPreReportCardMock.mockResolvedValue(card);
    listReportExpectationsMock.mockResolvedValue([]);
    markReportPreparedMock.mockResolvedValue({
      companyId: entry.companyId,
      eventKey: entry.eventKey,
      status: "prepared",
      preparedAt: "2026-07-01T00:00:00Z",
      processedAt: null,
      linkedReportDocumentId: null,
    });

    const { result } = renderHook(() => useReportSeason(null));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => result.current.toggleExpanded(entry));
    await waitFor(() => expect(result.current.cards[entryKey(entry)]).toBeDefined());
    getPreReportCardMock.mockClear();
    getPreReportCardMock.mockResolvedValue(card);

    await act(async () => {
      result.current.prepare(entry);
      await waitFor(() => expect(result.current.actionInFlightKey).toBeNull());
    });

    expect(getPreReportCardMock).toHaveBeenCalledTimes(1);
  });

  it("a scope change ignores a stale in-flight season read (the useCommandQuery request-seq gate)", async () => {
    const deferred: Array<(value: ReportSeasonResult) => void> = [];
    listReportSeasonMock.mockImplementation(
      () => new Promise((resolve) => deferred.push(resolve)),
    );

    const { result, rerender } = renderHook(
      ({ watchlistId }: { watchlistId: string | null }) => useReportSeason(watchlistId),
      { initialProps: { watchlistId: "watchlist-a" } },
    );
    await waitFor(() => expect(listReportSeasonMock).toHaveBeenCalledTimes(1));

    rerender({ watchlistId: "watchlist-b" });
    await waitFor(() => expect(listReportSeasonMock).toHaveBeenCalledTimes(2));

    const staleResult = { upcoming: [entry], past: [], calendarFreshness: { lastFetchedAt: null, stale: false } };
    const freshResult = { upcoming: [], past: [], calendarFreshness: { lastFetchedAt: null, stale: false } };

    // The NEWER (watchlist-b) request resolves first; the OLDER (watchlist-a)
    // settles late and must be discarded.
    await act(async () => {
      deferred[1](freshResult);
      await Promise.resolve();
      deferred[0](staleResult);
      await Promise.resolve();
    });

    expect(result.current.season?.upcoming).toEqual([]);
  });

  it("keys a card load failure into `cardErrors` — the season-level `error` stays null", async () => {
    listReportSeasonMock.mockResolvedValue({ upcoming: [entry], past: [], calendarFreshness: { lastFetchedAt: null, stale: false } });
    getPreReportCardMock.mockRejectedValue(new Error("card load failed"));

    const { result } = renderHook(() => useReportSeason(null));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => result.current.toggleExpanded(entry));
    await waitFor(() => expect(result.current.cardErrors[entryKey(entry)]).toBe("card load failed"));

    expect(result.current.error).toBeNull();
    expect(result.current.cards[entryKey(entry)]).toBeUndefined();
  });

  it("`reloadCard` (Refresh card) clears a card error and re-fetches just that card", async () => {
    listReportSeasonMock.mockResolvedValue({ upcoming: [entry], past: [], calendarFreshness: { lastFetchedAt: null, stale: false } });
    getPreReportCardMock.mockRejectedValueOnce(new Error("card load failed"));
    listReportExpectationsMock.mockResolvedValue([]);

    const { result } = renderHook(() => useReportSeason(null));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => result.current.toggleExpanded(entry));
    await waitFor(() => expect(result.current.cardErrors[entryKey(entry)]).toBeDefined());

    getPreReportCardMock.mockResolvedValueOnce(card);
    act(() => result.current.reloadCard(entry));
    await waitFor(() => expect(result.current.cards[entryKey(entry)]).toEqual(card));
    expect(result.current.cardErrors[entryKey(entry)]).toBeUndefined();
  });

  it("an expectation write `conflict` reloads the card + expectation (not just the expectation) before the lock message shows", async () => {
    listReportSeasonMock.mockResolvedValue({ upcoming: [entry], past: [], calendarFreshness: { lastFetchedAt: null, stale: false } });
    getPreReportCardMock.mockResolvedValue(card);
    listReportExpectationsMock.mockResolvedValue([]);

    const { result } = renderHook(() => useReportSeason(null));
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => result.current.toggleExpanded(entry));
    await waitFor(() => expect(result.current.cards[entryKey(entry)]).toEqual(card));

    createReportExpectationMock.mockRejectedValue(
      new CommandInvocationError({ code: "conflict", message: "facts already landed" }),
    );
    getPreReportCardMock.mockClear();
    getPreReportCardMock.mockResolvedValue(card);
    expectationReviewMock.mockResolvedValue({
      companyId: entry.companyId,
      eventKey: entry.eventKey,
      fiscalYear: 2026,
      periodType: "FY",
      stanceMd: "stance",
      frozenAt: "2026-07-01T00:00:00Z",
      factsAvailable: true,
      resolutionNoteMd: null,
      resolvedAt: null,
      metrics: [],
    });
    listReportExpectationsMock.mockResolvedValue([
      {
        id: "exp-1",
        companyId: entry.companyId,
        eventKey: entry.eventKey,
        fiscalYear: 2026,
        periodType: "FY",
        stanceMd: "stance",
        frozenAt: "2026-07-01T00:00:00Z",
        resolutionNoteMd: null,
        resolvedAt: null,
        createdAt: "2026-06-01T00:00:00Z",
        updatedAt: "2026-06-01T00:00:00Z",
        metrics: [],
      },
    ]);

    await act(async () => {
      await result.current.writeExpectation(entry, {
        fiscalYear: 2026,
        periodType: "FY",
        stanceMd: "stance",
        metrics: [],
      });
    });

    // `loadCard` (not merely `loadExpectation`) ran: it reloads the card AND
    // the expectation, which is how a bare `getPreReportCardMock` call count
    // of 1 shows here.
    expect(getPreReportCardMock).toHaveBeenCalledTimes(1);
    expect(result.current.expectations[entryKey(entry)]?.review?.factsAvailable).toBe(true);
  });
});
