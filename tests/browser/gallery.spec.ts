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
