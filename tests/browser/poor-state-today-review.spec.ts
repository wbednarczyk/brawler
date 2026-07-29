import { test, expect, openApp } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import { FAILED_RUN_REPORT_TITLE } from "../../src/test/scenarios/overlays";

// Epic #40 S2 (ADR 0091) — the morning review walked on POOR state. The night's
// autopilot run FAILED: the `failed-autopilot-run` overlay seeds the coherent
// triple a real failed run produces (run `status: "failed"` + a concrete
// `lastError` + its `notable` completion event), composed with the canonical J1
// morning-review scene.
//
// What must hold: the failure is NAMED in the stream — the rows state WHICH
// report failed and THAT it failed. A failed overnight run that reads like an
// ordinary quiet morning (or disappears entirely) is the exact defect class this
// epic exists to catch.

// The seeded report title, read from the overlay itself — never re-typed here.
const REPORT_TITLE = FAILED_RUN_REPORT_TITLE;

test.describe("poor state — Today morning review with a failed autopilot run", { tag: "@clickable" }, () => {
  test("the failed run is visible and named in the stream", async ({ page }) => {
    await primeMockScenario(page, {
      base: "rich",
      overlays: ["morning-review", "failed-autopilot-run"],
    });
    await openApp(page);

    const stream = page.getByLabel(/Attention stream|Strumień uwagi/).first();
    await expect(stream).toBeVisible();

    // 1. The completion event's statement names the report AND its outcome
    //    (`attentionEventTitleText`: "<report title> — Failed").
    const attentionRow = stream
      .locator('.today-stream-row[data-category="attention"]')
      .filter({ hasText: REPORT_TITLE });
    await expect(attentionRow).toHaveCount(1);
    await expect(attentionRow.locator(".today-row-title")).toHaveText(`${REPORT_TITLE} — Failed`);

    // 2. The run's own autopilot row names the same report and carries the
    //    explicit Failed chip — the failure is stated, never merely implied by a
    //    row that quietly went missing.
    const runRow = stream
      .locator('.today-stream-row[data-category="autopilot"]')
      .filter({ hasText: REPORT_TITLE });
    await expect(runRow).toHaveCount(1);
    await expect(runRow.getByText("Failed", { exact: true })).toBeVisible();
  });
});
