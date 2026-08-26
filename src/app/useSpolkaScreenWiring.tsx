import { useState } from "react";

import type { Company } from "../api/types";
import { SpolkaScreen } from "../screens/Spolka/SpolkaScreen";
import type { Tool as SpolkaTool } from "../screens/Spolka/route";

/**
 * Spółka screen wiring extracted from AppStateRoot (file-size ratchet, ADR
 * 0103) — mirrors `useTodayScreenWiring`'s split between a tiny state hook
 * and a host component assembled from the root's already-built controllers.
 */

// Spółka screen's workshop-bar request (F3a S1, ADR 0107): S1 only records
// which tool the user asked for — the tool host itself (dockview panes) is
// S2, so nothing reads this back yet.
export function useSpolkaTool() {
  const [, setSpolkaTool] = useState<SpolkaTool | null>(null);
  return setSpolkaTool;
}

type SpolkaScreenHostProps = {
  companies: Company[];
  selectedCompanyId: string | null;
  onOpenTool: (tool: SpolkaTool) => void;
  openInboxItem: (feedItemId: string, companyId: string) => void;
  refreshCompletionCount: number;
};

export function SpolkaScreenHost({
  companies,
  selectedCompanyId,
  onOpenTool,
  openInboxItem,
  refreshCompletionCount,
}: SpolkaScreenHostProps) {
  const spolkaCompany = companies.find((c) => c.id === selectedCompanyId);
  if (!spolkaCompany) return null;
  return (
    <SpolkaScreen
      companyId={spolkaCompany.id}
      company={spolkaCompany}
      onOpenTool={onOpenTool}
      // No root-level "open a report document by ref" seam exists yet
      // (repoctx search came up empty) — hosted in S2.
      onOpenDocument={() => {}}
      onOpenFeedItem={(feedItemId) => openInboxItem(feedItemId, spolkaCompany.id)}
      refreshCompletionCount={refreshCompletionCount}
    />
  );
}
