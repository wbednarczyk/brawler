import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";

// Extends the layout smoke-walk to the primary screens the original walk did
// not reach (ADR 0048 broad coverage). Each navigation asserts no horizontal
// page overflow; the auto console-error gate (harness fixture) additionally
// fails the test if a screen renders with an error or calls an unmocked command.
// ReportSeason's heading is date-driven, so it asserts layout only.
const SCREENS = [
  { nav: "Report Season", heading: null },
  { nav: "Research", heading: "Research" },
  { nav: "Events", heading: "Events" },
  { nav: "Transcripts", heading: "Transcripts" },
] as const;

test.describe("extended layout smoke-walk", () => {
  test("secondary screens lay out without horizontal overflow", async ({ page }) => {
    await openApp(page);
    const nav = page.getByLabel("Primary navigation");

    for (const screen of SCREENS) {
      await nav.getByRole("button", { name: screen.nav }).click();
      if (screen.heading) {
        await expect(page.getByRole("heading", { name: screen.heading, exact: true })).toBeVisible();
      }
      await expectNoPageOverflow(page);
    }
  });
});
