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
    await page.getByRole("button", { name: "Open GPW:CDR workspace" }).click();

    await page.getByRole("button", { name: "Fundamentals", exact: true }).click();

    const panel = page.getByLabel("Company fundamentals");
    await expect(panel).toBeVisible();
    await expect(page.getByText("Reporting periods")).toBeVisible();
    await expect(page.getByLabel("Financial facts matrix")).toBeVisible();

    await panel.screenshot({ path: `${SHOT_DIR}/fundamentals-${tag}.png` });

    // Inspect a fact detail (exercises the as-reported value + trend chart).
    await page.getByRole("button", { name: /^Revenue, / }).first().click();
    const detail = page.getByLabel("Financial fact detail");
    await expect(detail).toBeVisible();
    await detail.scrollIntoViewIfNeeded();
    await detail.screenshot({ path: `${SHOT_DIR}/fundamentals-fact-${tag}.png` });
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

    await page.getByRole("button", { name: /Extract from attachment/ }).click();

    // Proposals open in a centered review modal.
    const dialog = page.getByRole("dialog", { name: /Proposed KPI values/ });
    await expect(dialog).toBeVisible();
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
    await page.getByRole("button", { name: "Fetch report from IR page" }).click();

    await expect(page.getByLabel("IR page report candidates")).toBeVisible();
    await panel.screenshot({ path: `${SHOT_DIR}/extraction-ir-candidates-${tag}.png` });
  });
});

async function openApp(page: Page) {
  await page.goto("/");
  await expect(page.getByLabel("Primary navigation")).toBeVisible();
}

function navButton(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}
