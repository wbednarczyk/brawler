import { describe, expect, it } from "vitest";

import { pluralNoun } from "./plural";

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
});
