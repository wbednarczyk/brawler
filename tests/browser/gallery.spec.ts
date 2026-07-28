import { test, expect } from "@playwright/test";

// Visual/layout coherence check for the UI primitive gallery (gallery.html, a
// dev-only Vite entry). Runs across the configured viewport matrix. Asserts the
// catalog renders and does not introduce horizontal overflow at any supported
// width. A committed screenshot baseline is intentionally NOT used here: font
// rendering differs across WSL/Windows/CI, which makes pixel baselines flaky.
// To do local visual-regression, add `await expect(page).toHaveScreenshot()`
// and generate the baseline on your own machine.
test("primitive gallery renders without horizontal overflow", async ({ page }) => {
  await page.goto("/gallery.html");

  await expect(page.getByRole("heading", { name: "Brawler UI primitives", level: 1 })).toBeVisible();

  const hasOverflow = await page.evaluate(
    () => document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
  );
  expect(hasOverflow).toBe(false);
});

// Issue #209: the CI smoke-walk caught ui-status-chip-warn painting 3px past
// its box when the environment font metrics differ slightly from local. The
// chip class must shrink and clip inside a slot narrower than its label —
// never paint over the neighbor — regardless of font rendering.
test("a constrained status chip clips inside its slot instead of painting out", async ({ page }) => {
  await page.goto("/gallery.html");

  const demo = page.locator(".ui-chip-constrained-demo");
  const chip = demo.locator(".ui-status-chip");
  await expect(chip).toBeVisible();

  const slotBox = await demo.boundingBox();
  const chipBox = await chip.boundingBox();
  expect(chipBox!.width).toBeLessThanOrEqual(slotBox!.width + 1);
  expect(chipBox!.x + chipBox!.width).toBeLessThanOrEqual(slotBox!.x + slotBox!.width + 1);
});
