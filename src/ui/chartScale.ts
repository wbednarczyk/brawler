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

// Log-scale ticks (S1b, ADR 0107 dec. 4 — log axis is the house standard for
// price charts): unlike niceScale, bounds stay the raw data min/max (not
// expanded to a step multiple), so a scale=log consumer's y-mapping keeps its
// true geometric midpoint. Tick candidates are round 1/2/5 × 10^k mantissas
// (decade ticks, never a bare power of 10) filtered to the data span; a span
// too narrow for 3 mantissa ticks falls back to niceScale's ticks intersected
// with the span — still round prices, just not mantissa-aligned.
const LOG_MANTISSAS = [1, 2, 5];

export function niceLogScale(dataMin: number, dataMax: number): NiceScale {
  if (!(dataMin > 0) || !(dataMax > dataMin)) {
    return niceScale(dataMin, dataMax);
  }
  const lowDecade = Math.floor(Math.log10(dataMin)) - 1;
  const highDecade = Math.floor(Math.log10(dataMax)) + 1;
  const candidates: number[] = [];
  for (let decade = lowDecade; decade <= highDecade; decade++) {
    for (const mantissa of LOG_MANTISSAS) {
      candidates.push(Number((mantissa * 10 ** decade).toPrecision(12)));
    }
  }
  let ticks = candidates.filter((tick) => tick >= dataMin && tick <= dataMax);
  while (ticks.length > 6) ticks = ticks.filter((_, index) => index % 2 === 0);
  if (ticks.length < 3) {
    ticks = niceScale(dataMin, dataMax).ticks.filter((tick) => tick >= dataMin && tick <= dataMax);
  }
  if (ticks.length < 2) ticks = [dataMin, dataMax];
  return { min: dataMin, max: dataMax, ticks: ticks.sort((a, b) => a - b) };
}
