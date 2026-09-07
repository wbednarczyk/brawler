import { useCallback, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import { pluralNoun, type PluralForms } from "../../shared/locale/plural";
import type { LocaleCode } from "../../shared/locale";

// Dogfooding wave 2026-09, #6: period-keyed tables (the facts matrix, Pozycje
// × okresy) show only the newest MEASURED capacity of period columns, oldest
// hidden behind a full-height clickable expander column (owner storyboard
// round 1). Shared logic; each host measures its own period-group width and
// sticky-column width (facts: one <th>; Pozycje: value + delta headers).
const MIN_CAPACITY = 1;

export type UseVisiblePeriodsOptions = {
  // Ref to the host's horizontally-scrolling container (`.facts-matrix-scroll`
  // / `.fundamentals-periods-scroll`).
  scrollerRef: RefObject<HTMLElement | null>;
  // Total period columns available, oldest -> newest.
  total: number;
  // Measures ONE rendered period header group's width (facts: one <th>;
  // Pozycje: the value + delta headers of one period) — never a constant.
  // Returns 0 before anything is measurable.
  measurePeriodWidth: () => number;
  // Combined width of the non-period columns (sticky KPI/expander, a trend
  // column), subtracted from the scroller's client width; a function when it
  // depends on the rendered tier.
  stickyWidth: number | (() => number);
};

export type UseVisiblePeriodsResult = {
  // Index into the full period list where the visible slice starts.
  visibleStart: number;
  visibleCount: number;
  hiddenCount: number;
  expanded: boolean;
  toggle: () => void;
};

export function useVisiblePeriods({
  scrollerRef,
  total,
  measurePeriodWidth,
  stickyWidth,
}: UseVisiblePeriodsOptions): UseVisiblePeriodsResult {
  const [capacity, setCapacity] = useState(MIN_CAPACITY);
  const [expanded, setExpanded] = useState(false);
  // Latest measurer in a ref so `recompute`'s identity doesn't have to change
  // (and re-trigger the observer effect) just because the caller passed a new
  // closure this render.
  const measureRef = useRef(measurePeriodWidth);
  measureRef.current = measurePeriodWidth;

  const recompute = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const periodWidth = measureRef.current();
    if (!periodWidth) {
      // Zero width -> nothing measurable yet (e.g. no periods rendered).
      setCapacity(MIN_CAPACITY);
      return;
    }
    const available = scroller.clientWidth - (typeof stickyWidth === "function" ? stickyWidth() : stickyWidth);
    // No upper cap (owner 2026-09-07): the table fills the width it has.
    setCapacity(Math.max(MIN_CAPACITY, Math.floor(available / periodWidth)));
  }, [scrollerRef, stickyWidth]);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    recompute();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => recompute());
    observer.observe(scroller);
    return () => observer.disconnect();
    // `total` is included so a period-count change (fresh header cells) also
    // re-measures; `scrollerRef` identity is stable across renders (a ref).
  }, [recompute, total, scrollerRef]);

  // Expanded state ignores capacity changes (owner decision): a resize never
  // yanks a user who expanded and scrolled into history.
  const visibleCount = expanded ? total : Math.min(capacity, total);
  const visibleStart = Math.max(0, total - visibleCount);
  const hiddenCount = Math.max(0, total - visibleCount);

  return {
    visibleStart,
    visibleCount,
    hiddenCount,
    expanded,
    toggle: () => setExpanded((value) => !value),
  };
}

/** Natural (unstretched) width of a cell's content plus the cell's own
 * horizontal padding — table columns stretch to fill the table, so a cell's
 * `offsetWidth` is circular; the content's ink extent is not. `inner` is the
 * element whose contents to measure when the cell wraps them in a stretched
 * control (a `width: 100%` button). */
export function naturalCellWidth(cell: HTMLElement, inner: HTMLElement | null = null): number {
  const target = inner ?? cell;
  const range = document.createRange();
  range.selectNodeContents(target);
  const content = range.getBoundingClientRect().width;
  const pad = (el: HTMLElement) => {
    const style = getComputedStyle(el);
    return Number.parseFloat(style.paddingLeft) + Number.parseFloat(style.paddingRight);
  };
  return content + pad(cell) + (inner ? pad(inner) : 0);
}

// The expander column's accessible name/visible label combines an adjective
// ("earlier"/"starszy") and a noun ("period"/"okres") into one declined
// phrase (mirrors FRESH_FACT_FORMS/UPCOMING_EVENT_FORMS, plural.ts).
const EARLIER_PERIOD_FORMS: PluralForms = {
  en: ["earlier period", "earlier periods"],
  pl: ["starszy okres", "starsze okresy", "starszych okresów"],
};

/** Visible button text — no count (the count lives in the accessible name only). */
export function periodExpanderVisibleLabel(expanded: boolean, text: (value: string) => string): string {
  return expanded ? text("Collapse earlier") : text("Expand earlier");
}

/** Accessible name — names the hidden count when collapsed. */
export function periodExpanderAccessibleName(
  expanded: boolean,
  hiddenCount: number,
  locale: LocaleCode,
  text: (value: string) => string,
): string {
  if (expanded) return text("Collapse earlier");
  const noun = pluralNoun(locale, hiddenCount, EARLIER_PERIOD_FORMS);
  return locale === "pl" ? `Rozwiń ${hiddenCount} ${noun}` : `Expand ${hiddenCount} ${noun}`;
}
