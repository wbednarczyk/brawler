import { useEffect, useState } from "react";
import type { Company, FeedItem } from "../api/types";
import { SpolkaScreen } from "../screens/Spolka/SpolkaScreen";
import { useSpolkaToolHost, type SpolkaToolHostApi } from "../screens/Spolka/ToolHost";

/**
 * Spółka screen wiring extracted from AppStateRoot (file-size ratchet, ADR
 * 0103) — mirrors `useTodayScreenWiring`'s split between a tiny state hook
 * and a host component assembled from the root's already-built controllers.
 */

// The Spółka workshop's tool-host state (F3a S2, ADR 0107): ONE instance,
// owned above AppShell so cross-screen navigation (`guardedSetActiveSection`
// in AppStateRoot) and the window-close interceptor (`useAppLifecycleEffects`)
// can both gate through the same `guardNavigation`/`isDirty` — see
// `../screens/Spolka/ToolHost.tsx`.
export function useSpolkaTool(): SpolkaToolHostApi {
  return useSpolkaToolHost();
}

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
    spolkaTool.guardNavigation(() => setDisplayedCompanyId(selectedCompanyId));
    // eslint-disable-next-line react-hooks/exhaustive-deps -- guardNavigation is stable (useCallback); re-running on its identity would re-trigger the gate spuriously
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
      // No root-level "open a report document by ref" seam exists yet
      // (repoctx search came up empty) — hosted in a later slice.
      onOpenDocument={() => {}}
      onOpenFeedItem={(feedItemId) => openInboxItem(feedItemId, spolkaCompany.id)}
      refreshCompletionCount={refreshCompletionCount}
    />
  );
}
