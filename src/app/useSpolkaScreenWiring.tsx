import { useEffect, useState } from "react";
import type { Company, FeedItem } from "../api/types";
import { SpolkaScreen } from "../screens/Spolka/SpolkaScreen";
import type { SpolkaToolHostApi } from "../screens/Spolka/ToolHost";
export { useSpolkaToolHost } from "../screens/Spolka/ToolHost";
import type { Tool } from "../screens/Spolka/route";
import type { Section } from "./navigation";

/**
 * Spółka screen wiring extracted from AppStateRoot (file-size ratchet, ADR
 * 0103) — mirrors `useTodayScreenWiring`'s split between a tiny state hook
 * and a host component assembled from the root's already-built controllers.
 */

// One atomic company-workspace transition (F3a S2, ADR 0107, sol R1 finding
// 3): companyId + section + optional tool + optional claim highlight,
// committed together behind ONE dirty guard — never `selectedCompanyId` set
// ahead of the guard. Built by `AppStateRoot`'s `navigate` (it needs setters
// this file doesn't own) and called by every company-workspace entry point
// (`openCompanyWorkspaceById`, `openCompanyWorkspace`, `openCompanyClaims`,
// pinned row, GlobalSearch, palette "Open company:", Today "Open thesis").
export type SpolkaTransition = {
  companyId: string;
  section: Section;
  tool?: Tool;
  highlightClaimId?: string;
  /** Only for `section: "Cockpit"` — the named view to show (null = the
   * company-scoped legacy dashboard). Committed inside the same guard. */
  cockpitLayoutId?: string | null;
};


type SpolkaScreenHostProps = {
  companies: Company[];
  selectedCompanyId: string | null;
  spolkaTool: SpolkaToolHostApi;
  feedItems: FeedItem[];
  rootHighlightClaimId: string | null;
  openInboxItem: (feedItemId: string, companyId: string) => void;
  refreshCompletionCount: number;
};

export function SpolkaScreenHost({
  companies,
  selectedCompanyId,
  spolkaTool,
  feedItems,
  rootHighlightClaimId,
  openInboxItem,
  refreshCompletionCount,
}: SpolkaScreenHostProps) {
  // Switching company asks the dirty gate BEFORE the new company displaces
  // the old one (F3a S2, ADR 0107, plan §11 "Switch spółki"): a clean tool
  // closes and the switch proceeds immediately; a dirty tool keeps showing
  // the OLD company (draft untouched) until the user stays or discards.
  const [displayedCompanyId, setDisplayedCompanyId] = useState(selectedCompanyId);
  useEffect(() => {
    if (selectedCompanyId === displayedCompanyId) return;
    if (!selectedCompanyId) {
      setDisplayedCompanyId(null);
      return;
    }
    // `selectedCompanyId` already passed the guard once (`commitTool`/the
    // app-level `navigate`, sol R1 finding 3) when it was committed as part
    // of an atomic transition — guarding AGAIN here raced this effect's own
    // gate against the root's `aria-current` (which reflects
    // `selectedCompanyId` immediately), so a dirty Stay could leave the NEW
    // company selected in the sidebar while the core still rendered the old
    // one. A caller that bypasses `navigate` and sets `selectedCompanyId`
    // directly still needs this guard as a safety net.
    if (spolkaTool.lastGuardedCompanyIdRef.current === selectedCompanyId) {
      setDisplayedCompanyId(selectedCompanyId);
      return;
    }
    spolkaTool.guardNavigation(() => setDisplayedCompanyId(selectedCompanyId));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- guardNavigation/lastGuardedCompanyIdRef are stable (useCallback/ref); re-running on spolkaTool's identity would re-trigger the gate spuriously
  }, [selectedCompanyId, displayedCompanyId]);

  const spolkaCompany = companies.find((c) => c.id === displayedCompanyId);
  if (!spolkaCompany) return null;
  return (
    <SpolkaScreen
      companyId={spolkaCompany.id}
      company={spolkaCompany}
      spolkaTool={spolkaTool}
      feedItems={feedItems}
      rootHighlightClaimId={rootHighlightClaimId}
      // A KPI provenance ticket opens the `dokumenty` tool with its source
      // document highlighted (sol R1 finding 8) — the SAME company, so this
      // goes through `openTool` (already its own guard), not the atomic
      // `navigate` (which is for a company/section switch).
      onOpenDocument={(documentRef) => spolkaTool.openTool(spolkaCompany.id, { t: "dokumenty", documentId: documentRef })}
      onOpenFeedItem={(feedItemId) => openInboxItem(feedItemId, spolkaCompany.id)}
      refreshCompletionCount={refreshCompletionCount}
    />
  );
}
