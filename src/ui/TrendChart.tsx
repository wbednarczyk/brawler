// Dependency-free bar chart for a single KPI across reporting periods. Values
// are plotted against a zero baseline (so losses read as downward bars), with
// per-bar value labels and period labels. Themed via night-neon tokens; no
// animation, so reduced-motion needs no special handling.

export type TrendChartPoint = {
  label: string;
  value: number;
  // Optional preformatted value label (e.g. an as-reported figure). Falls back
  // to formatValue(value) when absent.
  display?: string;
};

export type TrendChartProps = {
  points: TrendChartPoint[];
  ariaLabel: string;
  formatValue?: (value: number) => string;
  height?: number;
  className?: string;
};

const VIEW_HEIGHT = 100;
const STEP = 64; // horizontal space per bar in view units
const BAR_WIDTH = 34;
const TOP_PAD = 16; // room for value labels
const BOTTOM_PAD = 16; // room for period labels

export function TrendChart({ points, ariaLabel, formatValue, height = 180, className }: TrendChartProps) {
  const usable = points.filter((point) => Number.isFinite(point.value));
  if (usable.length === 0) {
    return <div className={["ui-trend-chart-empty", className].filter(Boolean).join(" ")}>{ariaLabel}</div>;
  }

  const values = usable.map((point) => point.value);
  const domainMax = Math.max(0, ...values);
  const domainMin = Math.min(0, ...values);
  const domainSpan = domainMax - domainMin || 1;

  const plotTop = TOP_PAD;
  const plotBottom = VIEW_HEIGHT - BOTTOM_PAD;
  const plotHeight = plotBottom - plotTop;
  const zeroY = plotTop + (plotHeight * domainMax) / domainSpan;

  const width = usable.length * STEP;
  const format = formatValue ?? ((value: number) => String(value));

  return (
    <svg
      className={["ui-trend-chart", className].filter(Boolean).join(" ")}
      width="100%"
      height={height}
      viewBox={`0 0 ${width} ${VIEW_HEIGHT}`}
      role="img"
      aria-label={ariaLabel}
      preserveAspectRatio="xMinYMid meet"
    >
      {/* Zero baseline. */}
      <line x1={0} y1={zeroY} x2={width} y2={zeroY} className="ui-trend-chart-baseline" vectorEffect="non-scaling-stroke" />

      {usable.map((point, index) => {
        const center = index * STEP + STEP / 2;
        const valueHeight = (plotHeight * Math.abs(point.value)) / domainSpan;
        const isNegative = point.value < 0;
        const barY = isNegative ? zeroY : zeroY - valueHeight;
        const labelY = isNegative ? barY + valueHeight + 11 : barY - 4;

        return (
          <g key={`${point.label}-${index}`}>
            <rect
              x={center - BAR_WIDTH / 2}
              y={barY}
              width={BAR_WIDTH}
              height={Math.max(valueHeight, 0.6)}
              rx={2}
              className={isNegative ? "ui-trend-chart-bar ui-trend-chart-bar-negative" : "ui-trend-chart-bar"}
            />
            <text x={center} y={labelY} className="ui-trend-chart-value" textAnchor="middle">
              {point.display ?? format(point.value)}
            </text>
            <text x={center} y={VIEW_HEIGHT - 4} className="ui-trend-chart-label" textAnchor="middle">
              {point.label}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
