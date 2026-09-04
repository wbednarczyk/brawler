import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

import { useActivityController } from "./useActivityController";
import * as activityApi from "../api/activity";
import * as sourcesApi from "../api/sources";
import type { ActivityView } from "../api/generated/ActivityView";

vi.mock("../api/activity", () => ({
  getActivitySummary: vi.fn(),
  listActivity: vi.fn(),
}));
vi.mock("../api/sources", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../api/sources")>()),
  getSchedulerStatus: vi.fn(),
}));

const EMPTY_VIEW: ActivityView = { active: [], queued: [], recent: [], generatedAt: "2026-09-04T12:00:00Z" };

function mockedSummary() {
  return vi.mocked(activityApi.getActivitySummary);
}
function mockedView() {
  return vi.mocked(activityApi.listActivity);
}

beforeEach(() => {
  vi.mocked(activityApi.getActivitySummary).mockReset();
  vi.mocked(activityApi.listActivity).mockReset();
  vi.mocked(sourcesApi.getSchedulerStatus).mockReset();
});

describe("useActivityController — summary", () => {
  it("refreshSummary() succeeds independently of getSchedulerStatus rejecting elsewhere", async () => {
    mockedSummary().mockResolvedValue({ active: 3, queued: 1, lastFinishedAt: "2026-09-04T12:01:00Z" });
    vi.mocked(sourcesApi.getSchedulerStatus).mockRejectedValue(new Error("scheduler status down"));

    const { result } = renderHook(() => useActivityController({ enabled: true }));

    // Simulate the app-lifecycle tick calling BOTH siblings, one failing.
    void sourcesApi.getSchedulerStatus().catch(() => {});
    act(() => {
      result.current.refreshSummary();
    });

    await waitFor(() => expect(result.current.summary.active).toBe(3));
    expect(result.current.summary.queued).toBe(1);
  });

  it("keeps the last-known-good summary and sets error on a failed refresh", async () => {
    mockedSummary().mockResolvedValueOnce({ active: 2, queued: 0, lastFinishedAt: null });
    const { result } = renderHook(() => useActivityController({ enabled: true }));
    act(() => {
      result.current.refreshSummary();
    });
    await waitFor(() => expect(result.current.summary.active).toBe(2));

    mockedSummary().mockRejectedValueOnce(new Error("read failed"));
    act(() => {
      result.current.refreshSummary();
    });
    await waitFor(() => expect(result.current.error).toBe("Error: read failed"));
    expect(result.current.summary.active).toBe(2);
  });
});

describe("useActivityController — panel view polling", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("overlapping slow polls do not stack: a 12s response + 15s of ticks yields exactly one extra call after it resolves", async () => {
    let resolveFirst: (value: ActivityView) => void = () => {};
    const first = new Promise<ActivityView>((resolve) => {
      resolveFirst = resolve;
    });
    mockedView().mockReturnValueOnce(first).mockResolvedValue(EMPTY_VIEW);

    const { result } = renderHook(() => useActivityController({ enabled: true }));
    act(() => {
      result.current.setOpen(true);
    });
    expect(mockedView()).toHaveBeenCalledTimes(1);

    // Two poll ticks (5s, 10s) land while the first request is still in flight.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(mockedView()).toHaveBeenCalledTimes(1);

    // The in-flight request resolves at ~12s — exactly one coalesced follow-up fires.
    await act(async () => {
      resolveFirst(EMPTY_VIEW);
      await vi.advanceTimersByTimeAsync(2_000);
    });
    expect(mockedView()).toHaveBeenCalledTimes(2);
  });

  it("unmounting with a pending response leaves no state update and clears the poll timer", async () => {
    let resolvePending: (value: ActivityView) => void = () => {};
    mockedView().mockReturnValue(
      new Promise((resolve) => {
        resolvePending = resolve;
      }),
    );

    const { result, unmount } = renderHook(() => useActivityController({ enabled: true }));
    act(() => {
      result.current.setOpen(true);
    });
    expect(mockedView()).toHaveBeenCalledTimes(1);

    unmount();
    const callsAtUnmount = mockedView().mock.calls.length;

    // Resolving after unmount must not throw / warn (act() guards state updates).
    await act(async () => {
      resolvePending(EMPTY_VIEW);
      await vi.advanceTimersByTimeAsync(20_000);
    });

    // No further polling after unmount — the interval was cleared.
    expect(mockedView()).toHaveBeenCalledTimes(callsAtUnmount);
  });

  it("closing the panel clears the poll timer (no further ticks while closed)", async () => {
    mockedView().mockResolvedValue(EMPTY_VIEW);
    const { result } = renderHook(() => useActivityController({ enabled: true }));

    await act(async () => {
      result.current.setOpen(true);
      await vi.advanceTimersByTimeAsync(0);
    });
    const callsWhileOpen = mockedView().mock.calls.length;

    act(() => {
      result.current.setOpen(false);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(20_000);
    });
    expect(mockedView()).toHaveBeenCalledTimes(callsWhileOpen);
  });

  it("an error keeps the previous view and sets error", async () => {
    mockedView().mockResolvedValueOnce({ ...EMPTY_VIEW, generatedAt: "first" });
    const { result } = renderHook(() => useActivityController({ enabled: true }));
    await act(async () => {
      result.current.setOpen(true);
      await vi.advanceTimersByTimeAsync(0);
    });
    await waitFor(() => expect(result.current.view?.generatedAt).toBe("first"));

    mockedView().mockRejectedValueOnce(new Error("boom"));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5_000);
    });
    await waitFor(() => expect(result.current.error).toBe("Error: boom"));
    expect(result.current.view?.generatedAt).toBe("first");
  });
});
