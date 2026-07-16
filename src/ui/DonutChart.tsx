import { useId, type ReactNode } from "react";

// Donut chart for a small set of parts-of-a-whole categories (e.g. ownership
// structure by holder type). Hand-rolled SVG: each slice is a stroked circle
// segment (stroke-dasharray on a circumference-100 ring, r = 15.9155), with a
// ~2px gap between slices. Colour is carried entirely by a per-kind CSS class
// (never inline) so the same validated hue maps to the same holder TYPE
// everywhere; the `uncertain` kind renders hatched + neutral (derived free
// float, whose exact split is unknown below the disclosure threshold). The
// legend is rendered by the caller (it pairs labels with values); the primitive
// exposes matching swatch classes via `donutSwatchClass`.

// `unknown` = not-yet-classified holders: solid neutral, deliberately colourless
// (no identity yet) and texture-distinct from the hatched `uncertain` free float.
export type DonutSliceKind = "founder" | "ofe" | "tfi" | "misc" | "uncertain" | "unknown";

export type DonutSlice = {
  /** Stable identity for the React key. */
  key: string;
  /** Human label (used only for the slice's `<title>` a11y fallback). */
  label: string;
  /** Non-negative magnitude; slices are normalised against their sum. */
  value: number;
  kind: DonutSliceKind;
};

export type DonutChartProps = {
  slices: DonutSlice[];
  /** Accessible name for the whole chart (role="img"). */
  ariaLabel: string;
  /** Optional centred overlay (e.g. the free-float headline). */
  centerLabel?: ReactNode;
  /**
   * Optional hover explanation (native SVG `<title>` tooltip + a11y
   * description) — e.g. how the derived free float is computed.
   */
  tooltip?: string;
  /** Rendered pixel size (square); the SVG scales its viewBox to fit. */
  size?: number;
  className?: string;
};

/** CSS swatch class for a legend entry matching a slice kind. */
export function donutSwatchClass(kind: DonutSliceKind): string {
  return `ui-donut-swatch ui-donut-swatch-${kind}`;
}

// Ring circumference for r = 15.9155 is ~100, so a slice's dash length equals
// its percentage directly. The gap shortens each drawn arc so neighbours don't
// touch (~2px at the default size).
const RING_R = 15.9155;
const CIRCUMFERENCE = 100;
const GAP = 1.2;
// dashoffset 25 rotates the ring start to 12 o'clock.
const START_OFFSET = 25;

export function DonutChart({
  slices,
  ariaLabel,
  centerLabel,
  tooltip,
  size = 132,
  className,
}: DonutChartProps) {
  const hatchId = useId();
  const usable = slices.filter((slice) => Number.isFinite(slice.value) && slice.value > 0);
  const total = usable.reduce((sum, slice) => sum + slice.value, 0);

  if (total <= 0) {
    return (
      <div className={["ui-donut-empty", className].filter(Boolean).join(" ")}>{ariaLabel}</div>
    );
  }

  let offset = START_OFFSET;
  const segments = usable.map((slice) => {
    const pct = (slice.value / total) * CIRCUMFERENCE;
    const dash = Math.max(pct - GAP, 0.001);
    const dashArray = `${dash.toFixed(3)} ${(CIRCUMFERENCE - dash).toFixed(3)}`;
    const segment = (
      <circle
        key={slice.key}
        r={RING_R}
        cx={21}
        cy={21}
        fill="none"
        className={`ui-donut-slice ui-donut-slice-${slice.kind}`}
        stroke={slice.kind === "uncertain" ? `url(#${hatchId})` : undefined}
        strokeWidth={7}
        strokeDasharray={dashArray}
        strokeDashoffset={offset.toFixed(3)}
      >
        <title>{slice.label}</title>
      </circle>
    );
    offset -= pct;
    return segment;
  });

  return (
    <div className={["ui-donut", className].filter(Boolean).join(" ")}>
      <svg
        width={size}
        height={size}
        viewBox="0 0 42 42"
        role="img"
        aria-label={ariaLabel}
        className="ui-donut-svg"
      >
        {tooltip ? <title>{tooltip}</title> : null}
        <defs>
          <pattern
            id={hatchId}
            width="3"
            height="3"
            patternTransform="rotate(45)"
            patternUnits="userSpaceOnUse"
          >
            <line x1="0" y1="0" x2="0" y2="3" className="ui-donut-hatch-line" strokeWidth={1.4} />
          </pattern>
        </defs>
        {segments}
      </svg>
      {centerLabel ? <div className="ui-donut-center">{centerLabel}</div> : null}
    </div>
  );
}
