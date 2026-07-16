import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Ownership section (Basic Info panel, v0.56 T6, ADR 0072) must stay usable in a
// tall, narrow pane: no horizontal page overflow and the section container never
// scrolls sideways (donut stacks over the legend, holder rows wrap, the type
// chip stays with the name). Runs across the viewport matrix; the tightest
// signal comes from the `chromium-quarter-uw` (1280×1440) and
// `chromium-quarter-uw-125` (1024×1152) projects — a quarter of a 49" ultrawide.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openBasicInfo(page: Page) {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  const cockpit = page.getByRole("region", { name: "Research cockpit" });
  await expect(cockpit).toBeVisible();
  await cockpit.getByRole("button", { name: "Basic info", exact: true }).first().click();
  const pane = page.locator(".cockpit-pane", { has: page.getByLabel("Basic info") });
  await expect(pane).toBeVisible();
  return pane;
}

test("ownership section does not overflow horizontally in a narrow pane", async ({ page }) => {
  await openApp(page);
  const pane = await openBasicInfo(page);

  // The populated ownership section renders (rich scenario seeds CDR).
  const section = pane.locator(".ownership-section");
  await expect(section).toBeVisible();
  await expect(section.getByRole("img", { name: "Ownership structure by holder type" })).toBeVisible();

  // No document-level or panel-internal horizontal scrollbar anywhere.
  await expectNoPageOverflow(page);

  // The section container itself never scrolls sideways.
  const overflow = await section.evaluate((el) => ({
    scrollWidth: el.scrollWidth,
    clientWidth: el.clientWidth,
  }));
  expect(
    overflow.scrollWidth,
    `ownership section overflows horizontally (${overflow.scrollWidth} > ${overflow.clientWidth})`,
  ).toBeLessThanOrEqual(overflow.clientWidth + 1);
});
