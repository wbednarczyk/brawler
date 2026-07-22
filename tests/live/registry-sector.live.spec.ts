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

  // Persistent attention toasts can stack over the bottom controls and INTERCEPT
  // clicks (live evidence 2026-07-21: a VRC insider toast blocked "Odśwież katalog
  // spółek" for 3 minutes). Dismissing them is a legitimate user action; the
  // stacking/overlap design fix itself is v0.60 scope (card abd456e).
  // The persistent-toast stack covers the bottom-left controls and INTERCEPTS
  // clicks — a tracked functional defect (card abd456e, v0.60; live evidence
  // 2026-07-21). `force: true` does NOT help: it skips actionability but the
  // topmost element (the toast) still receives the event — verified live (the
  // refresh command never dispatched). For THIS spec's purpose (the directory
  // refresh flow) hide the toast viewport for the duration; the overlap defect
  // is asserted nowhere here and stays owned by its card.
  await page
    .locator(".ui-toast-viewport")
    .evaluateAll((els) => els.forEach((el) => ((el as HTMLElement).style.display = "none")))
    .catch(() => {});

  // Open the GPW directory source's detail panel — the header is a TOGGLE, and a
  // previous run (or the user) may have left the panel open; clicking blindly
  // would CLOSE it and orphan the refresh button (observed live 2026-07-21).
  const refreshButton = page.getByRole("button", {
    name: /Odśwież katalog spółek|Refresh company directory/,
  });
  if (!(await refreshButton.isVisible().catch(() => false))) {
    await page
      .getByRole("button", { name: /(Otwórz źródło|Open source): GPW Company Directory/ })
      .click();
  }
  await expect(refreshButton).toBeVisible({ timeout: 10_000 });

  // Trigger the directory refresh (covers GPW + NewConnect in one action).
  await refreshButton.click();

  const failure = page.getByText(/katalogu spółek nie powiodło się|Company directory refresh failed/);

  // Wait for the upsert summary ("N/M zapisanych wpisów") to appear.
  await expect(page.getByText(/zapisanych wpisów|saved entries/)).toBeVisible({
    timeout: 120_000,
  });
  await expect(failure).toHaveCount(0);

  await page.screenshot({ path: "test-results/live/registry-sector.png", fullPage: true });
});
