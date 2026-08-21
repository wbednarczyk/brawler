import { test, expect, openApp } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import { FAILED_RUN_REPORT_TITLE } from "../../src/test/scenarios/overlays";
import { SAMPLE_NOW } from "../../src/test/scenarios/entities";

// Epic #40 S2 (ADR 0091) — the morning review walked on POOR state. The night's
// autopilot run FAILED: the `failed-autopilot-run` overlay seeds the coherent
// triple a real failed run produces (run `status: "failed"` + a concrete
// `lastError` + its `notable` completion event), composed with the canonical J1
// morning-review scene.
//
// What must hold: the failure is NAMED in the day queue — the rows state WHICH
// report failed and THAT it failed. A failed overnight run that reads like an
// ordinary quiet morning (or disappears entirely) is the exact defect class this
// epic exists to catch.
//
// Retargeted to Dziś v2 (F2 #422): the root-fed completion event (still the
// SOLE carrier of "what happened" — `attentionEventTitleText`) lands as a
// `rows2/AttentionRow` in the day queue alongside the run's own item row
// (`rows2/AutopilotRunRow`, from `get_today_view`'s `items[]`). Assertion 2 is
// narrowed from the old spec: `AutopilotRunRow` (F2 S3) carries no per-status
// chip of its own any more (its `StatusChip` is a flat "Autopilot" regardless
// of outcome) — the ONE stated failure narration is the attention row
// (assertion 1), which already satisfies "state WHICH + THAT it failed"; the
// run's own row is checked only for presence (not silently dropped), not for
// a redundant status chip that no longer exists.

// The seeded report title, read from the overlay itself — never re-typed here.
const REPORT_TITLE = FAILED_RUN_REPORT_TITLE;

test.describe("poor state — Today morning review with a failed autopilot run", { tag: "@clickable" }, () => {
  test("the failed run is visible and named in the day queue", async ({ page }) => {
    // Both overlays fire their events AT `SAMPLE_NOW` — freeze the page clock
    // so `dayQueueModel`'s local-day bucketing agrees (j1-morning-review.spec.ts
    // pattern).
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    await primeMockScenario(page, {
      base: "rich",
      overlays: ["morning-review", "failed-autopilot-run"],
    });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const todaySection = page.locator(".dayq-section").first();

    // 1. The completion event's statement names the report AND its outcome
    //    (`attentionEventTitleText`: "<report title> — Failed") — the row
    //    carrying the "Review" action (the run's OWN row carries "Read report",
    //    same report title, so the action name disambiguates the two).
    const attentionRow = todaySection
      .locator(".dayq-row")
      .filter({ hasText: REPORT_TITLE, has: page.getByRole("button", { name: "Review" }) });
    await expect(attentionRow).toHaveCount(1);
    await expect(attentionRow.locator(".dayq-row-title")).toHaveText(`${REPORT_TITLE} — Failed`);

    // 2. The run's own row (the unread run `get_today_view` still surfaces)
    //    names the same report — visible, never silently missing.
    const runRow = todaySection
      .locator(".dayq-row")
      .filter({ hasText: REPORT_TITLE, has: page.getByRole("button", { name: "Read report" }) });
    await expect(runRow).toHaveCount(1);
  });
});
