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

    const workshop = spolka.getByRole("group", { name: "Workshop" });
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
    await spolka.getByRole("group", { name: "Workshop" }).getByRole("button", { name: "Open claims" }).click();
    await expect(spolka.getByLabel("Workshop tool")).toBeVisible();
    await expectNoPageOverflow(page);
    await assertContained(page.locator(".spolka-layout"), ".spolka-layout (claims tool open)");
  });

  test("core stacks to one column below ~1100px", async ({ page }) => {
    await openApp(page);
    const spolka = await openSpolka(page);
    const original = page.viewportSize();
    await page.setViewportSize({ width: 1000, height: 900 });
    const core = spolka.getByLabel("Company core");
    const columns = await core.evaluate((el) => getComputedStyle(el).gridTemplateColumns.trim().split(/\s+/).length);
    expect(columns, "core grid collapses to one column below the 1100px breakpoint").toBe(1);
    await expectNoPageOverflow(page);
    if (original) await page.setViewportSize(original);
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
