export type RangeBarRow = {
  key: string;
  label: string;
  /**
   * Implied range low ≤ base ≤ high, in one shared unit. Leave all three
   * null/undefined for a typed-absence row (renders `absentText` instead of a
   * bar) — never a fabricated 0-width bar.
   */
  low?: number | null;
  base?: number | null;
  high?: number | null;
  /** Pre-formatted trailing range text (e.g. "96–148 zł"); caller localizes it. */
  rangeText?: string;
  /** Rendered in place of the bar when the row is a typed absence. */
  absentText?: string;
};

export type RangeBarMarker = {
  value: number;
  /** Legend + accessible label for the shared marker line (e.g. "current price 132 zł"). */
  label: string;
};

export type RangeBarChartProps = {
  /** One row per method; every drawable row shares one domain so bars compare. */
  rows: RangeBarRow[];
  /** A single reference line (e.g. current price) drawn across every row. */
  marker?: RangeBarMarker | null;
  ariaLabel: string;
  /** Legend caption for the implied-range fill. */
  rangeLegendLabel: string;
  /** Legend caption for the marker line (only rendered when a marker is set). */
  markerLegendLabel?: string;
  className?: string;
};

const isNum = (value: number | null | undefined): value is number =>
  typeof value === "number" && Number.isFinite(value);

/// A horizontal "football field" of implied-value ranges — one bar per method on
/// ONE shared domain, with an optional reference marker (current price) drawn
/// across every row so the ranges read against the same axis. A method with no
/// range renders its typed absence text, never a zero-width bar (ADR 0089
/// dec. 4: typed absences, never NaN/0). Decision support only — the primitive
/// states ranges, it never colours a "cheap/expensive" verdict.
export function RangeBarChart({
  rows,
  marker,
  ariaLabel,
  rangeLegendLabel,
  markerLegendLabel,
  className,
}: RangeBarChartProps) {
  // Shared domain across every drawable low/high plus the marker, padded a touch
  // so end-of-range bars and an extreme marker aren't flush to the track edge.
  const values: number[] = [];
  for (const row of rows) {
    if (isNum(row.low)) values.push(row.low);
    if (isNum(row.high)) values.push(row.high);
    if (isNum(row.base)) values.push(row.base);
  }
  if (marker && isNum(marker.value)) values.push(marker.value);

  const rawMin = values.length ? Math.min(...values) : 0;
  const rawMax = values.length ? Math.max(...values) : 1;
  const pad = (rawMax - rawMin || Math.abs(rawMax) || 1) * 0.06;
  const min = rawMin - pad;
  const span = rawMax + pad - min || 1;
  const pos = (value: number) => {
    const p = ((value - min) / span) * 100;
    return Math.min(100, Math.max(0, p));
  };

  const markerPos = marker && isNum(marker.value) ? pos(marker.value) : null;

  return (
    <div
      role="group"
      aria-label={ariaLabel}
      className={["ui-range-bar", className].filter(Boolean).join(" ")}
    >
      <div className="ui-range-bar-rows">
        {rows.map((row) => {
          const drawable = isNum(row.low) && isNum(row.high) && row.high >= row.low;
          return (
            <div key={row.key} className="ui-range-bar-row">
              <span className="ui-range-bar-label">{row.label}</span>
              {drawable ? (
                <>
                  <div className="ui-range-bar-track" aria-hidden="true">
                    <i
                      className="ui-range-bar-fill"
                      style={{
                        left: `${pos(row.low as number).toFixed(2)}%`,
                        width: `${(pos(row.high as number) - pos(row.low as number)).toFixed(2)}%`,
                      }}
                    />
                    {isNum(row.base) ? (
                      <s
                        className="ui-range-bar-base"
                        style={{ left: `${pos(row.base).toFixed(2)}%` }}
                      />
                    ) : null}
                    {markerPos !== null ? (
                      <b
                        className="ui-range-bar-marker"
                        style={{ left: `${markerPos.toFixed(2)}%` }}
                      />
                    ) : null}
                  </div>
                  {row.rangeText ? (
                    <span className="ui-range-bar-range num-tabular">{row.rangeText}</span>
                  ) : (
                    <span className="ui-range-bar-range" />
                  )}
                </>
              ) : (
                <span className="ui-range-bar-absent">{row.absentText}</span>
              )}
            </div>
          );
        })}
      </div>
      <ul className="ui-range-bar-legend">
        <li>
          <span className="ui-range-bar-sw ui-range-bar-sw-range" aria-hidden="true" />
          {rangeLegendLabel}
        </li>
        {marker ? (
          <li>
            <span className="ui-range-bar-sw ui-range-bar-sw-marker" aria-hidden="true" />
            {markerLegendLabel ? `${markerLegendLabel} — ${marker.label}` : marker.label}
          </li>
        ) : null}
      </ul>
    </div>
  );
}
