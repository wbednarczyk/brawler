import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";

import { CandlestickChart } from "./CandlestickChart";

// S1b (F3a #429, ADR 0107 dec. 4): the "scale" prop and its log mapping.
// Linear-mode coverage (default rendering, ticks, empty state) already lives
// in charts.test.tsx and stays untouched — this file only covers the new
// scale prop's behavior.
describe("CandlestickChart scale prop", () => {
  const points = [
    { label: "2026-07-10", open: 100, high: 110, low: 95, close: 105 },
    { label: "2026-07-11", open: 105, high: 108, low: 90, close: 92 },
    { label: "2026-07-14", open: 92, high: 99, low: 91, close: 98 },
  ];

  it("defaults to linear when scale is omitted", () => {
    const { container, getByText } = render(
      <CandlestickChart ariaLabel="Price history" points={points} formatValue={(v) => `${v} PLN`} />,
    );
    expect(container.firstElementChild).toHaveAttribute("data-scale", "linear");
    // Same ticks as the pre-existing linear behavior (charts.test.tsx): max
    // high 110, min low 90 → nice step 5.
    for (const tick of [90, 95, 100, 105, 110]) {
      expect(getByText(`${tick} PLN`)).toBeInTheDocument();
    }
  });

  it("log scale maps the geometric midpoint to the vertical midpoint", () => {
    // Combined low=100, high=400 across the two candles → domain [100, 400].
    // 200 is the geometric mean of 100 and 400, so on a log axis it sits
    // exactly halfway down VIEW_HEIGHT=100 (y=50).
    const logPoints = [
      { label: "a", open: 150, high: 250, low: 100, close: 200 },
      { label: "b", open: 300, high: 400, low: 150, close: 350 },
    ];
    const { container } = render(
      <CandlestickChart ariaLabel="Price history" points={logPoints} scale="log" formatValue={(v) => `${v}`} />,
    );
    expect(container.firstElementChild).toHaveAttribute("data-scale", "log");
    const gridlines = Array.from(container.querySelectorAll("line.ui-candlestick-gridline"));
    // Fallback ticks for a 2-candidate mantissa span are niceScale's ticks
    // intersected with [100, 400]: [100, 200, 300, 400] ascending, same order
    // as the gridlines are drawn — index 1 is the value=200 gridline.
    expect(gridlines).toHaveLength(4);
    const y1 = Number(gridlines[1].getAttribute("y1"));
    expect(y1).toBeGreaterThanOrEqual(49.5);
    expect(y1).toBeLessThanOrEqual(50.5);
  });

  it("log ticks are round prices, never exponents", () => {
    const logPoints = [
      { label: "a", open: 150, high: 300, low: 100, close: 250 },
      { label: "b", open: 300, high: 800, low: 200, close: 700 },
    ];
    const { getByText, container } = render(
      <CandlestickChart ariaLabel="Price history" points={logPoints} scale="log" formatValue={(v) => `${v} PLN`} />,
    );
    for (const tick of [100, 200, 500]) {
      expect(getByText(`${tick} PLN`)).toBeInTheDocument();
    }
    expect(container.textContent).not.toMatch(/e[+-]?\d/i);
    expect(container.textContent).not.toMatch(/10\^/);
  });

  it("falls back to linear with a dev warning when a low is non-positive", () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const badPoints = [
      { label: "a", open: 10, high: 20, low: 0, close: 15 },
      { label: "b", open: 15, high: 25, low: 5, close: 20 },
    ];
    const { container } = render(
      <CandlestickChart ariaLabel="Price history" points={badPoints} scale="log" formatValue={(v) => `${v}`} />,
    );
    expect(container.firstElementChild).toHaveAttribute("data-scale", "linear");
    expect(warnSpy).toHaveBeenCalledTimes(1);
    warnSpy.mockRestore();
  });
});
