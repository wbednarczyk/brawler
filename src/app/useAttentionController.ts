import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  dismissAttentionEvent,
  listAlertRules,
  listAttentionEvents,
  markAttentionEventSeen,
  markAttentionEventsSeen,
  type AlertRule,
  type AttentionEvent,
} from "../api/attention";

/**
 * THE app-level owner of active (non-dismissed) attention events (ADR 0097
 * dec. 6). Today's stream, the Alerts fired list, and the sidebar Today badge
 * all consume this one state — per-screen copies drifted (a screen's optimistic
 * seen/dismiss left the others stale), and the badge would have been a third.
 *
 * Refresh runs on every event-producing seam: startup (behind the license
 * gate), the shared post-source-refresh view update (manual all-source,
 * single-source), and the scheduler mirror when a background refresh fired
 * (which also covers job-failure events raised by background work). A
 * request-generation guard drops out-of-order responses so a slow older fetch
 * can never overwrite a newer optimistic mutation or refresh.
 */
export type AttentionController = {
  /** Active (non-dismissed) events, newest-first — the backend list order. */
  events: AttentionEvent[];
  /** `ruleId -> rule`, so rows can show the owning rule's trigger context. */
  rulesById: Map<string, AlertRule>;
  loading: boolean;
  /**
   * True once the first fetch settled — the shell's polite live region only
   * announces count INCREASES after hydration, so the startup backlog is shown
   * (badge) but never replayed as an announcement (ADR 0097 dec. 4).
   */
  hydrated: boolean;
  /**
   * Unseen non-routine (`urgent` + `notable`) events — the sidebar Today badge
   * (ADR 0097 dec. 4). Scope is attention events only: autopilot runs carry
   * their own notificationState, claims have no seen flag (due work, not news).
   */
  unseenCount: number;
  /** Refetch events + rules (request-generation guarded). */
  refresh: () => void;
  /** Optimistically flip `seen`, then persist. Idempotent. */
  markSeen: (id: string) => void;
  /**
   * Batch "was on screen" marking (ADR 0097 dec. 5): Today calls this for every
   * loaded unseen event when its stream renders, so the badge clears on a
   * visit. One IPC call, optimistic local flip.
   */
  markManySeen: (ids: string[]) => void;
  /** Optimistically drop the event, then persist the dismissal. */
  dismiss: (id: string) => void;
};

export function useAttentionController(licenseCanUseApp: boolean): AttentionController {
  const [events, setEvents] = useState<AttentionEvent[]>([]);
  const [rules, setRules] = useState<AlertRule[]>([]);
  const [loading, setLoading] = useState(false);
  const [hydrated, setHydrated] = useState(false);
  // Request generation: bumped by every refresh AND every optimistic mutation,
  // so a fetch that started before the mutation can no longer land on top of it.
  const generationRef = useRef(0);
  const [refreshNonce, setRefreshNonce] = useState(0);

  const refresh = useCallback(() => setRefreshNonce((nonce) => nonce + 1), []);

  useEffect(() => {
    if (!licenseCanUseApp) {
      return;
    }
    const generation = (generationRef.current += 1);
    setLoading(true);
    Promise.all([listAttentionEvents(), listAlertRules()])
      .then(([nextEvents, nextRules]) => {
        if (generationRef.current !== generation) {
          return;
        }
        setEvents(nextEvents);
        setRules(nextRules);
      })
      .catch(() => {
        // A backend without attention data yet is not an error for the shell.
        if (generationRef.current !== generation) {
          return;
        }
        setEvents([]);
        setRules([]);
      })
      .finally(() => {
        if (generationRef.current === generation) {
          setLoading(false);
        }
        setHydrated(true);
      });
  }, [licenseCanUseApp, refreshNonce]);

  const markSeen = useCallback((id: string) => {
    generationRef.current += 1;
    setEvents((current) =>
      current.map((event) => (event.id === id ? { ...event, seen: true } : event)),
    );
    void markAttentionEventSeen(id).catch(() => {
      // Best-effort: a failed mark-seen just re-counts on the next refresh.
    });
  }, []);

  const markManySeen = useCallback((ids: string[]) => {
    if (ids.length === 0) {
      return;
    }
    generationRef.current += 1;
    const idSet = new Set(ids);
    setEvents((current) =>
      current.map((event) => (idSet.has(event.id) ? { ...event, seen: true } : event)),
    );
    void markAttentionEventsSeen(ids).catch(() => {
      // Best-effort: a failed batch just re-counts on the next refresh.
    });
  }, []);

  const dismiss = useCallback((id: string) => {
    generationRef.current += 1;
    setEvents((current) => current.filter((event) => event.id !== id));
    void dismissAttentionEvent(id).catch(() => {
      // Best-effort: a failed dismissal reappears on the next refresh.
    });
  }, []);

  const rulesById = useMemo(() => new Map(rules.map((rule) => [rule.id, rule])), [rules]);

  const unseenCount = useMemo(
    () =>
      events.filter(
        (event) => !event.seen && !event.dismissed && event.severity !== "routine",
      ).length,
    [events],
  );

  return useMemo(
    () => ({
      events,
      rulesById,
      loading,
      hydrated,
      unseenCount,
      refresh,
      markSeen,
      markManySeen,
      dismiss,
    }),
    [events, rulesById, loading, hydrated, unseenCount, refresh, markSeen, markManySeen, dismiss],
  );
}
