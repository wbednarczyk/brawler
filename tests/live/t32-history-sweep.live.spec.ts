import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { clearCompaniesFilter } from "./helpers/companiesList";

// T3.2 live-drive evidence (ADR 0077 §3): on the owner's real Windows app, run
// a manual history sweep on CBF from the Coverage panel and watch it complete.
// Deterministic-only by design (F3 gate), so the cost is CPU, not AI calls.
// Mutating by intent: sweep runs extract facts into the real database — that is
// the completion evidence the epic requires (CLAUDE.md: mocks are never
// completion evidence).

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("manual history sweep on CBF completes and reports runs", async () => {
  test.setTimeout(420_000);
  const { page } = connection;

  // Open the CBF cockpit.
  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await expect(nav).toBeVisible();
  await nav.getByRole("button", { name: /Companies|Spółki/ }).click();
  await clearCompaniesFilter(page);
  await page.locator('[data-company-id="company_gpw_cbf"] .company-row-main').click();
  await expect(page.getByLabel(/Research cockpit|Kokpit badawczy/)).toBeVisible();

  // Bring up the Coverage pane.
  await page.getByRole("button", { name: /^(Coverage|Pokrycie)$/ }).first().click();
  const pane = page.locator(".cockpit-pane", {
    has: page.locator(".company-coverage"),
  });
  await expect(pane).toBeVisible();
  await page.screenshot({ path: "test-results/live/t32-coverage-before.png", fullPage: true });

  // Kick the manual sweep.
  const sweepButton = pane.getByRole("button", {
    name: /Extract missing periods|Wydobądź brakujące okresy|Extracting…|Wydobywanie…/,
  });
  await expect(sweepButton).toBeEnabled();
  await sweepButton.click();

  // The status line must reach a completed sweep; runs then drain in the job
  // queue. Poll the panel status until it reports a completed sweep.
  const status = pane.locator(".coverage-action-status");
  await expect(status).not.toHaveText("", { timeout: 30_000 });
  console.log("status after click:", await status.textContent());
  await expect(status).toHaveText(/Extracted \d+|Wydobyto \d+|failed|niepowodzenie/i, {
    timeout: 360_000,
  });
  await expect(status).toHaveText(/Extracted \d+|Wydobyto \d+/, { timeout: 1_000 });
  const finalStatus = await status.textContent();
  console.log("final sweep status:", finalStatus);

  await page.screenshot({ path: "test-results/live/t32-coverage-after.png", fullPage: true });
});
