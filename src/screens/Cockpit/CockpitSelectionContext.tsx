import { createContext, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { Company, FeedItem } from "../../api/types";

// CockpitSelectionContext — the single source of truth for the cockpit's shared
// selection (decision 6A, ADR 0053). Linked panels react to ONE active
// selection: picking a feed item drives the Inspector and the selected
// company's Claims/Report-diff panels. This is the cockpit's reason to exist,
// so the selection lives in one store (consistent with the Architecture-v2
// per-domain view-model contexts) — NOT in per-panel controllers or an event
// bus. Panels subscribe to the slice they need via useCockpitSelection().
//
// Two entry points feed the same active selection:
//  - selectFeedItem (the Feed panel — the primary linked-triad source);
//  - selectCompany  (alternate sources: Watchlists, Report Season).
// Both resolve to a shared { feedItemId, companyId } so every company-scoped
// panel reads the same target.

export type CockpitSelection = {
  /** The active feed item, when the selection came from the Feed. */
  feedItemId: string | null;
  /** The active company — derived from the feed item, or set directly. */
  companyId: string | null;
};

export type CockpitSelectionValue = {
  selection: CockpitSelection;
  selectedFeedItem: FeedItem | null;
  selectedCompany: Company | null;
  /** Select a feed item; derives and sets the active company from it. */
  selectFeedItem: (feedItemId: string | null) => void;
  /** Select a company directly (Watchlists / Report Season as the source). */
  selectCompany: (companyId: string | null) => void;
  /** Replace the whole selection (used when restoring a saved layout). */
  setSelection: (selection: CockpitSelection) => void;
};

const Context = createContext<CockpitSelectionValue | null>(null);

/** Feed items carry the company as a qualified ticker; resolve it to an id. */
function companyIdForFeedItem(item: FeedItem | null, companies: Company[]): string | null {
  if (!item) return null;
  return companies.find((company) => company.qualifiedTicker === item.company)?.id ?? null;
}

export function CockpitSelectionProvider({
  companies,
  feedItems,
  initialCompanyId = null,
  children,
}: {
  companies: Company[];
  feedItems: FeedItem[];
  /** When set, the cockpit opens scoped to this company (Advanced layout from
   *  a company workspace, ADR 0054) instead of the first feed item. */
  initialCompanyId?: string | null;
  children: ReactNode;
}) {
  const [selection, setSelection] = useState<CockpitSelection>(() => {
    if (initialCompanyId) {
      return { feedItemId: null, companyId: initialCompanyId };
    }
    const first = feedItems[0] ?? null;
    return { feedItemId: first?.id ?? null, companyId: companyIdForFeedItem(first, companies) };
  });

  // If the cockpit is already mounted and the user opens Advanced layout for a
  // different company, re-focus that company.
  const lastInitialCompany = useRef(initialCompanyId);
  useEffect(() => {
    if (initialCompanyId && initialCompanyId !== lastInitialCompany.current) {
      lastInitialCompany.current = initialCompanyId;
      setSelection({ feedItemId: null, companyId: initialCompanyId });
    }
  }, [initialCompanyId]);

  // Feed data usually loads AFTER mount, so the initializer above runs on an
  // empty list and nothing is selected — leaving the Inspector/Claims/Diff panels
  // blank on first paint. Seed the first item once, when the feed arrives, but
  // only if nothing is selected yet (so a restored layout or a deliberate clear
  // after init is respected).
  const seeded = useRef(false);
  useEffect(() => {
    if (seeded.current || feedItems.length === 0) return;
    seeded.current = true;
    setSelection((current) =>
      current.feedItemId === null && current.companyId === null
        ? {
            feedItemId: feedItems[0].id,
            companyId: companyIdForFeedItem(feedItems[0], companies),
          }
        : current,
    );
  }, [feedItems, companies]);

  const value = useMemo<CockpitSelectionValue>(() => {
    const selectedFeedItem = selection.feedItemId
      ? (feedItems.find((item) => item.id === selection.feedItemId) ?? null)
      : null;
    const selectedCompany = selection.companyId
      ? (companies.find((company) => company.id === selection.companyId) ?? null)
      : null;
    return {
      selection,
      selectedFeedItem,
      selectedCompany,
      selectFeedItem: (feedItemId) => {
        const item = feedItemId
          ? (feedItems.find((candidate) => candidate.id === feedItemId) ?? null)
          : null;
        setSelection({ feedItemId, companyId: companyIdForFeedItem(item, companies) });
      },
      selectCompany: (companyId) => setSelection({ feedItemId: null, companyId }),
      setSelection,
    };
  }, [selection, companies, feedItems]);

  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useCockpitSelection(): CockpitSelectionValue {
  const value = useContext(Context);
  if (!value) {
    throw new Error("useCockpitSelection must be used within a CockpitSelectionProvider");
  }
  return value;
}
