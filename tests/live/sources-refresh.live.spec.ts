import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { startSourcesRefresh } from "./helpers/sourcesRefresh";

// Live regression for the 2026-07-14 owner report "refresh does not work":
// a manual sources refresh raced concurrent writers and died with
// "sqlite error: database is locked" (DEFERRED write txns hitting instant
// SQLITE_BUSY on WAL snapshot upgrade — fixed with BEGIN IMMEDIATE), and the
// yahoo-eod pull never stamped `last_success_at` so the source read as
// never-refreshed. This drives the REAL app: click refresh-all on Sources,
// expect no failure banner and a stamped Yahoo refresh. Write-light: the
// refresh itself is a normal in-app action the scheduler performs anyway.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("manual sources refresh completes without a database-locked failure and stamps Yahoo", async () => {
  const { page } = connection;
  // Issue #164 calibration: in the FULL live sweep this network-bound test runs
  // after ~12 min of app hammering and the real Yahoo/KNF pull eats the 300s
  // budget (solo it needs ~3.5 min). 480s reflects the measured full-sweep
  // reality — a genuine hang still fails, just later.
  test.setTimeout(480_000);

  // Navigate to the Sources screen.
  await page.getByRole("button", { name: /Źródła|Sources/ }).first().click();
  await expect(page.getByRole("heading", { name: /Źródła|Sources/ })).toBeVisible();

  // Kick a manual refresh-all (the action that failed with database-locked),
  // or join one the owner's app already has in flight (issue #308).
  const mode = await startSourcesRefresh(page);
  console.log(`live sources sweep ${mode}`);

  // The refresh includes the Yahoo 50-company sweep — give it real time, and
  // watch that the failure banner never appears while it runs.
  const failureBanner = page.getByText(/nie powiodło się|Source refresh failed/);
  const lockedError = page.getByText(/database is locked/);
  await page.waitForTimeout(5_000);
  await expect(lockedError).toHaveCount(0);

  // Wait until the sweep settles (refresh button re-enabled / no spinner) —
  // budget matches the test timeout above minus the assertion tail (#164).
  await expect
    .poll(
      async () => {
        if ((await lockedError.count()) > 0) return "locked";
        const yahooRow = page.getByText("Yahoo Finance EOD Quotes").locator("..").locator("..");
        const text = await yahooRow.textContent();
        return text?.includes("Jeszcze nie odświeżono") || text?.includes("Never refreshed")
          ? "pending"
          : "stamped";
      },
      { timeout: 420_000, intervals: [5_000] }
    )
    .toBe("stamped");

  await expect(lockedError).toHaveCount(0);
  await expect(failureBanner).toHaveCount(0);

  await page.screenshot({ path: "test-results/live/sources-refresh.png", fullPage: true });
});
