import { describe, expect, it } from "vitest";
import { tickerExchangeColor } from "./TickerLabel";

describe("tickerExchangeColor", () => {
  it("uses distinct colors for exchange prefixes", () => {
    expect(tickerExchangeColor("GPW:CDR")).toBe("#57d7ff");
    expect(tickerExchangeColor("NC:4MB")).toBe("#ffd166");
    expect(tickerExchangeColor("NASDAQ:MSFT")).toBe("#63c0e9");
    expect(tickerExchangeColor("NYSE:IBM")).toBe("#f0b85a");
  });

  it("assigns a stable palette color for future exchanges", () => {
    expect(tickerExchangeColor("LSE:VOD")).toBe(tickerExchangeColor("LSE:BARC"));
    expect(tickerExchangeColor("LSE:VOD")).not.toBe(tickerExchangeColor("FWB:SAP"));
  });
});
