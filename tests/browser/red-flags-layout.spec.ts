import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// The red-flags cockpit panel (v0.57 T7, ADR 0083 Decision 8/9) surfaces derived
// warning signals — active flags with a fixed-slot severity chip and a per-row
// acknowledge, plus a collapsed acknowledged-history group and a calm explicit
// empty state. Guard that it renders in the default cockpit, that acknowledging a
// flag moves it out of the active list, and that it never forces a horizontal
// scrollbar at a narrow window (the quarter-ultrawide range, DoD §B) in both its
// populated and empty states. The dual-execution mock runtime serves the seeded
// red flags (CD PROJEKT populated, ORLEN empty).

// F3a S3 (ADR 0107): opening a company lands the Spółka screen; the
// warning-signals content is the `sygnaly` workshop tool ("Open signals").
async function addRedFlagsPanel(page: import("@playwright/test").Page, companyId: string) {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page
    .getByLabel(/Primary navigation|Nawigacja główna/)
    .getByRole("button", { name: "Companies" })
    .click();
  await page.locator(`[data-company-id="${companyId}"] .company-row-main`).click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open signals");
  await palette.getByRole("button", { name: "Open signals", exact: true }).first().click();
}

test("red-flags panel renders active flags and does not overflow — populated", async ({ page }) => {
  await addRedFlagsPanel(page, "company_gpw_cdr");

  const panel = page.locator(".red-flags-panel");
  await expect(panel).toBeVisible();
  // Active flags render with their type labels + severity chips.
  await expect(panel.getByText("Auditor red flag")).toBeVisible();
  await expect(panel.getByText("Fund exit", { exact: true })).toBeVisible();
  await expect(panel.locator(".red-flags-severity-slot").first()).toBeVisible();

  await expectNoPageOverflow(page);
});

test("acknowledging a flag moves it out of the active list", async ({ page }) => {
  await addRedFlagsPanel(page, "company_gpw_cdr");

  const panel = page.locator(".red-flags-panel");
  await expect(panel.getByText("Auditor red flag")).toBeVisible();
  const activeBefore = await panel.locator(".red-flags-list > .red-flags-row").count();

  // Acknowledge the first active flag via the inline confirm.
  await panel.getByRole("button", { name: "Acknowledge" }).first().click();
  await panel.getByRole("button", { name: "Yes" }).click();

  await expect
    .poll(async () => panel.locator(".red-flags-list > .red-flags-row").count())
    .toBe(activeBefore - 1);
});

test("red-flags panel shows the calm empty state and does not overflow — empty", async ({ page }) => {
  await addRedFlagsPanel(page, "company_gpw_pkn");

  const panel = page.locator(".red-flags-panel");
  await expect(panel).toBeVisible();
  await expect(panel.locator(".red-flags-empty")).toBeVisible();

  await expectNoPageOverflow(page);
});
