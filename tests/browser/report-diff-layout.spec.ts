import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// Opening a company lands the cockpit dashboard (ADR 0057), which renders the
// Report documents panel (long, unbreakable ESPI filenames) alongside
// Fundamentals by default. Those grid columns must be pinned (minmax(0,1fr)) so
// the long rows truncate instead of forcing a horizontal scrollbar. This guards
// that no-horizontal-scroll contract at a narrow window (the dual-execution mock
// runtime serves the data).
test("company cockpit dashboard does not horizontally overflow at a narrow window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  // F3a S1 (ADR 0107): the row opens Spółka; the legacy cockpit is reached via the sidebar Dashboard entry until S3 freezes it.
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Dashboard" }).click();

  // The default dashboard panels are present: Fundamentals + Report documents
  // (whose long ESPI filenames are the overflow risk this test guards).
  await expect(page.getByLabel("Company fundamentals")).toBeVisible();
  await page.getByText("Report documents").first().waitFor();

  // No global horizontal scrollbar at this narrow width — the meaningful,
  // low-noise invariant (matches the smoke-walk overflow gate). The classic
  // `.company-workspace`/`.company-list` containers were removed with the
  // tab-based workspace; the cockpit is the deep-dive surface now.
  await expectNoPageOverflow(page);
});
