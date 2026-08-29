import { useCallback } from "react";

import {
  createAlertRule,
  deleteAlertRule,
  setAlertRuleEnabled,
  updateAlertRule,
  type AlertRule,
  type AlertRuleUpdate,
  type NewAlertRule,
} from "../../api/attention";
import { listCompanies } from "../../api/companies";
import { listWatchlists } from "../../api/watchlists";
import type { Company, Watchlist } from "../../api/types";
import type { AttentionController } from "../../app/useAttentionController";
import { useCommandQuery } from "../../shared/state/useCommandQuery";

export type AlertsLibraryData = {
  companies: Company[];
  watchlists: Watchlist[];
};

function fetchAlertsLibrary(): Promise<AlertsLibraryData> {
  return Promise.all([listCompanies(), listWatchlists()]).then(([companies, watchlists]) => ({
    companies,
    watchlists,
  }));
}

export type AlertsSectionErrors = {
  /** Rules and events share the attention controller: one read, one failure. */
  rules?: "unavailable";
  events?: "unavailable";
  /** Company/watchlist names for the scope picker and labels (this hook's own read). */
  scope?: "unavailable";
};

export type AlertsQueryStatus = "loading" | "success" | "partial" | "error";

/**
 * Alerts screen data layer (ADR 0106 dec. 2/4; ADR 0097 dec. 6). Rules are owned by the shared `AttentionController` — the
 * SAME single fetch Today and the sidebar badge read, never a second
 * `listAlertRules` copy that could drift. This hook fetches only what the
 * controller does not own: companies/watchlists, needed for the composer's
 * scope picker and for resolving a rule's/event's scope name. `status`
 * combines both sections' outcomes and never reports `success` before the
 * controller has hydrated (contract: `docs/plans/frontend-v2-f4a.md` §
 * Alerts state row Partial; ADR 0097's false-quiet ban — the "All quiet"
 * empty state must never render pre-hydration). Mutation helpers await the
 * command then call `attention.refresh()` — one owner, one refetch; the
 * rendered rule list converges because it reads `attention.rules` directly,
 * not a local copy.
 */
export function useAlertsQuery(attention: AttentionController) {
  const library = useCommandQuery(["alertsLibrary"], fetchAlertsLibrary);

  const refetch = useCallback(() => {
    library.refetch();
    attention.refresh();
    // library.refetch/attention.refresh are stable (useCallback, empty deps
    // in both useCommandQuery and useAttentionController) — omitted from
    // deps to avoid re-creating this on every render for no behavioral gain.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const sectionErrors: AlertsSectionErrors = {};
  if (library.status === "error") sectionErrors.scope = "unavailable";
  if (attention.error !== null) {
    sectionErrors.rules = "unavailable";
    sectionErrors.events = "unavailable";
  }

  // Never `success` (or, downstream, the quiet empty state) before the
  // controller has hydrated once (ADR 0097 dec. 6, the false-quiet ban). A
  // failed first read leaves the controller unhydrated WITH an error — that
  // is the error state with Retry, never an endless skeleton.
  const attentionPending = attention.loading || (!attention.hydrated && attention.error === null);
  const status: AlertsQueryStatus =
    attentionPending || library.status === "loading"
      ? "loading"
      : sectionErrors.rules && sectionErrors.events && sectionErrors.scope
        ? "error"
        : sectionErrors.rules || sectionErrors.events || sectionErrors.scope
          ? "partial"
          : "success";

  const { refresh: refreshAttention } = attention;

  const createRule = useCallback(
    (input: NewAlertRule): Promise<AlertRule> =>
      createAlertRule(input).then((rule) => {
        refreshAttention();
        return rule;
      }),
    [refreshAttention],
  );

  const updateRulePrice = useCallback(
    (input: AlertRuleUpdate): Promise<AlertRule> =>
      updateAlertRule(input).then((rule) => {
        refreshAttention();
        return rule;
      }),
    [refreshAttention],
  );

  const setRuleEnabled = useCallback(
    (id: string, enabled: boolean): Promise<AlertRule> =>
      setAlertRuleEnabled(id, enabled).then((rule) => {
        refreshAttention();
        return rule;
      }),
    [refreshAttention],
  );

  const removeRule = useCallback(
    (id: string): Promise<void> => deleteAlertRule(id).then(() => refreshAttention()),
    [refreshAttention],
  );

  const { dismiss, markSeen } = attention;
  const dismissEvent = useCallback((id: string): Promise<void> => dismiss(id), [dismiss]);
  const markEventSeen = useCallback((id: string): Promise<void> => markSeen(id), [markSeen]);

  return {
    rules: attention.rules,
    companies: library.data?.companies ?? [],
    watchlists: library.data?.watchlists ?? [],
    events: attention.events,
    status,
    sectionErrors,
    libraryError: library.error,
    eventsError: attention.error,
    refetch,
    createRule,
    updateRulePrice,
    setRuleEnabled,
    removeRule,
    dismissEvent,
    markEventSeen,
  };
}

export type AlertsQueryResult = ReturnType<typeof useAlertsQuery>;
