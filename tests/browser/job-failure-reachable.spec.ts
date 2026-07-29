import { test, expect, openApp } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import {
  JOB_FAILED_SUBJECT,
  JOB_FAILED_SYSTEM_ERROR,
} from "../../src/test/scenarios/overlays";

// Epic #40 S3 (ADR 0091 dec. 1) — reachability: "a capability is not done until a
// user can reach it". A background job that exhausts its retries now raises a
// system `job_failed` attention event; this walks the real UI to prove the failure
// arrives in the Today stream and is NAMED there — WHICH task died, and what it
// died on — in both scopes the backend can emit (company-scoped and
// workspace-wide). Before this slice five job kinds could fail with no surface at
// all, which is precisely what a browser spec, not a unit test, has to catch.

test.describe("job failure reaches Today", { tag: "@clickable" }, () => {
  test("a terminally failed job is visible and named in the stream", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["job-failed-event"] });
    await openApp(page);

    const stream = page.getByLabel(/Attention stream|Strumień uwagi/).first();
    await expect(stream).toBeVisible();

    // 1. The company-scoped failure names the TASK and the report it died on
    //    (`attentionEventTitleText`: "<task> failed — <subject>"), under the
    //    company's own ticker.
    const scopedRow = stream
      .locator('.today-stream-row[data-category="attention"]')
      .filter({ hasText: JOB_FAILED_SUBJECT });
    await expect(scopedRow).toHaveCount(1);
    await expect(scopedRow.locator(".today-row-title")).toHaveText(
      `Shareholder extraction failed — ${JOB_FAILED_SUBJECT}`,
    );
    await expect(scopedRow.getByText("ZZZJ", { exact: false })).toBeVisible();

    // 2. The workspace-wide failure (no company at all) still renders, stating the
    //    job's own error text — it is not silently dropped for lacking a ticker.
    const systemRow = stream
      .locator('.today-stream-row[data-category="attention"]')
      .filter({ hasText: JOB_FAILED_SYSTEM_ERROR });
    await expect(systemRow).toHaveCount(1);
    await expect(systemRow.locator(".today-row-title")).toHaveText(
      `Fundamentals pull failed — ${JOB_FAILED_SYSTEM_ERROR}`,
    );

    // 3. Both carry the background-task badge — the row states its category as a
    //    task failure, never as a generic alert.
    await expect(scopedRow.getByText("Background task", { exact: true })).toBeVisible();
    await expect(systemRow.getByText("Background task", { exact: true })).toBeVisible();
  });
});
