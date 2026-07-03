import { useEffect, useMemo, useState } from "react";
import { updateFeedItemState as persistFeedItemState } from "../../api/feed";
import type { Company, FeedItem } from "../../api/types";
import { useAiAnalysisController } from "../../app/useAiAnalysisController";

// Cockpit-native company feed state for one company (ADR 0057). It seeds a local
// item list from the global feed snapshot filtered to the company, owns the
// selection, persists read/save toggles through the real `update_feed_item_state`
// command (updating the local copy from the response), and composes the shared
// `useAiAnalysisController` so the dashboard `companyFeed` panel works with no
// AppStateRoot coupling — mirroring `useCockpitFundamentals`.
export function useCockpitCompanyFeed(company: Company, feedItems: FeedItem[]) {
  const companyItems = useMemo(
    () => feedItems.filter((item) => item.company === company.qualifiedTicker),
    [feedItems, company.qualifiedTicker],
  );

  // Local overrides keyed by id capture read/save toggles so the panel reflects
  // them without a global feed refresh; the underlying snapshot stays the source.
  const [overrides, setOverrides] = useState<Record<string, FeedItem>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Drop overrides + selection that no longer match the company's items (e.g. the
  // pinned company changed) so stale state never leaks across companies.
  useEffect(() => {
    setOverrides({});
    setSelectedId(null);
  }, [company.id]);

  const items = useMemo(
    () => companyItems.map((item) => overrides[item.id] ?? item),
    [companyItems, overrides],
  );
  const selectedFeedItem = items.find((item) => item.id === selectedId) ?? null;

  const ai = useAiAnalysisController({
    selectedFeedItem: null,
    selectedCompanyFeedItem: selectedFeedItem,
  });

  function toggleFeedItem(item: FeedItem) {
    setSelectedId((current) => (current === item.id ? null : item.id));
  }

  async function updateFeedItemState(item: FeedItem, update: (item: FeedItem) => FeedItem) {
    const next = update(item);
    const saved = await persistFeedItemState({ id: item.id, read: !next.unread, saved: next.saved });
    setOverrides((current) => ({ ...current, [item.id]: saved }));
  }

  return {
    company,
    items,
    selectedFeedItem,
    toggleFeedItem,
    updateFeedItemState,
    ...ai,
  };
}
