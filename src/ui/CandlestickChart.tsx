import { formatFinancialValue } from "../shared/format/financialValue";
import { niceLogScale, niceScale } from "./chartScale";

// Candlestick chart for a dense OHLC series (e.g. daily session bars): one
// wick (high–low) + body (open–close) per session, an up/down tone per
// candle, and the same readable framing as LineChart — min/mid/max y-scale
// labels with gridlines, first/last x labels. Labels are HTML beside/below a
// stretched SVG plot, so text never distorts as the width flexes. For a
// close-only series use LineChart.

export type CandlestickPoint = {
  label: string;
  open: number;
  high: number;
  low: number;
  close: number;
};

export type CandlestickChartProps = {
  points: CandlestickPoint[];
  ariaLabel: string;
  formatValue?: (value: number) => string;
  height?: number;
  className?: string;
  // Vertical axis mapping (S1b, ADR 0107 dec. 4): "log" is the house standard
  // for price history (equal % moves read as equal distances). Default stays
  // "linear" for full backward compat with existing consumers/snapshots.
  scale?: "linear" | "log";
};

const VIEW_HEIGHT = 100;
// Horizontal view units per candle; the body fills most of the slot. The SVG
// stretches to the container, so these only fix the body:gap proportion.
const STEP = 4;
const BODY_WIDTH = 2.8;

export function CandlestickChart({
  points,
  ariaLabel,
  formatValue,
  height = 120,
  className,
  scale = "linear",
}: CandlestickChartProps) {
  const usable = points.filter(
    (point) =>
      Number.isFinite(point.open) &&
      Number.isFinite(point.high) &&
      Number.isFinite(point.low) &&
      Number.isFinite(point.close),
  );
  if (usable.length < 2) {
    return (
      <div className={["ui-candlestick-chart-empty", className].filter(Boolean).join(" ")}>
        {ariaLabel}
      </div>
    );
  }

  // A non-positive low has no logarithm — fall back to linear for the whole
  // chart rather than render NaN geometry (S1b contract item 3).
  const canLog = scale === "log" && usable.every((point) => Number.isFinite(point.low) && point.low > 0);
  if (scale === "log" && !canLog && import.meta.env.DEV) {
    console.warn('CandlestickChart: scale="log" requires every low > 0 — falling back to linear.');
  }
  const effectiveScale = canLog ? "log" : "linear";

  // Round-number scale (owner report 2026-07-14): the domain expands to nice
  // tick values, so labels read "120 PLN", never "120,95 PLN". Ticks are
  // evenly spaced, so labels (top→bottom) and gridlines lay out uniformly.
  // (Log mode instead keeps the raw min/max — see niceLogScale.)
  const dataMin = Math.min(...usable.map((point) => point.low));
  const dataMax = Math.max(...usable.map((point) => point.high));
  const frame = effectiveScale === "log" ? niceLogScale(dataMin, dataMax) : niceScale(dataMin, dataMax);
  const span =
    effectiveScale === "log"
      ? Math.log10(frame.max) - Math.log10(frame.min) || 1
      : frame.max - frame.min || 1;
  const y = (value: number) =>
    effectiveScale === "log"
      ? VIEW_HEIGHT - (VIEW_HEIGHT * (Math.log10(value) - Math.log10(frame.min))) / span
      : VIEW_HEIGHT - (VIEW_HEIGHT * (value - frame.min)) / span;

  const width = usable.length * STEP;
  const format =
    formatValue ?? ((value: number) => formatFinancialValue({ valueNumeric: String(value) }));

  return (
    <div className={["ui-candlestick-chart", className].filter(Boolean).join(" ")} data-scale={effectiveScale}>
      <div className="ui-candlestick-chart-ylabels" aria-hidden="true">
        {[...frame.ticks].reverse().map((tick) => (
          <span key={tick}>{format(tick)}</span>
        ))}
      </div>
      <svg
        height={height}
        viewBox={`0 0 ${width} ${VIEW_HEIGHT}`}
        role="img"
        aria-label={ariaLabel}
        preserveAspectRatio="none"
      >
        {frame.ticks.map((tick) => (
          <line
            key={tick}
            x1={0}
            y1={y(tick)}
            x2={width}
            y2={y(tick)}
            className="ui-candlestick-gridline"
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {usable.map((point, index) => {
          const center = index * STEP + STEP / 2;
          const up = point.close >= point.open;
          const bodyTop = y(Math.max(point.open, point.close));
          const bodyHeight = Math.max(Math.abs(y(point.open) - y(point.close)), 0.75);
          const toneClass = up ? "ui-candlestick-up" : "ui-candlestick-down";
          return (
            <g key={`${point.label}-${index}`} className={toneClass}>
              <line
                x1={center}
                y1={y(point.high)}
                x2={center}
                y2={y(point.low)}
                className="ui-candlestick-wick"
                vectorEffect="non-scaling-stroke"
              />
              <rect
                x={center - BODY_WIDTH / 2}
                y={bodyTop}
                width={BODY_WIDTH}
                height={bodyHeight}
                className="ui-candlestick-body"
              />
            </g>
          );
        })}
      </svg>
      <div className="ui-candlestick-chart-xlabels" aria-hidden="true">
        <span>{usable[0].label}</span>
        <span>{usable[usable.length - 1].label}</span>
      </div>
    </div>
  );
}
