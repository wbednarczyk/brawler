import { test, expect, openApp, openCockpitPanel, expectNoPageOverflow } from "./helpers/harness";

// The screens that moved off the sidebar into the cockpit (ADR 0054) —
// Research / Notebook / Events / Report Season — no longer have their own nav
// button; they live as cockpit panels. Open each from the command palette and
// assert it renders without horizontal page overflow. The auto console-error
// gate (harness fixture) additionally fails on a render error / unmocked command,
// so this is the coverage that those screens still mount cleanly in their new home.
const PANELS = ["Research", "Notebook", "Events", "Report Season"] as const;

test.describe("cockpit-hosted screens lay out without overflow", () => {
  test("each former secondary screen renders as a cockpit panel without overflow", async ({ page }) => {
    await openApp(page);

    for (const panel of PANELS) {
      await openCockpitPanel(page, panel);
      await expectNoPageOverflow(page);
    }
  });
});
