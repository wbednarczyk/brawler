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

// F3a S3 (ADR 0107): opening a company lands the Spółka screen; the Basic
// info/ownership content is the `akcjonariat` workshop tool ("Otwórz
// akcjonariat" — plan "Mapowanie WSZYSTKICH intencji": Metadata→akcjonariat).
async function openBasicInfo(page: Page) {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open ownership");
  await palette.getByRole("option", { name: "Open ownership", exact: true }).first().click();
  const pane = page.getByRole("group", { name: "Workshop tool" });
  await expect(pane).toBeVisible();
  return pane;
}

// ADR 0072 decision 5: a threshold crossing is a marker on the trajectory. The
// component test pins which points get marked; this pins that they actually
// reach the rendered chart in a real browser, across the viewport matrix — the
// tick is small enough to slip under the visual baseline's pixel tolerance, so
// the screenshot alone would not notice it disappearing.
test("threshold crossings tick the stakes-over-time chart @clickable", async ({ page }) => {
  await openApp(page);
  const pane = await openBasicInfo(page);

  const chart = pane.locator(".ui-multi-line-chart");
  await expect(chart).toBeVisible();
  // The rich scenario seeds both founders with one ESPI-sourced point each.
  await expect(chart.locator("line.ui-multi-line-marker")).toHaveCount(2);
});

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

test("skin-in-the-game badge renders for corroborated founders without breaking layout", async ({
  page,
}) => {
  await openApp(page);
  const pane = await openBasicInfo(page);
  const section = pane.locator(".ownership-section");
  await expect(section).toBeVisible();

  // The seeded founders are corroborated by management holdings → a badge each
  // (one direct, one via a family-foundation vehicle).
  const badges = section.locator(".ownership-skin-badge");
  await expect(badges).toHaveCount(2);
  await expect(badges.first()).toHaveAttribute("title", /Jacek Duch/);
  await expect(badges.nth(1)).toHaveAttribute("title", /Dwernicki Fundacja Rodzinna/);

  // The badge sits in the trailing chip slot beside the type chip — the founder
  // row still shows its type chip, and the page never overflows sideways.
  await expect(section.locator(".ownership-type-chip").first()).toBeVisible();
  await expectNoPageOverflow(page);
});

test("insider block renders below the ownership section without breaking the narrow pane", async ({
  page,
}) => {
  await openApp(page);
  const pane = await openBasicInfo(page);

  // The "Insiderzy" block extends the Ownership area (v0.57 T6, ADR 0083 D7).
  const insider = pane.locator(".insider-section");
  await expect(insider).toBeVisible();
  await expect(insider.getByRole("heading", { name: "Insiders" })).toBeVisible();

  // The aggregate strip shows both rolling windows (seeded computed, ≥ 2 tx).
  await expect(insider.getByText("Last 90 days")).toBeVisible();
  await expect(insider.getByText("Last 12 months")).toBeVisible();

  // The timeline lists transactions with the role chip in its fixed slot.
  await expect(insider.locator(".insider-tx-row").first()).toBeVisible();
  await expect(insider.locator(".insider-role-slot").first()).toBeVisible();

  // No document-level overflow, and the section never scrolls sideways.
  await expectNoPageOverflow(page);
  const overflow = await insider.evaluate((el) => ({
    scrollWidth: el.scrollWidth,
    clientWidth: el.clientWidth,
  }));
  expect(
    overflow.scrollWidth,
    `insider section overflows horizontally (${overflow.scrollWidth} > ${overflow.clientWidth})`,
  ).toBeLessThanOrEqual(overflow.clientWidth + 1);
});

test("an unreadable shareholder table is flagged as an honest gap with no OCR action (ADR 0084 §4)", async ({
  page,
}) => {
  await openApp(page);
  const pane = await openBasicInfo(page);
  const section = pane.locator(".ownership-section");
  await expect(section).toBeVisible();

  // The tier-4 OCR pass is retired (ADR 0084 decision 4). A document the
  // deterministic parser cannot read is stated honestly as a flagged gap — never
  // guessed, never re-run — so the warnbox renders with a residual count and
  // carries no action to trigger extraction.
  const warnbox = section.locator(".ownership-warnbox");
  await expect(warnbox).toBeVisible();
  await expect(warnbox).toContainText(/could not be read|flagged/i);

  // Two seeded residuals → the count reports them (the retired path is gone).
  await expect(warnbox.locator(".ownership-warnbox-count")).toContainText("2");

  // No run affordance survives the retirement: no Read-with-OCR button and no
  // confirm-before-apply proposal card anywhere in the section.
  await expect(
    section.getByRole("button", { name: /Read with OCR|Odczytaj przez OCR/ }),
  ).toHaveCount(0);
  await expect(section.locator(".ownership-ocr-proposal")).toHaveCount(0);

  // No overflow in a narrow pane even with the warnbox present.
  await expectNoPageOverflow(page);
});
