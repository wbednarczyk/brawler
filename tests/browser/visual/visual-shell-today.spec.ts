import { test, expect, openApp } from "../helpers/harness";
import { shootScreen } from "./helpers";
import type { Page } from "@playwright/test";

// Visual baseline — Today home + the cockpit shell (ADR 0076 D7 / U11). Neither
// forces `.workspace` in its density/shell spec, so each is the M-equivalent
// only: the workspace at the project viewport (light shoots the same single M).

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("visual — shell + today", () => {
  test("Today home", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    // Dziś v2 readiness anchor (F2): the delta header + the first day section
    // render only after get_today_view resolves — the strip/stream is gone.
    await expect(page.locator(".dayq-delta-header")).toBeVisible();
    await expect(page.locator(".dayq-section").first()).toBeVisible();
    await shootScreen(page, "today");
  });

  test("Cockpit shell (company dashboard)", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Companies" }).click();
    await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
    await expect(page.getByRole("region", { name: "Research cockpit" })).toBeVisible();
    await expect(page.getByLabel("Company fundamentals")).toBeVisible();
    await shootScreen(page, "cockpit-shell");
  });
});
