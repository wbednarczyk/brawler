import { test, expect, openApp, expectNoPageOverflow, expectNoHorizontalOverflow } from "./helpers/harness";

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
    const buttons = nav.locator("button.nav-item");
    const count = await buttons.count();
    // Sanity: the spine must expose its modes + library destinations.
    expect(count, "sidebar should expose several destinations").toBeGreaterThan(4);

    for (let i = 0; i < count; i += 1) {
      const button = buttons.nth(i);
      const label = (await button.textContent())?.trim() || `#${i}`;
      await button.click();
      // Wait for the spine to reflect the navigation before measuring.
      await expect(button, `nav button "${label}" should activate`).toHaveClass(/nav-item-active/);
      // The page-level gate is the meaningful, low-noise invariant: the window
      // must never scroll horizontally on any destination.
      await expectNoPageOverflow(page);
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
