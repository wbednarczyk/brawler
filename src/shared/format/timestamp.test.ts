import { describe, it, expect } from "vitest";
import { formatFeedTimestamp } from "./timestamp";

describe("formatFeedTimestamp", () => {
  it("formats an ISO timestamp without seconds or the T separator", () => {
    // No-offset ISO is parsed as local time, so the hours are stable across TZs.
    expect(formatFeedTimestamp("2026-06-23T18:00:24")).toBe("2026-06-23 18:00");
  });

  it("passes through a friendly non-ISO label unchanged", () => {
    expect(formatFeedTimestamp("Today 09:12")).toBe("Today 09:12");
    expect(formatFeedTimestamp("Yesterday")).toBe("Yesterday");
  });

  it("returns an empty string for missing input", () => {
    expect(formatFeedTimestamp("")).toBe("");
    expect(formatFeedTimestamp(null)).toBe("");
  });
});
