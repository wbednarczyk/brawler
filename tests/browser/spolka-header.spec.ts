import type { Page } from "@playwright/test";
import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// Spółka header layout (dogfooding wave 2026-09, #2/#3; ADR 0107 amendment):
// `identity · picker · actions`, the picker centred on the header at M/L
// regardless of the identity block's width, single column below the S
// `@container pane` breakpoint (419px, the codebase-wide S convention). Runs
// across all 5 configured Playwright projects; the S-tier assertion forces a
// real viewport narrow enough to shrink `.workspace`'s own size container
// below the threshold (the header sits outside `.spolka-layout`'s own `pane`
// container, so it resolves against `.workspace`'s).

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function openSpolka(page: Page, companyId: string): Promise<import("@playwright/test").Locator> {
  await openApp(page);
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator(`[data-company-id="${companyId}"] .company-row-main`).click();
  const spolka = page.getByRole("region", { name: "Company view", exact: true });
  await expect(spolka).toBeVisible();
  return spolka;
}

test.describe("Spółka header", { tag: "@journey" }, () => {
  test("picker sits centred on the header, independent of the identity block's width", async ({ page }) => {
    // KGHM POLSKA MIEDZ S.A. — one of the longer seeded display names, so a
    // pass here holds regardless of how wide the identity block gets.
    const spolka = await openSpolka(page, "company_gpw_kgh");
    const header = page.locator(".spolka-header");
    const picker = spolka.getByRole("combobox", { name: "Company" });
    await expect(picker).toBeVisible();

    const headerBox = await header.boundingBox();
    const pickerBox = await picker.boundingBox();
    expect(headerBox, "header bounding box").toBeTruthy();
    expect(pickerBox, "picker bounding box").toBeTruthy();
    const headerCenterX = headerBox!.x + headerBox!.width / 2;
    const pickerCenterX = pickerBox!.x + pickerBox!.width / 2;
    expect(
      Math.abs(pickerCenterX - headerCenterX),
      `picker centre (${pickerCenterX}) must sit within 24px of the header centre (${headerCenterX})`,
    ).toBeLessThanOrEqual(24);
  });

  test("header stacks to one column at the S tier with no horizontal overflow", async ({ page }) => {
    const spolka = await openSpolka(page, "company_gpw_kgh");
    const original = page.viewportSize();
    // `.workspace` width ≈ viewport width − the 232px sidebar; 480px total
    // measures at ~407px, comfortably under the 419px `@container pane` S
    // threshold.
    await page.setViewportSize({ width: 480, height: 900 });

    const header = page.locator(".spolka-header");
    const columns = await header.evaluate((el) => getComputedStyle(el).gridTemplateColumns.trim().split(/\s+/).length);
    expect(columns, "header collapses to a single column below the S breakpoint").toBe(1);
    await expect(spolka.getByRole("combobox", { name: "Company" })).toBeVisible();
    await expectNoPageOverflow(page);

    if (original) await page.setViewportSize(original);
  });

  test("typing a ticker substring + Enter switches to that company", async ({ page }) => {
    const spolka = await openSpolka(page, "company_gpw_cdr");
    const picker = spolka.getByRole("combobox", { name: "Company" });
    await picker.click();
    await picker.fill("kgh");
    await expect(spolka.getByRole("option", { name: "GPW:KGH · KGHM POLSKA MIEDZ S.A." })).toBeVisible();
    await page.keyboard.press("Enter");

    await expect(page.getByRole("region", { name: "Company view", exact: true })).toHaveAttribute(
      "data-company-id",
      "company_gpw_kgh",
    );
    await expect(page.getByRole("heading", { level: 1, name: "KGHM POLSKA MIEDZ S.A." })).toBeVisible();
  });
});
