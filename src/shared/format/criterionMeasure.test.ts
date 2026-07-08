import { describe, expect, it } from "vitest";
import { formatCriterionMeasure } from "./criterionMeasure";

// Owner T7 round-2 finding (2026-07-07): the Quality panel rendered the raw
// decimal-exact engine value ("0.2862473442785260575536259831") next to
// "roe >= 15%". Measured values must render through the format layer:
// percent-threshold criteria as a percentage, ratios with sane precision,
// large magnitudes humanized — full precision never shown inline.
describe("formatCriterionMeasure", () => {
  it("renders a percent-threshold measure as a locale percentage", () => {
    expect(formatCriterionMeasure("0.2862473442785260575536259831", "15%", "pl")).toBe("28,6%");
    expect(formatCriterionMeasure("0.2463032947934365", "8%", "en")).toBe("24.6%");
    // The engine normalizes the stored threshold to a decimal ("0.15"); the
    // percent intent then comes from the criterion expression text.
    expect(formatCriterionMeasure("0.2862473442785260575536259831", "0.15 roe >= 15%", "pl")).toBe(
      "28,6%",
    );
  });

  it("renders a plain ratio with two decimals", () => {
    expect(formatCriterionMeasure("2.3333333333", "2.5", "pl")).toBe("2,33");
    expect(formatCriterionMeasure("0.5", null, "en")).toBe("0.50");
  });

  it("humanizes large magnitudes instead of printing every digit", () => {
    expect(formatCriterionMeasure("128500000", "100000000", "pl")).toBe("128,5 mln");
    expect(formatCriterionMeasure("1250", null, "en")).toBe("1.3 k");
  });

  it("falls back to the raw text when the value is not numeric", () => {
    expect(formatCriterionMeasure("n/a", "15%", "pl")).toBe("n/a");
  });
});
