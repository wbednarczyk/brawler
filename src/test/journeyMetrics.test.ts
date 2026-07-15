import { describe, expect, it } from "vitest";
import {
  JOURNEY_METRIC_NAMES,
  JourneyMetricsRecorder,
  assertValidBudgetsFile,
  evaluateJourneyBudget,
  formatBudgetFailure,
  requireJourneyBudget,
  resolveBudget,
  tightenSuggestion,
  type JourneyBudgetsFile,
} from "./journeyMetrics";

// Pure accounting core for journey friction metrics (ADR 0074 pt 3, ADR 0081 Q3).
// No Playwright dependency — tests/browser/helpers/journey.ts is the thin
// Playwright adapter that drives DOM actions and delegates counting/budget
// evaluation to this module, so the accounting logic is unit-testable here.

const validFile: JourneyBudgetsFile = {
  schemaVersion: 2,
  journeys: {
    J1: { interactions: 7, screenTransitions: 3, modalOpens: 0, contextLosses: 0 },
    J2: {
      interactions: 17,
      screenTransitions: 5,
      modalOpens: 1,
      contextLosses: 0,
      byProject: { "chromium-quarter-uw-125": { interactions: 19 } },
    },
  },
};

describe("assertValidBudgetsFile", () => {
  it("rejects a schema v1 file (no schemaVersion key)", () => {
    expect(() => assertValidBudgetsFile({ J1: 7, J2: 17 })).toThrow(/schemaVersion/i);
  });

  it("rejects a journey entry missing a required metric", () => {
    expect(() =>
      assertValidBudgetsFile({
        schemaVersion: 2,
        journeys: { J1: { interactions: 7, screenTransitions: 3, modalOpens: 0 } },
      }),
    ).toThrow(/contextLosses/);
  });

  it("accepts a well-formed schema v2 file", () => {
    expect(() => assertValidBudgetsFile(validFile)).not.toThrow();
  });
});

describe("requireJourneyBudget / resolveBudget", () => {
  it("throws an actionable error for a missing journey entry", () => {
    expect(() => requireJourneyBudget(validFile, "J9")).toThrow(/No budget entry for journey "J9"/);
  });

  it("resolves the base metric when no byProject override exists", () => {
    expect(resolveBudget(validFile, "J1", "interactions", "chromium-compact")).toBe(7);
  });

  it("resolves a byProject override over the base metric", () => {
    expect(resolveBudget(validFile, "J2", "interactions", "chromium-quarter-uw-125")).toBe(19);
    expect(resolveBudget(validFile, "J2", "interactions", "chromium-compact")).toBe(17);
  });
});

describe("JourneyMetricsRecorder — interactions", () => {
  it("counts one interaction per recordInteraction call", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.recordInteraction("click A");
    recorder.recordInteraction("click B");
    expect(recorder.snapshot().interactions).toBe(2);
  });
});

describe("JourneyMetricsRecorder — screen transitions", () => {
  it("does not count the first markScreen as a transition", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.markScreen("Today");
    expect(recorder.snapshot().screenTransitions).toBe(0);
  });

  it("counts a transition only when the screen name changes", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.markScreen("Today");
    recorder.markScreen("Today"); // still on Today — no transition
    recorder.markScreen("Company workspace"); // transition 1
    recorder.markScreen("Today"); // transition 2
    expect(recorder.snapshot().screenTransitions).toBe(2);
  });
});

describe("JourneyMetricsRecorder — modal opens", () => {
  it("ignores a dialog already open at the prior marker", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.markModal("KPI extraction");
    recorder.markModal("KPI extraction"); // already open — not a second open
    expect(recorder.snapshot().modalOpens).toBe(1);
  });

  it("counts a newly named modal as a new open", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.markModal("New view");
    recorder.markModal("Command palette");
    expect(recorder.snapshot().modalOpens).toBe(2);
  });

  it("re-counts the same modal name after a screen transition closed it", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.markModal("KPI extraction");
    recorder.markScreen("Company workspace");
    recorder.markModal("KPI extraction");
    expect(recorder.snapshot().modalOpens).toBe(2);
  });
});

describe("JourneyMetricsRecorder — context loss", () => {
  it("catches a loss when the observed context key changes unexpectedly", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.preserveContext("today-filter:autopilot");
    recorder.preserveContext("today-filter:none"); // lost without a deliberate reset
    expect(recorder.snapshot().contextLosses).toBe(1);
  });

  it("ignores a deliberate reset (null) before the key changes", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.preserveContext("today-filter:autopilot");
    recorder.preserveContext(null); // deliberate reset
    recorder.preserveContext("today-filter:none"); // fresh baseline, not a loss
    expect(recorder.snapshot().contextLosses).toBe(0);
  });

  it("does not count repeating the same preserved key", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.preserveContext("today-filter:autopilot");
    recorder.preserveContext("today-filter:autopilot");
    expect(recorder.snapshot().contextLosses).toBe(0);
  });
});

describe("evaluateJourneyBudget", () => {
  it("returns no failures when every metric is within budget", () => {
    const recorder = new JourneyMetricsRecorder();
    recorder.recordInteraction("click");
    const failures = evaluateJourneyBudget({
      file: validFile,
      journeyId: "J1",
      project: "chromium-compact",
      viewport: "1366x768",
      metrics: recorder.snapshot(),
      trace: recorder.getTrace(),
    });
    expect(failures).toEqual([]);
  });

  it("reports each hard metric that exceeds its own limit with an actionable message", () => {
    const recorder = new JourneyMetricsRecorder();
    for (let i = 0; i < 8; i += 1) recorder.recordInteraction(`click ${i}`);
    const failures = evaluateJourneyBudget({
      file: validFile,
      journeyId: "J1",
      project: "chromium-compact",
      viewport: "1366x768",
      metrics: recorder.snapshot(),
      trace: recorder.getTrace(),
    });
    expect(failures).toHaveLength(1);
    expect(failures[0]).toMatch(/J1/);
    expect(failures[0]).toMatch(/interactions/);
    expect(failures[0]).toMatch(/8/);
    expect(failures[0]).toMatch(/7/);
    expect(failures[0]).toMatch(/chromium-compact/);
    expect(failures[0]).toMatch(/1366x768/);
    expect(failures[0]).toMatch(/click 0/); // trace is included
  });

  it("uses the byProject override as the limit when one is recorded", () => {
    const recorder = new JourneyMetricsRecorder();
    for (let i = 0; i < 18; i += 1) recorder.recordInteraction(`click ${i}`);
    const failuresNarrow = evaluateJourneyBudget({
      file: validFile,
      journeyId: "J2",
      project: "chromium-quarter-uw-125",
      viewport: "1024x1152",
      metrics: recorder.snapshot(),
      trace: recorder.getTrace(),
    });
    expect(failuresNarrow).toEqual([]); // 18 <= byProject override of 19

    const failuresCompact = evaluateJourneyBudget({
      file: validFile,
      journeyId: "J2",
      project: "chromium-compact",
      viewport: "1366x768",
      metrics: recorder.snapshot(),
      trace: recorder.getTrace(),
    });
    expect(failuresCompact).toHaveLength(1); // 18 > base budget of 17
  });
});

describe("formatBudgetFailure", () => {
  it("names journey, metric, actual, limit, project, viewport, and includes the trace", () => {
    const message = formatBudgetFailure({
      journeyId: "J1",
      metric: "modalOpens",
      actual: 2,
      limit: 1,
      project: "chromium-wide",
      viewport: "1920x1080",
      trace: [{ kind: "modal", detail: "KPI extraction" }],
    });
    expect(message).toMatch(/J1/);
    expect(message).toMatch(/modalOpens/);
    expect(message).toMatch(/2/);
    expect(message).toMatch(/1/);
    expect(message).toMatch(/chromium-wide/);
    expect(message).toMatch(/1920x1080/);
    expect(message).toMatch(/KPI extraction/);
  });
});

describe("tightenSuggestion", () => {
  it("suggests tightening when the metric is more than 2 under its floor", () => {
    const suggestion = tightenSuggestion("J1", "interactions", 3, 7, "chromium-compact", "1366x768");
    expect(suggestion).toMatch(/tighten/i);
  });

  it("returns null when the metric is close to its floor", () => {
    expect(tightenSuggestion("J1", "interactions", 6, 7, "chromium-compact", "1366x768")).toBeNull();
  });
});

describe("JOURNEY_METRIC_NAMES", () => {
  it("is the exact schema v2 metric set", () => {
    expect([...JOURNEY_METRIC_NAMES].sort()).toEqual(
      ["contextLosses", "interactions", "modalOpens", "screenTransitions"].sort(),
    );
  });
});
