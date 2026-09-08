import { test, expect, openApp, setPaneSize, type Page } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import { expectTableCellTextInset } from "./helpers/interactionContracts";

// Owner guardrail (dogfooding wave 2026-09, round 2): "text touching the
// cell/table edge looks bad" — every data table keeps its text ≥ 6 px inside
// each cell. Covers the Spółka core KPI + Coverage tables and the Fundamentals
// facts matrix + Pozycje × okresy (many-periods scenario: sticky KPI column,
// expander column, hidden trend at S).

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

const TIERS = {
  S: { width: 380, height: 700 },
  M: { width: 600, height: 700 },
  L: { width: 900, height: 700 },
} as const;

async function openCompany(page: Page, companyId: string) {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator(`[data-company-id="${companyId}"] .company-row-main`).click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
}

async function openFundamentals(page: Page) {
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open fundamentals");
  await palette.getByRole("option", { name: "Open fundamentals", exact: true }).first().click();
  await expect(page.getByLabel("Financial facts matrix")).toBeVisible();
}

for (const [tierName, size] of Object.entries(TIERS)) {
  test(`Fundamentals period tables keep their text inside the cells @ ${tierName}`, async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["many-periods-fundamentals"] });
    await openApp(page);
    await openCompany(page, "company_gpw_manyperiods");
    await openFundamentals(page);
    await setPaneSize(page, { ...size, pane: page.locator(".spolka-layout") });
    await page.waitForTimeout(300);
    await expectTableCellTextInset(page.locator(".facts-matrix"));
    await expectTableCellTextInset(page.locator(".fundamentals-periods-table"));
    const expander = page.locator(".facts-matrix-expander-button");
    if ((await expander.count()) > 0) {
      await expander.click();
      await page.waitForTimeout(200);
      await expectTableCellTextInset(page.locator(".facts-matrix"));
    }
  });
}

test("Spółka core KPI and Coverage tables keep their text inside the cells", async ({ page }) => {
  await openApp(page);
  await openCompany(page, "company_gpw_cdr");
  await expectTableCellTextInset(page.locator(".spolka-kpi-table"));
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open coverage");
  await palette.getByRole("option", { name: "Open coverage", exact: true }).first().click();
  const coverage = page.locator(".coverage-table");
  await expect(coverage).toBeVisible();
  await expectTableCellTextInset(coverage);
});
