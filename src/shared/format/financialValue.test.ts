import { describe, expect, it } from "vitest";

import { formatFinancialValue, formatFixedDecimal, formatFixedPercent } from "./financialValue";

describe("formatFinancialValue", () => {
  it("renders the as-reported figure with its scale word and currency", () => {
    expect(
      formatFinancialValue({
        valueNumeric: "2439300000",
        currency: "PLN",
        asReportedValue: "2 439,3",
        asReportedScale: "mln",
        valueKind: "monetary",
      }),
    ).toBe("2 439,3 mln PLN");
  });

  it("keeps the document's scale word even when the UI locale is English", () => {
    expect(
      formatFinancialValue(
        {
          valueNumeric: "142312000",
          currency: "PLN",
          asReportedValue: "142 312",
          asReportedScale: "tys.",
          valueKind: "monetary",
        },
        "en",
      ),
    ).toBe("142 312 tys. PLN");
  });

  it("appends % for as-reported percentages without doubling it", () => {
    expect(
      formatFinancialValue({
        valueNumeric: "0.421",
        asReportedValue: "42,1",
        asReportedScale: "",
        valueKind: "percentage",
      }),
    ).toBe("42,1 %");

    expect(
      formatFinancialValue({
        valueNumeric: "0.421",
        asReportedValue: "42,1%",
        valueKind: "percentage",
      }),
    ).toBe("42,1%");
  });

  it("renders an as-reported per-share figure with currency, no scale word", () => {
    expect(
      formatFinancialValue({
        valueNumeric: "478",
        currency: "PLN",
        asReportedValue: "4,78",
        asReportedScale: "",
        valueKind: "monetary",
        unit: "per_share",
      }),
    ).toBe("4,78 PLN");
  });

  it("auto-scales a monetary base value to millions in the fallback path", () => {
    expect(
      formatFinancialValue(
        { valueNumeric: "319700000", currency: "PLN", valueKind: "monetary" },
        "pl",
      ),
    ).toBe("319,7 mln PLN");
  });

  it("auto-scales to thousands and billions in the fallback path", () => {
    expect(
      formatFinancialValue({ valueNumeric: "142312", currency: "PLN", valueKind: "monetary" }, "pl"),
    ).toBe("142,3 tys. PLN");
    expect(
      formatFinancialValue({ valueNumeric: "2439300000", currency: "PLN", valueKind: "monetary" }, "en"),
    ).toBe("2.4 B PLN");
  });

  it("formats small monetary fallbacks without a scale word", () => {
    expect(
      formatFinancialValue({ valueNumeric: "842", currency: "PLN", valueKind: "monetary" }, "en"),
    ).toBe("842 PLN");
  });

  it("formats counts with grouping and no currency", () => {
    expect(
      formatFinancialValue({ valueNumeric: "100250000", valueKind: "count", unit: "shares" }, "pl"),
    ).toBe("100 250 000");
  });

  it("formats a percentage fallback with a percent sign", () => {
    expect(
      formatFinancialValue({ valueNumeric: "42.1", valueKind: "percentage" }, "en"),
    ).toBe("42.1 %");
  });

  it("ignores a junk numeric as-reported scale (no stray '1')", () => {
    // Count with a bare "1" scale from the extractor.
    expect(
      formatFinancialValue({
        valueNumeric: "15661260",
        asReportedValue: "15 661 260",
        asReportedScale: "1",
        valueKind: "count",
      }),
    ).toBe("15 661 260");
    // Per-share figure: drop the "1", keep the currency.
    expect(
      formatFinancialValue({
        valueNumeric: "863",
        currency: "PLN",
        asReportedValue: "8,63",
        asReportedScale: "1",
        valueKind: "monetary",
        unit: "per_share",
      }),
    ).toBe("8,63 PLN");
  });

  it("falls back to the raw string when the base value is not numeric", () => {
    expect(formatFinancialValue({ valueNumeric: "n/a", valueKind: "monetary" })).toBe("n/a");
  });
});

describe("formatFixedDecimal / formatFixedPercent", () => {
  it("never renders negative zero (live harvest 2026-07-15, shortPositions panel)", () => {
    expect(formatFixedPercent(-0, "pl")).toBe("0,00%");
    expect(formatFixedPercent(-0.0004, "pl")).toBe("0,00%");
    expect(formatFixedDecimal(-0, "en")).toBe("0.00");
  });

  it("keeps real negatives negative", () => {
    expect(formatFixedPercent(-0.53, "pl")).toBe("-0,53%");
  });
});
