import { Check } from "lucide-react";

import { formatListTimestamp } from "../../../shared/format/datetime";
import { useLocale } from "../../../shared/locale";
import { ITEM_FORMS, UNSEEN_FORMS, pluralNoun } from "../../../shared/locale/plural";
import { Button } from "../../../ui";
import type { RelativeDay } from "../dayQueueModel";

export type DayHeaderProps = {
  day: string;
  relativeDay: RelativeDay;
  total: number;
  unseen: number;
  /** Derived (`allSeen`) OR the manual `todayReviewedDays` override (plan
   * decision 5) — the caller resolves which; this component only renders. */
  collapsed: boolean;
  /** "Otwórz dzień" — undefined when not collapsed (nothing to expand). */
  onExpand?: () => void;
  /** "Oznacz dzień jako przejrzany" (plan decision 5's manual gesture) —
   * undefined when already collapsed (nothing left to mark). */
  onMarkReviewed?: () => void;
};

/** Day-section header (F2 S3): mono day label + counters, or — collapsed —
 * a single summary line with a checkmark and the "Otwórz dzień" recovery
 * link (Delta.dc.html). */
export function DayHeader({
  day,
  relativeDay,
  total,
  unseen,
  collapsed,
  onExpand,
  onMarkReviewed,
}: DayHeaderProps) {
  const { text, locale } = useLocale();
  const dayLabel =
    relativeDay === "today"
      ? text("Today")
      : relativeDay === "yesterday"
        ? text("Yesterday")
        : formatListTimestamp(day, locale);
  const countLabel =
    unseen > 0
      ? `${total} ${pluralNoun(locale, total, ITEM_FORMS)} · ${unseen} ${pluralNoun(locale, unseen, UNSEEN_FORMS)}`
      : `${total} ${pluralNoun(locale, total, ITEM_FORMS)}`;

  if (collapsed) {
    return (
      <div className="dayq-day-header dayq-day-header-collapsed" data-dayq-day-collapsed="true">
        <Check aria-hidden="true" size={14} className="dayq-day-check" />
        <span className="dayq-day-label">{dayLabel}</span>
        <span className="dayq-day-count">{countLabel}</span>
        {onExpand ? (
          <Button className="dayq-day-expand" variant="minimal" type="button" onClick={onExpand}>
            {text("Open day")}
          </Button>
        ) : null}
      </div>
    );
  }

  return (
    <div className="dayq-day-header" data-dayq-day-collapsed="false">
      <span className="dayq-day-label">{dayLabel}</span>
      <span className="dayq-day-count">{countLabel}</span>
      {onMarkReviewed ? (
        <Button className="dayq-day-expand" variant="minimal" type="button" onClick={onMarkReviewed}>
          {text("Mark day reviewed")}
        </Button>
      ) : null}
    </div>
  );
}
