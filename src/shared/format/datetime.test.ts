import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  companyEventDueClass,
  companyEventDueLabel,
  formatDayRangeLabel,
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

  // A domain-date-only event (bare YYYY-MM-DD, or an explicit 00:00) carries no
  // meaningful time — render the date alone, never a misleading "… 00:00".
  it("drops a midnight/bare-date time in the same-year branch", () => {
    expect(formatListTimestamp("2026-03-02", "pl", "", NOW)).toBe("2 mar");
    expect(formatListTimestamp("2026-03-02T00:00:00", "en", "", NOW)).toBe("Mar 2");
  });

  it("drops the time for a midnight today / yesterday / weekday", () => {
    expect(formatListTimestamp("2026-07-05T00:00:00", "pl", "", NOW)).toBe("dziś");
    expect(formatListTimestamp("2026-07-04", "en", "", NOW)).toBe("yesterday");
    expect(formatListTimestamp("2026-06-30", "en", "", NOW)).toBe("Tue");
  });

  it("still shows a meaningful (non-midnight) time", () => {
    expect(formatListTimestamp("2026-03-02T09:12:00", "en", "", NOW)).toBe("Mar 2, 09:12");
    expect(formatListTimestamp("2026-07-05T00:01:00", "en", "", NOW)).toBe("today 00:01");
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

describe("formatDayRangeLabel — Dziś v2 'Wcześniej' rollup weekday range (F2 S5)", () => {
  it("joins two distinct weekdays with an en-dash", () => {
    expect(formatDayRangeLabel("2026-06-29", "2026-06-30", "pl")).toBe("pon–wt");
    expect(formatDayRangeLabel("2026-06-29", "2026-06-30", "en")).toBe("Mon–Tue");
  });

  it("collapses to a single weekday abbreviation when both ends are the same day", () => {
    expect(formatDayRangeLabel("2026-06-30", "2026-06-30", "pl")).toBe("wt");
  });
});

describe("moved helpers preserve behavior", () => {
  // companyEventDueLabel/Class read `new Date()` internally (no injectable now),
  // so pin the wall clock — otherwise the offset dates and the function's own
  // clock can straddle midnight and shift the relative bucket by a day.
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

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
