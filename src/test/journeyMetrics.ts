// Pure accounting core for journey friction metrics beyond raw clicks (ADR 0074
// pt 3, ADR 0081 Q3). No Playwright dependency: this module tracks interaction/
// screen/modal/context events and evaluates them against the budgets.json v2
// schema, so the counting and budget logic is unit-testable without a browser.
// `tests/browser/helpers/journey.ts` is the thin Playwright adapter that drives
// real DOM actions and delegates all counting/evaluation to this module — there
// is no second accounting implementation to drift from this one.
//
// Hard/ratcheted metrics (asserted by assertBudget): interactions, distinct
// screen transitions, newly opened dialogs, and context loss when the spec
// explicitly calls preserveContext. Feedback timing and pre-scroll visibility
// are advisory only (attached to the Playwright result, not evaluated here).

export type JourneyMetricName = "interactions" | "screenTransitions" | "modalOpens" | "contextLosses";

export const JOURNEY_METRIC_NAMES: readonly JourneyMetricName[] = [
  "interactions",
  "screenTransitions",
  "modalOpens",
  "contextLosses",
];

export type JourneyBudget = {
  interactions: number;
  screenTransitions: number;
  modalOpens: number;
  contextLosses: number;
  /**
   * Present only where a real narrow-pane flow genuinely requires different
   * user actions (plan Q3). Must not be used to hide a common regression: a
   * regression that shows up on every project belongs in the base metric.
   */
  byProject?: Record<string, Partial<Record<JourneyMetricName, number>>>;
};

export type JourneyBudgetsFile = {
  schemaVersion: number;
  journeys: Record<string, JourneyBudget>;
};

export type JourneyTraceEntry = {
  kind: "interaction" | "screen" | "modal" | "contextPreserved" | "contextReset" | "contextLoss";
  detail: string;
};

/** Validate the budgets.json shape; throws with an actionable message on any v1/malformed file. */
export function assertValidBudgetsFile(file: unknown): asserts file is JourneyBudgetsFile {
  if (typeof file !== "object" || file === null) {
    throw new Error("tests/browser/journeys/budgets.json is not an object");
  }
  const candidate = file as Record<string, unknown>;
  if (candidate.schemaVersion !== 2) {
    throw new Error(
      "tests/browser/journeys/budgets.json must declare schemaVersion 2 " +
        `(interactions/screenTransitions/modalOpens/contextLosses per journey); got ${JSON.stringify(candidate.schemaVersion)}. ` +
        "Migrate the file to schema v2 (see docs/plans/ux-quality-loop-v2.md Q3 § Budget shape).",
    );
  }
  if (typeof candidate.journeys !== "object" || candidate.journeys === null) {
    throw new Error('tests/browser/journeys/budgets.json is missing a "journeys" object (schema v2)');
  }
  for (const [journeyId, raw] of Object.entries(candidate.journeys as Record<string, unknown>)) {
    if (typeof raw !== "object" || raw === null) {
      throw new Error(`budgets.json journey "${journeyId}" is not an object`);
    }
    const entry = raw as Record<string, unknown>;
    for (const metric of JOURNEY_METRIC_NAMES) {
      if (typeof entry[metric] !== "number") {
        throw new Error(`budgets.json journey "${journeyId}" is missing numeric metric "${metric}"`);
      }
    }
  }
}

export function requireJourneyBudget(file: JourneyBudgetsFile, journeyId: string): JourneyBudget {
  const entry = file.journeys[journeyId];
  if (!entry) {
    throw new Error(`No budget entry for journey "${journeyId}" in tests/browser/journeys/budgets.json`);
  }
  return entry;
}

/** The effective limit for a metric: a per-project override when recorded, else the base floor. */
export function resolveBudget(
  file: JourneyBudgetsFile,
  journeyId: string,
  metric: JourneyMetricName,
  project?: string | null,
): number {
  const entry = requireJourneyBudget(file, journeyId);
  const override = project ? entry.byProject?.[project]?.[metric] : undefined;
  return override ?? entry[metric];
}

/** Tracks interaction/screen/modal/context-preservation events for one journey run. */
export class JourneyMetricsRecorder {
  private interactions = 0;
  private screenTransitions = 0;
  private modalOpens = 0;
  private contextLosses = 0;
  private lastScreen: string | null = null;
  private lastModal: string | null = null;
  private preservedContext: string | null = null;
  private readonly trace: JourneyTraceEntry[] = [];

  /** One wrapper call (click/fill/press/selectOption) = one interaction. */
  recordInteraction(detail: string): void {
    this.interactions += 1;
    this.trace.push({ kind: "interaction", detail });
  }

  /**
   * Records landing on a named screen. Counts a transition only when the name
   * differs from the last marked screen — repeating the current screen, or the
   * very first mark of the run, is not a transition. Navigating to a screen is
   * assumed to close any modal opened on the previous screen, so a modal with
   * the same name can legitimately reopen later without being ignored as
   * "already open".
   */
  markScreen(name: string): void {
    if (this.lastScreen !== null && this.lastScreen !== name) {
      this.screenTransitions += 1;
    }
    this.lastScreen = name;
    this.lastModal = null;
    this.trace.push({ kind: "screen", detail: name });
  }

  /**
   * Records a dialog/modal becoming the active one. Ignores a dialog already
   * open at the prior marker (calling markModal twice in a row with the same
   * name — e.g. re-asserting state inside the same modal — is not a second
   * open); a differently named modal, or the same name reopened after a screen
   * transition, counts as a new open.
   */
  markModal(name: string): void {
    if (this.lastModal !== name) {
      this.modalOpens += 1;
      this.trace.push({ kind: "modal", detail: name });
    } else {
      this.trace.push({ kind: "modal", detail: `${name} (already open — not counted)` });
    }
    this.lastModal = name;
  }

  /**
   * Declares the context key the journey expects to still hold (e.g. a filter
   * or selection identity). `null` is a deliberate reset — it clears the
   * baseline without counting a loss, so the next non-null call establishes a
   * fresh expectation instead of being compared against the pre-reset key. A
   * non-null key that differs from a non-null prior key is a context loss.
   */
  preserveContext(key: string | null): void {
    if (key === null) {
      this.preservedContext = null;
      this.trace.push({ kind: "contextReset", detail: "deliberate reset" });
      return;
    }
    if (this.preservedContext !== null && this.preservedContext !== key) {
      this.contextLosses += 1;
      this.trace.push({
        kind: "contextLoss",
        detail: `expected "${this.preservedContext}", observed "${key}"`,
      });
    } else {
      this.trace.push({ kind: "contextPreserved", detail: key });
    }
    this.preservedContext = key;
  }

  snapshot(): Record<JourneyMetricName, number> {
    return {
      interactions: this.interactions,
      screenTransitions: this.screenTransitions,
      modalOpens: this.modalOpens,
      contextLosses: this.contextLosses,
    };
  }

  getTrace(): readonly JourneyTraceEntry[] {
    return this.trace;
  }
}

export type MetricCheckContext = {
  journeyId: string;
  metric: JourneyMetricName;
  actual: number;
  limit: number;
  project: string;
  viewport: string;
  trace: readonly JourneyTraceEntry[];
};

/** An actionable failure message naming journey, metric, actual, limit, project, viewport, and trace. */
export function formatBudgetFailure(ctx: MetricCheckContext): string {
  const traceLines = ctx.trace.map((entry) => `  [${entry.kind}] ${entry.detail}`).join("\n");
  return (
    `Journey "${ctx.journeyId}" exceeded its "${ctx.metric}" budget: ` +
    `${ctx.actual} > ${ctx.limit} (project ${ctx.project}, viewport ${ctx.viewport}). ` +
    "A journey got longer/noisier — this is a UX regression. Fix the flow, or (if the " +
    "extra step is genuinely required) raise the budget in budgets.json AND ux-journeys.md " +
    "deliberately.\n" +
    `Trace:\n${traceLines}`
  );
}

/** Evaluates every hard metric against its effective (byProject-aware) limit; returns one message per breach. */
export function evaluateJourneyBudget(params: {
  file: JourneyBudgetsFile;
  journeyId: string;
  project: string;
  viewport: string;
  metrics: Record<JourneyMetricName, number>;
  trace: readonly JourneyTraceEntry[];
}): string[] {
  requireJourneyBudget(params.file, params.journeyId); // throws if the journey has no entry at all
  const failures: string[] = [];
  for (const metric of JOURNEY_METRIC_NAMES) {
    const limit = resolveBudget(params.file, params.journeyId, metric, params.project);
    const actual = params.metrics[metric];
    if (actual > limit) {
      failures.push(
        formatBudgetFailure({
          journeyId: params.journeyId,
          metric,
          actual,
          limit,
          project: params.project,
          viewport: params.viewport,
          trace: params.trace,
        }),
      );
    }
  }
  return failures;
}

/** A log-only nudge to ratchet a floor down when a metric lands comfortably under it (parity with the v1 interaction-only ratchet). */
export function tightenSuggestion(
  journeyId: string,
  metric: JourneyMetricName,
  actual: number,
  limit: number,
  project: string,
  viewport: string,
): string | null {
  if (actual < limit - 2) {
    return (
      `[journey ${journeyId}] measured ${metric}=${actual}, budget ${limit} ` +
      `(project ${project}, viewport ${viewport}). The journey is measurably shorter than its ` +
      `floor — tighten budgets.json to ${actual + 1} to ratchet the improvement in.`
    );
  }
  return null;
}
