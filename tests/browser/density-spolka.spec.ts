import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import { primeMockScenario } from "./helpers/mockRuntime";
import type { Locator, Page } from "@playwright/test";

// Density/overflow contract for the Spółka screen (F3a S1–S3, ADR 0107,
// ADR 0076 D6). Unlike the workshop-tool density matrix (density-companies /
// density-quality-docs, container-query tiers on `.spolka-layout`), the core
// two-column-to-one-column stack is a viewport media query (spolka.css
// `@media (max-width: 1100px)`), not a container query — so it's exercised via
// a real `page.setViewportSize`, not `setPaneSize`.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openSpolka(page: Page, companyId = "company_gpw_cdr"): Promise<Locator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator(`[data-company-id="${companyId}"] .company-row-main`).click();
  const spolka = page.getByRole("region", { name: "Company view", exact: true });
  await expect(spolka).toBeVisible();
  return spolka;
}

async function assertContained(locator: Locator, label: string) {
  const box = await locator.evaluate((el) => ({ scrollWidth: el.scrollWidth, clientWidth: el.clientWidth }));
  expect(box.scrollWidth, `${label} overflows (${box.scrollWidth} > ${box.clientWidth})`).toBeLessThanOrEqual(
    box.clientWidth + 1,
  );
}

test.describe("Spółka density", { tag: "@clickable" }, () => {
  test("at rest: no overflow, inner container contained, workshop bar wraps without clipping", async ({ page }) => {
    await openApp(page);
    const spolka = await openSpolka(page);
    await expectNoPageOverflow(page);
    // `.spolka-layout` carries the density contracts' `container: pane / size`
    // (spolka.css) — the inner scroll region every hosted tool resolves against.
    await assertContained(page.locator(".spolka-layout"), ".spolka-layout");

    const workshop = spolka.getByRole("toolbar", { name: "Workshop" });
    const buttons = workshop.getByRole("button");
    const count = await buttons.count();
    for (let i = 0; i < count; i += 1) {
      const clipped = await buttons.nth(i).evaluate((el) => el.scrollWidth > el.clientWidth + 1);
      expect(clipped, `workshop bar button #${i} label is clipped`).toBe(false);
    }
  });

  test("with the claims tool open: no overflow, inner container contained", async ({ page }) => {
    await openApp(page);
    const spolka = await openSpolka(page);
    await spolka.getByRole("toolbar", { name: "Workshop" }).getByRole("button", { name: "Claims", exact: true }).click();
    await expect(spolka.getByLabel("Workshop tool")).toBeVisible();
    await expectNoPageOverflow(page);
    await assertContained(page.locator(".spolka-layout"), ".spolka-layout (claims tool open)");
  });

  test("core stacks to one column below ~1100px; the body scrolls internally and the workshop bar stays fixed", async ({ page }) => {
    await primeMockScenario(page, "rich");
    await openApp(page);
    const spolka = await openSpolka(page);
    const original = page.viewportSize();
    await page.setViewportSize({ width: 1000, height: 900 });
    const core = spolka.getByLabel("Company core");
    const columns = await core.evaluate((el) => getComputedStyle(el).gridTemplateColumns.trim().split(/\s+/).length);
    expect(columns, "core grid collapses to one column below the 1100px breakpoint").toBe(1);
    await expectNoPageOverflow(page);

    const workshop = spolka.getByRole("toolbar", { name: "Workshop" });
    const panelBox = await spolka.boundingBox();
    const workshopBox = await workshop.boundingBox();
    const restDelta = Math.abs(workshopBox!.y + workshopBox!.height - (panelBox!.y + panelBox!.height));
    expect(restDelta, "workshop bar bottom must equal the panel bottom below the breakpoint too").toBeLessThanOrEqual(2);

    // The body — not the workshop bar — carries the stacked column's overflow.
    const bodyScroll = page.locator(".spolka-body-scroll");
    await bodyScroll.evaluate((el) => {
      el.scrollTop = el.scrollHeight;
    });
    const workshopBoxAfterScroll = await workshop.boundingBox();
    const afterDelta = Math.abs(
      workshopBoxAfterScroll!.y + workshopBoxAfterScroll!.height - (panelBox!.y + panelBox!.height),
    );
    expect(afterDelta, "workshop bar bottom must equal the panel bottom after scrolling the stacked body").toBeLessThanOrEqual(2);

    if (original) await page.setViewportSize(original);
  });

  // P1 layout fix (owner dogfooding v0.74, item 1): the screen fills the
  // panel height with NO outer scroll — the workshop bar is a fixed sibling
  // of the scrolling body, never part of it, so it can never scroll out of
  // view. `.spolka-layout` itself never scrolls (each card scrolls its own
  // overflow); the workshop bar's bottom edge stays glued to the panel's
  // bottom both at rest and after a card scrolls.
  test("fixed workshop bar: no outer scroll, bar glued to the panel bottom (rich scenario, 1366×768 and 1920×1080)", async ({ page }) => {
    await primeMockScenario(page, "rich");
    await openApp(page);
    const spolka = await openSpolka(page);
    const layout = page.locator(".spolka-layout");
    const workshop = spolka.getByRole("toolbar", { name: "Workshop" });

    for (const viewport of [
      { width: 1366, height: 768 },
      { width: 1920, height: 1080 },
    ]) {
      await page.setViewportSize(viewport);

      const noOuterScroll = await layout.evaluate((el) => el.scrollHeight <= el.clientHeight + 1);
      expect(noOuterScroll, `.spolka-layout must not scroll at ${viewport.width}×${viewport.height}`).toBe(true);

      const panelBox = await spolka.boundingBox();
      const workshopBox = await workshop.boundingBox();
      expect(panelBox, "panel bounding box").toBeTruthy();
      expect(workshopBox, "workshop bar bounding box").toBeTruthy();
      const restDelta = Math.abs(workshopBox!.y + workshopBox!.height - (panelBox!.y + panelBox!.height));
      expect(restDelta, "workshop bar bottom must equal the panel bottom at rest").toBeLessThanOrEqual(2);

      // Scroll a card's own overflow — the workshop bar must not move.
      await spolka.getByLabel("Annual KPI table").evaluate((el) => {
        el.scrollTop = el.scrollHeight;
      });
      const workshopBoxAfterScroll = await workshop.boundingBox();
      const afterDelta = Math.abs(
        workshopBoxAfterScroll!.y + workshopBoxAfterScroll!.height - (panelBox!.y + panelBox!.height),
      );
      expect(afterDelta, "workshop bar bottom must equal the panel bottom after scrolling a card").toBeLessThanOrEqual(2);
    }
  });

  test("hostile overlay keeps feed rows contained", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["hostile-content"] });
    await openApp(page);
    const spolka = await openSpolka(page, "company_gpw_hostile");
    await expectNoPageOverflow(page);
    const feed = spolka.getByLabel("Company feed");
    await expect(feed).toBeVisible();
    await assertContained(feed, "Company feed (hostile overlay)");
  });
});
