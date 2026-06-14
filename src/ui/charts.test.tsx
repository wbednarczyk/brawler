import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";

import { Sparkline } from "./Sparkline";
import { TrendChart } from "./TrendChart";

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
