import { FileText } from "lucide-react";

import type { TodayItem } from "../../../api/generated/TodayItem";
import { formatListTimestamp } from "../../../shared/format/datetime";
import { useLocale } from "../../../shared/locale";
import { StatusChip } from "../../../ui";
import { RowShell } from "./RowShell";

export type FilingItem = Extract<TodayItem, { kind: "filing" }>;

/**
 * The `filing` kind covers both verb-dictionary variants (plan decision 6):
 * a report WITH attachments ("Przeczytaj raport", lands in the report
 * viewer) vs a bare ESPI/EBI notice ("Otwórz komunikat", lands on the item in
 * Inbox) — `presentationKind` is the single switch (S1's DTO, never guessed
 * from the title).
 */
export function FilingRow({ item, onOpen }: { item: FilingItem; onOpen: () => void }) {
  const { text, locale } = useLocale();
  const isReport = item.presentationKind === "report";

  return (
    <RowShell
      icon={<FileText aria-hidden="true" size={18} />}
      ticker={item.qualifiedTicker}
      chip={
        <StatusChip className="dayq-chip" tone="official">
          {`ESPI · ${formatListTimestamp(item.publishedAt, locale)}`}
        </StatusChip>
      }
      title={item.title}
      actionLabel={isReport ? text("Read report") : text("Open filing")}
      onAction={onOpen}
      emphasis={isReport}
    />
  );
}
