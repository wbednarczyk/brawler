import { test, expect, openApp } from "./helpers/harness";

test.describe("debug", { tag: "@clickable" }, () => {
  test("inspect hostile row overlap", async ({ page }) => {
    await openApp(page);
    await page.getByLabel(/Primary navigation|Nawigacja główna/).getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();

    await page.getByRole("textbox", { name: /Search|Szukaj/ }).first().fill("wyniki za I półrocze");
    const hostileRow = page.locator('[data-feed-item-id="feed_results_report"]');
    await expect(hostileRow).toBeVisible();

    await page.setViewportSize({ width: 600, height: 700 });
    const workspace = page.locator(".workspace");
    await workspace.evaluate((el) => {
      (el as HTMLElement).style.setProperty("inline-size", "600px", "important");
      (el as HTMLElement).style.setProperty("block-size", "700px", "important");
    });

    const info = await page.evaluate(() => {
      const row = document.querySelector('[data-feed-item-id="feed_results_report"]');
      const detail = document.querySelector(".detail-pane");
      const rowRect = row?.getBoundingClientRect();
      const detailRect = detail?.getBoundingClientRect();
      const detailAttrs = detail
        ? { open: detail.getAttribute("data-detail-open"), display: getComputedStyle(detail).display, visibility: getComputedStyle(detail).visibility }
        : null;
      const atPoint = rowRect
        ? document.elementFromPoint(rowRect.x + rowRect.width / 2, rowRect.y + rowRect.height / 2)
        : null;
      return {
        rowRect,
        detailRect,
        detailAttrs,
        atPointTag: atPoint?.tagName,
        atPointClass: (atPoint as HTMLElement | null)?.className,
      };
    });
    console.log(JSON.stringify(info, null, 2));
  });
});
