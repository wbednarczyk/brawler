import { useCallback, useEffect, useRef, useState } from "react";

import { getActivitySummary, listActivity, type ActivitySummary, type ActivityView } from "../api/activity";

/**
 * The Activity center's app-level state (ADR 0109 dec. 6, plan § D4 item 1):
 * a topbar SUMMARY (active/queued counts, last-finished time) refreshed on
 * the shared 15 s scheduler-mirror tick (`useAppLifecycleEffects.ts`, a
 * SIBLING of `getSchedulerStatus()` — independent of its success) and after
 * the user's own source refresh; a panel VIEW (grouped items) refreshed on
 * open and every 5 s while open. Attention-controller posture
 * (`useAttentionController.ts`): one request at a time per resource, one
 * coalesced follow-up (a tick that lands mid-flight sets a pending flag
 * instead of stacking a second request), last-known-good state kept under a
 * typed `error`, no state update after unmount.
 */
export type ActivityController = {
  summary: ActivitySummary;
  /** `null` before the first successful `refreshView()` — the panel renders
   * its Skeleton state until this hydrates, exactly once (`hydrated`). */
  view: ActivityView | null;
  /** True only for the FIRST (pre-hydration) in-flight `refreshView()` — a
   * background 5 s poll never flashes the panel's skeleton. */
  loading: boolean;
  hydrated: boolean;
  /** The last read failure (summary OR view), raw backend message, or null.
   * Last-known-good `view`/`summary` stay rendered under it. */
  error: string | null;
  refreshSummary: () => void;
  refreshView: () => void;
  open: boolean;
  setOpen: (open: boolean) => void;
};

const EMPTY_SUMMARY: ActivitySummary = { active: 0, queued: 0, lastFinishedAt: null };
const VIEW_POLL_MS = 5_000;

/** One in-flight request + one coalesced follow-up for a single resource —
 * a tick arriving while a request is in flight sets `pending` instead of
 * issuing a second overlapping request; once the in-flight one settles, a
 * pending tick fires exactly one follow-up (never more, however many ticks
 * landed during the wait). */
function useCoalescedResource<T>(
  enabled: boolean,
  fetch: () => Promise<T>,
  onSettled: (result: { value: T; error: null } | { value: undefined; error: string }) => void,
) {
  const inFlightRef = useRef(false);
  const pendingRef = useRef(false);
  const mountedRef = useRef(true);
  const fetchRef = useRef(fetch);
  fetchRef.current = fetch;
  const onSettledRef = useRef(onSettled);
  onSettledRef.current = onSettled;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const run = useCallback(() => {
    if (!enabled) return;
    if (inFlightRef.current) {
      pendingRef.current = true;
      return;
    }
    inFlightRef.current = true;
    fetchRef
      .current()
      .then((value) => {
        if (!mountedRef.current) return;
        onSettledRef.current({ value, error: null });
      })
      .catch((cause) => {
        if (!mountedRef.current) return;
        onSettledRef.current({ value: undefined, error: String(cause) });
      })
      .finally(() => {
        inFlightRef.current = false;
        if (!mountedRef.current) return;
        if (pendingRef.current) {
          pendingRef.current = false;
          run();
        }
      });
  }, [enabled]);

  return run;
}

export function useActivityController({ enabled }: { enabled: boolean }): ActivityController {
  const [summary, setSummary] = useState<ActivitySummary>(EMPTY_SUMMARY);
  const [view, setView] = useState<ActivityView | null>(null);
  const [loading, setLoading] = useState(false);
  const [hydrated, setHydrated] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const hydratedRef = useRef(false);

  const refreshSummary = useCoalescedResource(enabled, getActivitySummary, (result) => {
    if (result.error === null) {
      setSummary(result.value);
      return;
    }
    // Last-known-good summary stays on screen; only the error surfaces.
    setError(result.error);
  });

  const refreshView = useCoalescedResource(enabled, listActivity, (result) => {
    setLoading(false);
    if (result.error === null) {
      setView(result.value);
      setError(null);
      hydratedRef.current = true;
      setHydrated(true);
      return;
    }
    // Last-known-good view stays on screen; only the error surfaces.
    setError(result.error);
  });

  const refreshViewWithLoading = useCallback(() => {
    if (!hydratedRef.current) {
      setLoading(true);
    }
    refreshView();
  }, [refreshView]);

  // Panel poll: on open, and every 5 s while it stays open — cleared on
  // close/unmount, never stacking (the coalescing above already prevents an
  // overlapping request; this only stops issuing NEW ticks).
  useEffect(() => {
    if (!open || !enabled) return undefined;
    refreshViewWithLoading();
    const intervalId = window.setInterval(refreshViewWithLoading, VIEW_POLL_MS);
    return () => {
      window.clearInterval(intervalId);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- refreshViewWithLoading's identity is stable (see useCoalescedResource); re-running this effect on every render would restart the poll interval
  }, [open, enabled]);

  return {
    summary,
    view,
    loading,
    hydrated,
    error,
    refreshSummary,
    refreshView,
    open,
    setOpen,
  };
}
