import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

import { useAttentionController } from "./useAttentionController";
import type { AttentionEvent } from "../api/attention";
import { makeAlertRule } from "../test/scenarios/entities";

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
  vi.clearAllMocks();
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

  // fixA finding 3 (ADR 0097 dec. 6): the Alerts screen's rules section reads
  // this array directly instead of re-fetching its own `listAlertRules` copy —
  // the controller must expose the fetched rules, not just the id-keyed map.
  it("exposes the fetched rules as an array, in sync with rulesById", async () => {
    const rule = makeAlertRule("rule_a", "signal_category", "company_1");
    vi.mocked(listAlertRules).mockResolvedValue([rule]);
    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(result.current.hydrated).toBe(true));

    expect(result.current.rules).toEqual([rule]);
    expect(result.current.rulesById.get("rule_a")).toEqual(rule);
  });

  it("a response that raced past a mutation is discarded, then a re-fetch converges and loading settles", async () => {
    const first = deferred<AttentionEvent[]>();
    vi.mocked(listAttentionEvents)
      .mockReturnValueOnce(first.promise) // in-flight when the mutation lands
      .mockResolvedValue([]); // the convergence re-fetch returns the post-dismiss truth

    const { result } = renderHook(() => useAttentionController(true));
    // The fetch starts asynchronously (behind the mutation-chain gate) — wait
    // until it is genuinely in flight before racing the mutation against it.
    await waitFor(() => expect(listAttentionEvents).toHaveBeenCalledTimes(1));

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

  it("an unchanged poll preserves state identity AND stays silent (no loading flip after hydration)", async () => {
    vi.mocked(listAttentionEvents).mockResolvedValue([event("a")]);
    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(result.current.hydrated).toBe(true));
    const before = result.current.events;

    act(() => result.current.refresh());
    // Post-hydration refreshes are background work — Today's skeleton keys on
    // `loading`, so a 15s poll must never flip it.
    const sawLoading = result.current.loading;
    await waitFor(() => expect(vi.mocked(listAttentionEvents).mock.calls.length).toBeGreaterThan(1));
    await waitFor(() => expect(result.current.events).toBe(before));
    expect(sawLoading).toBe(false);
    expect(result.current.loading).toBe(false);
  });

  it("a changed witnessUrl on an otherwise-identical event IS applied (full-fidelity compare)", async () => {
    vi.mocked(listAttentionEvents)
      .mockResolvedValueOnce([event("a", { witnessUrl: null })])
      .mockResolvedValueOnce([event("a", { witnessUrl: "https://gpw.pl/k?id=1" })]);
    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(result.current.events).toHaveLength(1));

    act(() => result.current.refresh());
    // Review routes on witnessUrl — a reconciliation re-run that fills it in on
    // the same event id must reach the UI, never be dropped as "unchanged".
    await waitFor(() =>
      expect(result.current.events[0].witnessUrl).toBe("https://gpw.pl/k?id=1"),
    );
  });

  it("a pending COALESCED refresh also waits for the mutation's write to commit", async () => {
    const initial = deferred<AttentionEvent[]>();
    vi.mocked(listAttentionEvents)
      .mockReturnValueOnce(initial.promise)
      .mockResolvedValue([]);
    const persist = deferred<void>();
    vi.mocked(dismissAttentionEvent).mockReturnValueOnce(persist.promise);

    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(listAttentionEvents).toHaveBeenCalledTimes(1));
    // While the initial fetch is in flight: an optimistic dismiss (persist
    // pending) AND a refresh that coalesces behind the in-flight fetch.
    await act(async () => {
      void result.current.dismiss("a").catch(() => {});
      result.current.refresh();
    });
    const callsBeforeStale = vi.mocked(listAttentionEvents).mock.calls.length;

    // The stale response settles and releases the coalesced follow-up — but
    // that follow-up must NOT read until the dismiss write commits, or it
    // would accept pre-mutation backend data and momentarily resurrect the
    // dismissed event.
    await act(async () => {
      initial.resolve([event("a")]);
      await initial.promise;
    });
    expect(vi.mocked(listAttentionEvents).mock.calls.length).toBe(callsBeforeStale);

    await act(async () => {
      persist.resolve();
      await persist.promise;
    });
    await waitFor(() => expect(result.current.events).toEqual([]));
  });

  it("a convergence re-fetch waits for the mutation's write to commit", async () => {
    const initial = deferred<AttentionEvent[]>();
    vi.mocked(listAttentionEvents)
      .mockReturnValueOnce(initial.promise)
      .mockResolvedValue([]);
    const persist = deferred<void>();
    vi.mocked(dismissAttentionEvent).mockReturnValueOnce(persist.promise);

    const { result } = renderHook(() => useAttentionController(true));
    await waitFor(() => expect(listAttentionEvents).toHaveBeenCalledTimes(1));
    await act(async () => {
      void result.current.dismiss("a").catch(() => {});
    });
    const callsBeforeStale = vi.mocked(listAttentionEvents).mock.calls.length;

    // The stale initial response arrives — discarded, but the replacement
    // fetch must NOT start until the dismiss write settles (else it could read
    // pre-mutation backend data and accept it).
    await act(async () => {
      initial.resolve([event("a")]);
      await initial.promise;
    });
    expect(vi.mocked(listAttentionEvents).mock.calls.length).toBe(callsBeforeStale);

    await act(async () => {
      persist.resolve();
      await persist.promise;
    });
    await waitFor(() =>
      expect(vi.mocked(listAttentionEvents).mock.calls.length).toBeGreaterThan(callsBeforeStale),
    );
    await waitFor(() => expect(result.current.events).toEqual([]));
  });
});
