import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { getSchedulerStatus } from "../api/sources";
import { useAppLifecycleEffects } from "./useAppLifecycleEffects";

vi.mock("../api/sources", async (importOriginal) => ({
  ...(await importOriginal<object>()),
  getSchedulerStatus: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(),
}));
const getSchedulerStatusMock = vi.mocked(getSchedulerStatus);
const getCurrentWindowMock = vi.mocked(getCurrentWindow);

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

describe("useAppLifecycleEffects — native window close reads the latest host (sol R1 finding 2)", () => {
  beforeEach(() => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    getSchedulerStatusMock.mockResolvedValue({ sourceNextDueMs: {}, registryNextDueMs: null } as never);
  });
  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.clearAllMocks();
  });

  it("a tool opened AFTER the listener installs still gates the native close request", async () => {
    let closeHandler: ((event: { preventDefault(): void }) => void) | undefined;
    const onCloseRequested = vi.fn((handler: (event: { preventDefault(): void }) => void) => {
      closeHandler = handler;
      return Promise.resolve(() => {});
    });
    const destroy = vi.fn();
    getCurrentWindowMock.mockReturnValue({ onCloseRequested, destroy } as never);

    // A NEW `spolkaTool` object every render — same shape `useSpolkaToolHost()`
    // produces. `cleanTool` is what the close listener would capture at
    // mount (`[]` deps) if it read the prop directly instead of a ref.
    const cleanGuardNavigation = vi.fn();
    const dirtyGuardNavigation = vi.fn((next: () => void) => next());
    const cleanTool = { isDirty: () => false, guardNavigation: cleanGuardNavigation };
    const dirtyTool = { isDirty: () => true, guardNavigation: dirtyGuardNavigation };

    const { rerender } = renderHook(
      (spolkaTool: typeof cleanTool) =>
        useAppLifecycleEffects(
          props({
            activeSection: "Today",
            companies: [],
            filteredFeedItems: [],
            sourceAdapters: [],
            selectedNotebookEntry: null,
            selectedNotebookScreenEntry: null,
            licenseCanUseApp: true,
            spolkaTool,
          }),
        ),
      { initialProps: cleanTool },
    );

    await waitFor(() => expect(onCloseRequested).toHaveBeenCalledTimes(1));

    // The dirty tool opens on a LATER render — the exact stale-closure shape
    // of the bug (the effect's `[]` deps captured render-1's `spolkaTool`,
    // forever bound to "no tool open").
    rerender(dirtyTool);

    const preventDefault = vi.fn();
    closeHandler!({ preventDefault });

    // Still only ONE subscription — no re-listen leak on every render.
    expect(onCloseRequested).toHaveBeenCalledTimes(1);
    // … but the handler reads the LATEST host: a stale closure would have
    // called the clean render-1 host forever and silently let the close
    // through undirtied.
    expect(preventDefault).toHaveBeenCalledTimes(1);
    expect(dirtyGuardNavigation).toHaveBeenCalledTimes(1);
    expect(cleanGuardNavigation).not.toHaveBeenCalled();
  });

  it("a clean tool never prevents the native close", async () => {
    let closeHandler: ((event: { preventDefault(): void }) => void) | undefined;
    const onCloseRequested = vi.fn((handler: (event: { preventDefault(): void }) => void) => {
      closeHandler = handler;
      return Promise.resolve(() => {});
    });
    getCurrentWindowMock.mockReturnValue({ onCloseRequested, destroy: vi.fn() } as never);

    const guardNavigation = vi.fn();
    renderHook(() =>
      useAppLifecycleEffects(
        props({
          activeSection: "Today",
          companies: [],
          filteredFeedItems: [],
          sourceAdapters: [],
          selectedNotebookEntry: null,
          selectedNotebookScreenEntry: null,
          licenseCanUseApp: true,
          spolkaTool: { isDirty: () => false, guardNavigation },
        }),
      ),
    );

    await waitFor(() => expect(onCloseRequested).toHaveBeenCalledTimes(1));

    const preventDefault = vi.fn();
    closeHandler!({ preventDefault });

    expect(preventDefault).not.toHaveBeenCalled();
    expect(guardNavigation).not.toHaveBeenCalled();
  });
});
