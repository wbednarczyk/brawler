import { expect, type Page, test } from "@playwright/test";

// Visual iteration harness for the fundamentals views. It is not a hard
// regression gate — it drives the panels in browser-smoke mode and captures
// screenshots under tests/browser/__screens__/ so the UI can be eyeballed and
// iterated at both desktop viewports. The AI KPI-extraction flows this harness
// used to capture were retired with the in-app AI layer (ADR 0084).

const SHOT_DIR = "tests/browser/__screens__";

test.describe("fundamentals visual harness", () => {
  test("captures the fundamentals panel", async ({ page }, testInfo) => {
    const tag = testInfo.project.name;
    await openApp(page);

    await navButton(page, "Companies").click();
    // Click the name/ticker area (what a user clicks) rather than the row centre,
    // which can fall on a watchlist chip.
    await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
    // F3a S1 (ADR 0107): the row opens Spółka; the legacy cockpit is reached via the sidebar Dashboard entry until S3 freezes it.
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Dashboard" }).click();

    // Opening a company lands the cockpit dashboard (ADR 0057); the Fundamentals
    // panel is shown directly, no tab.
    const panel = page.getByLabel("Company fundamentals");
    await expect(panel).toBeVisible();
    await expect(page.getByRole("heading", { name: "Reporting periods" })).toBeVisible();
    await expect(page.getByLabel("Financial facts matrix")).toBeVisible();

    await panel.screenshot({ path: `${SHOT_DIR}/fundamentals-${tag}.png` });

    // Inspect a fact detail (exercises the as-reported value + trend chart).
    await page.getByRole("button", { name: /^Revenue, / }).first().click();
    const detail = page.getByLabel("Financial fact detail");
    await expect(detail).toBeVisible();
    await detail.scrollIntoViewIfNeeded();
    await detail.screenshot({ path: `${SHOT_DIR}/fundamentals-fact-${tag}.png` });
  });

  test("captures the fundamentals panel in Polish", async ({ page }, testInfo) => {
    const tag = testInfo.project.name;
    await page.goto("/?locale=pl");

    await page.getByLabel("Nawigacja główna").getByRole("button", { name: "Spółki" }).click();
    await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
    // F3a S1 (ADR 0107): the row opens Spółka; the legacy cockpit is reached via the sidebar Dashboard entry until S3 freezes it.
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Dashboard" }).click();

    // Cockpit dashboard (ADR 0057) shows the Fundamentals panel directly.
    const panel = page.getByLabel("Wskaźniki finansowe spółki");
    await expect(panel).toBeVisible();
    // Localized KPI labels (not English / internal ids). Scoped to the facts
    // matrix specifically: the §A5 "Pozycje × okresy" section in the same panel
    // also carries a "Przychody" row, so an unscoped panel match is ambiguous.
    await expect(
      page.getByLabel("Tabela faktów finansowych").getByText("Przychody", { exact: true }),
    ).toBeVisible();
    await panel.screenshot({ path: `${SHOT_DIR}/fundamentals-pl-${tag}.png` });

    await page.getByRole("button", { name: /^Przychody, / }).first().click();
    const detail = page.getByLabel("Szczegóły faktu finansowego");
    await expect(detail).toBeVisible();
    await detail.scrollIntoViewIfNeeded();
    await detail.screenshot({ path: `${SHOT_DIR}/fundamentals-fact-pl-${tag}.png` });
  });
});

async function openApp(page: Page) {
  await page.goto("/");
  await expect(page.getByLabel("Primary navigation")).toBeVisible();
  // Today is the default landing (ADR 0054); these flows start from the Inbox feed.
  await page.getByLabel("Primary navigation").getByRole("button", { name: "Inbox" }).click();
  await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
}

function navButton(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}
