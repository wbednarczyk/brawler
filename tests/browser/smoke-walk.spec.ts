import { test, expect, openApp, expectNoPageOverflow, expectNoHorizontalOverflow } from "./helpers/harness";

// Cheap, broad coverage of the recurring layout-overflow class: walk every
// primary screen and assert the page never scrolls horizontally and no element
// blows out its box. Runs across the full viewport matrix (playwright.config.ts),
// which includes the tall/narrow quarter-ultrawide windows the app is run in.
// The console-error gate (auto fixture) also applies to every step here.

const SCREENS = ["Inbox", "Companies", "Watchlists", "Sources", "Notebooks", "Settings"] as const;

test.describe("layout smoke-walk", () => {
  test("every primary screen lays out without horizontal overflow", async ({ page }) => {
    await openApp(page);
    const nav = page.getByLabel("Primary navigation");

    for (const screen of SCREENS) {
      await nav.getByRole("button", { name: screen }).click();
      await expect(page.getByRole("heading", { name: screen, exact: true })).toBeVisible();
      // The page-level gate is the meaningful, low-noise invariant: the window
      // must never scroll horizontally. (Per-element deep scans are reserved for
      // the detail rail / modal below, where overflow:hidden hides the symptom.)
      await expectNoPageOverflow(page);
    }
  });

  test("a report feed item renders rich detail without clipping the rail", async ({ page }) => {
    await openApp(page);

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
