import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// The Spółka `diff` tool ("Report diff") renders long, unbreakable report
// reference labels (periodLabel/title/documentId fall-through, ReportDiffPanel
// `refLabel`) in `.company-report-documents` grid columns pinned to
// `minmax(0,1fr)`, so long rows truncate instead of forcing a horizontal
// scrollbar. This guards that contract at a narrow window.
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
