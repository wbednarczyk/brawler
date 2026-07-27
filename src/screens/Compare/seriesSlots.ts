// Shared series-slot colour helper for the Compare mode (ADR 0089 dec. 6).
//
// Selection index → one of four categorical colour slots, kept 1:1 with
// `MultiLineChart`'s coloured series (`.ui-multi-line-series-{0..3}` in
// `ui.css`, which resolve to `--own-founder / --own-ofe / --own-tfi
// --own-misc`). The chip dots, table row dots, and the A4 trend overlay must
// all key off the SAME slot index for a company, so a company reads as one
// colour across chip ↔ table ↔ chart. Lives in the Compare screen module so
// §A4 (the trend overlay) reuses it without importing the whole screen.

/** The number of coloured slots — matches `MULTI_LINE_MAX_SERIES` (do not raise). */
export const COMPARE_SERIES_SLOTS = 4;

/**
 * CSS class carrying the slot's colour as the `--compare-dot` custom property
 * (see `compare.css`). Beyond the fourth company the slots cycle — the colour
 * is a grouping aid, not an identity, and the chart caps the coloured series at
 * four regardless.
 */
export function seriesSlotClass(index: number): string {
  return `compare-series-${index % COMPARE_SERIES_SLOTS}`;
}
