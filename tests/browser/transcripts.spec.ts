import { test, expect, openApp, setPaneSize, resetPaneSize } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Transcripts — the changed-workflow first red journey test (F4b S2 contract
// § Transcripts "First red journey test" / #430a/#430b): the composer must
// stay inside its card at the narrowest supported pane, and a fetched
// transcript must show exactly one status chip (never two, the #430b defect).

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

test.describe("Transcripts — changed workflow", { tag: "@clickable" }, () => {
  test("the composer stays inside the card at S", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Transcripts" }).click();
    const region = page.getByRole("region", { name: "Transcripts" });
    await expect(region).toBeVisible();

    const pane = page.locator(".workspace");
    await setPaneSize(page, { width: 380, height: 700, pane });

    const composer = page.locator(".transcript-composer").first();
    await expect(composer).toBeVisible();
    const box = await composer.evaluate((el) => ({
      scrollWidth: el.scrollWidth,
      clientWidth: el.clientWidth,
    }));
    expect(box.scrollWidth).toBeLessThanOrEqual(box.clientWidth + 1);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1)).toBe(true);

    await resetPaneSize(page, pane);
  });

  test("a fetched transcript shows exactly one status chip", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Transcripts" }).click();
    const region = page.getByRole("region", { name: "Transcripts" });
    await expect(region).toBeVisible();

    await page.getByLabel("Recording link").fill("https://www.youtube.com/watch?v=transcriptsjourney");
    await page.getByRole("button", { name: "Fetch transcript" }).click();

    const row = region.locator(".transcript-row-block").first();
    await expect(row).toBeVisible();
    await expect(row.locator("[data-transcript-status]")).toHaveCount(1);
  });
});
