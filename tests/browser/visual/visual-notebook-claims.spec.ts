import { test, expect, openApp, openCockpitPanel } from "../helpers/harness";
import { shootPanel } from "./helpers";
import type { Locator, Page } from "@playwright/test";

// Visual baseline — Notebook (company + global) + Claims panels (ADR 0076 D7 /
// U11), mirroring density-notebook-claims.spec.ts.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openCompanyDashboardPanel(page: Page, tab: string, rootSelector: string): Promise<Locator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.getByRole("button", { name: "Open GPW:CDR dashboard" }).click();
  const cockpit = page.getByLabel("Research cockpit");
  await expect(cockpit).toBeVisible();
  await cockpit.getByRole("button", { name: tab, exact: true }).first().click();
  const pane = page.locator(".cockpit-pane", { has: page.locator(rootSelector) });
  await expect(pane).toBeVisible();
  return pane;
}

test.describe("visual — notebook + claims", () => {
  test("Notebook (company) across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyDashboardPanel(page, "Notebook", ".notebook-workspace");
    await expect(pane.locator(".notebook-list")).toBeVisible();
    await shootPanel(page, pane, "notebook-company");
  });

  test("Notebooks (global screen) across pane tiers", async ({ page }) => {
    await openApp(page);
    await openCockpitPanel(page, "Notebook");
    const pane = page.locator(".cockpit-pane", { has: page.locator(".notebooks-screen") });
    await expect(pane).toBeVisible();
    // Land on a company with notes so the list is populated for every tier.
    await pane.getByRole("button", { name: "Open notebook company: GPW:CDR" }).click();
    await expect(pane.locator(".notebooks-notes-list .notebook-row").first()).toBeVisible();
    await shootPanel(page, pane, "notebooks-global");
  });

  test("Claims across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyDashboardPanel(page, "Claims", ".company-claims-panel");
    // The claims body folds behind the queue summary at the default short pane;
    // shootPanel resizes to a tall S/M/L pane first, so assert the panel root here
    // (the list visibility is a per-tier concern the density spec already covers).
    await expect(pane.locator(".company-claims-panel")).toBeVisible();
    await shootPanel(page, pane, "claims");
  });
});
