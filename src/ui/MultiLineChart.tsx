import type { ReactNode } from "react";

import { niceScale } from "./chartScale";

export type MultiLineSeries = {
  key: string;
  label: string;
  /** Optional trailing legend value (e.g. the current stake), rendered beside the label. */
  legendValue?: ReactNode;
  /**
   * Neutral series (e.g. a derived free float): dashed grey line outside the
   * categorical hue slots — it neither takes a colour nor counts toward the
   * four-series cap.
   */
  neutral?: boolean;
  /**
   * `marked` singles a point out as an event on the line — a disclosure that
   * matters, not just another sample (ownership: an ESPI threshold crossing,
   * ADR 0072 decision 5). Rendered as a dot in the series' own hue, so the
   * event reads as belonging to that holder rather than as a separate layer.
   */
  points: { label: string; value: number; marked?: boolean }[];
  /** Accessible description of what a marked point means on this series. */
  markerLabel?: string;
};

export type MultiLineChartProps = {
  /** Up to four series share one y-scale and one x-domain (sorted point labels). */
  series: MultiLineSeries[];
  ariaLabel: string;
  formatValue?: (value: number) => string;
  height?: number;
  className?: string;
};

const VIEW = 100;
export const MULTI_LINE_MAX_SERIES = 4;

/// Multi-series trend lines on ONE shared scale — separate y-scales would make
/// a 7% stake look like a 17% one. Hue follows the series slot (validated
/// categorical set, never cycled); series beyond four must be folded by the
/// caller before rendering. The x-domain is the sorted union of point labels,
/// so series with different disclosure dates still align.
export function MultiLineChart({
  series,
  ariaLabel,
  formatValue,
  height = 140,
  className,
}: MultiLineChartProps) {
  const coloured = series.filter((serie) => !serie.neutral).slice(0, MULTI_LINE_MAX_SERIES);
  const neutral = series.filter((serie) => serie.neutral);
  const drawable = [...coloured, ...neutral]
    .map((serie) => ({
      ...serie,
      points: serie.points.filter((point) => Number.isFinite(point.value)),
    }))
    .filter((serie) => serie.points.length >= 2);
  if (drawable.length === 0) {
    return (
      <div className={["ui-line-chart-empty", className].filter(Boolean).join(" ")}>
        {ariaLabel}
      </div>
    );
  }

  const xLabels = [...new Set(drawable.flatMap((serie) => serie.points.map((p) => p.label)))].sort();
  const xIndex = new Map(xLabels.map((label, index) => [label, index]));
  const xFor = (label: string) =>
    xLabels.length < 2 ? 0 : (VIEW * (xIndex.get(label) ?? 0)) / (xLabels.length - 1);

  const values = drawable.flatMap((serie) => serie.points.map((point) => point.value));
  const scale = niceScale(Math.min(...values), Math.max(...values));
  const span = scale.max - scale.min || 1;
  const yFor = (value: number) => VIEW - (VIEW * (value - scale.min)) / span;

  const format = formatValue ?? ((value: number) => String(value));

  return (
    <div className={["ui-multi-line-chart", className].filter(Boolean).join(" ")}>
      <div className="ui-line-chart-ylabels" aria-hidden="true">
        {[...scale.ticks].reverse().map((tick) => (
          <span key={tick}>{format(tick)}</span>
        ))}
      </div>
      <svg height={height} viewBox={`0 0 ${VIEW} ${VIEW}`} role="img" aria-label={ariaLabel} preserveAspectRatio="none">
        {scale.ticks.map((tick) => (
          <line
            key={tick}
            className="ui-line-chart-gridline"
            x1="0"
            x2={VIEW}
            y1={yFor(tick).toFixed(2)}
            y2={yFor(tick).toFixed(2)}
          />
        ))}
        {drawable.map((serie, index) => (
          <polyline
            key={serie.key}
            className={`ui-multi-line-series ${serie.neutral ? "ui-multi-line-series-neutral" : `ui-multi-line-series-${index}`}`}
            fill="none"
            points={serie.points
              .map((point) => `${xFor(point.label).toFixed(2)},${yFor(point.value).toFixed(2)}`)
              .join(" ")}
          />
        ))}
        {/* Event markers are drawn after every line so a crossing is never hidden
            by a later series painted over it. A short VERTICAL tick rather than a
            dot: the chart scales non-uniformly (`preserveAspectRatio="none"`), so
            a circle would render as a stretched ellipse, while a vertical segment
            keeps its shape and reads as "something happened on this date". */}
        {drawable.flatMap((serie, index) =>
          serie.points
            .filter((point) => point.marked)
            .map((point) => {
              const y = yFor(point.value);
              return (
                <line
                  key={`${serie.key}:${point.label}`}
                  className={`ui-multi-line-marker ${serie.neutral ? "ui-multi-line-series-neutral" : `ui-multi-line-series-${index}`}`}
                  x1={xFor(point.label).toFixed(2)}
                  x2={xFor(point.label).toFixed(2)}
                  y1={Math.max(0, y - 4).toFixed(2)}
                  y2={Math.min(VIEW, y + 4).toFixed(2)}
                >
                  <title>{`${serie.markerLabel ?? serie.label}: ${point.label} · ${format(point.value)}`}</title>
                </line>
              );
            }),
        )}
      </svg>
      <div className="ui-line-chart-xlabels" aria-hidden="true">
        <span>{xLabels[0]}</span>
        <span>{xLabels[xLabels.length - 1]}</span>
      </div>
      <ul className="ui-multi-line-legend">
        {drawable.map((serie, index) => (
          <li key={serie.key}>
            <span
              className={`ui-multi-line-swatch ${serie.neutral ? "ui-multi-line-series-neutral" : `ui-multi-line-series-${index}`}`}
              aria-hidden="true"
            />
            <span className="ui-multi-line-legend-label">{serie.label}</span>
            {serie.legendValue !== undefined ? (
              <b className="num-tabular">{serie.legendValue}</b>
            ) : null}
          </li>
        ))}
      </ul>
    </div>
  );
}
