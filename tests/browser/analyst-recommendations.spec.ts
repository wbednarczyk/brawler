import { test, expect, openApp, expectNoPageOverflow, setPaneSize } from "./helpers/harness";

// Analyst-recommendations pinned panel (v0.58 A3, ADR 0073). The first red
// journey test from the experience contract: "pinned panel lists attributed
// recommendations for the selected company" (J6 buy/pass), plus the signal badge
// in the feed (J1 morning review) and a narrow-pane inner-scroll overflow guard.
// Opening a company lands the cockpit dashboard with the view company set, so the
// palette-added `analystRecommendations` FOLLOW panel resolves to that company;
// the dual-execution mock runtime serves the seeded history (CD PROJEKT
// populated, ORLEN empty).

async function addRecommendationsPanel(
  page: import("@playwright/test").Page,
  companyId: string,
) {
  await openApp(page);
  await page
    .getByLabel(/Primary navigation|Nawigacja główna/)
    .getByRole("button", { name: "Companies" })
    .click();
  await page.locator(`[data-company-id="${companyId}"] .company-row-main`).click();

  await page.getByRole("button", { name: "Add panel" }).click();
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open panel: Analyst recommendations");
  await palette
    .getByRole("button", { name: "Open panel: Analyst recommendations", exact: true })
    .first()
    .click();
}

test("pinned panel lists attributed recommendations for the selected company", async ({ page }) => {
  await addRecommendationsPanel(page, "company_gpw_cdr");

  const panel = page.locator(".analyst-recs-panel");
  await expect(panel).toBeVisible();

  // Not-advice attribution is stated inline (ADR 0073 hard rule).
  await expect(panel.getByText(/not investment advice|nie porada inwestycyjna/)).toBeVisible();

  // Attributed history: firm + verbatim rating + the local wall-clock date.
  await expect(panel.getByText("Noble Securities").first()).toBeVisible();
  await expect(panel.getByText("akumuluj")).toBeVisible();
  await expect(panel.getByText("18.06.2026").first()).toBeVisible();

  // The latest-target summary card carries its firm + date attribution.
  await expect(panel.getByText(/Latest target price|Najnowsza cena docelowa/)).toBeVisible();

  // The broker PDF is an external link (secondary action).
  await expect(panel.getByRole("link", { name: /Broker PDF|PDF raportu/ }).first()).toBeVisible();

  // Experience contract § 6: a quiet read surface — no primary action anywhere.
  expect(await panel.locator("[data-ux-primary-action]").count()).toBe(0);

  await expectNoPageOverflow(page);
});

test("empty company shows the calm empty state, not a ghost table", async ({ page }) => {
  await addRecommendationsPanel(page, "company_gpw_pkn");

  const panel = page.locator(".analyst-recs-panel");
  await expect(panel).toBeVisible();
  await expect(panel.locator(".analyst-recs-empty")).toBeVisible();
  await expect(panel.locator(".analyst-recs-row")).toHaveCount(0);

  await expectNoPageOverflow(page);
});

test("the recommendation panel does not overflow at a ~230px pane (inner scroll)", async ({ page }) => {
  await addRecommendationsPanel(page, "company_gpw_cdr");

  const panel = page.locator(".analyst-recs-panel");
  await expect(panel).toBeVisible();

  // Force the hosting pane to the quarter-ultrawide narrow width (frame 7).
  const pane = page.locator(".cockpit-pane", { has: panel });
  await setPaneSize(page, { width: 230, pane });

  // The list + rows must wrap inside the pane, never scroll horizontally.
  await expect(panel.getByText("Noble Securities").first()).toBeVisible();
  const containers: Array<[string, import("@playwright/test").Locator]> = [
    ["panel", panel],
    ["list", panel.locator(".analyst-recs-list")],
    ["summary", panel.locator(".analyst-recs-summary")],
  ];
  for (const [name, locator] of containers) {
    const overflow = await locator.first().evaluate((el) => {
      const node = el as HTMLElement;
      return node.scrollWidth - node.clientWidth;
    });
    expect(overflow, `${name} overflows horizontally`).toBeLessThanOrEqual(1);
  }

  await expectNoPageOverflow(page);
});

test("a recommendation-change signal badge shows in the company feed", async ({ page }) => {
  await openApp(page);
  await page
    .getByLabel(/Primary navigation|Nawigacja główna/)
    .getByRole("button", { name: "Inbox" })
    .click();

  // The seeded recommendation_change signal renders its verbatim badge chip in a
  // feed row (not just the category filter's <option>).
  await expect(
    page
      .locator(".signal-badge", {
        hasText: /Analyst recommendation change|Zmiana rekomendacji analityka/,
      })
      .first(),
  ).toBeVisible();
});
