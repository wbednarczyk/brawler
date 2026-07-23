import { describe, expect, it } from "vitest";

import { EVENT_FORMS, FACT_FORMS, RUN_FORMS, pluralNoun } from "./plural";

const COMPANY = { en: ["company", "companies"], pl: ["spółka", "spółki", "spółek"] } as const;

describe("pluralNoun", () => {
  it("uses Polish three-form plurals", () => {
    expect(pluralNoun("pl", 1, COMPANY)).toBe("spółka");
    expect(pluralNoun("pl", 2, COMPANY)).toBe("spółki");
    expect(pluralNoun("pl", 4, COMPANY)).toBe("spółki");
    expect(pluralNoun("pl", 5, COMPANY)).toBe("spółek");
    expect(pluralNoun("pl", 12, COMPANY)).toBe("spółek"); // teens are "many"
    expect(pluralNoun("pl", 14, COMPANY)).toBe("spółek");
    expect(pluralNoun("pl", 18, COMPANY)).toBe("spółek"); // the screenshot case
    expect(pluralNoun("pl", 22, COMPANY)).toBe("spółki"); // 22 -> few
    expect(pluralNoun("pl", 0, COMPANY)).toBe("spółek");
  });

  it("uses English one/other", () => {
    expect(pluralNoun("en", 1, COMPANY)).toBe("company");
    expect(pluralNoun("en", 0, COMPANY)).toBe("companies");
    expect(pluralNoun("en", 18, COMPANY)).toBe("companies");
  });

  // Shared across Today's autopilot run card and the Fundamentals header
  // (bug e77a1a2 part 3: "40 fakty zapisanych" was wrong declension).
  it("declines the shared FACT_FORMS correctly in Polish", () => {
    expect(pluralNoun("pl", 1, FACT_FORMS)).toBe("fakt");
    expect(pluralNoun("pl", 3, FACT_FORMS)).toBe("fakty");
    expect(pluralNoun("pl", 40, FACT_FORMS)).toBe("faktów");
  });

  // Today's per-company group count-chip units (owner dogfooding 2026-07-23).
  it("declines EVENT_FORMS and RUN_FORMS units", () => {
    expect(pluralNoun("pl", 1, EVENT_FORMS)).toBe("zdarzenie");
    expect(pluralNoun("pl", 4, EVENT_FORMS)).toBe("zdarzenia");
    expect(pluralNoun("pl", 5, EVENT_FORMS)).toBe("zdarzeń");
    expect(pluralNoun("pl", 2, RUN_FORMS)).toBe("runy");
    expect(pluralNoun("pl", 5, RUN_FORMS)).toBe("runów");
    expect(pluralNoun("en", 2, EVENT_FORMS)).toBe("events");
    expect(pluralNoun("en", 1, RUN_FORMS)).toBe("run");
  });
});
