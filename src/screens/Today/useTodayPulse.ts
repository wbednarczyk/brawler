import { useCallback, useEffect, useMemo, useState } from "react";

import { listClaimsToVerify, type ClaimToVerify } from "../../api/managementClaims";
import {
  listAutopilotRuns,
  setAutopilotRunNotificationState,
  undoAutopilotRun as undoAutopilotRunCommand,
  type AutopilotRun,
} from "../../api/autopilot";
import {
  dismissAttentionEvent,
  listAlertRules,
  listAttentionEvents,
  markAttentionEventSeen,
  type AlertRule,
  type AttentionEvent,
} from "../../api/attention";
import type { Company } from "../../api/types";
import { useReportSeason } from "../ReportSeason/useReportSeason";

/** A claim awaiting verification, decorated with its company for the home digest. */
export type PulseClaim = {
  claim: ClaimToVerify["claim"];
  companyId: string;
  qualifiedTicker: string;
  displayName: string;
  bucket: "overdue" | "due";
};

export type TodayPulse = {
  season: ReturnType<typeof useReportSeason>;
  claims: PulseClaim[];
  claimsLoading: boolean;
  claimsError: string | null;
  /** Unread autopilot-run notifications (the North Star "what changed", ADR 0055). */
  autopilotRuns: AutopilotRun[];
  autopilotLoading: boolean;
  /** Dismiss one run's notification and drop it from the list. */
  dismissAutopilotRun: (runId: string) => void;
  /**
   * Revert exactly the facts a run produced (ADR 0055 §4 run-level undo).
   * Resolves once the pulse list reflects the reverted run (empty
   * `producedFactIds`); the count reverted is recorded for the "undone" badge.
   */
  undoAutopilotRun: (runId: string) => Promise<void>;
  /** `runId -> facts reverted` for runs undone this session (the "Undone" badge). */
  undoneAutopilotRuns: Record<string, number>;
  /** Open (non-dismissed) fired alerts (ADR 0068), newest-fired-first per company. */
  attentionEvents: AttentionEvent[];
  attentionLoading: boolean;
  /** `ruleId -> rule`, so a row can show its trigger's category/price context. */
  attentionRulesById: Map<string, AlertRule>;
  /** Optimistically drop the row, then persist the dismissal. */
  dismissAttentionEventRow: (id: string) => void;
  /** Optimistically flip `seen`, then persist it. Idempotent — safe to call on every open. */
  markAttentionEventSeenRow: (id: string) => void;
  /** Refetch the pinned-companies claims (per-category error-strip retry, ADR 0087). */
  retryClaims: () => void;
  /**
   * DISMISSED attention events, newest-first — the read-only Archive view (owner
   * 2026-07-23). Dismiss is an acknowledgement, never a delete: nothing is lost.
   * Loaded lazily (`loadArchive`) so an ordinary active session never pays for it.
   */
  archivedAttentionEvents: AttentionEvent[];
  archiveLoading: boolean;
  /** Fetch the dismissed archive on demand (idempotent per-open; re-runs a refetch). */
  loadArchive: () => void;
};

/**
 * Today/Pulse attention data (ADR 0054). Composes app-wide read models that
 * already exist: report season (recently-reported + upcoming, one call) and
 * claims-to-verify for the user's **pinned** companies (a bounded set — the
 * curated spine, not every watchlist company — so the home never fans out an
 * unbounded N+1). Full app-wide triage state (accept/snooze/dismiss) and the
 * autonomous-pipeline "one notification" land with the v0.48/v0.49 epics; this
 * is the home structure they plug into.
 */
export function useTodayPulse(pinnedCompanyIds: string[], companies: Company[]): TodayPulse {
  const season = useReportSeason(null);
  const [claims, setClaims] = useState<PulseClaim[]>([]);
  const [claimsLoading, setClaimsLoading] = useState(false);
  const [claimsError, setClaimsError] = useState<string | null>(null);
  // Bumped by `retryClaims` to re-run the claims effect on demand (error retry).
  const [claimsNonce, setClaimsNonce] = useState(0);
  const retryClaims = useCallback(() => setClaimsNonce((n) => n + 1), []);

  const [autopilotRuns, setAutopilotRuns] = useState<AutopilotRun[]>([]);
  const [autopilotLoading, setAutopilotLoading] = useState(false);
  const [undoneAutopilotRuns, setUndoneAutopilotRuns] = useState<Record<string, number>>({});

  const companyById = useMemo(
    () => new Map(companies.map((company) => [company.id, company])),
    [companies],
  );

  // Unread autopilot-run notifications surface here as first-class "what changed"
  // (ADR 0054/0055). Independent of the pinned set — a run is a notification.
  useEffect(() => {
    let cancelled = false;
    setAutopilotLoading(true);
    listAutopilotRuns({ notificationState: "unread", limit: 20 })
      .then((runs) => {
        if (!cancelled) {
          setAutopilotRuns(runs);
        }
      })
      .catch(() => {
        // A backend without autopilot data yet is not an error for the home.
        if (!cancelled) {
          setAutopilotRuns([]);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setAutopilotLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const dismissAutopilotRun = useCallback((runId: string) => {
    // Optimistically drop it, then persist the dismissal.
    setAutopilotRuns((runs) => runs.filter((run) => run.id !== runId));
    void setAutopilotRunNotificationState({ runId, notificationState: "dismissed" }).catch(() => {
      // Best-effort: a failed dismissal just reappears on next load.
    });
  }, []);

  // Run-level undo (ADR 0055 §4): unlike dismiss, this mutates real data
  // (deletes the facts the run produced), so the local pulse list only updates
  // after the command confirms — no optimistic delete of financial facts.
  const undoAutopilotRun = useCallback((runId: string) => {
    return undoAutopilotRunCommand(runId).then((result) => {
      setAutopilotRuns((runs) =>
        runs.map((run) => (run.id === runId ? { ...run, producedFactIds: [] } : run)),
      );
      setUndoneAutopilotRuns((prior) => ({ ...prior, [runId]: result.revertedFactIds.length }));
    });
  }, []);
  // Fired alerts (ADR 0068): app-wide, independent of the pinned set — like
  // autopilot runs, a fired alert is a notification, not scoped to the spine.
  // Loaded alongside its rules so a row/toast can render "what fired" (the
  // rule's category/price context), not just the bare event.
  const [attentionEvents, setAttentionEvents] = useState<AttentionEvent[]>([]);
  const [attentionRules, setAttentionRules] = useState<AlertRule[]>([]);
  const [attentionLoading, setAttentionLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setAttentionLoading(true);
    Promise.all([listAttentionEvents(), listAlertRules()])
      .then(([events, rules]) => {
        if (!cancelled) {
          setAttentionEvents(events);
          setAttentionRules(rules);
        }
      })
      .catch(() => {
        // A backend without attention data yet is not an error for the home.
        if (!cancelled) {
          setAttentionEvents([]);
          setAttentionRules([]);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setAttentionLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const attentionRulesById = useMemo(
    () => new Map(attentionRules.map((rule) => [rule.id, rule])),
    [attentionRules],
  );

  const dismissAttentionEventRow = useCallback((id: string) => {
    setAttentionEvents((events) => events.filter((event) => event.id !== id));
    void dismissAttentionEvent(id).catch(() => {
      // Best-effort: a failed dismissal just reappears on next load.
    });
  }, []);

  const markAttentionEventSeenRow = useCallback((id: string) => {
    setAttentionEvents((events) =>
      events.map((event) => (event.id === id ? { ...event, seen: true } : event)),
    );
    void markAttentionEventSeen(id).catch(() => {
      // Best-effort: a failed mark-seen just re-fires on next observation.
    });
  }, []);

  // Archive (dismissed events), loaded LAZILY: the effect runs only once the view
  // switches to Archive (archiveNonce > 0), so an active-only session never fetches
  // it. `includeDismissed` returns both states; keep only the dismissed ones,
  // newest-first (the backend already orders by firedAt desc). Independent state,
  // so archive data NEVER reaches the toast wiring (which reads `attentionEvents`).
  const [archivedAttentionEvents, setArchivedAttentionEvents] = useState<AttentionEvent[]>([]);
  const [archiveLoading, setArchiveLoading] = useState(false);
  const [archiveNonce, setArchiveNonce] = useState(0);
  const loadArchive = useCallback(() => setArchiveNonce((n) => n + 1), []);

  useEffect(() => {
    if (archiveNonce === 0) return;
    let cancelled = false;
    setArchiveLoading(true);
    listAttentionEvents({ companyId: null, includeDismissed: true })
      .then((events) => {
        if (!cancelled) setArchivedAttentionEvents(events.filter((event) => event.dismissed));
      })
      .catch(() => {
        if (!cancelled) setArchivedAttentionEvents([]);
      })
      .finally(() => {
        if (!cancelled) setArchiveLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [archiveNonce]);

  // Key on the joined id list so the effect re-runs when the pin set changes,
  // not on every render (settings hands us a fresh array each time).
  const pinnedKey = pinnedCompanyIds.join(",");

  useEffect(() => {
    const ids = pinnedKey ? pinnedKey.split(",") : [];
    if (ids.length === 0) {
      setClaims([]);
      setClaimsLoading(false);
      setClaimsError(null);
      return;
    }

    let cancelled = false;
    setClaimsLoading(true);
    setClaimsError(null);

    Promise.all(
      ids.map(async (id) => {
        const result = await listClaimsToVerify(id);
        const company = companyById.get(id);
        const decorate = (item: ClaimToVerify, bucket: "overdue" | "due"): PulseClaim => ({
          claim: item.claim,
          companyId: id,
          qualifiedTicker: company?.qualifiedTicker ?? "",
          displayName: company?.displayName ?? "",
          bucket,
        });
        return [
          ...result.overdue.map((item) => decorate(item, "overdue")),
          ...result.due.map((item) => decorate(item, "due")),
        ];
      }),
    )
      .then((perCompany) => {
        if (!cancelled) {
          setClaims(perCompany.flat());
        }
      })
      .catch((cause) => {
        if (!cancelled) {
          setClaimsError(cause instanceof Error ? cause.message : String(cause));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setClaimsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [pinnedKey, companyById, claimsNonce]);

  return {
    season,
    claims,
    claimsLoading,
    claimsError,
    autopilotRuns,
    autopilotLoading,
    dismissAutopilotRun,
    undoAutopilotRun,
    undoneAutopilotRuns,
    attentionEvents,
    attentionLoading,
    attentionRulesById,
    dismissAttentionEventRow,
    markAttentionEventSeenRow,
    retryClaims,
    archivedAttentionEvents,
    archiveLoading,
    loadArchive,
  };
}
