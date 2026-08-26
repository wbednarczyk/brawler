import { test, expect, openApp } from "../helpers/harness";
import { shootPanel, shootRegion } from "./helpers";
import type { Locator, Page } from "@playwright/test";

// Visual baseline — Quality + Report-documents panels (ADR 0076 D7 / U11),
// mirroring density-quality-docs.spec.ts.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openDashboard(page: Page) {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  // F3a S1 (ADR 0107): the row opens Spółka; the legacy cockpit is reached via the sidebar Dashboard entry until S3 freezes it.
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Dashboard" }).click();
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

  // The pane shot above is clipped at the tier height, so the actions footer
  // sat outside every baseline: epic #398 added a third action there and not
  // one baseline pixel moved. Shot as its own region so the actions — the most
  // clicked part of the panel — actually have visual coverage.
  test("Coverage actions footer", async ({ page }) => {
    await openApp(page);
    const pane = await openCoverage(page);
    const actions = pane.locator(".coverage-actions");
    await expect(actions).toBeVisible();
    await shootRegion(page, pane, actions, "coverage-actions");
  });

  // Same blind spot, second instance (ADR 0045 guardrail harvest): the
  // unnamed-positions list is below the fold AND behind a disclosure, so it
  // was in no baseline either — and it shipped with the position name clipped
  // to one character at this width. A region shot catches a layout
  // regression LOCALLY — honest scope: CI ignores screenshot comparisons
  // (playwright.config.ts `ignoreSnapshots` on CI), so this reddens on the
  // developer's machine and in `make ui-smoke`, not in CI (sol review
  // finding 11).
  test("Unnamed positions list", async ({ page }) => {
    await openApp(page);
    const pane = await openCoverage(page);
    const capture = pane.locator(".coverage-raw-capture");
    await capture.getByRole("button", { name: /Show the unnamed positions/ }).click();
    const list = capture.locator(".coverage-uncrosswalked-concepts");
    await expect(list).toBeVisible();
    await shootRegion(page, pane, list, "coverage-unnamed-positions");
  });
});
