import { useCallback } from "react";

import {
  createAlertRule,
  deleteAlertRule,
  listAlertRules,
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
  rules: AlertRule[];
  companies: Company[];
  watchlists: Watchlist[];
};

function fetchAlertsLibrary(): Promise<AlertsLibraryData> {
  return Promise.all([listAlertRules(), listCompanies(), listWatchlists()]).then(
    ([rules, companies, watchlists]) => ({ rules, companies, watchlists }),
  );
}

export type AlertsSectionErrors = {
  rules?: "unavailable";
  events?: "unavailable";
};

export type AlertsQueryStatus = "loading" | "success" | "partial" | "error";

/**
 * Alerts screen data layer (F4a S4a, ADR 0106 dec. 2/4): the rule +
 * company/watchlist read goes through `useCommandQuery` (fetch-on-mount,
 * stale-response discard, explicit `refetch`, no cache-patching). Fired
 * events stay owned by the shared `AttentionController` (ADR 0097 dec. 6,
 * `src/app/useAttentionController.ts`) — merged in here as a second,
 * independent section instead of re-fetched, so Today/Alerts/the sidebar
 * badge keep exactly one event fetch. `status`/`sectionErrors` combine both
 * sections' outcomes (contract: `docs/plans/frontend-v2-f4a.md` § Alerts,
 * state row Partial). Mutation helpers await the command then refetch both
 * sections a rule change can affect (a rule's own list, and — because
 * `AttentionController` also holds `rulesById` and can cascade-drop the
 * rule's events — the attention controller too).
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
  if (library.status === "error") sectionErrors.rules = "unavailable";
  if (attention.error !== null) sectionErrors.events = "unavailable";

  const status: AlertsQueryStatus =
    library.status === "loading"
      ? "loading"
      : sectionErrors.rules && sectionErrors.events
        ? "error"
        : sectionErrors.rules || sectionErrors.events
          ? "partial"
          : "success";

  const createRule = useCallback(
    (input: NewAlertRule): Promise<AlertRule> =>
      createAlertRule(input).then((rule) => {
        refetch();
        return rule;
      }),
    [refetch],
  );

  const updateRulePrice = useCallback(
    (input: AlertRuleUpdate): Promise<AlertRule> =>
      updateAlertRule(input).then((rule) => {
        refetch();
        return rule;
      }),
    [refetch],
  );

  const setRuleEnabled = useCallback(
    (id: string, enabled: boolean): Promise<AlertRule> =>
      setAlertRuleEnabled(id, enabled).then((rule) => {
        refetch();
        return rule;
      }),
    [refetch],
  );

  const removeRule = useCallback(
    (id: string): Promise<void> => deleteAlertRule(id).then(() => refetch()),
    [refetch],
  );

  const { dismiss, markSeen } = attention;
  const dismissEvent = useCallback((id: string): Promise<void> => dismiss(id), [dismiss]);
  const markEventSeen = useCallback((id: string): Promise<void> => markSeen(id), [markSeen]);

  return {
    rules: library.data?.rules ?? [],
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
