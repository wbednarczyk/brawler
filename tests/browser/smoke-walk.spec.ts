import {
  test,
  expect,
  openApp,
  expectNoPageOverflow,
  expectNoHorizontalOverflow,
  expectNoA11yViolations,
} from "./helpers/harness";

// Cheap, broad coverage of the recurring layout-overflow class: walk every
// sidebar destination and assert the page never scrolls horizontally. Runs
// across the full viewport matrix (playwright.config.ts), which includes the
// tall/narrow quarter-ultrawide windows the app is run in. The console-error
// gate (auto fixture) also applies to every step here.
//
// DATA-DRIVEN ON PURPOSE (guardrail, ADR 0045): it iterates the actual rendered
// sidebar nav buttons rather than a hardcoded screen list, so a new mode/screen
// is overflow-checked automatically and a nav rename/slim never silently drops a
// screen from coverage — the failure mode that let the Today/Cockpit modes ship
// unchecked when the nav was restructured for ADR 0054.

test.describe("layout smoke-walk", () => {
  test("every sidebar destination lays out without horizontal overflow", async ({ page }) => {
    await openApp(page);
    const nav = page.getByLabel("Primary navigation");
    // Destinations only: the "New view" add button (`nav-item-add`) is an action
    // that opens the view creator (ADR 0057), not a navigable destination — walk
    // the real destinations and exclude it, so the overflow invariant still
    // covers every screen without asserting an action button "activates".
    const buttons = nav.locator("button.nav-item:not(.nav-item-add)");
    const count = await buttons.count();
    // Sanity: the spine must expose its modes + library destinations.
    expect(count, "sidebar should expose several destinations").toBeGreaterThan(4);

    for (let i = 0; i < count; i += 1) {
      const button = buttons.nth(i);
      const label = (await button.textContent())?.trim() || `#${i}`;
      await button.click();
      // Wait for the spine to reflect the navigation before measuring. A section
      // button gets `nav-item-active`; a pinned cockpit-view / company button
      // marks its wrapping `.pinned-row` active instead — accept either.
      await expect(async () => {
        const active = await button.evaluate(
          (el) =>
            el.classList.contains("nav-item-active") || el.closest(".pinned-row-active") !== null,
        );
        expect(active, `nav button "${label}" should activate`).toBe(true);
      }).toPass({ timeout: 5000 });
      // The page-level gate is the meaningful, low-noise invariant: the window
      // must never scroll horizontally on any destination.
      await expectNoPageOverflow(page);
      // Real-browser a11y gate on every destination, in both themes (the matrix
      // includes the light project) — catches contrast + ARIA regressions (U9).
      await expectNoA11yViolations(page, `destination "${label}"`);
    }
  });

  test("a report feed item renders rich detail without clipping the rail", async ({ page }) => {
    await openApp(page);
    // Today is the default home (ADR 0054); the full feed lives in the Inbox.
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Inbox" }).click();

    await page.getByLabel(/Select feed item: CD PROJEKT/).first().click();
    const detail = page.getByLabel("Feed item details");
    await expect(detail).toBeVisible();
    await expect(detail.getByLabel("AI KPI extraction")).toBeVisible();
    await expect(detail.getByLabel("AI analysis")).toBeVisible();

    // No descendant of the fixed-width rail forces it wider than its track.
    await expectNoHorizontalOverflow(detail);
    await expectNoPageOverflow(page);
  });
});
