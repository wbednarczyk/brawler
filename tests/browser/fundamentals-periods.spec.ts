import { test, expect, openApp, openPalette, setPaneSize, type Page } from "./helpers/harness";
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
  const searchLabel = locale === "pl" ? "Szukaj poleceń" : "Search commands";
  const optionLabel = locale === "pl" ? "Otwórz narzędzie: Fundamenty" : "Open tool: Fundamentals";
  const palette = await openPalette(page);
  await palette.getByLabel(searchLabel).fill(optionLabel);
  await palette.getByRole("option", { name: optionLabel, exact: true }).first().click();
  const matrixLabel = locale === "pl" ? "Tabela faktów finansowych" : "Financial facts matrix";
  await expect(page.getByLabel(matrixLabel)).toBeVisible();
}

/** Independent expectation: expand the matrix so EVERY period renders,
 * measure each period column's natural width (text ink + padding + borders,
 * the same definition the app uses but computed here, from all periods),
 * read the fixed/expander widths from the rendered CSS, apply the contract
 * (all fit without the expander → all; else reserve the expander), then
 * collapse again. */
async function measure(page: Page) {
  const scroller = page.locator(".facts-matrix-scroll");
  const periodHeaders = page.locator(
    '.facts-matrix thead th[scope="col"]:not(.facts-matrix-corner):not(.facts-matrix-expander):not(.facts-matrix-trend-head)',
  );
  const expander = page.locator(".facts-matrix-expander-button");
  const hadExpander = (await expander.count()) > 0;
  if (hadExpander) {
    await expander.click();
    await expect(periodHeaders).toHaveCount(TOTAL_PERIODS);
  }
  const capacity = await scroller.evaluate((el, total) => {
    const pad = (node: Element) => {
      const style = getComputedStyle(node);
      return ["paddingLeft", "paddingRight", "borderLeftWidth", "borderRightWidth"]
        .map((key) => Number.parseFloat(style[key as keyof CSSStyleDeclaration] as string) || 0)
        .reduce((a, b) => a + b, 0);
    };
    const natural = (cell: Element) => {
      const inner = cell.querySelector(".facts-matrix-cell") ?? cell;
      const range = document.createRange();
      range.selectNodeContents(inner);
      return range.getBoundingClientRect().width + pad(cell) + (inner === cell ? 0 : pad(inner));
    };
    let widest = 0;
    for (const cell of el.querySelectorAll("[data-period-cell]")) widest = Math.max(widest, natural(cell));
    const corner = el.querySelector(".facts-matrix-corner") as HTMLElement;
    const trend = el.querySelector(".facts-matrix-trend-head") as HTMLElement | null;
    const trendShown = trend && getComputedStyle(trend).display !== "none";
    const fixed =
      corner.offsetWidth +
      (trendShown ? Math.max(Number.parseFloat(getComputedStyle(trend).minWidth) || 0, natural(trend)) : 0);
    const expanderWidth = Number.parseFloat(getComputedStyle(el).getPropertyValue("--period-expander-width")) || 0;
    const available = el.clientWidth - fixed;
    if (total * widest <= available) return total;
    return Math.max(1, Math.floor((available - expanderWidth) / widest));
  }, TOTAL_PERIODS);
  if (hadExpander) {
    await expander.click();
    await page.waitForTimeout(150);
  }
  return { capacity, periodHeaders };
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
    // BEFORE any interaction — no horizontal scroll needed to see it — and the
    // collapsed table fills but never overflows its box (columns stretch;
    // the capacity came from natural widths).
    const scroller = page.locator(".facts-matrix-scroll");
    const scrollerBox = (await scroller.boundingBox())!;
    const newestHeaderBox = (await periodHeaders.last().boundingBox())!;
    expect(newestHeaderBox.x + newestHeaderBox.width).toBeLessThanOrEqual(scrollerBox.x + scrollerBox.width + 1);
    await expect(periodHeaders).toHaveCount(capacity);
    const tableWidth = (await page.locator(".facts-matrix").boundingBox())!.width;
    expect(tableWidth, "collapsed table does not overflow its box").toBeLessThanOrEqual(scrollerBox.width + 1);
    expect(tableWidth, "collapsed table fills its box").toBeGreaterThanOrEqual(scrollerBox.width - 2);

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
    '.facts-matrix thead th[scope="col"]:not(.facts-matrix-corner):not(.facts-matrix-expander):not(.facts-matrix-trend-head)',
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
  // A wide pane fits every period (no cap) — force a tier that hides some.
  await setPaneSize(page, { ...TIER_SIZE.M, pane: page.locator(".spolka-layout") });
  await page.waitForTimeout(300);

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

// Guardrail harvest (integration 2026-09-07): three defects the gates above
// let through — a percentage-height label in the rowSpan cell inflated every
// row to hundreds of px, the single visible period header stretched to the
// table width so the measured capacity stayed at 1 on any tier, and the zebra
// tint was imperceptible. The base "rich" company (4 short periods) must show
// them all at L with compact rows, and the stripe must be visible.
async function openSmokeCompanyFundamentals(page: Page) {
  await openApp(page);
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  const palette = await openPalette(page);
  await palette.getByLabel("Search commands").fill("Open tool: Fundamentals");
  await palette.getByRole("option", { name: "Open tool: Fundamentals", exact: true }).first().click();
  await expect(page.getByLabel("Financial facts matrix")).toBeVisible();
}

test("a four-period company at L shows every period, no expander column, compact rows", async ({ page }) => {
  await openSmokeCompanyFundamentals(page);
  await setPaneSize(page, { ...TIER_SIZE.L, pane: page.locator(".spolka-layout") });
  await page.waitForTimeout(400);
  const periodHeaders = page.locator(
    '.facts-matrix thead th[scope="col"]:not(.facts-matrix-corner):not(.facts-matrix-expander):not(.facts-matrix-trend-head)',
  );
  await expect(periodHeaders).toHaveCount(4);
  await expect(page.locator(".facts-matrix-expander")).toHaveCount(0);
  const rowHeights = await page.locator(".facts-matrix tbody tr").evaluateAll((rows) =>
    rows.map((row) => row.getBoundingClientRect().height),
  );
  expect(rowHeights.length).toBeGreaterThan(0);
  for (const height of rowHeights) expect(height).toBeLessThanOrEqual(48);
});

function luminance(color: string): number {
  // `rgb(r, g, b)` or `color(srgb r g b)` — Chromium returns the latter for
  // `color-mix()` results.
  const numbers = color.match(/[\d.]+/g)?.map(Number) ?? [];
  const [r, g, b] = color.startsWith("color(") ? numbers.slice(0, 3) : numbers.slice(0, 3).map((v) => v / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

test("zebra rows are perceptible and continuous under the sticky column", async ({ page }) => {
  await openManyPeriodsFundamentals(page);
  await setPaneSize(page, { ...TIER_SIZE.L, pane: page.locator(".spolka-layout") });
  await page.waitForTimeout(400);
  const backgrounds = await page.locator(".facts-matrix tbody tr").evaluateAll((rows) =>
    rows.slice(0, 2).map((row) => Array.from(row.children).map((cell) => getComputedStyle(cell).backgroundColor)),
  );
  const [odd, even] = backgrounds;
  const oddSticky = luminance(odd[0]);
  const evenSticky = luminance(even[0]);
  expect(Math.abs(evenSticky - oddSticky)).toBeGreaterThanOrEqual(0.02);
  // The sticky KPI cell and the value cells share the row's tint.
  const evenValue = even.find((_, index) => index > 0 && !even[index].startsWith("rgba(0, 0, 0, 0)"));
  expect(evenValue).toBeDefined();
  expect(luminance(evenValue!)).toBeCloseTo(evenSticky, 2);
});

// Pozycje × okresy across tiers (sol R3 blocker 2): the S tier folds the Δ
// columns, so a width cached at S must not drive the capacity at M/L (and the
// reverse). The oracle re-measures the DISPLAYED columns at every tier.
async function measurePositions(page: Page) {
  const scroller = page.locator(".fundamentals-periods-scroll");
  const groups = page.locator('.fundamentals-periods-table thead th[data-period-col="value"]');
  const total = Number(await scroller.evaluate((el) => el.dataset.totalPeriods ?? "0"));
  const expander = page.locator(".fundamentals-periods-expander-button");
  const hadExpander = (await expander.count()) > 0;
  if (hadExpander) {
    await expander.click();
    await expect(groups).toHaveCount(total);
  }
  const capacity = await scroller.evaluate((el, total) => {
    const pad = (node: Element) => {
      const style = getComputedStyle(node);
      return ["paddingLeft", "paddingRight", "borderLeftWidth", "borderRightWidth"]
        .map((key) => Number.parseFloat(style[key as keyof CSSStyleDeclaration] as string) || 0)
        .reduce((a, b) => a + b, 0);
    };
    const widest = new Map<string, number>();
    for (const cell of el.querySelectorAll<HTMLElement>("[data-period-col]")) {
      if (getComputedStyle(cell).display === "none") continue;
      const range = document.createRange();
      range.selectNodeContents(cell);
      const width = range.getBoundingClientRect().width + pad(cell);
      const col = cell.dataset.periodCol as string;
      widest.set(col, Math.max(widest.get(col) ?? 0, width));
    }
    let group = 0;
    for (const width of widest.values()) group += width;
    const corner = el.querySelector(".fundamentals-periods-corner") as HTMLElement;
    const expanderWidth = Number.parseFloat(getComputedStyle(el).getPropertyValue("--period-expander-width")) || 0;
    const available = el.clientWidth - corner.offsetWidth;
    if (total * group <= available) return total;
    return Math.max(1, Math.floor((available - expanderWidth) / group));
  }, total);
  if (hadExpander) {
    await expander.click();
    await page.waitForTimeout(150);
  }
  return { capacity, groups, scroller };
}

for (const order of [["S", "M", "L"], ["L", "M", "S"]] as const) {
  test(`Pozycje × okresy collapsed capacity follows the tier ${order.join("→")}`, async ({ page }) => {
    await openManyPeriodsFundamentals(page);
    const pane = page.locator(".spolka-layout");
    for (const tier of order) {
      await setPaneSize(page, { ...TIER_SIZE[tier], pane });
      await page.waitForTimeout(400);
      const { capacity, groups, scroller } = await measurePositions(page);
      await expect(groups).toHaveCount(capacity);
      const box = await scroller.evaluate((el) => ({ scrollWidth: el.scrollWidth, clientWidth: el.clientWidth }));
      expect(box.scrollWidth, `collapsed Pozycje never overflows at ${tier}`).toBeLessThanOrEqual(box.clientWidth + 1);
    }
  });
}

// Webfont swap (sol R4): the first measuring pass may run in the fallback
// font; after the real face swaps in (`font-display: swap`) the cache is
// invalidated and the collapsed count matches the oracle on the final metrics.
test("a late webfont swap re-measures the collapsed capacity", async ({ page }) => {
  await page.route("**/*.woff2", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 1500));
    await route.continue();
  });
  await openManyPeriodsFundamentals(page);
  const pane = page.locator(".spolka-layout");
  await setPaneSize(page, { ...TIER_SIZE.M, pane });
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(400);
  const { capacity, periodHeaders } = await measure(page);
  await expect(periodHeaders).toHaveCount(capacity);
  const box = await page.locator(".facts-matrix-scroll").evaluate((el) => ({ scrollWidth: el.scrollWidth, clientWidth: el.clientWidth }));
  expect(box.scrollWidth, "collapsed table fits after the font swap").toBeLessThanOrEqual(box.clientWidth + 1);
});
