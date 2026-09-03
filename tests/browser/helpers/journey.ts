import { expect, test, type Locator, type Page } from "@playwright/test";
import budgetsJson from "../journeys/budgets.json" with { type: "json" };
import {
  JOURNEY_METRIC_NAMES,
  JourneyMetricsRecorder,
  assertValidBudgetsFile,
  evaluateJourneyBudget,
  tightenSuggestion,
  type JourneyBudgetsFile,
} from "../../../src/test/journeyMetrics";

// Journey interaction/friction wrapper (ADR 0074 pt 3, ADR 0081 Q3). The
// counting/budget-evaluation logic itself lives in the Playwright-free
// `src/test/journeyMetrics.ts` (unit-tested there); this module is the thin
// Playwright adapter that drives real DOM actions and the semantic observation
// points (`data-app-section`, `data-company-id`, `aria-current="page"`) and
// records them into the shared accounting core. Never mix a raw `locator.click()`
// into a measured journey — everything the user does goes through this wrapper.

assertValidBudgetsFile(budgetsJson);
const budgetsFile = budgetsJson as JourneyBudgetsFile;

export type Journey = {
  click(locator: Locator): Promise<void>;
  /**
   * The explicit experience-contract primary action for the current decision
   * surface. Same as `click`, but also records (advisory only, attached to the
   * Playwright result — never a hard gate) whether the action was inside the
   * surface's visible scrollport before Playwright's auto-scroll moved it there.
   */
  clickPrimary(surface: Locator, action: Locator): Promise<void>;
  fill(locator: Locator, value: string): Promise<void>;
  /** Real key-by-key typing (`pressSequentially`) — one interaction, like `fill`, but through the keyboard (a keyboard-only journey never uses `fill`). */
  type(locator: Locator, value: string): Promise<void>;
  press(target: Locator | Page, key: string): Promise<void>;
  selectOption(locator: Locator, value: string): Promise<void>;
  /** Declares the screen now on-screen; counts a transition when it differs from the last marked screen. */
  markScreen(name: string): Promise<void>;
  /** Declares a dialog/modal now active; ignores a dialog already open at the prior marker. */
  markModal(name: string): Promise<void>;
  /** Declares the context key expected to survive; `null` is a deliberate reset. */
  preserveContext(key: string | null): Promise<void>;
  /**
   * Waits for a feedback locator to become visible and attaches the elapsed
   * time to the Playwright result for review. Advisory only — never a
   * wall-clock hard gate (machine-load-sensitive milliseconds must not block
   * `make check`, ADR 0081).
   */
  expectFeedback(locator: Locator): Promise<void>;
  /** Asserts every hard metric (interactions, screenTransitions, modalOpens, contextLosses) against its budget. */
  assertBudget(): Promise<void>;
};

// A Page has `keyboard`; a Locator does not — the discriminator for `press`.
function isPage(target: Locator | Page): target is Page {
  return "keyboard" in target;
}

// Describes a locator BEFORE the action runs, never after: an action (click,
// especially) can navigate away or unmount its own target, and describing a
// now-detached locator with an unbounded default-timeout `evaluate()` stalled
// the whole test budget waiting for an element that will never reappear
// (harvested during Q3 implementation). Every internal call here is bounded.
async function describeLocator(locator: Locator): Promise<string> {
  const text = await locator
    .first()
    .innerText({ timeout: 500 })
    .catch(() => "");
  if (text.trim()) return text.trim().slice(0, 80);
  return await locator
    .first()
    .evaluate((el) => (el as HTMLElement).tagName.toLowerCase(), { timeout: 500 })
    .catch(() => "locator");
}

export function journey(page: Page, id: string): Journey {
  const recorder = new JourneyMetricsRecorder();

  return {
    async click(locator) {
      const description = await describeLocator(locator);
      await locator.click();
      recorder.recordInteraction(`click ${description}`);
    },

    async clickPrimary(surface, action) {
      const description = await describeLocator(action);
      const [surfaceBox, actionBox] = await Promise.all([
        surface.boundingBox().catch(() => null),
        action.boundingBox().catch(() => null),
      ]);
      const visibleBeforeScroll =
        surfaceBox && actionBox
          ? actionBox.y >= surfaceBox.y && actionBox.y + actionBox.height <= surfaceBox.y + surfaceBox.height
          : null;
      await action.click();
      recorder.recordInteraction(`clickPrimary ${description}`);
      test.info().annotations.push({
        type: "journey-primary-action-scroll",
        description:
          `${id}: primary action visible inside its surface before Playwright auto-scroll = ` +
          `${visibleBeforeScroll === null ? "unknown (no bounding box)" : String(visibleBeforeScroll)} (advisory).`,
      });
    },

    async fill(locator, value) {
      const description = await describeLocator(locator);
      await locator.fill(value);
      recorder.recordInteraction(`fill ${description} = "${value}"`);
    },

    async type(locator, value) {
      const description = await describeLocator(locator);
      await locator.pressSequentially(value);
      recorder.recordInteraction(`type ${description} = "${value}"`);
    },

    async press(target, key) {
      if (isPage(target)) await target.keyboard.press(key);
      else await target.press(key);
      recorder.recordInteraction(`press "${key}"`);
    },

    async selectOption(locator, value) {
      const description = await describeLocator(locator);
      await locator.selectOption(value);
      recorder.recordInteraction(`selectOption ${description} = "${value}"`);
    },

    async markScreen(name) {
      recorder.markScreen(name);
    },

    async markModal(name) {
      recorder.markModal(name);
    },

    async preserveContext(key) {
      recorder.preserveContext(key);
    },

    async expectFeedback(locator) {
      const start = Date.now();
      await expect(locator).toBeVisible();
      const elapsedMs = Date.now() - start;
      test.info().annotations.push({
        type: "journey-feedback",
        description: `${id}: feedback visible after ${elapsedMs}ms (advisory, not a hard gate).`,
      });
    },

    async assertBudget() {
      const viewport = JSON.stringify(page.viewportSize());
      const project = test.info().project.name;
      const metrics = recorder.snapshot();
      const trace = recorder.getTrace();

      const failures = evaluateJourneyBudget({
        file: budgetsFile,
        journeyId: id,
        project,
        viewport,
        metrics,
        trace,
      });
      expect(failures, failures.join("\n\n")).toEqual([]);

      const entry = budgetsFile.journeys[id];
      for (const metric of JOURNEY_METRIC_NAMES) {
        const limit = entry.byProject?.[project]?.[metric] ?? entry[metric];
        const suggestion = tightenSuggestion(id, metric, metrics[metric], limit, project, viewport);
        // eslint-disable-next-line no-console
        if (suggestion) console.log(suggestion);
      }
    },
  };
}
