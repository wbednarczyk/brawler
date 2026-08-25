import { FileText } from "lucide-react";

import type { TodayItem } from "../../../api/generated/TodayItem";
import { formatListTimestamp } from "../../../shared/format/datetime";
import { useLocale } from "../../../shared/locale";
import { StatusChip } from "../../../ui";
import { splitDocumentTitle } from "../documentTitle";
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
  // A `report_documents.title` may be a filename glued onto the human title
  // (ADR 0091) — never rendered as the row's own statement (anti-filename
  // gate, `streamCopy.test.tsx`); the filename survives on the quiet meta
  // line instead of being silently dropped.
  const { statement, filename } = splitDocumentTitle(item.title);

  return (
    <RowShell
      id={item.feedItemId}
      icon={<FileText aria-hidden="true" size={18} />}
      ticker={item.qualifiedTicker}
      chip={
        <StatusChip className="dayq-chip" tone="official">
          {`ESPI · ${formatListTimestamp(item.publishedAt, locale)}`}
        </StatusChip>
      }
      title={statement ?? item.title}
      meta={filename}
      actionLabel={isReport ? text("Read report") : text("Open filing")}
      onAction={onOpen}
      emphasis={isReport}
    />
  );
}
