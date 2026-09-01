import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useCommandQuery } from "./useCommandQuery";

// Deferred promise helper — lets a test control exactly when a fetcher
// resolves, so the stale-response-discard seam (case 3) is reproducible.
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("useCommandQuery", () => {
  it("fetches on mount: loading -> success with data", async () => {
    const fetcher = vi.fn().mockResolvedValue("data-1");
    const { result } = renderHook(() => useCommandQuery(["key-a"], fetcher));

    expect(result.current.status).toBe("loading");
    expect(result.current.data).toBeNull();

    await waitFor(() => expect(result.current.status).toBe("success"));
    expect(result.current.data).toBe("data-1");
    expect(result.current.error).toBeNull();
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it("captures a rejection as status error, never throws", async () => {
    const failure = new Error("boom");
    const fetcher = vi.fn().mockRejectedValue(failure);
    const { result } = renderHook(() => useCommandQuery(["key-a"], fetcher));

    await waitFor(() => expect(result.current.status).toBe("error"));
    expect(result.current.data).toBeNull();
    expect(result.current.error).toBe(failure);
  });

  it("discards a stale response that resolves after the key already changed", async () => {
    const slow = deferred<string>();
    const fast = deferred<string>();
    const fetcher = vi.fn().mockImplementation(() => slow.promise);

    const { result, rerender } = renderHook(
      ({ key }: { key: string }) => useCommandQuery([key], fetcher),
      { initialProps: { key: "a" } },
    );

    // Switch to key B before A resolves; B's fetcher is a fast promise.
    fetcher.mockImplementation(() => fast.promise);
    rerender({ key: "b" });

    fast.resolve("b-data");
    await waitFor(() => expect(result.current.status).toBe("success"));
    expect(result.current.data).toBe("b-data");

    // A's late resolution must never overwrite B's settled state.
    await act(async () => {
      slow.resolve("a-data-stale");
      await Promise.resolve();
    });
    expect(result.current.data).toBe("b-data");
    expect(result.current.status).toBe("success");
  });

  it("refetch() re-invokes the fetcher and updates data", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce("first").mockResolvedValueOnce("second");
    const { result } = renderHook(() => useCommandQuery(["key-a"], fetcher));

    await waitFor(() => expect(result.current.data).toBe("first"));

    act(() => {
      result.current.refetch();
    });

    await waitFor(() => expect(result.current.data).toBe("second"));
    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  it("re-runs exactly once per key change (no double-fetch loop)", async () => {
    const fetcher = vi.fn().mockResolvedValue("data");
    const { rerender } = renderHook(
      ({ key }: { key: string }) => useCommandQuery([key], fetcher),
      { initialProps: { key: "a" } },
    );

    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1));

    rerender({ key: "a" });
    // Same key (by value) — must not trigger an extra fetch.
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(fetcher).toHaveBeenCalledTimes(1);

    rerender({ key: "b" });
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));

    rerender({ key: "b" });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(fetcher).toHaveBeenCalledTimes(2);
  });


  it("refetch resolves after data is committed", async () => {
    const fetcher = vi.fn().mockResolvedValueOnce("first").mockResolvedValueOnce("second");
    const { result } = renderHook(() => useCommandQuery(["key-a"], fetcher));

    await waitFor(() => expect(result.current.data).toBe("first"));

    // F4b S1: `refetch` becomes `() => Promise<void>` — today it returns
    // `void` (the `run` callback's return value), so this call site reddens
    // on the returned-value shape alone, independent of any microtask race.
    const returned = result.current.refetch();
    expect(returned).toBeInstanceOf(Promise);

    await act(async () => {
      await returned;
    });

    expect(result.current.data).toBe("second");
    expect(result.current.status).toBe("success");
  });

  it("resolves an overlapping refetch race to the NEWER request's data", async () => {
    const deferred: Array<(v: string) => void> = [];
    const fetcher = vi.fn(
      () => new Promise<string>((resolve) => deferred.push(resolve)),
    );
    const { result } = renderHook(() => useCommandQuery(["k"], fetcher));
    act(() => {
      result.current.refetch();
    });
    expect(fetcher).toHaveBeenCalledTimes(2);
    // The NEWER request resolves first; the OLDER settles late and must be
    // discarded by the per-run seq gate — this reddens if the gate is removed.
    await act(async () => {
      deferred[1]("newer");
      await Promise.resolve();
      deferred[0]("older-late");
      await Promise.resolve();
    });
    expect(result.current.status).toBe("success");
    expect(result.current.data).toBe("newer");
  });
});
