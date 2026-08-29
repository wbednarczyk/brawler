import { createElement, type Dispatch, type SetStateAction } from "react";
import { AlertsScreen } from "../screens/Alerts/AlertsScreen";

import type { AttentionController } from "./useAttentionController";
import type { Section } from "./navigation";

type AlertsScreenWiringInput = {
  attention: AttentionController;
  openCompanyWorkspaceById: (companyId: string) => void;
  setActiveSection: Dispatch<SetStateAction<Section>>;
};

/**
 * Composes the Alerts screen's props (F4a S4b) — extracted from AppStateRoot
 * (file-size ratchet, ADR 0103; same composer pattern as
 * `buildWatchlistsScreenProps`/`buildTodayScreenProps`). `openInbox` wraps
 * `setActiveSection("Inbox")`, mirroring Today's own `openInbox` wiring
 * (`useTodayScreenWiring.ts`) — the fired-row fallback destination for a
 * company-less SYSTEM event.
 */
export function buildAlertsScreenProps(input: AlertsScreenWiringInput) {
  return {
    attention: input.attention,
    openCompanyWorkspaceById: input.openCompanyWorkspaceById,
    openInbox: () => input.setActiveSection("Inbox"),
  };
}

/** Mounts the Alerts screen from the root's attention controller + navigation. */
export function AlertsScreenHost(input: AlertsScreenWiringInput) {
  return createElement(AlertsScreen, buildAlertsScreenProps(input));
}
