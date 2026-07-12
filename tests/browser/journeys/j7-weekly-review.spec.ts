import {
  test,
  expect,
  openApp,
  journey,
  setPaneSize,
  expectNoPageOverflow,
  expectNoA11yViolations,
  type Journey,
} from "../helpers/harness";
import type { Locator, Page } from "@playwright/test";

// J7 — Weekly review (docs/ux-journeys.md, ADR 0074). Trigger: the weekend
// ritual. Current path: open the week calendar (Events) to see what's coming →
// scan the watchlist overview → plan the week. Done well: next week's dates are
// known and the research backlog is explicit.
//
// Future steps (dated): v0.63 watchlist command center — heatmap + leaderboard
// and the research-gap detector — replace the plain watchlist overview and the
// vague "research gaps" step; they join this journey and its budget then.

async function openCockpitPanelViaJourney(j: Journey, page: Page, label: string): Promise<void> {
  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await j.click(nav.getByRole("button", { name: "New view" }));
  const createModal = page.getByRole("dialog", { name: "New view" });
  await j.fill(createModal.getByLabel("View name"), `${label} test view`);
  await j.click(createModal.getByRole("button", { name: "Create view" }));
  await expect(page.getByLabel("Research cockpit")).toBeVisible();
  await expect(page.getByText("Pick a panel").first()).toBeVisible();
  await j.click(page.getByRole("button", { name: "Add panel" }));
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await j.fill(palette.getByLabel("Search commands"), `Open panel: ${label}`);
  await j.click(palette.getByRole("button", { name: `Open panel: ${label}`, exact: true }).first());
}

test.describe("J7 — weekly review", { tag: "@journey" }, () => {
  test("scan the week calendar and the watchlist overview", async ({ page }) => {
    const j = journey(page, "J7");
    await openApp(page);

    // The week calendar — Events at the L tier renders the week grid.
    await openCockpitPanelViaJourney(j, page, "Events");
    const eventsPane: Locator = page.locator(".cockpit-pane", { has: page.locator(".events-layout") });
    await expect(eventsPane).toBeVisible();
    await setPaneSize(page, { width: 900, height: 700, pane: eventsPane });
    await expect(eventsPane.locator(".event-week-grid")).toBeVisible();
    await expectNoPageOverflow(page);
    // Gate the week grid's a11y (ADR 0045): keeps the horizontally scrollable
    // week region keyboard-reachable (scrollable-region-focusable).
    await expectNoA11yViolations(page, "Events week grid");

    // Watchlist overview — what's on the list this week.
    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Watchlists" }));
    const watchlistRow = page
      .getByLabel("Watchlists", { exact: true })
      .getByRole("button")
      .first();
    await expect(watchlistRow).toBeVisible();
    await j.click(watchlistRow);
    await expect(page.getByLabel("Selected watchlist")).toBeVisible();
    await expect(page.getByLabel("Companies in watchlist")).toBeVisible();
    await expectNoA11yViolations(page, "Weekly review — watchlist overview");
    await expectNoPageOverflow(page);

    j.assertBudget();
  });
});
