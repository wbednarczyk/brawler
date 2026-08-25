import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook } from "@testing-library/react";

import { getSchedulerStatus } from "../api/sources";
import { useAppLifecycleEffects } from "./useAppLifecycleEffects";

vi.mock("../api/sources", async (importOriginal) => ({
  ...(await importOriginal<object>()),
  getSchedulerStatus: vi.fn(),
}));
const getSchedulerStatusMock = vi.mocked(getSchedulerStatus);

// The hook takes ~40 root callbacks/values; this guard only exercises the
// scheduler-mirror branch, so every unlisted member is an inert vi.fn()/null
// via a Proxy — a per-field literal would be 40 lines of noise that drifts.
function props(overrides: Record<string, unknown>) {
  return new Proxy(overrides, {
    get: (target, key: string) => (key in target ? target[key] : vi.fn()),
  }) as never;
}

describe("useAppLifecycleEffects — scheduler mirror (sol R3 guard)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("a fired scheduled refresh advances onRefreshCompletion — the Dziś query key must move without any manual refresh", async () => {
    const onRefreshCompletion = vi.fn();
    // Poll 1 arms the previous-due map; poll 2 moves an adapter's next-due
    // forward, which is the "a refresh fired" transition the mirror detects.
    getSchedulerStatusMock
      .mockResolvedValueOnce({ sourceNextDueMs: { espi: 1000 }, registryNextDueMs: null } as never)
      .mockResolvedValue({ sourceNextDueMs: { espi: 2000 }, registryNextDueMs: null } as never);

    renderHook(() =>
      useAppLifecycleEffects(
        props({
          activeSection: "Today",
          companies: [],
          filteredFeedItems: [],
          sourceAdapters: [],
          notebookEntries: [],
          selectedNotebookEntry: null,
          selectedNotebookScreenEntry: null,
          selectedCompany: null,
          selectedFeedItem: null,
          licenseCanUseApp: true,
          onRefreshCompletion,
        }),
      ),
    );

    // Poll 1 (immediate) arms the previous-due map — no transition yet.
    await vi.advanceTimersByTimeAsync(0);
    expect(onRefreshCompletion).not.toHaveBeenCalled();
    // Poll 2 (one 15s interval tick) sees espi's next-due jump 1000→2000 —
    // the "a scheduled refresh fired" transition — and must bump the signal.
    await vi.advanceTimersByTimeAsync(15_000);
    expect(onRefreshCompletion).toHaveBeenCalledTimes(1);
  });
});
