import { test, expect, openApp } from "../helpers/harness";
import { shootPanel } from "./helpers";
import type { Locator, Page } from "@playwright/test";

// Visual baseline — company dashboard panels (ADR 0076 D7 / U11), cluster mirror
// of density-companies.spec.ts. Same navigation the density spec uses; each panel
// is snapshotted at S/M/L pane widths (M only under the light project).

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openCompanyDashboard(page: Page, tabName: string, panelLabel: string): Promise<Locator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  const cockpit = page.getByRole("region", { name: "Research cockpit" });
  await expect(cockpit).toBeVisible();
  await cockpit.getByRole("button", { name: tabName, exact: true }).first().click();
  const pane = page.locator(".cockpit-pane", { has: page.getByLabel(panelLabel) });
  await expect(pane).toBeVisible();
  return pane;
}

test.describe("visual — company dashboard panels", () => {
  test("Fundamentals across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyDashboard(page, "Fundamentals", "Company fundamentals");
    await expect(pane.getByLabel("Financial facts matrix")).toBeVisible();
    await shootPanel(page, pane, "fundamentals");
  });

  test("Feed (company) across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyDashboard(page, "Feed", "Company feed");
    // Select a feed item so the detail renders (split at L, stacked/overlay else).
    await pane.locator("[data-company-feed-row]").first().click();
    await expect(pane.locator(".company-feed-detail")).toBeVisible();
    await shootPanel(page, pane, "company-feed");
  });
});
