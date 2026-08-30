import { useCallback, useEffect, useRef, useState } from "react";

export type CommandQueryState<T> =
  | { status: "loading"; data: null; error: null }
  | { status: "success"; data: T; error: null }
  | { status: "error"; data: null; error: unknown };

const LOADING = { status: "loading", data: null, error: null } as const;

/**
 * Thin shared self-fetch hook (ADR 0106 dec. 2) formalizing the pattern
 * already hand-rolled by `FundamentalsPanel`/`ReportDiffPanel`/etc: fetch on
 * mount and on `key` change, discard any response that resolves after a
 * newer request started (the `useAttentionController` requestSeqRef idiom),
 * expose `refetch()`. No cache between mounts, no timers, no listeners
 * (ADR 0106 dec. 4 — invalidation is key-change + explicit refetch only).
 *
 * `key` is compared by JSON.stringify — pass primitives/plain-serializable
 * values (e.g. `[companyId]`), not functions/class instances/undefined-y
 * shapes that don't round-trip through JSON.
 *
 * `refetch()` returns a `Promise<void>` that resolves once its own response
 * has been processed (state committed on success, or discarded as stale by
 * the request-seq gate) — F4b S1, so mutation call sites can
 * `await refetch()` before reading the reloaded state.
 *
 * Lives under `src/shared/state` (moved from `src/app/state` in F1 S4): it
 * has no AppStateRoot/composition-root coupling — a plain React hook over a
 * fetcher callback — and `src/shared/components` consumers (this hook's
 * whole reason to exist per its own docstring) cannot import `src/app`
 * (the frontend layer contract, docs/modularization-design.md § Frontend
 * layer contract, ESLint-enforced `no-restricted-imports`).
 */
export function useCommandQuery<T>(
  key: readonly unknown[],
  fetcher: () => Promise<T>,
): CommandQueryState<T> & { refetch: () => Promise<void> } {
  const [state, setState] = useState<CommandQueryState<T>>(LOADING);
  const requestSeqRef = useRef(0);
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;
  const keyJson = JSON.stringify(key);

  const run = useCallback(async () => {
    const requestSeq = (requestSeqRef.current += 1);
    setState(LOADING);
    try {
      const data = await fetcherRef.current();
      if (requestSeqRef.current === requestSeq) {
        setState({ status: "success", data, error: null });
      }
    } catch (error: unknown) {
      if (requestSeqRef.current === requestSeq) {
        setState({ status: "error", data: null, error });
      }
    }
  }, []);

  useEffect(() => {
    void run();
    // No cleanup on purpose: every `run()` bumps the seq, so any earlier
    // in-flight response is already discarded observably, and React 18+
    // silently no-ops a setState that lands after unmount — an unmount
    // cleanup would guard nothing a behavioral test could redden on
    // (sol F1 round-3 verdict; StrictMode's dev double-mount just costs one
    // extra ~1 ms local IPC call).
    // eslint-disable-next-line react-hooks/exhaustive-deps -- keyed on keyJson (the documented comparison), not `run`/`key` identity
  }, [keyJson]);

  return { ...state, refetch: run };
}
