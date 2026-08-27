import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
  type Journey,
} from "../helpers/harness";
import type { Page } from "@playwright/test";

// J7 — Weekly review (docs/ux-journeys.md, ADR 0074, ADR 0107). Trigger: the
// weekend ritual. F3a redefinition (plan § Lista zgód 5): the view-creation
// leg is frozen; all four task legs stay, each entered through its own
// screen: the week calendar (Events) → the watchlist overview (Watchlists,
// unchanged) → the research review queue (Research) → deepening on a
// company (Spółka).
//
// Budget: re-baselined at this redefinition's first honest measurement + 1
// (consent 5, ADR 0107) — see budgets.json.

async function openScreenViaJourney(j: Journey, page: Page, label: string): Promise<void> {
  // The global ⌘K shortcut is deliberately suppressed while focus sits in an
  // editable field (src/shared/shortcuts/index.ts `suppressWhenEditable`) — a
  // prior screen's own text field (e.g. Watchlists' "Create" name input) can
  // still hold focus here. Not a counted journey interaction: it mirrors a
  // real user's mouse move away from a field before reaching for a shortcut.
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await j.press(page, "Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await j.markModal("Command palette");
  await j.fill(palette.getByLabel("Search commands"), `Open screen: ${label}`);
  await j.click(palette.getByRole("button", { name: `Open screen: ${label}`, exact: true }).first());
}

test.describe("J7 — weekly review", { tag: "@journey" }, () => {
  test("weekly review walks Events week → Watchlists overview → Research queue → Spółka", async ({ page }) => {
    const j = journey(page, "J7");
    await openApp(page);
    await j.markScreen("Today");

    // Leg 1: Events — the week calendar, a global screen (F3a S3; the
    // cockpit "New view" creator this journey used before is frozen).
    await openScreenViaJourney(j, page, "Events");
    const eventsLayout = page.locator(".events-layout");
    await expect(eventsLayout).toBeVisible();
    await j.markScreen("Events");
    // Whichever mode the screen renders at this project's real viewport —
    // it's a full screen now, not a forced cockpit pane (plan §2 note: never
    // force a pane in a journey).
    const weekGrid = eventsLayout.locator(".event-week-grid");
    if (await weekGrid.isVisible()) {
      await expect(weekGrid).toBeVisible();
    } else {
      await expect(eventsLayout.locator(".event-row-block").first()).toBeVisible();
    }
    await expectNoPageOverflow(page);
    await expectNoA11yViolations(page, "Events (weekly review)");

    // Leg 2: Watchlists overview (unchanged path).
    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Watchlists" }));
    await j.markScreen("Watchlists");
    const watchlistRow = page.getByLabel("Watchlists", { exact: true }).getByRole("button").first();
    await expect(watchlistRow).toBeVisible();
    await j.click(watchlistRow);
    await expect(page.getByLabel("Selected watchlist")).toBeVisible();
    await expect(page.getByLabel("Companies in watchlist")).toBeVisible();
    await expectNoA11yViolations(page, "Weekly review — watchlist overview");
    await expectNoPageOverflow(page);

    // Leg 3: Research — the watchlist-scoped review queue (the research
    // debts, not vague guilt).
    await openScreenViaJourney(j, page, "Research");
    await j.markScreen("Research");
    const research = page.locator(".research-panel");
    await expect(research).toBeVisible();
    await j.click(research.getByRole("button", { name: "Watchlist" }));
    const reviewQueue = research.getByLabel("Watchlist company review queue");
    await expect(reviewQueue).toBeVisible();
    await expectNoA11yViolations(page, "Research review queue (weekly review)");
    await expectNoPageOverflow(page);

    // Leg 4: Spółka — deepening on a company from the watchlist, via the
    // review queue row's own "Open company" action (owner decision
    // 2026-08-26, ADR 0107) — no palette round-trip needed anymore. Scoped
    // to the CDR row: the full browser mock runtime seeds many watchlist
    // members, so an unscoped "Open company" match is ambiguous.
    const cdrQueueRow = reviewQueue.locator(".research-company-queue-row", { hasText: "CDR" });
    await j.click(cdrQueueRow.getByRole("button", { name: "Open company" }));
    const spolka = page.getByRole("region", { name: "Company view", exact: true });
    await expect(spolka).toBeVisible();
    await j.markScreen("Spółka");
    await expectNoPageOverflow(page);
    await expectNoA11yViolations(page, "Spółka (weekly review deepening)");

    await j.assertBudget();
  });
});
