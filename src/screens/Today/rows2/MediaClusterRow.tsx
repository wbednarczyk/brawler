import { Newspaper } from "lucide-react";

import type { TodayItem } from "../../../api/generated/TodayItem";
import { formatListTimestamp } from "../../../shared/format/datetime";
import { useLocale } from "../../../shared/locale";
import { StatusChip } from "../../../ui";
import { RowShell } from "./RowShell";

export type MediaClusterItem = Extract<TodayItem, { kind: "mediaCluster" }>;

/**
 * `Public media` items are always grouped per (company, day) by S1, even a
 * lone article (`count === 1`). The single-article shape gets its own verb
 * ("Otwórz artykuł", Main.dc.html) — a real "×N" cluster gets "Otwórz w
 * Inbox" (plan decision 6: media never anchors a single feed item, only the
 * company-scoped Inbox filter).
 */
export function MediaClusterRow({ item, onOpen }: { item: MediaClusterItem; onOpen: () => void }) {
  const { text, locale } = useLocale();
  const isSingle = item.count === 1;

  return (
    <RowShell
      icon={<Newspaper aria-hidden="true" size={18} />}
      ticker={item.qualifiedTicker}
      chip={
        <StatusChip className="dayq-chip" tone="media">
          {isSingle ? `MEDIA · ${formatListTimestamp(item.latestPublishedAt, locale)}` : `MEDIA ×${item.count}`}
        </StatusChip>
      }
      title={item.topTitles.join(", ")}
      actionLabel={isSingle ? text("Open article") : text("Open in the Inbox")}
      onAction={onOpen}
    />
  );
}
