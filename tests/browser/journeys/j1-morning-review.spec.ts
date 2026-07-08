import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";

// J1 — Morning review (docs/ux-journeys.md, ADR 0074). Trigger: opening the app
// at the start of the day. The cross-screen path: land on Today/Pulse → triage
// the attention stream (filter by a counter tile) → open the 1–2 items that
// matter into their company workspace → back to Today. Interactions are driven
// through the journey() wrapper so the step count is measured against the budget.
//
// Future step (dated): v0.54 morning briefing — read at the top of Today before
// triage — is not built yet; when it ships it joins this journey and its budget.

test.describe("J1 — morning review", { tag: "@journey" }, () => {
  test("triage the attention stream and open what matters", async ({ page }) => {
    const j = journey(page, "J1");
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // Key screen: Today is the day's landing — it must be axe-clean and not
    // overflow before any interaction.
    await expectNoA11yViolations(page, "Today (morning review)");
    await expectNoPageOverflow(page);

    const autopilotRows = page.locator('li[data-category="autopilot"]');
    const changedRows = page.locator('li[data-category="changed"]');
    await expect(autopilotRows.first()).toBeVisible();
    expect(await changedRows.count()).toBeGreaterThan(0);

    // Triage: the Autopilot counter filters the stream to just those items.
    const autopilotTile = page
      .getByRole("group", { name: "Filter the stream" })
      .getByRole("button", { name: /Autopilot/ });
    await j.click(autopilotTile);
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "true");
    await expect(changedRows).toHaveCount(0);

    // Restore the full stream to scan everything that arrived.
    await j.click(autopilotTile);
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "false");
    expect(await changedRows.count()).toBeGreaterThan(0);

    // Open the first item that matters (an autopilot run) into its workspace.
    await j.click(autopilotRows.first().getByRole("button", { name: "Review" }));
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await expectNoPageOverflow(page);

    // Back to Today.
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // Open the second item that matters (a "what changed" report) then return.
    await j.click(changedRows.first().getByRole("button", { name: "Review" }));
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await expectNoPageOverflow(page);

    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    j.assertBudget();
  });
});
