import { test, expect, openApp } from "./helpers/harness";

// Company workspace renders inline inside the scrollable `.company-list`, and its
// panels (report documents, the report-over-report diff — ADR 0052) hold long,
// unbreakable ESPI filenames + nowrap section headings. The grid columns down that
// chain must be pinned (minmax(0,1fr)) so those rows truncate instead of forcing a
// horizontal scrollbar on `.company-list`. This guards that no-horizontal-scroll
// contract at a narrow window (the dual-execution mock runtime serves the data).
test("company workspace does not horizontally overflow at a narrow window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("button", { name: "Fundamentals", exact: true }).click();

  // Report documents (long ESPI filenames) + the report-diff panel both render.
  await page.getByText("Report documents").first().waitFor();
  await page.getByText("Report comparison").first().waitFor();
  // Scope to the report-diff panel: the sidebar now has a "Compare" mode button
  // too (ADR 0054), so an unscoped name match would hit the nav, not the panel.
  await page.locator(".report-diff-panel").getByRole("button", { name: "Compare" }).first().click();
  await page.getByText(/changed section/).first().waitFor();

  const overflow = await page.evaluate(() => {
    const measure = (selector: string) => {
      const el = document.querySelector(selector) as HTMLElement | null;
      return el ? el.scrollWidth - el.clientWidth : -1;
    };
    return {
      doc: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      companyList: measure(".company-list"), // the scrollable container that showed the bug
      workspace: measure(".company-workspace"),
      reportDocs: measure(".company-report-documents"),
      reportDiff: measure(".report-diff-panel"),
    };
  });

  // No global scrollbar and — critically — no horizontal scroll on the container or panels.
  expect(overflow.doc).toBeLessThanOrEqual(1);
  expect(overflow.companyList).toBeLessThanOrEqual(1);
  expect(overflow.workspace).toBeLessThanOrEqual(1);
  expect(overflow.reportDocs).toBeLessThanOrEqual(1);
  expect(overflow.reportDiff).toBeLessThanOrEqual(1);
});
