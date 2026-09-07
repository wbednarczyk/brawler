import { test, expect, openApp, setPaneSize, type Page } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import { periodExpanderAccessibleName } from "../../src/screens/Companies/useVisiblePeriods";

// Dogfooding wave 2026-09, #6 — the "own gate" commit: newest MEASURED-
// capacity periods visible, oldest hidden behind the full-height clickable
// expander column (owner storyboard round 1). The `many-periods-fundamentals`
// overlay (src/test/scenarios/overlays.ts) seeds "Many Periods Test S.A."
// (ticker ZZZN, company_gpw_manyperiods) with 14 annual periods — the base
// "rich" scenario's companies carry only 2, never enough to force collapse.

const MANY_PERIODS_COMPANY_ID = "company_gpw_manyperiods";
const TOTAL_PERIODS = 14;

const TIER_SIZE = {
  S: { width: 380, height: 700 },
  M: { width: 600, height: 700 },
  L: { width: 900, height: 700 },
} as const;

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openManyPeriodsFundamentals(page: Page, locale: "en" | "pl" = "en") {
  await primeMockScenario(page, { base: "rich", overlays: ["many-periods-fundamentals"] });
  await openApp(page, locale === "pl" ? "/?locale=pl" : "/");
  const companiesLabel = locale === "pl" ? "Spółki" : "Companies";
  await nav(page).getByRole("button", { name: companiesLabel }).click();
  await page.locator(`[data-company-id="${MANY_PERIODS_COMPANY_ID}"] .company-row-main`).click();
  const companyViewLabel = locale === "pl" ? "Widok spółki" : "Company view";
  await page.getByRole("region", { name: companyViewLabel }).waitFor();
  await page.keyboard.press("Control+K");
  const paletteLabel = locale === "pl" ? "Paleta poleceń" : "Command palette";
  const searchLabel = locale === "pl" ? "Szukaj poleceń" : "Search commands";
  const optionLabel = locale === "pl" ? "Otwórz fundamenty" : "Open fundamentals";
  const palette = page.getByRole("dialog", { name: paletteLabel });
  await palette.getByLabel(searchLabel).fill(optionLabel);
  await palette.getByRole("option", { name: optionLabel, exact: true }).first().click();
  const matrixLabel = locale === "pl" ? "Tabela faktów finansowych" : "Financial facts matrix";
  await expect(page.getByLabel(matrixLabel)).toBeVisible();
}

/** Mirrors useVisiblePeriods.ts's own capacity formula so the test's
 * expectation is derived from the SAME measured numbers the app used —
 * never a hardcoded count (plan requirement). */
function expectedCapacity(scrollerWidth: number, periodWidth: number, stickyWidth: number): number {
  if (!periodWidth) return 1;
  const available = scrollerWidth - stickyWidth;
  const raw = Math.floor(available / periodWidth);
  return Math.min(8, Math.max(1, raw));
}

async function measure(page: Page) {
  const scroller = page.locator(".facts-matrix-scroll");
  const scrollerWidth = await scroller.evaluate((el) => el.clientWidth);
  const stickyWidth = await page
    .locator(".facts-matrix-corner")
    .evaluate((el) => (el as HTMLElement).offsetWidth) +
    (await page
      .locator(".facts-matrix-expander")
      .first()
      .evaluate((el) => (el as HTMLElement).offsetWidth)) +
    // The Trend column absorbs the table's slack; its min-width is the floor
    // the host subtracts (`FACTS_TREND_COLUMN_WIDTH`).
    (await page
      .locator(".facts-matrix-trend-head")
      .evaluate((el) => Number.parseFloat(getComputedStyle(el).minWidth)));
  const periodHeaders = page.locator(
    '.facts-matrix thead th[scope="col"]:not(.facts-matrix-corner):not(.facts-matrix-trend-head)',
  );
  const periodWidth = await periodHeaders.first().evaluate((el) => (el as HTMLElement).offsetWidth);
  const capacity = expectedCapacity(scrollerWidth, periodWidth, stickyWidth);
  return { scrollerWidth, stickyWidth, periodWidth, capacity, periodHeaders };
}

for (const [tierName, size] of Object.entries(TIER_SIZE)) {
  test(`geometric contract at ${tierName}: newest period fits before interaction, expander names the hidden count`, async ({
    page,
  }) => {
    await openManyPeriodsFundamentals(page);
    const pane = page.locator(".spolka-layout");
    await setPaneSize(page, { ...size, pane });

    // Force the resize tick to land (ResizeObserver is async) before measuring.
    await expect(page.locator(".facts-matrix-corner")).toBeVisible();
    await page.waitForTimeout(400);

    const { capacity, periodHeaders } = await measure(page);
    const expectedHidden = TOTAL_PERIODS - capacity;

    // Newest period header's right edge lies inside the scroller's client box
    // BEFORE any interaction — no horizontal scroll needed to see it.
    const scroller = page.locator(".facts-matrix-scroll");
    const scrollerBox = (await scroller.boundingBox())!;
    const newestHeaderBox = (await periodHeaders.last().boundingBox())!;
    expect(newestHeaderBox.x + newestHeaderBox.width).toBeLessThanOrEqual(scrollerBox.x + scrollerBox.width + 1);

    if (expectedHidden > 0) {
      const expander = page.locator(".facts-matrix-expander-button");
      await expect(expander).toBeVisible();
      const expectedName = periodExpanderAccessibleName(false, expectedHidden, "en", (v) => v);
      await expect(expander).toHaveAccessibleName(expectedName);

      // The column's box spans the whole table body height (rowSpan).
      const expanderBox = (await page.locator(".facts-matrix-expander").last().boundingBox())!;
      const tbodyBox = (await page.locator(".facts-matrix tbody").boundingBox())!;
      expect(expanderBox.height).toBeGreaterThanOrEqual(tbodyBox.height * 0.9);
    } else {
      // 8 is the max capacity — a wide enough tier can fit all 14 only if the
      // clamp were absent, which it never is, so this branch documents intent
      // rather than being reachable with today's fixture; kept for safety.
      await expect(page.locator(".facts-matrix-expander-button")).toHaveCount(0);
    }

    // The KPI header's accessible name is unaffected by the expander column.
    await expect(page.getByRole("columnheader", { name: "KPI", exact: true })).toBeVisible();
  });
}

test("expanding shows every period, scrolls to the newest, and the column stays sticky", async ({ page }) => {
  await openManyPeriodsFundamentals(page);
  const pane = page.locator(".spolka-layout");
  await setPaneSize(page, { ...TIER_SIZE.M, pane });
  await page.waitForTimeout(150);

  const scroller = page.locator(".facts-matrix-scroll");
  const scrollLeftBefore = await scroller.evaluate((el) => el.scrollLeft);
  expect(scrollLeftBefore, "no scroll before interaction").toBe(0);

  const expanderCell = page.locator(".facts-matrix-expander").last();
  // Click anywhere in the column (its default center) — the whole column is
  // the hit area.
  await expanderCell.click();

  await expect(page.locator(".facts-matrix-expander-button")).toHaveAccessibleName("Collapse earlier");
  const periodHeaders = page.locator(
    '.facts-matrix thead th[scope="col"]:not(.facts-matrix-corner):not(.facts-matrix-trend-head)',
  );
  await expect(periodHeaders).toHaveCount(TOTAL_PERIODS);

  const scrollLeftAfter = await scroller.evaluate((el) => el.scrollLeft);
  expect(scrollLeftAfter, "scrolled to the newest period on expand").toBeGreaterThan(0);

  // The column stays visible (sticky) after scrolling — its left edge still
  // sits within the scroller's visible box, not scrolled out of view.
  const scrollerBox = (await scroller.boundingBox())!;
  const expanderBoxAfterScroll = (await expanderCell.boundingBox())!;
  expect(expanderBoxAfterScroll.x).toBeGreaterThanOrEqual(scrollerBox.x - 1);
  expect(expanderBoxAfterScroll.x).toBeLessThan(scrollerBox.x + scrollerBox.width);

  // Resizing across a tier boundary while expanded must not yank the user
  // back or change the scroll position.
  await setPaneSize(page, { ...TIER_SIZE.S, pane });
  await page.waitForTimeout(150);
  const scrollLeftAfterResize = await scroller.evaluate((el) => el.scrollLeft);
  expect(scrollLeftAfterResize, "resize while expanded does not change scrollLeft").toBe(scrollLeftAfter);
  await expect(periodHeaders).toHaveCount(TOTAL_PERIODS);
});

test("Tab reaches the expander column control, with a visible focus ring", async ({ page }) => {
  await openManyPeriodsFundamentals(page);

  await page.getByRole("textbox", { name: "Find a position" }).focus();
  await page.keyboard.press("Tab");

  const expander = page.locator(".facts-matrix-expander-button");
  await expect(expander).toBeFocused();

  const outline = await expander.evaluate((el) => {
    const style = getComputedStyle(el as HTMLElement, ":focus-visible");
    return { style: style.outlineStyle, width: style.outlineWidth };
  });
  expect(outline.style, `expander control has no visible focus ring (outline-style: ${outline.style})`).not.toBe(
    "none",
  );

  // Enter activates the control like a click.
  await page.keyboard.press("Enter");
  await expect(expander).toHaveAccessibleName("Collapse earlier");
});

test("both locales' expander names render correctly", async ({ page }) => {
  await openManyPeriodsFundamentals(page, "pl");
  const pane = page.locator(".spolka-layout");
  await setPaneSize(page, { ...TIER_SIZE.M, pane });
  await page.waitForTimeout(150);

  const { capacity } = await measure(page);
  const expectedHidden = TOTAL_PERIODS - capacity;
  test.skip(expectedHidden <= 0, "nothing hidden at this measured capacity");

  const expander = page.locator(".facts-matrix-expander-button");
  const expectedName = periodExpanderAccessibleName(false, expectedHidden, "pl", (v) => v);
  await expect(expander).toHaveAccessibleName(expectedName);
  await expect(expander).toHaveText("Rozwiń starsze");
});
