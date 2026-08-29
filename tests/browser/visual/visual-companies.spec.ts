import { test, expect, openApp } from "../helpers/harness";
import { shootPanel } from "./helpers";
import type { Locator, Page } from "@playwright/test";

// Visual baseline — company dashboard panels (ADR 0076 D7 / U11), cluster mirror
// of density-companies.spec.ts. Same navigation the density spec uses; each panel
// is snapshotted at S/M/L pane widths (M only under the light project).

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

// F3a S3 (ADR 0107): opening a company lands the Spółka screen; each panel is
// now a workshop tool, opened via the ⌘K palette.
async function openCompanyTool(page: Page, toolLabel: string): Promise<Locator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill(toolLabel);
  await palette.getByRole("button", { name: toolLabel, exact: true }).first().click();
  // `.spolka-layout`, not the tool group itself, carries the density
  // contracts' `container: pane / size` (spolka.css).
  await expect(page.getByRole("group", { name: "Workshop tool" })).toBeVisible();
  return page.locator(".spolka-layout");
}

test.describe("visual — company dashboard panels", () => {
  test("Fundamentals across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyTool(page, "Open fundamentals");
    await expect(pane.getByLabel("Financial facts matrix")).toBeVisible();
    await shootPanel(page, pane, "fundamentals");
  });

  test("Basic info across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyTool(page, "Open ownership");
    await expect(pane.getByText("ISIN")).toBeVisible();
    await shootPanel(page, pane, "basic-info");
  });

  test("Feed (company) across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCompanyTool(page, "Open feed");
    // Select a feed item so the detail renders (split at L, stacked/overlay else).
    await pane.locator("[data-company-feed-row]").first().click();
    await expect(pane.locator(".company-feed-detail")).toBeVisible();
    await shootPanel(page, pane, "company-feed");
  });

  // F4a S2 — Companies library language pass (docs/plans/frontend-v2-f4a.md §
  // Companies library): the library screen itself (add form + toolbar + row
  // list), not a Spółka workshop tool — sized on `.workspace` like the other
  // sidebar screens (visual-utility.spec.ts's Watchlists/Transcripts baselines).
  test("Companies library across pane tiers", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Companies" }).click();
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await shootPanel(page, page.locator(".workspace"), "companies-library");
  });
});
