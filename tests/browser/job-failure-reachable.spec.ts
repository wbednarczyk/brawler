import { test, expect, openApp } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import {
  JOB_FAILED_SUBJECT,
  JOB_FAILED_SYSTEM_ERROR,
} from "../../src/test/scenarios/overlays";
import { SAMPLE_NOW } from "../../src/test/scenarios/entities";

// Epic #40 S3 (ADR 0091 dec. 1) — reachability: "a capability is not done until a
// user can reach it". A background job that exhausts its retries now raises a
// system `job_failed` attention event; this walks the real UI to prove the failure
// arrives in the Today day queue and is NAMED there — WHICH task died, and what it
// died on — in both scopes the backend can emit (company-scoped and
// workspace-wide). Before this slice five job kinds could fail with no surface at
// all, which is precisely what a browser spec, not a unit test, has to catch.
//
// Retargeted to Dziś v2 (F2 #422): root-fed attention events still land as
// `rows2/AttentionRow`s in the day queue (`dayQueueModel.ts` merges them by
// local day) — only the DOM (`li[data-category]` → `.dayq-row`,
// `.today-row-title` → `.dayq-row-title`) moved.

test.describe("job failure reaches Today", { tag: "@clickable" }, () => {
  test("a terminally failed job is visible and named in the day queue", async ({ page }) => {
    // The overlay fires both events AT `SAMPLE_NOW` — freeze the page clock
    // so `dayQueueModel`'s local-day bucketing agrees (j1-morning-review.spec.ts
    // pattern).
    await page.clock.setFixedTime(new Date(SAMPLE_NOW));
    await primeMockScenario(page, { base: "rich", overlays: ["job-failed-event"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const todaySection = page.locator(".dayq-section").first();

    // 1. The company-scoped failure names the TASK and the report it died on
    //    (`attentionEventTitleText`: "<task> failed — <subject>"), under the
    //    company's own ticker.
    const scopedRow = todaySection.locator(".dayq-row").filter({ hasText: JOB_FAILED_SUBJECT });
    await expect(scopedRow).toHaveCount(1);
    await expect(scopedRow.locator(".dayq-row-title")).toHaveText(
      `Shareholder extraction failed — ${JOB_FAILED_SUBJECT}`,
    );
    await expect(scopedRow.getByText("ZZZJ", { exact: false })).toBeVisible();

    // 2. The workspace-wide failure (no company at all) still renders, stating the
    //    job's own error text — it is not silently dropped for lacking a ticker.
    const systemRow = todaySection.locator(".dayq-row").filter({ hasText: JOB_FAILED_SYSTEM_ERROR });
    await expect(systemRow).toHaveCount(1);
    await expect(systemRow.locator(".dayq-row-title")).toHaveText(
      `Fundamentals pull failed — ${JOB_FAILED_SYSTEM_ERROR}`,
    );

    // 3. Both carry the background-task badge — the row states its category as a
    //    task failure, never as a generic alert.
    await expect(scopedRow.getByText("Background task", { exact: true })).toBeVisible();
    await expect(systemRow.getByText("Background task", { exact: true })).toBeVisible();
  });
});
