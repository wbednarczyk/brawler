import { describe, expect, it } from "vitest";

import {
  companyEventDueClass,
  companyEventDueLabel,
  formatDetailTimestamp,
  formatListTimestamp,
  formatPollInterval,
  formatWeekRange,
} from "./datetime";

// Fixed reference "now": Sunday 2026-07-05 12:00 local. All list-format rows
// (ADR 0076 Decision 4) are asserted against it in both locales.
const NOW = new Date(2026, 6, 5, 12, 0, 0);

describe("formatListTimestamp — ADR 0076 D4 relative table", () => {
  const rows: Array<[string, string, string, string]> = [
    // input,                    label,        pl,               en
    ["2026-07-05T09:12:00", "today", "dziś 09:12", "today 09:12"],
    ["2026-07-04T17:40:00", "yesterday", "wczoraj 17:40", "yesterday 17:40"],
    ["2026-06-30T18:00:00", "<7 days (weekday)", "wt 18:00", "Tue 18:00"],
    ["2026-03-02T09:12:00", "same year", "2 mar, 09:12", "Mar 2, 09:12"],
    ["2025-07-02T09:12:00", "older", "2 lip 2025", "Jul 2, 2025"],
  ];

  for (const [input, label, pl, en] of rows) {
    it(`${label}: pl`, () => {
      expect(formatListTimestamp(input, "pl", "", NOW)).toBe(pl);
    });
    it(`${label}: en`, () => {
      expect(formatListTimestamp(input, "en", "", NOW)).toBe(en);
    });
  }

  it("never string-surgeries a non-ISO input (the 'oday' bug) — returns it verbatim", () => {
    expect(formatListTimestamp("Today 09:12", "pl", "", NOW)).toBe("Today 09:12");
    expect(formatListTimestamp("Yesterday", "en", "", NOW)).toBe("Yesterday");
  });

  it("returns the empty label for null/blank input", () => {
    expect(formatListTimestamp(null, "pl", "—", NOW)).toBe("—");
    expect(formatListTimestamp("   ", "en", "", NOW)).toBe("");
  });

  it("renders no seconds in list contexts", () => {
    expect(formatListTimestamp("2026-07-05T09:12:47", "en", "", NOW)).toBe("today 09:12");
  });
});

describe("formatDetailTimestamp — full YYYY-MM-DD HH:MM, no seconds", () => {
  it("formats an ISO timestamp without seconds", () => {
    expect(formatDetailTimestamp("2026-06-23T18:00:24")).toBe("2026-06-23 18:00");
  });

  it("returns a non-ISO label verbatim (never replace('T',' ') surgery)", () => {
    expect(formatDetailTimestamp("Today 09:12")).toBe("Today 09:12");
  });

  it("returns the empty label for blank input", () => {
    expect(formatDetailTimestamp(null, "Not set")).toBe("Not set");
    expect(formatDetailTimestamp("")).toBe("Not set");
  });
});

describe("formatWeekRange — localized 'D mon – D mon' with en-dash", () => {
  it("pl", () => {
    expect(formatWeekRange("2026-06-30", "2026-07-04", "pl")).toBe("30 cze – 4 lip");
  });
  it("en", () => {
    expect(formatWeekRange("2026-06-30", "2026-07-04", "en")).toBe("Jun 30 – Jul 4");
  });
});

describe("moved helpers preserve behavior", () => {
  it("companyEventDueLabel/Class relative to today, localized (audit K8)", () => {
    const local = (offsetDays: number) => {
      const d = new Date();
      d.setDate(d.getDate() + offsetDays);
      return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
        d.getDate(),
      ).padStart(2, "0")}`;
    };
    expect(companyEventDueLabel(local(0), "en")).toBe("Today");
    expect(companyEventDueLabel(local(0), "pl")).toBe("Dziś");
    expect(companyEventDueLabel(local(1), "pl")).toBe("Jutro");
    expect(companyEventDueLabel(local(2), "pl")).toBe("Za 2 dni");
    expect(companyEventDueLabel(local(2), "en")).toBe("In 2 days");
    expect(companyEventDueLabel(local(-1), "pl")).toBe("Po terminie");
    expect(companyEventDueLabel(local(-1), "en")).toBe("Past");
    expect(companyEventDueClass(local(0))).toBe("event-due-today");
  });

  it("formatPollInterval humanizes durations", () => {
    expect(formatPollInterval(900)).toBe("15 min");
    expect(formatPollInterval(86400)).toBe("1 day");
    expect(formatPollInterval(30)).toBe("30s");
  });
});
