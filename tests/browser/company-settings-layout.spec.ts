import { test, expect, openApp, expectNoHorizontalOverflow, expectNoPageOverflow } from "./helpers/harness";

// CompanySettingsManager (ADR 0056) is the master-detail per-company settings
// surface reachable via "Manage settings" on the Companies screen. No
// Playwright spec exercised settings mode before this (issue 05a8c610):
// `.company-settings` only collapsed to one column at `max-width: 1100px`,
// but the supported narrow window is 1280px wide (chromium-quarter-uw in
// playwright.config.ts), so the two-column grid could render cramped/
// overflowing at that width instead of stacking.
test("company settings master-detail stacks and stays overflow-free at narrow windows", async ({
  page,
}) => {
  await openApp(page);
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
  await page.getByRole("button", { name: "Manage settings" }).click();

  const panel = page.getByLabel("Company settings");
  const master = panel.locator(".company-settings-master");
  const detail = panel.locator(".company-settings-detail");
  const list = panel.locator(".company-settings-list");

  await expect(panel).toBeVisible();
  await expect(list).toBeVisible();

  // No horizontal overflow on the panel itself or its inner scroll container
  // (the company list scrolls its own overflow — a document-level check alone
  // would miss a blowout there, per the harvested no-horizontal-scroll rule).
  await expectNoHorizontalOverflow(panel);
  await expectNoHorizontalOverflow(list);
  await expectNoPageOverflow(page);

  const viewport = page.viewportSize();
  const supportedNarrow = viewport !== null && viewport.width <= 1280;

  const masterBox = await master.boundingBox();
  const detailBox = await detail.boundingBox();
  expect(masterBox).not.toBeNull();
  expect(detailBox).not.toBeNull();

  if (supportedNarrow) {
    // Single column at the supported narrow width: the detail column stacks
    // below the master column (same left edge) rather than sitting beside it.
    expect(Math.abs(detailBox!.x - masterBox!.x)).toBeLessThanOrEqual(1);
    expect(detailBox!.y).toBeGreaterThanOrEqual(masterBox!.y + masterBox!.height - 1);
  } else {
    // Two columns at the wider desktop viewports: master and detail sit side
    // by side (detail starts at or after master's right edge).
    expect(detailBox!.x).toBeGreaterThanOrEqual(masterBox!.x + masterBox!.width - 1);
  }
});
