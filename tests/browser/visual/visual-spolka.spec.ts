import { test, expect, openApp } from "../helpers/harness";
import { shootScreen } from "./helpers";
import type { Page } from "@playwright/test";

// Visual baseline — the Spółka screen itself (F3a S1–S3, ADR 0107; ADR 0076
// D7/U11). Same navigation visual-companies.spec.ts uses for the company's
// workshop-tool panels; this spec shoots the SCREEN — glance bar + core at
// rest, and with a tool occupying the core zone — hosted in `.workspace` like
// the other full-screen baselines (today, cockpit-shell), so `shootScreen`
// (not `shootPanel`) is the right helper. M tier only (dark + light), per the
// screen-baseline convention for `.workspace`-hosted screens.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openSpolka(page: Page): Promise<void> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await expect(page.getByRole("region", { name: "Company view", exact: true })).toBeVisible();
}

test.describe("visual — Spółka screen", () => {
  test("at rest: glance bar + co-visible core", async ({ page }) => {
    await openApp(page);
    await openSpolka(page);
    const spolka = page.getByRole("region", { name: "Company view", exact: true });
    await expect(spolka.getByLabel("Company glance bar")).toBeVisible();
    await expect(spolka.getByLabel("Company core")).toBeVisible();
    await shootScreen(page, "spolka-rest");
  });

  test("with the claims tool open", async ({ page }) => {
    await openApp(page);
    await openSpolka(page);
    const spolka = page.getByRole("region", { name: "Company view", exact: true });
    await spolka.getByRole("group", { name: "Workshop" }).getByRole("button", { name: "Open claims" }).click();
    await expect(spolka.getByLabel("Workshop tool")).toBeVisible();
    await shootScreen(page, "spolka-tool-claims");
  });
});
