import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// F3a S3 (ADR 0107 decision 5): the legacy `dashboard:company_gpw_cdr` layout
// (frozen, reachable via its "Legacy dashboard · CDR" Widoki row) renders the
// Report documents panel (long, unbreakable ESPI filenames) alongside
// Fundamentals by default. Those grid columns must be pinned (minmax(0,1fr)) so
// the long rows truncate instead of forcing a horizontal scrollbar. This guards
// that no-horizontal-scroll contract at a narrow window (the dual-execution mock
// runtime serves the data).
test("legacy dashboard does not horizontally overflow at a narrow window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page
    .getByLabel("Primary navigation")
    .getByRole("button", { name: "Legacy dashboard · CDR" })
    .click();

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
