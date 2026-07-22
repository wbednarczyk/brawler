import { test, expect, openApp } from "../helpers/harness";
import { shootPanel } from "./helpers";
import type { Locator, Page } from "@playwright/test";

// Visual baseline — Quality + Report-documents panels (ADR 0076 D7 / U11),
// mirroring density-quality-docs.spec.ts.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openDashboard(page: Page) {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await expect(page.getByLabel("Research cockpit")).toBeVisible();
}

async function openQuality(page: Page): Promise<Locator> {
  await openDashboard(page);
  await page.getByRole("button", { name: /Quality/ }).first().click();
  const pane = page.locator(".cockpit-pane", { has: page.locator(".quality-panel") });
  await expect(pane).toBeVisible();
  return pane;
}

async function openDocuments(page: Page): Promise<Locator> {
  await openDashboard(page);
  await page.getByRole("button", { name: "Report documents" }).first().click();
  const pane = page.locator(".cockpit-pane", {
    has: page.locator('.company-report-documents[aria-label="Report documents"]'),
  });
  await expect(pane).toBeVisible();
  return pane;
}

async function openCoverage(page: Page): Promise<Locator> {
  await openDashboard(page);
  await page.getByRole("button", { name: "Coverage" }).first().click();
  const pane = page.locator(".cockpit-pane", {
    has: page.locator('.company-coverage[aria-label="Coverage"]'),
  });
  await expect(pane).toBeVisible();
  return pane;
}

test.describe("visual — quality + report documents", () => {
  test("Quality across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openQuality(page);
    await expect(pane.locator(".quality-scorecard-summary")).toBeVisible();
    await shootPanel(page, pane, "quality");
  });

  test("Report documents across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openDocuments(page);
    await expect(pane.getByRole("link").first()).toBeVisible();
    await shootPanel(page, pane, "report-documents");
  });

  test("Coverage across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCoverage(page);
    await expect(pane.locator("table.coverage-table")).toBeVisible();
    await shootPanel(page, pane, "coverage");
  });
});
