import { formatFinancialValue } from "../shared/format/financialValue";
import { niceScale } from "./chartScale";

// Scaled line chart for a dense series (e.g. daily closing prices): the
// Sparkline's line plus the scale a reading needs — min/mid/max y-axis labels
// with gridlines and first/last x labels. Labels are HTML beside/below a
// stretched SVG plot, so text never distorts as the width flexes. For a
// per-period bar reading use TrendChart; for an axis-less inline trend use
// Sparkline.

export type LineChartPoint = {
  label: string;
  value: number;
};

export type LineChartProps = {
  points: LineChartPoint[];
  ariaLabel: string;
  formatValue?: (value: number) => string;
  height?: number;
  className?: string;
};

const VIEW = 100;

export function LineChart({ points, ariaLabel, formatValue, height = 120, className }: LineChartProps) {
  const usable = points.filter((point) => Number.isFinite(point.value));
  if (usable.length < 2) {
    return <div className={["ui-line-chart-empty", className].filter(Boolean).join(" ")}>{ariaLabel}</div>;
  }

  const values = usable.map((point) => point.value);
  // Round-number scale (owner report 2026-07-14): labels sit on nice tick
  // values, never on raw data extremes with loose decimals.
  const scale = niceScale(Math.min(...values), Math.max(...values));
  const span = scale.max - scale.min || 1;

  const coords = usable.map((point, index) => {
    const x = (VIEW * index) / (usable.length - 1);
    // SVG y grows downward; invert so larger values sit higher.
    const y = VIEW - (VIEW * (point.value - scale.min)) / span;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  });
  const yFor = (value: number) => VIEW - (VIEW * (value - scale.min)) / span;

  const format =
    formatValue ?? ((value: number) => formatFinancialValue({ valueNumeric: String(value) }));
  const tone = values[values.length - 1] >= values[0] ? "up" : "down";

  return (
    <div className={["ui-line-chart", `ui-line-chart-${tone}`, className].filter(Boolean).join(" ")}>
      <div className="ui-line-chart-ylabels" aria-hidden="true">
        {[...scale.ticks].reverse().map((tick) => (
          <span key={tick}>{format(tick)}</span>
        ))}
      </div>
      <svg
        height={height}
        viewBox={`0 0 ${VIEW} ${VIEW}`}
        role="img"
        aria-label={ariaLabel}
        preserveAspectRatio="none"
      >
        {scale.ticks.map((tick) => (
          <line
            key={tick}
            x1={0}
            y1={yFor(tick)}
            x2={VIEW}
            y2={yFor(tick)}
            className="ui-line-chart-gridline"
            vectorEffect="non-scaling-stroke"
          />
        ))}
        <polyline
          className="ui-line-chart-line"
          points={coords.join(" ")}
          fill="none"
          stroke="currentColor"
          strokeWidth={1.5}
          vectorEffect="non-scaling-stroke"
        />
      </svg>
      <div className="ui-line-chart-xlabels" aria-hidden="true">
        <span>{usable[0].label}</span>
        <span>{usable[usable.length - 1].label}</span>
      </div>
    </div>
  );
}
