import { describe, it, expect } from "vitest";

import { factsRecordedLabel, tierLabel } from "./FundamentalsPanel";

const identity = (value: string) => value;

// Bug e77a1a2 part 3: the header used a `n === 1 ? "fact" : "facts"` two-way
// ternary for both the noun and "recorded", which cannot express Polish's
// three plural categories — a real run's "40 fakty zapisanych" was wrong
// declension (should be "40 faktów zapisanych").
describe("factsRecordedLabel", () => {
  it("uses English one/other for both the noun and the adjective", () => {
    expect(factsRecordedLabel(1, "en")).toBe("1 fact recorded");
    expect(factsRecordedLabel(0, "en")).toBe("0 facts recorded");
    expect(factsRecordedLabel(40, "en")).toBe("40 facts recorded");
  });

  it("declines the Polish noun and adjective together (three plural categories)", () => {
    expect(factsRecordedLabel(1, "pl")).toBe("1 fakt zapisany");
    expect(factsRecordedLabel(3, "pl")).toBe("3 fakty zapisane");
    expect(factsRecordedLabel(40, "pl")).toBe("40 faktów zapisanych");
  });
});

describe("tierLabel", () => {
  it("maps the AI-confirmed-fact tier (Radicle 4fde931) to the AI label", () => {
    expect(tierLabel("ai", identity)).toBe("AI");
  });

  it("maps the legacy ai_text tier to the same AI label", () => {
    expect(tierLabel("ai_text", identity)).toBe("AI");
  });

  it("maps the tagged ESEF/iXBRL tier to its label", () => {
    expect(tierLabel("esef", identity)).toBe("ESEF (tagged)");
  });

  it("falls back to the raw tier string for an unknown tier", () => {
    expect(tierLabel("mystery_tier", identity)).toBe("mystery_tier");
  });
});
