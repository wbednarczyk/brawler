import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PriceContextSection, type PriceContext } from "./PriceContextSection";

// Mock DTO fixture — no Tauri command is wired for T5 (orchestrator connects
// the backend); this exercises the render contract against the shape only.
function makePriceContext(overrides: Partial<PriceContext> = {}): PriceContext {
  return {
    lastClose: 142.5,
    lastDate: "2026-07-10",
    changeAbs: 3.2,
    changePct: 2.3,
    currency: "PLN",
    week52High: 168.0,
    week52Low: 98.4,
    week52HighDistPct: -15.2,
    week52LowDistPct: 44.8,
    marketCap: 12_450_000_000,
    ratios: {
      pe: 14.2,
      pbv: 2.1,
      evEbitda: 9.4,
      divYield: 1.8,
      fcfYield: 5.6,
      ownHistPercentile: 0.62,
    },
    history: Array.from({ length: 30 }, (_, i) => ({
      date: `2026-06-${String((i % 28) + 1).padStart(2, "0")}`,
      open: 129 + i,
      high: 132 + i,
      low: 128 + i,
      close: 130 + i,
    })),
    fetchedAt: "2026-07-10T18:05:00Z",
    ...overrides,
  };
}

describe("PriceContextSection (v0.53 T5)", () => {
  it("renders the populated readout: close, change, 52w range, ratios, staleness", () => {
    render(<PriceContextSection data={makePriceContext()} />);

    expect(screen.getByRole("heading", { name: "Price context" })).toBeInTheDocument();
    // Close + colored change.
    expect(screen.getByText(/142(\.|,)5/)).toBeInTheDocument();
    expect(screen.getByText(/\+.*3(\.|,)2.*PLN/)).toBeInTheDocument();
    // 52w range readout.
    expect(screen.getByText("52w high")).toBeInTheDocument();
    expect(screen.getByText("52w low")).toBeInTheDocument();
    // Ratio readout via InfoGrid.
    expect(screen.getByText("P/E")).toBeInTheDocument();
    expect(screen.getByText("14.2")).toBeInTheDocument();
    // Staleness attribution.
    expect(screen.getByText(/Prices as of/)).toBeInTheDocument();
    // Price history chart rendered (downsampled from 30 daily points).
    expect(screen.getByRole("img", { name: "Price history" })).toBeInTheDocument();
  });

  it("renders '—' for null ratios and market cap without crashing", () => {
    render(
      <PriceContextSection
        data={makePriceContext({
          marketCap: null,
          ratios: { pe: null, pbv: null, evEbitda: null, divYield: null, fcfYield: null, ownHistPercentile: null },
        })}
      />,
    );

    expect(screen.getByText("P/E")).toBeInTheDocument();
    // Six null fields (3 in the range grid's market cap + 5 ratios = every
    // dash-rendered value in this fixture) — assert at least one dash renders.
    expect(screen.getAllByText("—").length).toBeGreaterThanOrEqual(6);
  });

  it("renders the no_quotes empty state without a source-health action", () => {
    render(<PriceContextSection data={makePriceContext({ emptyReason: "no_quotes" })} />);

    expect(screen.getByText("No price data is available for this company yet.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /source health/i })).not.toBeInTheDocument();
  });

  it("renders the unmapped_ticker empty state with a source-health link when provided", async () => {
    const user = userEvent.setup();
    const onViewSourceHealth = vi.fn();

    render(
      <PriceContextSection
        data={makePriceContext({ emptyReason: "unmapped_ticker" })}
        onViewSourceHealth={onViewSourceHealth}
      />,
    );

    expect(
      screen.getByText("This company's ticker is not mapped to a market data source yet."),
    ).toBeInTheDocument();
    const action = screen.getByRole("button", { name: /check source health/i });
    await user.click(action);
    expect(onViewSourceHealth).toHaveBeenCalledTimes(1);
  });

  it("stays legible in a narrow (~960px) pane container without overflowing its own root", () => {
    // jsdom has no real layout engine (container queries can't be exercised
    // here — that is browser-only per docs/ui-authoring.md), so this is a
    // structural proxy: render inside a fixed-width wrapper (960px, the
    // narrow end of the supported pane range) and assert the section renders
    // its content through primitives (InfoGrid/TrendChart) rather than a
    // fixed-px element that would force a wrapper-level scrollbar. Real
    // no-clip verification is the Playwright narrow-viewport matrix.
    const { container } = render(
      <div style={{ width: "960px" }}>
        <PriceContextSection data={makePriceContext()} />
      </div>,
    );

    const chart = container.querySelector(".price-context-history-chart svg") as SVGSVGElement | null;
    expect(chart).toBeTruthy();
    // The CandlestickChart's plot carries a viewBox and its stylesheet rule
    // pins width:100% — it scales with the pane instead of blowing out a
    // 960px cell (jsdom cannot compute the CSS itself).
    expect(chart?.getAttribute("viewBox")).toBeTruthy();
    expect(screen.getByText("Price context")).toBeInTheDocument();
  });
});
