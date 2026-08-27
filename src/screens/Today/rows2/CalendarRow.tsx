import { CalendarClock } from "lucide-react";

import type { TodayItem } from "../../../api/generated/TodayItem";
import { formatCompanyEventType } from "../../../shared/formatting/labels";
import { formatListTimestamp } from "../../../shared/format/datetime";
import { useLocale } from "../../../shared/locale";
import { StatusChip } from "../../../ui";
import { RowShell } from "./RowShell";

export type CalendarItem = Extract<TodayItem, { kind: "calendar" }>;

/**
 * An upcoming calendar entry carries no action (Wiersze.dc.html "bez akcji —
 * czeka") — it waits until the actual filing/report supersedes it, which
 * lands as its own `filing` row once it arrives.
 */
export function CalendarRow({ item }: { item: CalendarItem }) {
  const { text, locale } = useLocale();

  return (
    <RowShell
      icon={<CalendarClock aria-hidden="true" size={18} />}
      ticker={item.qualifiedTicker}
      chip={
        <StatusChip className="dayq-chip" tone="official">
          {text(formatCompanyEventType(item.eventType))}
        </StatusChip>
      }
      title={item.title}
      meta={<span className="num-tabular">{formatListTimestamp(item.eventDate, locale)}</span>}
    />
  );
}
