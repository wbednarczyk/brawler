import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// Today/Pulse redesigned to journey J1 (ADR 0076 U-Rb): a single prioritized
// stream with roving j/k navigation plus a counters column that filters the
// stream. Clickable coverage for the two interactions the shell smoke test does
// not exercise. The browser runtime locale is en.
test.describe("Today stream — filter + keyboard", { tag: "@clickable" }, () => {
  test("a counter tile filters the stream to its category and restores it", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // The seed carries an autopilot run + report ("what changed") feed items.
    const autopilotRows = page.locator('li[data-category="autopilot"]');
    const changedRows = page.locator('li[data-category="changed"]');
    await expect(autopilotRows.first()).toBeVisible();
    expect(await changedRows.count()).toBeGreaterThan(0);

    const autopilotTile = page
      .getByRole("group", { name: "Filter the stream" })
      .getByRole("button", { name: /Autopilot/ });

    await autopilotTile.click();
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "true");
    // Only autopilot rows remain; the "what changed" rows are filtered out.
    await expect(changedRows).toHaveCount(0);
    await expect(autopilotRows.first()).toBeVisible();

    await autopilotTile.click();
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "false");
    expect(await changedRows.count()).toBeGreaterThan(0);

    await expectNoPageOverflow(page);
  });

  test("j/k moves roving focus across the stream's action buttons", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const rowButtons = page.locator('[data-today-row="true"]');
    expect(await rowButtons.count()).toBeGreaterThanOrEqual(2);

    await rowButtons.nth(0).focus();
    await expect(rowButtons.nth(0)).toBeFocused();

    await page.keyboard.press("j");
    await expect(rowButtons.nth(1)).toBeFocused();

    await page.keyboard.press("k");
    await expect(rowButtons.nth(0)).toBeFocused();

    await page.keyboard.press("ArrowDown");
    await expect(rowButtons.nth(1)).toBeFocused();
  });
});
