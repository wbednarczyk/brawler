import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// F3a S3 (ADR 0107 decision 5) / ADR 0108: the legacy `dashboard:company_gpw_cdr`
// layout (which used to render the Report documents panel alongside
// Fundamentals) is retired along with the docking engine. Its overflow risk —
// long, unbreakable report reference labels (periodLabel/title/documentId
// fall-through, ReportDiffPanel `refLabel`) — now lives in the Spółka `diff`
// workshop tool ("Report diff"), which reuses the same
// `.company-report-documents` grid columns (pinned `minmax(0,1fr)`) so long
// rows truncate instead of forcing a horizontal scrollbar. This guards that
// no-horizontal-scroll contract at a narrow window (the dual-execution mock
// runtime serves the data).
test("Report diff tool does not horizontally overflow at a narrow window", async ({ page }) => {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page
    .getByLabel(/Primary navigation|Nawigacja główna/)
    .getByRole("button", { name: "Companies" })
    .click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();

  const spolka = page.getByRole("region", { name: "Company view", exact: true });
  await expect(spolka).toBeVisible();
  await spolka.getByLabel("Workshop").getByRole("button", { name: "Report diff", exact: true }).click();

  const tool = spolka.getByLabel("Workshop tool");
  await expect(tool).toBeVisible();
  await expect(tool).toHaveAttribute("data-tool", "diff");
  await expect(tool.getByRole("group", { name: "Report comparison" })).toBeVisible();

  // No global horizontal scrollbar at this narrow width — the meaningful,
  // low-noise invariant (matches the smoke-walk overflow gate).
  await expectNoPageOverflow(page);
});
