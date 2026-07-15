// Shared "nice scale" for the framed charts (LineChart, CandlestickChart):
// instead of labeling the raw data extremes (which yields grosze like
// "152,4 PLN"), the domain expands to round tick values — step chosen from
// {1, 2, 2.5, 5} × 10^k so ~4 intervals cover the data span, ticks at step
// multiples (owner report 2026-07-14: "po co pokazywać grosze"). Ticks are
// evenly spaced by construction, so the chart frame can lay labels and
// gridlines out with plain flex/loops — no per-label positioning.

export type NiceScale = {
  min: number;
  max: number;
  ticks: number[];
};

function niceStep(raw: number): number {
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const normalized = raw / magnitude;
  const factor = normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 2.5 ? 2.5 : normalized <= 5 ? 5 : 10;
  return factor * magnitude;
}

export function niceScale(dataMin: number, dataMax: number, intervals = 4): NiceScale {
  const span = dataMax - dataMin;
  if (!Number.isFinite(span) || span <= 0) {
    // Flat or degenerate series: pad ±1 step around the value.
    const step = niceStep(Math.max(Math.abs(dataMin) / 10, 1e-6));
    const min = Math.floor(dataMin / step) * step - step;
    const max = min + 2 * step;
    return { min, max, ticks: [min, min + step, max] };
  }
  const step = niceStep(span / intervals);
  const min = Math.floor(dataMin / step) * step;
  const max = Math.ceil(dataMax / step) * step;
  const count = Math.round((max - min) / step);
  const ticks = Array.from({ length: count + 1 }, (_, index) =>
    // Round away float drift (0.30000000000000004) at fractional steps.
    Number((min + index * step).toPrecision(12)),
  );
  return { min, max, ticks };
}
