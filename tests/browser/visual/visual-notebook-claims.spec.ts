import { test, expect, openApp } from "../helpers/harness";
import { shootPanel } from "./helpers";
import type { Locator, Page } from "@playwright/test";

// Visual baseline — Notebook (company + global) + Claims panels (ADR 0076 D7 /
// U11), mirroring density-notebook-claims.spec.ts.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

// F3a S2/S3 (ADR 0107): company panels live in the Spółka screen's workshop
// tools now — `toolButton` is the WorkshopBar's "Open <tool>" label, and
// `.spolka-layout` is the tool's `pane` size container (spolka.css).
async function openCompanyTool(page: Page, toolButton: string, rootSelector: string): Promise<Locator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.getByRole("button", { name: "Open GPW:CDR" }).click();
  await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
  await page.getByRole("button", { name: toolButton, exact: true }).click();
  const pane = page.locator(".spolka-layout");
  await expect(pane.locator(rootSelector)).toBeVisible();
  return pane;
}

test.describe("visual — notebook + claims", () => {
  test("Notebook (company) across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyTool(page, "Notebook", ".notebook-workspace");
    await expect(pane.locator(".notebook-list")).toBeVisible();
    await shootPanel(page, pane, "notebook-company");
  });

  test("Claims across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyTool(page, "Claims", ".company-claims-panel");
    // The claims body folds behind the queue summary at the default short pane;
    // shootPanel resizes to a tall S/M/L pane first, so assert the panel root here
    // (the list visibility is a per-tier concern the density spec already covers).
    await expect(pane.locator(".company-claims-panel")).toBeVisible();
    await shootPanel(page, pane, "claims");
  });
});
