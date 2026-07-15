import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import type { PriceContext } from "../../api/generated/PriceContext";
import { PriceContextSection } from "../../screens/Companies/PriceContextSection";
import wireFixture from "./priceContextPopulated.json";

// Cross-language populated-path contract (review 2026-07-14, M1). The fixture
// is the EXACT wire JSON the Rust command emits — pinned byte-for-byte by
// `commands::market_data::tests::populated_price_context_matches_the_shared_wire_fixture`.
// Rendering it through the real component (typed as the generated DTO) closes
// the gap the mock-fidelity corpus leaves: the corpus only covers empty
// states, so a populated-branch drift (field casing, percent scaling, null
// handling) would otherwise pass the gate silently. If this file's shape ever
// disagrees with the generated type, `tsc` reddens; if a value stops
// rendering, this test reddens; if the Rust wire format moves, the Rust side
// reddens — update BOTH sides deliberately.
describe("populated PriceContext wire contract (v0.53 M1)", () => {
  const data: PriceContext = wireFixture;

  it("the real Rust wire JSON renders through PriceContextSection", () => {
    render(<PriceContextSection data={data} />);

    expect(screen.getByRole("heading", { name: "Price context" })).toBeInTheDocument();
    // Close and the 52w bounds from the wire values (the numbers also appear
    // in the chart labels, so match all occurrences).
    expect(screen.getAllByText(/233\sPLN/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/250\sPLN/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/200\sPLN/).length).toBeGreaterThan(0);
    // The percentile is suppressed below the 20-session coverage floor
    // (this fixture carries 4 bars), so it renders as an em dash.
    // Null ratios (pe/pbv/...) render as an em dash, never a crash.
    expect(screen.getAllByText("—").length).toBeGreaterThan(0);
  });

  it("the wire fixture carries the pinned scaling conventions", () => {
    // Percent fields arrive pre-scaled to points; the percentile stays a raw fraction.
    expect(data.changePct).toBeCloseTo(0.8658, 3);
    expect(data.week52LowDistPct).toBeCloseTo(16.5, 6);
    // Coverage floor: below 20 sessions the percentile is null, never noise.
    expect(data.ratios.ownHistPercentile).toBeNull();
    expect(data.history.length).toBeLessThan(20);
    // A populated payload omits emptyReason entirely (serde skip), it is not null-filled.
    expect("emptyReason" in (wireFixture as Record<string, unknown>)).toBe(false);
  });
});
