import { useCallback, useState } from "react";

import type { Company, SourceAdapter } from "../api/types";
import type { CompanyWorkspaceTab } from "../screens/Companies/companyTypes";
import type { TodayScreenProps } from "../screens/Today/TodayScreen";
import type { AttentionController } from "./useAttentionController";
import type { Section } from "./navigation";

/**
 * Today/nav wiring extracted from AppStateRoot (fix wave B finding 12 — the
 * plan's micro-extraction mandate over another file-size-ratchet pin raise,
 * ADR 0106). Two pieces, split across the render because of a real ordering
 * constraint: the refresh-completion COUNTER must exist before
 * `useSourceRefreshController` is constructed (it consumes the bump
 * callback), while the screen's full PROPS object can only be assembled once
 * every dependency it composes — the nav callbacks, `refreshSources` itself —
 * already exists, later in the same render.
 */

/**
 * The refresh-completion signal `get_today_view`'s query key coordinates on
 * (fix wave B finding 1a): increments once per COMPLETED source refresh,
 * independent of whether the root-fed attention state's own identity
 * happened to change on that pass — the old `["todayView", attention.events]`
 * key relied on attention's array changing, which a same-shape poll leaves
 * untouched even after a refresh that ingested new filings/media.
 */
export function useRefreshCompletionSignal() {
  const [refreshCompletionCount, setRefreshCompletionCount] = useState(0);
  const bumpRefreshCompletionCount = useCallback(() => setRefreshCompletionCount((count) => count + 1), []);
  return { refreshCompletionCount, bumpRefreshCompletionCount };
}

type TodayScreenPropsInput = {
  attention: AttentionController;
  companies: Company[];
  openCompanyWorkspace: (companyId: string, tab: CompanyWorkspaceTab) => void;
  openInboxItem: (feedItemId: string, companyId: string) => void;
  openCompanyInbox: (company: Company) => void;
  openCompanyClaims: (companyId: string, claimId: string) => void;
  openExternalUrl: (url: string) => void;
  sourceAdapters: SourceAdapter[];
  refreshSources: (trigger: "manual") => Promise<void> | Promise<unknown>;
  todayReviewedDays: string[];
  updateTodayReviewedDays: (days: string[]) => void;
  refreshCompletionCount: number;
  setActiveSection: (section: Section) => void;
};

/** Assembles `TodayScreen`'s full props object from the root's already-built
 * controllers — a plain composer (no state of its own), called once every
 * input it needs exists. */
export function buildTodayScreenProps(input: TodayScreenPropsInput): TodayScreenProps {
  return {
    attention: input.attention,
    companies: input.companies,
    openCompanyWorkspace: input.openCompanyWorkspace,
    openInboxItem: input.openInboxItem,
    openCompanyInbox: input.openCompanyInbox,
    openInbox: () => input.setActiveSection("Inbox"),
    openCompanyClaims: input.openCompanyClaims,
    openExternalUrl: input.openExternalUrl,
    sourceAdapters: input.sourceAdapters,
    openSources: () => input.setActiveSection("Sources"),
    refreshSources: input.refreshSources,
    todayReviewedDays: input.todayReviewedDays,
    updateTodayReviewedDays: input.updateTodayReviewedDays,
    refreshCompletionCount: input.refreshCompletionCount,
  };
}
