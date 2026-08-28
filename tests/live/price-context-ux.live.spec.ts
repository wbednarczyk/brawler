import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { clearCompaniesFilter } from "./helpers/companiesList";

// Live regression for the 2026-07-14 owner UX reports on the v0.53 price
// context: (1) "a price chart with no scale shows nothing" — the history line
// must render y-scale labels and the date span; (2) "what is this chip junk" —
// the sector taxonomy must NOT render as an always-on chip wall.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("price history renders with a readable scale and the sector field has no chip wall", async ({}) => {
  const { page } = connection;
  test.setTimeout(120_000);

  // The owner's own path: Companies → row click → Spółka → Fundamentals
  // workshop tool (ADR 0107/0108: the row lands the Spółka screen directly,
  // no cockpit dashboard).
  await page.getByRole("button", { name: /Spółki|Companies/ }).first().click();
  await clearCompaniesFilter(page);
  const row = page.locator("button[data-company-row]").first();
  await expect(row).toBeVisible({ timeout: 15_000 });
  await row.click();
  await page
    .getByRole("button", { name: /^(Fundamenty|Fundamentals)$/ })
    .first()
    .click();

  // (1) The price candlestick chart carries a y scale (3 labels), an x date
  // span, and real candles.
  const chart = page.locator(".price-context-history-chart");
  await expect(chart).toBeVisible({ timeout: 30_000 });
  expect(await chart.locator(".ui-candlestick-chart-ylabels > span").count()).toBeGreaterThanOrEqual(3);
  await expect(chart.locator(".ui-candlestick-chart-xlabels > span")).toHaveCount(2);
  expect(await chart.locator("rect.ui-candlestick-body").count()).toBeGreaterThan(1);
  // Round ticks: no y label carries loose decimals like "120,95 PLN".
  for (const label of await chart.locator(".ui-candlestick-chart-ylabels > span").allTextContents()) {
    expect(label).not.toMatch(/,\d\d? PLN/);
  }

  // (2) No always-on sector chip wall: without typing a narrowing query, the
  // suggestion group is absent.
  await expect(page.getByRole("group", { name: /Registry sectors|Sektory z rejestru/ })).toHaveCount(0);

  await page.screenshot({ path: "test-results/live/price-context-ux.png", fullPage: true });
});

test("Basic info panel shows read-only identity facts with edit behind one toggle", async ({}) => {
  const { page } = connection;
  test.setTimeout(120_000);

  // The Basic info panel is hosted inside the Spółka Ownership workshop tool
  // now (ADR 0107/0108, toolRegistry.tsx `akcjonariat` → CompanyBasicInfoPanel)
  // — continuing from the previous test's company context.
  await page
    .getByRole("group", { name: /Warsztat|Workshop/ })
    .getByRole("button", { name: /^(Akcjonariat|Ownership)$/ })
    .click();
  const tool = page.getByRole("group", { name: /Narzędzie warsztatu|Workshop tool/ });
  await expect(tool).toBeVisible();

  const panel = tool.locator(".basic-info-panel");
  await expect(panel).toBeVisible({ timeout: 15_000 });
  // Read-only facts render; edit fields stay hidden until the toggle.
  await expect(panel.getByText(/Nazwa|Name/).first()).toBeVisible();
  await expect(panel.getByText(/ISIN/).first()).toBeVisible();
  await expect(panel.locator(".basic-info-edit")).toHaveCount(0);

  await panel.getByRole("button", { name: /Edytuj|Edit/ }).first().click();
  await expect(panel.locator(".basic-info-edit")).toBeVisible();

  await page.screenshot({ path: "test-results/live/basic-info-panel.png", fullPage: true });
});
