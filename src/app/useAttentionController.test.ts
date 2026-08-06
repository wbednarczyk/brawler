import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

import { useAttentionController } from "./useAttentionController";
import type { AttentionEvent } from "../api/attention";

// Controlled-promise seams: each api call resolves only when the test says so,
// so the request/mutation races the controller must survive are deterministic.
vi.mock("../api/attention", () => ({
  listAttentionEvents: vi.fn(),
  listAlertRules: vi.fn(),
  markAttentionEventSeen: vi.fn(),
  markAttentionEventsSeen: vi.fn(),
  dismissAttentionEvent: vi.fn(),
}));

import {
  dismissAttentionEvent,
  listAlertRules,
  listAttentionEvents,
  markAttentionEventsSeen,
} from "../api/attention";

function event(id: string, overrides: Partial<AttentionEvent> = {}): AttentionEvent {
  return {
    id,
    ruleId: null,
    triggerType: "source_reconciliation",
    companyId: "company_1",
    evidenceType: "source_reconciliation",
    evidenceRef: `recon_${id}`,
    firedAt: "2026-08-01T00:00:00Z",
    seen: false,
    dismissed: false,
    severity: "urgent",
    evidenceTitle: null,
    evidenceDetail: null,
    witnessUrl: null,
    ...overrides,
  };
}

type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void; reject: (cause: unknown) => void };
function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.mocked(listAlertRules).mockResolvedValue([]);
  vi.mocked(markAttentionEventsSeen).mockResolvedValue(undefined);
  vi.mocked(dismissAttentionEvent).mockResolvedValue(undefined);
});

describe("useAttentionController — request/mutation sequencing (ADR 0097 dec. 6)", () => {
  it("hydrates and exposes the unseen non-routine count", async () => {
    vi.mocked(listAttentionEvents).mockResolvedValue([
      event("a"),
      event("b", { severity: "routine" }),
      event("c", { seen: true }),
    ]);
    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(result.current.hydrated).toBe(true));
    expect(result.current.unseenCount).toBe(1);
    expect(result.current.loading).toBe(false);
  });

  it("a response that raced past a mutation is discarded, then a re-fetch converges and loading settles", async () => {
    const first = deferred<AttentionEvent[]>();
    vi.mocked(listAttentionEvents)
      .mockReturnValueOnce(first.promise) // in-flight when the mutation lands
      .mockResolvedValue([]); // the convergence re-fetch returns the post-dismiss truth

    const { result } = renderHook(() => useAttentionController(true));

    // Optimistic dismiss while the initial fetch is STILL in flight.
    await act(async () => {
      void result.current.dismiss("a");
    });
    // The stale fetch now resolves with pre-dismiss data — it must NOT clobber.
    await act(async () => {
      first.resolve([event("a")]);
      await first.promise;
    });

    await waitFor(() => expect(result.current.loading).toBe(false));
    // Converged on the post-mutation truth, never the stale snapshot.
    expect(result.current.events).toEqual([]);
    expect(result.current.hydrated).toBe(true);
  });

  it("a failed fetch keeps the last-known-good events and surfaces error — never a false quiet", async () => {
    vi.mocked(listAttentionEvents).mockResolvedValueOnce([event("keep")]);
    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(result.current.events).toHaveLength(1));

    vi.mocked(listAttentionEvents).mockRejectedValueOnce(new Error("db locked"));
    act(() => result.current.refresh());

    await waitFor(() => expect(result.current.error).toContain("db locked"));
    expect(result.current.events).toHaveLength(1);
    expect(result.current.unseenCount).toBe(1);
    expect(result.current.loading).toBe(false);
  });

  it("a startup failure does NOT hydrate — the recovered backlog stays an un-announced hydration", async () => {
    vi.mocked(listAttentionEvents).mockRejectedValueOnce(new Error("boot race"));
    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(result.current.error).toContain("boot race"));
    expect(result.current.hydrated).toBe(false);

    vi.mocked(listAttentionEvents).mockResolvedValueOnce([event("late")]);
    act(() => result.current.refresh());
    await waitFor(() => expect(result.current.hydrated).toBe(true));
    expect(result.current.error).toBeNull();
    expect(result.current.events).toHaveLength(1);
  });

  it("a failed mutation re-syncs from the backend and rejects so screens can surface it", async () => {
    vi.mocked(listAttentionEvents).mockResolvedValue([event("a")]);
    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(result.current.events).toHaveLength(1));

    vi.mocked(dismissAttentionEvent).mockRejectedValueOnce(new Error("write failed"));
    let rejected: unknown = null;
    await act(async () => {
      await result.current.dismiss("a").catch((cause) => {
        rejected = cause;
      });
    });
    expect(String(rejected)).toContain("write failed");
    // The re-sync restored the backend truth (the event is still there).
    await waitFor(() => expect(result.current.events).toHaveLength(1));
  });

  it("an unchanged poll keeps the same state reference — the steady state is render-free", async () => {
    vi.mocked(listAttentionEvents).mockResolvedValue([event("a")]);
    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(result.current.events).toHaveLength(1));
    const before = result.current.events;

    act(() => result.current.refresh());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.events).toBe(before);
  });
});
