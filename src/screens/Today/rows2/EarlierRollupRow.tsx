import { useLocale } from "../../../shared/locale";
import { ITEM_FORMS, UNSEEN_FORMS, pluralNoun } from "../../../shared/locale/plural";
import { Button } from "../../../ui";

export type EarlierRollupRowProps = {
  /** Mono weekday range, e.g. "pon–wt" (`formatDayRangeLabel`). */
  rangeLabel: string;
  count: number;
  unseen: number;
  /** "Otwórz dni" — expands the rolled-up days into individual day sections
   * (plan decision 5). */
  onExpand: () => void;
};

/** The "Wcześniej" rollup line (F2 S5, Delta.dc.html "WCZEŚNIEJ W TYM
 * TYGODNIU · pn–wt · 14 pozycji · Otwórz dni") — every day bucket older than
 * the two freshest-with-content display slots collapses into this ONE line.
 * Reuses `DayHeader`'s collapsed anatomy (same classes) since it is visually
 * the same "summary line + recovery link" shape, just merging N days instead
 * of one. */
export function EarlierRollupRow({ rangeLabel, count, unseen, onExpand }: EarlierRollupRowProps) {
  const { text, locale } = useLocale();
  const countLabel =
    unseen > 0
      ? `${rangeLabel} · ${count} ${pluralNoun(locale, count, ITEM_FORMS)} · ${unseen} ${pluralNoun(locale, unseen, UNSEEN_FORMS)}`
      : `${rangeLabel} · ${count} ${pluralNoun(locale, count, ITEM_FORMS)}`;

  return (
    <div className="dayq-day-header dayq-day-header-collapsed" data-dayq-day-collapsed="true">
      <span className="dayq-day-label">{text("Earlier")}</span>
      <span className="dayq-day-count">{countLabel}</span>
      <Button className="dayq-day-expand" variant="minimal" type="button" onClick={onExpand}>
        {text("Open days")}
      </Button>
    </div>
  );
}
