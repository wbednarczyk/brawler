// Tiny dependency-free inline trend line, sized to sit inside a table cell or
// label. Themed via currentColor so callers control the hue with `color`.

export type SparklineProps = {
  values: number[];
  width?: number;
  height?: number;
  ariaLabel?: string;
  className?: string;
};

export function Sparkline({ values, width = 84, height = 22, ariaLabel, className }: SparklineProps) {
  const points = values.filter((value) => Number.isFinite(value));
  if (points.length < 2) {
    // Nothing meaningful to trend; render a flat placeholder for stable layout.
    return <span className={["ui-sparkline-empty", className].filter(Boolean).join(" ")} aria-hidden="true" />;
  }

  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min || 1;
  const pad = 2;
  const innerW = width - pad * 2;
  const innerH = height - pad * 2;

  const coords = points.map((value, index) => {
    const x = pad + (innerW * index) / (points.length - 1);
    // SVG y grows downward; invert so larger values sit higher.
    const y = pad + innerH - (innerH * (value - min)) / span;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  });

  const lastValue = points[points.length - 1];
  const firstValue = points[0];
  const tone = lastValue >= firstValue ? "up" : "down";

  return (
    <svg
      className={["ui-sparkline", `ui-sparkline-${tone}`, className].filter(Boolean).join(" ")}
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      role={ariaLabel ? "img" : "presentation"}
      aria-label={ariaLabel}
      preserveAspectRatio="none"
    >
      <polyline points={coords.join(" ")} fill="none" stroke="currentColor" strokeWidth={1.5} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}
