import { test, expect, openApp, setPaneSize, type Page } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";

// Dogfooding wave 2026-09, #5 — sticky first column / header guard.
//
// Collapsed period tables are NOT scroll containers (`overflow-x: clip;
// overflow-y: visible`), so their `thead` pins to the tool body's scroll;
// an expanded table becomes a bounded 2-axis box with the header pinned
// inside (owner round 2). Sticky cells keep an opaque background and the
// z-index ladder body 1 / header 2 / corner 3. The `many-periods-fundamentals`
// overlay seeds 14 periods × 6 long-label KPI rows so both axes overflow.

const MANY_PERIODS_COMPANY_ID = "company_gpw_manyperiods";

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openManyPeriodsFundamentals(page: Page) {
  await primeMockScenario(page, { base: "rich", overlays: ["many-periods-fundamentals"] });
  await openApp(page, "/");
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator(`[data-company-id="${MANY_PERIODS_COMPANY_ID}"] .company-row-main`).click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill("Open tool: Fundamentals");
  await palette.getByRole("option", { name: "Open tool: Fundamentals", exact: true }).first().click();
  await expect(page.getByLabel("Financial facts matrix")).toBeVisible();
}

async function opaqueBackground(page: Page, selector: string) {
  return page.locator(selector).first().evaluate((el) => getComputedStyle(el).backgroundColor);
}

function expectOpaque(value: string, label: string) {
  expect(value, `${label} background opaque`).not.toBe("rgba(0, 0, 0, 0)");
  expect(value, `${label} background opaque`).not.toBe("transparent");
}

async function zIndexOf(page: Page, selector: string) {
  return page.locator(selector).first().evaluate((el) => Number(getComputedStyle(el).zIndex) || 0);
}

const TIERS = {
  S: { width: 380, height: 700 },
  M: { width: 600, height: 700 },
  L: { width: 900, height: 700 },
} as const;

for (const [tierName, size] of Object.entries(TIERS)) {
  test(`facts matrix: sticky scroller is bounded, cells opaque, header outranks body @ ${tierName}`, async ({
    page,
  }) => {
    await openManyPeriodsFundamentals(page);
    const pane = page.locator(".spolka-layout");
    await setPaneSize(page, { ...size, pane });
    await expect(page.locator(".facts-matrix-corner")).toBeVisible();
    await page.waitForTimeout(300);

    // Collapsed: not a scroll container — the header pins to the tool body's
    // scroll (owner 2026-09-07). Expanded: a bounded 2-axis scroller.
    const collapsedStyle = await page.locator(".facts-matrix-scroll").evaluate((el) => {
      const style = getComputedStyle(el);
      return { overflowY: style.overflowY, overflowX: style.overflowX, maxHeight: style.maxHeight };
    });
    expect(collapsedStyle.overflowY, "collapsed scroller is not a vertical scroll container").toBe("visible");
    expect(collapsedStyle.overflowX, "collapsed scroller clips instead of scrolling").toBe("clip");
    expect(collapsedStyle.maxHeight, "collapsed scroller is unbounded").toBe("none");
    const pinned = await page.evaluate(() => {
      const body = document.querySelector(".spolka-tool-body") as HTMLElement;
      const th = document.querySelector(".facts-matrix thead th") as HTMLElement;
      const table = document.querySelector(".facts-matrix") as HTMLElement;
      // Scroll the tool body so the table's top passes the viewport top but
      // its bottom stays in view — the header must stay pinned at the top.
      const tableTop = table.getBoundingClientRect().top - body.getBoundingClientRect().top + body.scrollTop;
      body.scrollTop = tableTop + 60;
      return new Promise<{ headerTop: number; bodyTop: number; tableBottom: number }>((resolve) =>
        requestAnimationFrame(() =>
          resolve({
            headerTop: th.getBoundingClientRect().top,
            bodyTop: body.getBoundingClientRect().top,
            tableBottom: table.getBoundingClientRect().bottom,
          }),
        ),
      );
    });
    expect(pinned.tableBottom, "table still partly in view").toBeGreaterThan(pinned.bodyTop + 40);
    expect(Math.abs(pinned.headerTop - pinned.bodyTop), "header pinned to the tool body's top").toBeLessThanOrEqual(2);
    await page.evaluate(() => {
      (document.querySelector(".spolka-tool-body") as HTMLElement).scrollTop = 0;
    });

    await page.locator(".facts-matrix-expander-button").click();
    await page.waitForTimeout(200);
    const expandedStyle = await page.locator(".facts-matrix-scroll").evaluate((el) => {
      const style = getComputedStyle(el);
      return { overflowY: style.overflowY, maxHeight: style.maxHeight };
    });
    expect(expandedStyle.overflowY, "expanded scroller is a vertical scroll container").toBe("auto");
    expect(expandedStyle.maxHeight, "expanded scroller has a bounded height").not.toBe("none");

    // Every sticky cell keeps an opaque background — the thing that stops
    // scrolled values from bleeding through the pinned column/header.
    for (const selector of [".facts-matrix-corner", ".facts-matrix-kpi", ".facts-matrix-expander"]) {
      expectOpaque(await opaqueBackground(page, selector), selector);
    }

    // Stacking: the header row's cells outrank the body's sticky cells, so a
    // header never loses to a body row scrolling past it.
    const cornerZ = await zIndexOf(page, ".facts-matrix-corner");
    const kpiZ = await zIndexOf(page, ".facts-matrix-kpi");
    expect(cornerZ, "header corner outranks the body KPI column").toBeGreaterThan(kpiZ);
    const headerExpanderZ = await zIndexOf(page, "thead .facts-matrix-expander");
    const bodyExpanderZ = await zIndexOf(page, "tbody .facts-matrix-expander");
    expect(headerExpanderZ, "header expander outranks the body expander").toBeGreaterThan(bodyExpanderZ);

    // Horizontal scroll: the sticky KPI column's left edge never moves.
    const kpiBoxBefore = (await page.locator(".facts-matrix-kpi").first().boundingBox())!;
    await page.locator(".facts-matrix-scroll").evaluate((el) => {
      el.scrollLeft = el.scrollWidth;
    });
    await page.waitForTimeout(150);
    const kpiBoxAfter = (await page.locator(".facts-matrix-kpi").first().boundingBox())!;
    expect(
      Math.abs(kpiBoxAfter.x - kpiBoxBefore.x),
      "sticky KPI column doesn't move on horizontal scroll",
    ).toBeLessThan(2);

    // Vertical scroll: the sticky header's top edge never moves once docked.
    const cornerBoxBefore = (await page.locator(".facts-matrix-corner").boundingBox())!;
    await page.locator(".facts-matrix-scroll").evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    await page.waitForTimeout(150);
    const cornerBoxAfter = (await page.locator(".facts-matrix-corner").boundingBox())!;
    expect(
      Math.abs(cornerBoxAfter.y - cornerBoxBefore.y),
      "sticky header doesn't move on vertical scroll",
    ).toBeLessThan(2);
    // And it stays opaque + visible after both scrolls, not scrolled away.
    await expect(page.locator(".facts-matrix-corner")).toBeVisible();
    expectOpaque(await opaqueBackground(page, ".facts-matrix-corner"), ".facts-matrix-corner (post-scroll)");
  });

  test(`Pozycje × okresy: sticky scroller is bounded, cells opaque, header outranks body @ ${tierName}`, async ({
    page,
  }) => {
    await openManyPeriodsFundamentals(page);
    const pane = page.locator(".spolka-layout");
    await setPaneSize(page, { ...size, pane });
    await expect(page.locator(".fundamentals-periods-corner")).toBeVisible();
    await page.waitForTimeout(300);

    const collapsed = await page.locator(".fundamentals-periods-scroll").evaluate((el) => {
      const style = getComputedStyle(el);
      return { overflowY: style.overflowY, overflowX: style.overflowX, maxHeight: style.maxHeight };
    });
    expect(collapsed.overflowY, "collapsed scroller is not a vertical scroll container").toBe("visible");
    expect(collapsed.overflowX, "collapsed scroller clips instead of scrolling").toBe("clip");
    expect(collapsed.maxHeight, "collapsed scroller is unbounded").toBe("none");
    const expanderButton = page.locator(".fundamentals-periods-expander-button");
    if ((await expanderButton.count()) > 0) {
      await expanderButton.click();
      await page.waitForTimeout(200);
      const expanded = await page.locator(".fundamentals-periods-scroll").evaluate((el) => {
        const style = getComputedStyle(el);
        return { overflowY: style.overflowY, maxHeight: style.maxHeight };
      });
      expect(expanded.overflowY, "expanded scroller is a vertical scroll container").toBe("auto");
      expect(expanded.maxHeight, "expanded scroller has a bounded height").not.toBe("none");
    }

    for (const selector of [
      ".fundamentals-periods-corner",
      ".fundamentals-periods-kpi",
      ".fundamentals-periods-expander",
    ]) {
      expectOpaque(await opaqueBackground(page, selector), selector);
    }

    const cornerZ = await zIndexOf(page, ".fundamentals-periods-corner");
    const kpiZ = await zIndexOf(page, ".fundamentals-periods-kpi");
    expect(cornerZ, "header corner outranks the body KPI column").toBeGreaterThan(kpiZ);

    const kpiBoxBefore = (await page.locator(".fundamentals-periods-kpi").first().boundingBox())!;
    await page.locator(".fundamentals-periods-scroll").evaluate((el) => {
      el.scrollLeft = el.scrollWidth;
    });
    await page.waitForTimeout(150);
    const kpiBoxAfter = (await page.locator(".fundamentals-periods-kpi").first().boundingBox())!;
    expect(
      Math.abs(kpiBoxAfter.x - kpiBoxBefore.x),
      "sticky KPI column doesn't move on horizontal scroll",
    ).toBeLessThan(2);

    const cornerBoxBefore = (await page.locator(".fundamentals-periods-corner").boundingBox())!;
    await page.locator(".fundamentals-periods-scroll").evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    await page.waitForTimeout(150);
    const cornerBoxAfter = (await page.locator(".fundamentals-periods-corner").boundingBox())!;
    expect(
      Math.abs(cornerBoxAfter.y - cornerBoxBefore.y),
      "sticky header doesn't move on vertical scroll",
    ).toBeLessThan(2);
    await expect(page.locator(".fundamentals-periods-corner")).toBeVisible();
    expectOpaque(
      await opaqueBackground(page, ".fundamentals-periods-corner"),
      ".fundamentals-periods-corner (post-scroll)",
    );
  });
}
