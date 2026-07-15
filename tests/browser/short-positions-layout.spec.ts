import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// The KNF short-selling panel (v0.55 T4b, ADR 0069 decision 3) renders variable
// content — long institutional holder names in a positions table plus a
// kind-phrased change history — inside a cockpit pane. Guard that it never forces
// a horizontal scrollbar at a narrow window (the quarter-ultrawide range, DoD §B)
// in BOTH its populated and (most common) empty states. Opening a company lands
// the cockpit dashboard with the view company set, so the palette-added
// `shortPositions` FOLLOW panel resolves to that company; the dual-execution mock
// runtime serves the seeded register (CD PROJEKT populated, ORLEN empty).

async function addShortPositionsPanel(page: import("@playwright/test").Page, companyId: string) {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page
    .getByLabel(/Primary navigation|Nawigacja główna/)
    .getByRole("button", { name: "Companies" })
    .click();
  await page.locator(`[data-company-id="${companyId}"] .company-row-main`).click();

  await page.getByRole("button", { name: "Add panel" }).click();
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open panel: Short selling (KNF)");
  await palette
    .getByRole("button", { name: "Open panel: Short selling (KNF)", exact: true })
    .first()
    .click();
}

test("short-selling panel does not overflow at a narrow window — populated", async ({ page }) => {
  await addShortPositionsPanel(page, "company_gpw_cdr");

  const panel = page.locator(".short-positions-panel");
  await expect(panel).toBeVisible();
  // Populated: the positions table (with its long holder names) is present and
  // lives inside its own bounded horizontal scroller.
  await expect(panel.locator(".short-positions-table")).toBeVisible();
  await expect(panel.locator(".short-positions-table-scroll")).toBeVisible();

  await expectNoPageOverflow(page);
});

test("short-selling panel does not overflow at a narrow window — empty", async ({ page }) => {
  await addShortPositionsPanel(page, "company_gpw_pkn");

  const panel = page.locator(".short-positions-panel");
  await expect(panel).toBeVisible();
  await expect(panel.locator(".short-positions-empty")).toBeVisible();

  await expectNoPageOverflow(page);
});
