import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";

import { CandlestickChart } from "./CandlestickChart";
import { niceScale } from "./chartScale";
import { DonutChart } from "./DonutChart";
import { LineChart } from "./LineChart";
import { MultiLineChart } from "./MultiLineChart";

describe("niceScale", () => {
  it("expands a price-like span to round ticks (owner report 2026-07-14)", () => {
    // The real ABE case: 89.5–152.4 must not label "120,95 PLN".
    expect(niceScale(89.5, 152.4)).toEqual({ min: 80, max: 160, ticks: [80, 100, 120, 140, 160] });
  });

  it("handles penny-stock spans with fractional steps", () => {
    const scale = niceScale(2.35, 2.6);
    expect(scale.ticks[0]).toBeLessThanOrEqual(2.35);
    expect(scale.ticks[scale.ticks.length - 1]).toBeGreaterThanOrEqual(2.6);
    // Ticks stay clean decimals, no float drift.
    for (const tick of scale.ticks) {
      expect(String(tick).length).toBeLessThanOrEqual(6);
    }
  });

  it("pads a flat series instead of collapsing to a zero span", () => {
    const scale = niceScale(100, 100);
    expect(scale.max).toBeGreaterThan(scale.min);
    expect(scale.ticks.length).toBeGreaterThanOrEqual(3);
  });
});

describe("CandlestickChart", () => {
  const points = [
    { label: "2026-07-10", open: 100, high: 110, low: 95, close: 105 },
    { label: "2026-07-11", open: 105, high: 108, low: 90, close: 92 },
    { label: "2026-07-14", open: 92, high: 99, low: 91, close: 98 },
  ];

  it("renders one wick+body per session with an up/down tone", () => {
    const { container } = render(
      <CandlestickChart ariaLabel="Price history" points={points} formatValue={(v) => `${v} PLN`} />,
    );
    expect(container.querySelectorAll("line.ui-candlestick-wick")).toHaveLength(3);
    expect(container.querySelectorAll("rect.ui-candlestick-body")).toHaveLength(3);
    expect(container.querySelectorAll("g.ui-candlestick-up")).toHaveLength(2);
    expect(container.querySelectorAll("g.ui-candlestick-down")).toHaveLength(1);
  });

  it("scales the y axis to round ticks spanning the high/low extremes", () => {
    const { getByText, container } = render(
      <CandlestickChart ariaLabel="Price history" points={points} formatValue={(v) => `${v} PLN`} />,
    );
    // max high = 110, min low = 90 → nice step 5: 90/95/100/105/110.
    for (const tick of [90, 95, 100, 105, 110]) {
      expect(getByText(`${tick} PLN`)).toBeInTheDocument();
    }
    expect(container.querySelectorAll("line.ui-candlestick-gridline")).toHaveLength(5);
    expect(getByText("2026-07-10")).toBeInTheDocument();
    expect(getByText("2026-07-14")).toBeInTheDocument();
  });

  it("renders an empty placeholder when there is not enough data to plot", () => {
    const { container, getByText } = render(
      <CandlestickChart ariaLabel="Price history" points={points.slice(0, 1)} />,
    );
    expect(container.querySelector("svg")).toBeNull();
    expect(getByText("Price history")).toBeInTheDocument();
  });
});
import { Sparkline } from "./Sparkline";
import { TrendChart } from "./TrendChart";

describe("LineChart", () => {
  const points = [
    { label: "2026-01-02", value: 200 },
    { label: "2026-06-01", value: 250 },
    { label: "2026-07-14", value: 233 },
  ];

  it("renders the line with a round-number scale and first/last labels", () => {
    const { container, getByText } = render(
      <LineChart ariaLabel="Price history" points={points} formatValue={(v) => `${v} PLN`} />,
    );
    const polyline = container.querySelector("polyline.ui-line-chart-line");
    expect(polyline).not.toBeNull();
    expect(polyline?.getAttribute("points")?.trim().split(/\s+/)).toHaveLength(3);
    // Nice scale: 200–250 expands to round ticks 200/220/240/260 — never raw
    // extremes with loose decimals (owner report 2026-07-14).
    expect(getByText("260 PLN")).toBeInTheDocument();
    expect(getByText("240 PLN")).toBeInTheDocument();
    expect(getByText("220 PLN")).toBeInTheDocument();
    expect(getByText("200 PLN")).toBeInTheDocument();
    expect(container.querySelectorAll("line.ui-line-chart-gridline")).toHaveLength(4);
    // The x span is readable: first and last point labels.
    expect(getByText("2026-01-02")).toBeInTheDocument();
    expect(getByText("2026-07-14")).toBeInTheDocument();
  });

  it("marks an upward series distinctly from a downward one", () => {
    const up = render(
      <LineChart ariaLabel="up" points={[{ label: "a", value: 1 }, { label: "b", value: 5 }]} />,
    );
    expect(up.container.firstElementChild?.getAttribute("class")).toContain("ui-line-chart-up");
    const down = render(
      <LineChart ariaLabel="down" points={[{ label: "a", value: 5 }, { label: "b", value: 1 }]} />,
    );
    expect(down.container.firstElementChild?.getAttribute("class")).toContain("ui-line-chart-down");
  });

  it("renders an empty placeholder when there is not enough data to plot", () => {
    const { container, getByText } = render(
      <LineChart ariaLabel="Price history" points={[{ label: "a", value: 2 }]} />,
    );
    expect(container.querySelector("svg")).toBeNull();
    expect(getByText("Price history")).toBeInTheDocument();
  });
});

describe("Sparkline", () => {
  it("renders a polyline with one point per value", () => {
    const { container } = render(<Sparkline values={[1, 2, 3]} ariaLabel="trend" />);
    const polyline = container.querySelector("polyline");
    expect(polyline).not.toBeNull();
    expect(polyline?.getAttribute("points")?.trim().split(/\s+/)).toHaveLength(3);
  });

  it("marks an upward series distinctly from a downward one", () => {
    const up = render(<Sparkline values={[1, 5]} />);
    expect(up.container.querySelector("svg")?.getAttribute("class")).toContain("ui-sparkline-up");
    const down = render(<Sparkline values={[5, 1]} />);
    expect(down.container.querySelector("svg")?.getAttribute("class")).toContain("ui-sparkline-down");
  });

  it("renders a stable placeholder when there is not enough data to trend", () => {
    const { container } = render(<Sparkline values={[2]} />);
    expect(container.querySelector("svg")).toBeNull();
    expect(container.querySelector(".ui-sparkline-empty")).not.toBeNull();
  });
});

describe("TrendChart", () => {
  it("renders one bar and label per point with formatted values", () => {
    const { container, getByText } = render(
      <TrendChart
        ariaLabel="Revenue by period"
        points={[
          { label: "Q1", value: 100 },
          { label: "Q2", value: 200 },
        ]}
        formatValue={(value) => `${value} PLN`}
      />,
    );
    expect(container.querySelectorAll("rect.ui-trend-chart-bar")).toHaveLength(2);
    expect(getByText("100 PLN")).toBeInTheDocument();
    expect(getByText("Q2")).toBeInTheDocument();
  });

  it("renders an empty state when no finite points are given", () => {
    const { getByText } = render(<TrendChart ariaLabel="No data" points={[]} />);
    expect(getByText("No data")).toBeInTheDocument();
  });

  it("marks negative bars distinctly", () => {
    const { container } = render(
      <TrendChart ariaLabel="loss" points={[{ label: "Q1", value: -50 }]} />,
    );
    expect(container.querySelector("rect.ui-trend-chart-bar-negative")).not.toBeNull();
  });
});

describe("DonutChart", () => {
  const slices = [
    { key: "founder", label: "Founders", value: 41.4, kind: "founder" as const },
    { key: "ofe", label: "OFE", value: 11.3, kind: "ofe" as const },
    { key: "float", label: "Free float", value: 46.8, kind: "uncertain" as const },
  ];

  it("renders one colour-classed segment per positive slice", () => {
    const { container } = render(
      <DonutChart ariaLabel="Ownership structure by holder type" slices={slices} />,
    );
    expect(container.querySelectorAll("circle.ui-donut-slice")).toHaveLength(3);
    expect(container.querySelector("circle.ui-donut-slice-founder")).not.toBeNull();
    expect(container.querySelector("circle.ui-donut-slice-ofe")).not.toBeNull();
    expect(container.querySelector("circle.ui-donut-slice-uncertain")).not.toBeNull();
  });

  it("exposes an accessible name on an image-role SVG", () => {
    const { getByRole } = render(
      <DonutChart ariaLabel="Ownership structure by holder type" slices={slices} />,
    );
    expect(getByRole("img", { name: "Ownership structure by holder type" })).toBeInTheDocument();
  });

  it("hatches the uncertain (free-float) slice via a pattern stroke", () => {
    const { container } = render(<DonutChart ariaLabel="Ownership" slices={slices} />);
    expect(container.querySelector("pattern .ui-donut-hatch-line")).not.toBeNull();
    const uncertain = container.querySelector("circle.ui-donut-slice-uncertain");
    expect(uncertain?.getAttribute("stroke")).toMatch(/^url\(#/);
  });

  it("drops zero/negative slices and renders an empty placeholder for no data", () => {
    const { container, getByText } = render(
      <DonutChart ariaLabel="Nothing disclosed" slices={[{ key: "a", label: "a", value: 0, kind: "misc" }]} />,
    );
    expect(container.querySelector("svg")).toBeNull();
    expect(getByText("Nothing disclosed")).toBeInTheDocument();
  });

  it("renders the centre label overlay when provided", () => {
    const { getByText } = render(
      <DonutChart ariaLabel="Ownership" slices={slices} centerLabel={<span>46,8%</span>} />,
    );
    expect(getByText("46,8%")).toBeInTheDocument();
  });
});

describe("MultiLineChart", () => {
  const series = [
    {
      key: "a",
      label: "Holder A",
      legendValue: "25,2%",
      points: [
        { label: "2024-12-31", value: 25.5 },
        { label: "2026-03-31", value: 25.2 },
      ],
    },
    {
      key: "b",
      label: "Holder B",
      points: [
        { label: "2025-12-31", value: 6.0 },
        { label: "2026-03-31", value: 6.1 },
      ],
    },
  ];

  it("draws every series as a slot-coloured polyline on one shared scale", () => {
    const { container } = render(
      <MultiLineChart ariaLabel="Top holders — capital % over time" series={series} />,
    );
    expect(container.querySelectorAll("polyline.ui-multi-line-series")).toHaveLength(2);
    expect(container.querySelector("polyline.ui-multi-line-series-0")).not.toBeNull();
    expect(container.querySelector("polyline.ui-multi-line-series-1")).not.toBeNull();
    // Shared x-domain: first/last labels come from the union of all series.
    expect(container.textContent).toContain("2024-12-31");
    expect(container.textContent).toContain("2026-03-31");
  });

  it("renders a legend with the value beside each series label", () => {
    const { container, getByText } = render(
      <MultiLineChart ariaLabel="Top holders — capital % over time" series={series} />,
    );
    expect(container.querySelectorAll(".ui-multi-line-legend li")).toHaveLength(2);
    const label = getByText("Holder A");
    expect(label.parentElement?.textContent).toContain("25,2%");
  });

  it("caps at four series and empty-states below two drawable points", () => {
    const many = Array.from({ length: 6 }, (_, i) => ({
      key: `s${i}`,
      label: `S${i}`,
      points: [
        { label: "2025-01-01", value: i + 1 },
        { label: "2025-06-30", value: i + 2 },
      ],
    }));
    const { container } = render(
      <MultiLineChart ariaLabel="capped" series={many} />,
    );
    expect(container.querySelectorAll("polyline.ui-multi-line-series")).toHaveLength(4);

    const { container: empty } = render(
      <MultiLineChart
        ariaLabel="not enough data"
        series={[{ key: "x", label: "X", points: [{ label: "2025-01-01", value: 1 }] }]}
      />,
    );
    expect(empty.querySelector(".ui-line-chart-empty")).not.toBeNull();
  });

  it("renders a neutral (dashed) series outside the colour slots", () => {
    const { container } = render(
      <MultiLineChart
        ariaLabel="with float"
        series={[
          ...series,
          {
            key: "float",
            label: "Free float",
            neutral: true,
            points: [
              { label: "2024-12-31", value: 74.5 },
              { label: "2026-03-31", value: 68.5 },
            ],
          },
        ]}
      />,
    );
    expect(container.querySelector("polyline.ui-multi-line-series-neutral")).not.toBeNull();
    // Neutral doesn't consume a categorical slot: slots 0 and 1 stay the holders'.
    expect(container.querySelectorAll("polyline.ui-multi-line-series-0, polyline.ui-multi-line-series-1")).toHaveLength(2);
  });
});
