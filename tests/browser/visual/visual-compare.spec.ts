import { test, expect, openApp } from "../helpers/harness";
import { shootScreen } from "./helpers";
import type { Page } from "@playwright/test";

// Visual baseline — Compare (ADR 0089 §A3 + §A7, ADR 0076 D7 / U11). A named
// full-screen destination hosted in `.workspace`: M at the project viewport +
// forced S/L (dark), M-only (light). The browser harness seeds a populated
// Profil comparison for GPW:CDR (PLN) + GPW:CBF (EUR); Profil is the default
// view, so selecting the pair computes reactively (no submit) and the all-KPI
// pivot renders.
function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("visual — Compare", () => {
  test("Compare Profil pivot across tiers", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Compare" }).click();
    await page.getByLabel(/Add company/).selectOption({ value: "company_gpw_cdr" });
    await page.getByLabel(/Add company/).selectOption({ value: "company_gpw_cbf" });
    await expect(page.getByRole("table")).toBeVisible();

    await shootScreen(page, "compare", { forced: ["S", "L"] });
  });
});
