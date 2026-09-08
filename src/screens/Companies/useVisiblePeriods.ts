import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
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
  // Ref to the host's period-table wrapper (`.facts-matrix-scroll` /
  // `.fundamentals-periods-scroll`).
  scrollerRef: RefObject<HTMLElement | null>;
  // Total period columns available, oldest -> newest.
  total: number;
  // Changes whenever the data behind the cells changes (periods, rows,
  // locale, granularity) — triggers a measuring render pass in which the host
  // renders EVERY period (hidden) so the measurement is independent of the
  // visible slice.
  measureKey: string;
  // Widest natural (unstretched) period group across ALL rendered periods —
  // called only during the measuring pass. Returns 0 when nothing is rendered.
  measurePeriodWidth: () => number;
  // Rendered width of the non-period columns (sticky KPI column, a trend
  // column) — re-read on every resize (tiers fold columns).
  measureFixedWidth: () => number;
  // Rendered width of the expander column, reserved only when some periods
  // must hide (re-read on every resize).
  measureExpanderWidth: () => number;
  // Names the tier-dependent column set currently displayed (e.g. which Δ
  // columns a container query shows); a change re-runs the measuring pass.
  measureVariant?: () => string;
};

export type UseVisiblePeriodsResult = {
  // Index into the full period list where the visible slice starts.
  visibleStart: number;
  visibleCount: number;
  hiddenCount: number;
  expanded: boolean;
  toggle: () => void;
  // True during the one hidden render pass that measures every period.
  measuring: boolean;
};

export function useVisiblePeriods({
  scrollerRef,
  total,
  measureKey,
  measurePeriodWidth,
  measureFixedWidth,
  measureExpanderWidth,
  measureVariant,
}: UseVisiblePeriodsOptions): UseVisiblePeriodsResult {
  const [capacity, setCapacity] = useState(MIN_CAPACITY);
  const [expanded, setExpanded] = useState(false);
  const [measuredKey, setMeasuredKey] = useState<string | null>(null);
  const periodWidthRef = useRef(0);
  // Latest measurers in refs so the callbacks' identity never re-triggers
  // an effect (a new closure per render is the norm).
  const measuredVariantRef = useRef("");
  const measurers = useRef({ measurePeriodWidth, measureFixedWidth, measureExpanderWidth, measureVariant });
  measurers.current = { measurePeriodWidth, measureFixedWidth, measureExpanderWidth, measureVariant };
  const measuring = total > 0 && measuredKey !== measureKey;

  const recompute = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    // A tier flip that folds/unfolds columns invalidates the cached width.
    const variant = measurers.current.measureVariant?.() ?? "";
    if (variant !== measuredVariantRef.current) {
      setMeasuredKey(null);
      return;
    }
    const periodWidth = periodWidthRef.current;
    if (!periodWidth) {
      setCapacity(MIN_CAPACITY);
      return;
    }
    const available = scroller.clientWidth - measurers.current.measureFixedWidth();
    // Every period fits without the expander → no column, nothing hidden;
    // otherwise reserve the expander column first (no upper cap, owner
    // 2026-09-07: the table fills the width it has).
    if (total * periodWidth <= available) {
      setCapacity(total);
      return;
    }
    const withExpander = available - measurers.current.measureExpanderWidth();
    setCapacity(Math.max(MIN_CAPACITY, Math.floor(withExpander / periodWidth)));
  }, [scrollerRef, total]);

  // The measuring pass: the host has rendered every period (hidden) — read
  // the widest natural group once, before paint, then settle.
  useLayoutEffect(() => {
    if (!measuring) return;
    periodWidthRef.current = measurers.current.measurePeriodWidth();
    measuredVariantRef.current = measurers.current.measureVariant?.() ?? "";
    setMeasuredKey(measureKey);
    recompute();
  }, [measuring, measureKey, recompute]);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    recompute();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => recompute());
    observer.observe(scroller);
    return () => observer.disconnect();
  }, [recompute, scrollerRef]);

  // Webfonts load with `font-display: swap`: a pass measured in the fallback
  // font is stale once the real face swaps in, and the swap changes no
  // wrapper size (no ResizeObserver tick) — re-measure on every font load.
  useEffect(() => {
    const fonts = typeof document === "undefined" ? undefined : document.fonts;
    if (!fonts || typeof fonts.addEventListener !== "function") return;
    let cancelled = false;
    const invalidate = () => {
      if (!cancelled) setMeasuredKey(null);
    };
    fonts.addEventListener("loadingdone", invalidate);
    if (fonts.status === "loading") void fonts.ready.then(invalidate, () => undefined);
    return () => {
      cancelled = true;
      fonts.removeEventListener("loadingdone", invalidate);
    };
  }, []);

  // Expanded state ignores capacity changes (owner decision): a resize never
  // yanks a user who expanded and scrolled into history.
  const visibleCount = expanded || measuring ? total : Math.min(capacity, total);
  const visibleStart = Math.max(0, total - visibleCount);
  const hiddenCount = Math.max(0, total - visibleCount);

  return {
    visibleStart,
    visibleCount,
    hiddenCount,
    expanded,
    toggle: () => setExpanded((value) => !value),
    measuring,
  };
}

/** Natural (unstretched) width of a cell's content plus the cell's own
 * horizontal padding — table columns stretch to fill the table, so a cell's
 * `offsetWidth` is circular; the content's ink extent is not. Padding and
 * borders of the cell (and of `inner`, the element whose contents to measure
 * when the cell wraps them in a stretched `width: 100%` control) are added. */
export function naturalCellWidth(cell: HTMLElement, inner: HTMLElement | null = null): number {
  const target = inner ?? cell;
  const range = document.createRange();
  range.selectNodeContents(target);
  const content = range.getBoundingClientRect().width;
  const pad = (el: HTMLElement) => {
    const style = getComputedStyle(el);
    return (
      Number.parseFloat(style.paddingLeft) +
      Number.parseFloat(style.paddingRight) +
      Number.parseFloat(style.borderLeftWidth) +
      Number.parseFloat(style.borderRightWidth)
    );
  };
  return content + pad(cell) + (inner ? pad(inner) : 0);
}

/** A CSS length custom property of `el` in px (0 when unset). */
export function cssLengthVar(el: Element | null, name: string): number {
  if (!el) return 0;
  return Number.parseFloat(getComputedStyle(el).getPropertyValue(name)) || 0;
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
