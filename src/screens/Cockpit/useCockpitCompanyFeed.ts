import { useEffect, useMemo, useState } from "react";
import { updateFeedItemState as persistFeedItemState } from "../../api/feed";
import type { Company, FeedItem } from "../../api/types";

// Cockpit-native company feed state for one company (ADR 0057). It seeds a local
// item list from the global feed snapshot filtered to the company, owns the
// selection, persists read/save toggles through the real `update_feed_item_state`
// command (updating the local copy from the response), so the dashboard
// `companyFeed` panel works with no AppStateRoot coupling — mirroring
// `useCockpitFundamentals`.
export function useCockpitCompanyFeed(
  company: Company,
  feedItems: FeedItem[],
  // Pre-selects one item on mount (Spółka `feedItem` tool, F3a S2 — the
  // fallback when the Inbox per-kind renderer is too entangled to host
  // standalone): marks it read on open, mirroring the Inbox's shared
  // "opening marks read" behavior.
  initialSelectedFeedItemId?: string | null,
) {
  const companyItems = useMemo(
    () => feedItems.filter((item) => item.company === company.qualifiedTicker),
    [feedItems, company.qualifiedTicker],
  );

  // Local overrides keyed by id capture read/save toggles so the panel reflects
  // them without a global feed refresh; the underlying snapshot stays the source.
  const [overrides, setOverrides] = useState<Record<string, FeedItem>>({});
  const [selectedId, setSelectedId] = useState<string | null>(initialSelectedFeedItemId ?? null);

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

  function toggleFeedItem(item: FeedItem) {
    setSelectedId((current) => (current === item.id ? null : item.id));
  }

  async function updateFeedItemState(item: FeedItem, update: (item: FeedItem) => FeedItem) {
    const next = update(item);
    const saved = await persistFeedItemState({ id: item.id, read: !next.unread, saved: next.saved });
    setOverrides((current) => ({ ...current, [item.id]: saved }));
  }

  useEffect(() => {
    if (!initialSelectedFeedItemId) return;
    const item = companyItems.find((candidate) => candidate.id === initialSelectedFeedItemId);
    if (item?.unread) {
      void updateFeedItemState(item, (feedItem) => ({ ...feedItem, unread: false }));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- mark-read runs once for the initial pre-selection only
  }, [initialSelectedFeedItemId]);

  return {
    company,
    items,
    selectedFeedItem,
    toggleFeedItem,
    updateFeedItemState,
  };
}
