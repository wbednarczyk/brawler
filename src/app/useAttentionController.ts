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
 * Freshness: refresh runs on startup (behind the license gate), on the shared
 * post-source-refresh view update (manual all-source, single-source), and on
 * EVERY scheduler-mirror poll tick — background work (autopilot completions,
 * terminal job failures) raises events with no frontend-visible trigger of its
 * own, so the poll is the seam that keeps the badge honest without a push
 * channel. A poll that returns identical data skips the state update, so the
 * steady state causes no re-renders.
 *
 * Consistency: requests and mutations are sequenced separately. Only the
 * newest request may apply its response or settle `loading`/`hydrated`; a
 * response that raced past an optimistic mutation is discarded and replaced by
 * a scheduled re-fetch (state converges, nothing sticks). A failed fetch keeps
 * the last-known-good events (never a false quiet/zero badge) and surfaces
 * `error` for the consuming screens' error strips. Mutations return their
 * persistence promise — a failure re-syncs from the backend and rethrows so a
 * screen can surface it.
 */
export type AttentionController = {
  /** Active (non-dismissed) events, newest-first — the backend list order. */
  events: AttentionEvent[];
  /**
   * Active alert rules, backend order — the Alerts screen's rules section
   * reads this directly instead of re-fetching its own copy (ADR 0097
   * dec. 6: one owner; ).
   */
  rules: AlertRule[];
  /** `ruleId -> rule`, so rows can show the owning rule's trigger context. */
  rulesById: Map<string, AlertRule>;
  loading: boolean;
  /**
   * True once a fetch APPLIED its data — the shell's polite live region only
   * announces count increases after hydration, so the startup backlog is shown
   * (badge) but never replayed as an announcement (ADR 0097 dec. 4).
   */
  hydrated: boolean;
  /**
   * The last fetch failure (raw backend message), or null. Last-known-good
   * events stay rendered under it — an errored attention state must never look
   * quiet (the ADR 0081 Q9 posture).
   */
  error: string | null;
  /**
   * Unseen non-routine (`urgent` + `notable`) events — the sidebar Today badge
   * (ADR 0097 dec. 4). Scope is attention events only: autopilot runs carry
   * their own notificationState, claims have no seen flag (due work, not news).
   */
  unseenCount: number;
  /** Refetch events + rules (sequenced; safe to call from any seam). */
  refresh: () => void;
  /** Optimistically flip `seen`, then persist. Rejects after re-syncing on failure. */
  markSeen: (id: string) => Promise<void>;
  /**
   * Batch "was on screen" marking (ADR 0097 dec. 5): Today calls this for every
   * loaded unseen event when its stream renders, so the badge clears on a
   * visit. One IPC call, optimistic local flip; a failure re-syncs (the badge
   * relights honestly) and rejects.
   */
  markManySeen: (ids: string[]) => Promise<void>;
  /** Optimistically drop the event, then persist. Rejects after re-syncing on failure. */
  dismiss: (id: string) => Promise<void>;
};

/**
 * Full-fidelity change detector so steady-state polls preserve state identity.
 * Serialized comparison covers EVERY field (a reconciliation re-run can update
 * `witnessUrl` on the same event id — a hand-picked field list silently
 * discarded exactly the field Review routes on) and stays complete as the
 * payload grows. Both sides come off the same serde/mock serializers, so key
 * order is stable; the lists are small (active events + rules).
 */
function sameCollection<T>(a: T[], b: T[]): boolean {
  return a.length === b.length && JSON.stringify(a) === JSON.stringify(b);
}

export function useAttentionController(licenseCanUseApp: boolean): AttentionController {
  const [events, setEvents] = useState<AttentionEvent[]>([]);
  const [rules, setRules] = useState<AlertRule[]>([]);
  const [loading, setLoading] = useState(false);
  const [hydrated, setHydrated] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Request sequence: only the NEWEST request may apply data or settle
  // loading/hydrated — an older in-flight response is ignored entirely.
  const requestSeqRef = useRef(0);
  // Mutation sequence: a response from a request started BEFORE the latest
  // optimistic mutation must not overwrite it — it is discarded and a re-fetch
  // is scheduled instead, so state converges without clobbering the mutation.
  const mutationSeqRef = useRef(0);
  // The chain of in-flight persistence writes (errors swallowed here — the
  // mutation's own caller handles them). A convergence re-fetch waits on it so
  // it can never read the backend BEFORE the optimistic write committed and
  // accept pre-mutation data.
  const mutationChainRef = useRef<Promise<void>>(Promise.resolve());
  // In-flight coalescing: one fetch at a time; refreshes arriving mid-flight
  // fold into a single follow-up instead of stacking superseding requests.
  const inFlightRef = useRef(false);
  const pendingRef = useRef(false);
  // Mirror of `hydrated` readable synchronously inside the effect: background
  // refreshes after hydration are SILENT (no loading flip) — a 15s poll must
  // never flash Today's skeleton.
  const hydratedRef = useRef(false);
  const [refreshNonce, setRefreshNonce] = useState(0);

  const refresh = useCallback(() => setRefreshNonce((nonce) => nonce + 1), []);

  useEffect(() => {
    if (!licenseCanUseApp) {
      return;
    }
    if (inFlightRef.current) {
      pendingRef.current = true;
      return;
    }
    inFlightRef.current = true;
    const requestSeq = (requestSeqRef.current += 1);
    if (!hydratedRef.current) {
      setLoading(true);
    }
    const settle = () => {
      inFlightRef.current = false;
      if (pendingRef.current) {
        pendingRef.current = false;
        setRefreshNonce((nonce) => nonce + 1);
      }
    };
    // EVERY fetch waits for the persistence chain first — a read that starts
    // before an optimistic write commits would capture the post-mutation
    // sequence yet pre-mutation backend data and be accepted. The loop makes
    // wait + snapshot atomic: if a NEW mutation was appended while we waited,
    // wait again on the extended chain, and only then snapshot the sequence —
    // so every write known at read time has committed, and anything later
    // counts as "raced past this request" and triggers convergence.
    let mutationSeqAtStart = 0;
    // Returns the sequence SYNCHRONOUSLY with the loop's final still-latest
    // check — a later `.then` would open a microtask gap in which a fresh
    // mutation could bump the sequence past its own un-awaited write.
    const awaitMutationChainAndSnapshot = async (): Promise<number> => {
      let chain: Promise<void>;
      do {
        chain = mutationChainRef.current;
        await chain;
      } while (chain !== mutationChainRef.current);
      return mutationSeqRef.current;
    };
    awaitMutationChainAndSnapshot()
      .then((seq) => {
        mutationSeqAtStart = seq;
        return Promise.all([listAttentionEvents(), listAlertRules()]);
      })
      .then(([nextEvents, nextRules]) => {
        settle();
        if (requestSeqRef.current !== requestSeq) {
          return; // a newer request owns the state now
        }
        if (mutationSeqRef.current !== mutationSeqAtStart) {
          // A mutation landed while this fetch was in flight — its data is
          // stale against the optimistic state. Converge via a fresh fetch,
          // but only AFTER the mutation's write committed (else the re-read
          // could accept pre-mutation backend data).
          void mutationChainRef.current.then(() => setRefreshNonce((nonce) => nonce + 1));
          return;
        }
        setEvents((current) => (sameCollection(current, nextEvents) ? current : nextEvents));
        setRules((current) => (sameCollection(current, nextRules) ? current : nextRules));
        setError(null);
        setLoading(false);
        hydratedRef.current = true;
        setHydrated(true);
      })
      .catch((cause) => {
        settle();
        if (requestSeqRef.current !== requestSeq) {
          return;
        }
        // Keep the last-known-good events — a transient read failure must
        // never blank the badge or render a false quiet state. `hydrated`
        // stays false until a fetch APPLIES data: if startup errors and a
        // later poll recovers, the recovered backlog is still hydration, not
        // an announcement (ADR 0097 dec. 4).
        setError(String(cause));
        setLoading(false);
      });
  }, [licenseCanUseApp, refreshNonce]);

  // A mutation failure re-syncs from the backend (the optimistic flip may be a
  // lie) and rethrows so the calling screen can surface it. Every persistence
  // promise also joins the mutation chain the convergence re-fetch awaits.
  const persistMutation = useCallback(
    (persist: Promise<void>) => {
      mutationChainRef.current = mutationChainRef.current
        .then(() => persist)
        .catch(() => {});
      return persist.catch((cause) => {
        refresh();
        throw cause;
      });
    },
    [refresh],
  );

  const markSeen = useCallback(
    (id: string) => {
      mutationSeqRef.current += 1;
      setEvents((current) =>
        current.map((event) => (event.id === id ? { ...event, seen: true } : event)),
      );
      return persistMutation(markAttentionEventSeen(id));
    },
    [persistMutation],
  );

  const markManySeen = useCallback(
    (ids: string[]) => {
      if (ids.length === 0) {
        return Promise.resolve();
      }
      mutationSeqRef.current += 1;
      const idSet = new Set(ids);
      setEvents((current) =>
        current.map((event) => (idSet.has(event.id) ? { ...event, seen: true } : event)),
      );
      return persistMutation(markAttentionEventsSeen(ids));
    },
    [persistMutation],
  );

  const dismiss = useCallback(
    (id: string) => {
      mutationSeqRef.current += 1;
      setEvents((current) => current.filter((event) => event.id !== id));
      return persistMutation(dismissAttentionEvent(id));
    },
    [persistMutation],
  );

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
      rules,
      rulesById,
      loading,
      hydrated,
      error,
      unseenCount,
      refresh,
      markSeen,
      markManySeen,
      dismiss,
    }),
    [
      events,
      rules,
      rulesById,
      loading,
      hydrated,
      error,
      unseenCount,
      refresh,
      markSeen,
      markManySeen,
      dismiss,
    ],
  );
}
