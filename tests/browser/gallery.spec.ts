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

// F3c S2 (#197, plan § Design 7/8): one global :focus-visible ring, stylelint-
// guarded against outline: none. Walk Tab (not a programmatic .focus() call —
// that does not reliably trigger :focus-visible) through every tabbable in
// the gallery and assert each shows a >= 2px outline while focused. Runs
// dark on the 4 dark projects and light on chromium-compact-light.
test("every focusable in the gallery shows a visible keyboard focus ring", async ({ page }) => {
  await page.goto("/gallery.html");
  const root = page.locator(".primitive-gallery");
  await expect(root).toBeVisible();

  const tabbableCount = await root.evaluate((el) => {
    const selector =
      'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
    return el.querySelectorAll(selector).length;
  });
  expect(tabbableCount).toBeGreaterThan(0);

  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur?.());

  // A compound native widget (DateField's <input type="date">) has internal
  // day/month/year segments Tab cycles through before actually leaving the
  // element — document.activeElement stays the SAME host element across
  // those internal presses, and Chromium reads as transiently unfocused
  // (outline-style "none") on the exact press that finally moves on. Tag
  // each newly-focused element with a marker so the walk only asserts once
  // it has genuinely landed on the NEXT element, not mid-internal-transition.
  function readFocused() {
    return page.evaluate(() => {
      const el = document.activeElement as HTMLElement;
      let id = el.getAttribute("data-gallery-tabwalk-id");
      if (!id) {
        id = Math.random().toString(36).slice(2);
        el.setAttribute("data-gallery-tabwalk-id", id);
      }
      const style = getComputedStyle(el);
      return {
        id,
        outlineStyle: style.outlineStyle,
        outlineWidth: style.outlineWidth,
        tag: el.tagName,
        name: el.getAttribute("aria-label") ?? el.textContent?.trim().slice(0, 40) ?? "",
      };
    });
  }

  await page.keyboard.press("Tab");
  let current = await readFocused();

  // Bound the walk by the tabbable count (+1 slack) — real navigation stops
  // itself once focus leaves the gallery root. Every visited element is
  // counted so a walk that leaves the root early (a stray Tab handler, an
  // element the browser skips) cannot pass vacuously (sol diff R1).
  const visited = new Set<string>();
  for (let i = 0; i < tabbableCount + 1; i += 1) {
    const insideRoot = await root.evaluate((el) => el.contains(document.activeElement));
    if (!insideRoot) break;
    visited.add(current.id);

    expect(
      current.outlineStyle,
      `${current.tag} "${current.name}" has no visible focus ring (outline-style: ${current.outlineStyle})`,
    ).not.toBe("none");
    expect(
      parseFloat(current.outlineWidth),
      `${current.tag} "${current.name}" focus ring is thinner than 2px (outline-width: ${current.outlineWidth})`,
    ).toBeGreaterThanOrEqual(2);

    // Advance to the next genuinely different element, absorbing any
    // internal-segment Tab stops a compound widget consumes along the way.
    const previousId = current.id;
    for (let guard = 0; guard < 5 && current.id === previousId; guard += 1) {
      await page.keyboard.press("Tab");
      current = await readFocused();
    }
  }
  // Browsers legitimately skip a few "tabbables" the selector counts (e.g. a
  // disabled-looking control, an element sized to zero); the walk must still
  // have covered the overwhelming majority — a lone early exit fails here.
  expect(
    visited.size,
    `only ${visited.size} of ${tabbableCount} gallery tabbables were visited before focus left the root`,
  ).toBeGreaterThanOrEqual(Math.floor(tabbableCount * 0.9));
});
