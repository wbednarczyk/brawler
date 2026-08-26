import {
  test,
  expect,
  openApp,
  setPaneSize,
  resetPaneSize,
  expectNoHorizontalOverflow,
} from "./helpers/harness";

// Mechanical defect #2 (F3a study, docs/plans/frontend-v2-f3a-study.md §
// "Mechanical defects found by the study"): at a narrow (compact) pane the
// coverage table's Data/Flagged columns fell entirely off the visible pane,
// reachable only via `.coverage-scroll`'s horizontal scroll a user never
// triggers — effectively dropping the payload. Fixed by relocating those
// counts under the period label as a second line at the compact tier
// (`@container pane (max-width: 640px)`) instead of hiding them.

async function openCoveragePanel(page: import("@playwright/test").Page, companyId: string) {
  await page.setViewportSize({ width: 1008, height: 900 });
  await openApp(page);
  await page
    .getByLabel(/Primary navigation|Nawigacja główna/)
    .getByRole("button", { name: "Companies" })
    .click();
  await page.locator(`[data-company-id="${companyId}"] .company-row-main`).click();
  // F3a S3 (ADR 0107): the row opens Spółka; the frozen legacy cockpit for
  // this company is reached via its "Legacy dashboard · TICKER" Widoki row.
  const ticker = companyId.split("_").pop()?.toUpperCase();
  await page
    .getByLabel("Primary navigation")
    .getByRole("button", { name: `Legacy dashboard · ${ticker}` })
    .click();
  await page.getByRole("button", { name: "Coverage", exact: true }).first().click();
}

test("the compact-tier pane keeps the Data and Flagged counts under the period label", async ({
  page,
}) => {
  await openCoveragePanel(page, "company_gpw_cdr");

  const panel = page.locator('.company-coverage[data-company-id="company_gpw_cdr"]');
  await expect(panel).toBeVisible();
  const pane = page.locator(".cockpit-pane", { has: panel });

  // Force the compact tier (<=640px pane width) regardless of the project's
  // base viewport — the tier switch is pane-width-driven, not window-driven.
  await setPaneSize(page, { width: 600, height: 700, pane });

  const row = panel.locator("tbody tr").first();
  const meta = row.locator(".coverage-period-meta");
  await expect(meta).toBeVisible();
  await expect(meta).toContainText("Data");
  await expect(meta).toContainText("Flagged");

  // The Data/Flagged columns themselves fold away at this tier — the payload
  // moved, it was never simply deleted.
  await expect(row.locator("td").nth(2)).toBeHidden();
  await expect(row.locator("td").nth(3)).toBeHidden();

  await expectNoHorizontalOverflow(panel.locator(".coverage-scroll"));

  await resetPaneSize(page, pane);
});

test("a wide pane shows the Data and Flagged columns, not the compact-tier summary", async ({
  page,
}) => {
  await openCoveragePanel(page, "company_gpw_cdr");

  const panel = page.locator('.company-coverage[data-company-id="company_gpw_cdr"]');
  const pane = page.locator(".cockpit-pane", { has: panel });
  // Force a pane comfortably above the compact tier — the default cockpit
  // dashboard's Coverage tab can itself land under 640px at a 1008px window.
  await setPaneSize(page, { width: 900, height: 700, pane });

  const row = panel.locator("tbody tr").first();

  await expect(row.locator(".coverage-period-meta")).toBeHidden();
  await expect(row.locator("td").nth(2)).toBeVisible();
  await expect(row.locator("td").nth(3)).toBeVisible();

  await expectNoHorizontalOverflow(panel.locator(".coverage-scroll"));

  await resetPaneSize(page, pane);
});
