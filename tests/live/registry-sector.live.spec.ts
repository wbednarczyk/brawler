import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// Live verification of v0.53 T3 sector ingestion: the registry parser ships
// in the build, but sectors only land when a directory refresh actually runs
// against the live GPW/NewConnect pages. This drives the REAL app: open the
// GPW Company Directory source, trigger a manual directory refresh, expect a
// successful upsert result and no failure banner. The DB-side sector coverage
// is asserted separately (read-only sqlite) by the driver.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("manual company-directory refresh completes and reports saved entries", async () => {
  const { page } = connection;
  test.setTimeout(180_000);

  await page.getByRole("button", { name: /Źródła|Sources/ }).first().click();
  await expect(page.getByRole("heading", { name: /Źródła|Sources/ })).toBeVisible();

  // Open the GPW directory source's detail panel.
  await page
    .getByRole("button", { name: /(Otwórz źródło|Open source): GPW Company Directory/ })
    .click();

  // Trigger the directory refresh (covers GPW + NewConnect in one action).
  await page
    .getByRole("button", { name: /Odśwież katalog spółek|Refresh company directory/ })
    .click();

  const failure = page.getByText(/katalogu spółek nie powiodło się|Company directory refresh failed/);

  // Wait for the upsert summary ("N/M zapisanych wpisów") to appear.
  await expect(page.getByText(/zapisanych wpisów|saved entries/)).toBeVisible({
    timeout: 120_000,
  });
  await expect(failure).toHaveCount(0);

  await page.screenshot({ path: "test-results/live/registry-sector.png", fullPage: true });
});
