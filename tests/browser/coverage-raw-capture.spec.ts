import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// The Coverage panel's raw tagged-fact capture proof + promotion (ADR 0100,
// epic #398 final slice): the funnel compacted to one InfoGrid line, with a
// disclosure into the "positions the program doesn't know yet" list and its
// "Show in Fundamentals" promote action.
//
// Guarded here (browser, not jsdom): the compact line and the expanded list
// both render inside the real Spółka Coverage workshop tool without forcing a
// horizontal scrollbar at the narrow quarter-ultrawide window (DoD §B), and
// the promote action updates the row in place. The dual-execution mock
// runtime serves a fixed sample for CD PROJEKT only (Layer 1 has no seeding
// command in the real backend either).

// F3a S3 (ADR 0107): opening a company lands the Spółka screen; Coverage is
// the `pokrycie` workshop tool, opened via the ⌘K palette's "Open coverage".
async function openCoveragePanel(page: import("@playwright/test").Page) {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page
    .getByLabel(/Primary navigation|Nawigacja główna/)
    .getByRole("button", { name: "Companies" })
    .click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open coverage");
  await palette.getByRole("option", { name: "Open coverage", exact: true }).first().click();
}

test("the raw capture line renders the counts and does not overflow", async ({ page }) => {
  await openCoveragePanel(page);

  const section = page.locator(
    '.company-coverage[data-company-id="company_gpw_cdr"] .coverage-raw-capture',
  );
  await expect(section).toBeVisible();
  await expect(section.getByText("426")).toBeVisible();
  await expect(section.getByText("68")).toBeVisible();

  await expectNoPageOverflow(page);
});

test("expanding the unnamed-positions list shows rows and does not overflow", async ({
  page,
}) => {
  await openCoveragePanel(page);

  const section = page.locator(
    '.company-coverage[data-company-id="company_gpw_cdr"] .coverage-raw-capture',
  );
  await section.getByRole("button", { name: /Show the unnamed positions/ }).click();

  const list = section.locator(".coverage-uncrosswalked-concepts");
  await expect(list).toBeVisible();
  await expect(list.getByText("SomeStandardConceptNotYetCurated", { exact: true })).toBeVisible();
  await expect(list.getByText("Pozostałe usługi obce", { exact: true })).toBeVisible();
  // The standard concept with no curated name states plainly that it has no
  // translation — never a synthesized Polish name.
  await expect(list.getByText("no translation yet")).toBeVisible();

  await expectNoPageOverflow(page);
});

test("promoting a position updates the row in place", async ({ page }) => {
  await openCoveragePanel(page);

  const section = page.locator(
    '.company-coverage[data-company-id="company_gpw_cdr"] .coverage-raw-capture',
  );
  await section.getByRole("button", { name: /Show the unnamed positions/ }).click();

  const list = section.locator(".coverage-uncrosswalked-concepts");
  const row = list.locator("li", { hasText: "Pozostałe usługi obce" });
  await row.getByRole("button", { name: /Show in Fundamentals/ }).click();

  await expect(row.getByText("In Fundamentals")).toBeVisible();
  await expect(row.getByRole("button", { name: /Show in Fundamentals/ })).toHaveCount(0);

  await expectNoPageOverflow(page);
});
