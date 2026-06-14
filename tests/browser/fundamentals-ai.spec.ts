import { expect, type Page, test } from "@playwright/test";

// Visual iteration harness for the v0.37 fundamentals + AI KPI extraction views.
// It is not a hard regression gate — it drives the panels in browser-smoke mode
// and captures screenshots under tests/browser/__screens__/ so the UI can be
// eyeballed and iterated at both desktop viewports.

const SHOT_DIR = "tests/browser/__screens__";

test.describe("fundamentals + AI KPI extraction visual harness", () => {
  test("captures the fundamentals panel", async ({ page }, testInfo) => {
    const tag = testInfo.project.name;
    await openApp(page);

    await navButton(page, "Companies").click();
    // Click the name/ticker area (what a user clicks) rather than the row centre,
    // which can fall on a watchlist chip.
    await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();

    await page.getByRole("button", { name: "Fundamentals", exact: true }).click();

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
    await page.getByRole("button", { name: "Wskaźniki finansowe", exact: true }).click();

    const panel = page.getByLabel("Wskaźniki finansowe spółki");
    await expect(panel).toBeVisible();
    // Localized KPI labels (not English / internal ids).
    await expect(panel.getByText("Przychody", { exact: true })).toBeVisible();
    await panel.screenshot({ path: `${SHOT_DIR}/fundamentals-pl-${tag}.png` });

    await page.getByRole("button", { name: /^Przychody, / }).first().click();
    const detail = page.getByLabel("Szczegóły faktu finansowego");
    await expect(detail).toBeVisible();
    await detail.scrollIntoViewIfNeeded();
    await detail.screenshot({ path: `${SHOT_DIR}/fundamentals-fact-pl-${tag}.png` });
  });

  test("captures the KPI extraction review flow", async ({ page }, testInfo) => {
    const tag = testInfo.project.name;
    await openApp(page);

    // First feed item is CD PROJEKT's "Official report" with a PDF attachment.
    await page
      .getByLabel(/Select feed item: CD PROJEKT/)
      .first()
      .click();

    const panel = page.getByLabel("AI KPI extraction");
    await expect(panel).toBeVisible();
    await panel.screenshot({ path: `${SHOT_DIR}/extraction-idle-${tag}.png` });

    // The interactive flow lives in a modal launched from the compact rail panel.
    await panel.getByRole("button", { name: "Extract KPIs" }).click();
    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await expect(dialog).toBeVisible();

    await dialog.getByRole("button", { name: /Extract from attachment/ }).click();

    await expect(dialog.getByText("Revenue")).toBeVisible();
    await expect(dialog.getByText("Wishlist additions")).toBeVisible();
    await dialog.screenshot({ path: `${SHOT_DIR}/extraction-proposals-${tag}.png` });

    // Confirm the first known KPI to capture the post-action state.
    await dialog.getByRole("button", { name: "Confirm" }).first().click();
    await expect(dialog.getByText("confirmed").first()).toBeVisible();
    await dialog.screenshot({ path: `${SHOT_DIR}/extraction-confirmed-${tag}.png` });
  });

  test("captures the IR candidate fallback", async ({ page }, testInfo) => {
    const tag = testInfo.project.name;
    await openApp(page);

    await page
      .getByLabel(/Select feed item: CD PROJEKT/)
      .first()
      .click();

    const panel = page.getByLabel("AI KPI extraction");
    await expect(panel).toBeVisible();
    await panel.getByRole("button", { name: "Extract KPIs" }).click();

    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Fetch report from IR page" }).click();

    await expect(page.getByLabel("IR page report candidates")).toBeVisible();
    await dialog.screenshot({ path: `${SHOT_DIR}/extraction-ir-candidates-${tag}.png` });
  });

  test("captures the extraction source buttons for a results report (pl)", async ({ page }, testInfo) => {
    const tag = testInfo.project.name;
    // Polish locale + a real-world results report with long PDF attachment names.
    await page.goto("/?locale=pl");

    await page.getByText(/wyniki za I półrocze 2026/).first().click();

    // Compact launcher in the rail; the source buttons (long PDF names) now live
    // in the modal, where there is room for them to lay out without clipping.
    const panel = page.locator(".kpi-extraction-launcher");
    await expect(panel).toBeVisible();
    await panel.getByRole("button", { name: "Wyodrębnij KPI" }).click();

    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/H1_25_26_Sprawozdanie Zarządu\.pdf/)).toBeVisible();
    await dialog.screenshot({ path: `${SHOT_DIR}/extraction-sources-pl-${tag}.png` });
    // Close the modal so the full-inbox screenshot/overflow check sees the rail.
    await dialog.getByRole("button", { name: "Close dialog" }).click();

    // Full inbox: detail pane must not be clipped horizontally at this width.
    await page.screenshot({ path: `${SHOT_DIR}/inbox-full-${tag}.png`, fullPage: true });
    const overflow = await page.locator(".content-grid").evaluate((el) => ({
      scrollWidth: el.scrollWidth,
      clientWidth: el.clientWidth,
    }));
    expect(overflow.scrollWidth).toBeLessThanOrEqual(overflow.clientWidth + 1);
  });
});

async function openApp(page: Page) {
  await page.goto("/");
  await expect(page.getByLabel("Primary navigation")).toBeVisible();
}

function navButton(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}
