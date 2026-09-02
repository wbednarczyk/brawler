import {
  test,
  expect,
  openApp,
  openScreen,
  expectNoA11yViolations,
  expectNoPageOverflow,
} from "./helpers/harness";

// The global screens reachable only via the ⌘K palette's "Open screen: …"
// entries (F3a S3, ADR 0107 decision 5: Research / Events / Report Season) —
// standalone routes, mounted full-screen in `.workspace`, since ADR 0108
// retired the "Add panel" surface that used to host them as cockpit
// dashboard tabs. Open each and assert it renders without horizontal page
// overflow. The auto console-error gate (harness fixture) additionally fails
// on a render error / unmocked command, so this is the coverage that those
// screens still mount cleanly in their (post-retirement) home. Notebooks and
// Decision journal retired as standalone routes in F4c S2 (ADR 0108
// amendment) — note/decision work happens in the Spółka `notatnik`/`dziennik`
// workshop tools now, covered by the density/journeys specs.
//
// `heading` is the screen's leading H1 text; `level` is the heading level it
// renders (PanelHeader → h1).
const PANELS = [
  { tab: "Research", heading: "Research", level: 1 },
  { tab: "Events", heading: "Events", level: 1 },
  { tab: "Report Season", heading: "Report Season", level: 1 },
] as const;

// The D6 compact-header rule (ADR 0076 Decision 6) applies only inside a Spółka
// tool frame; standalone screens render their leading heading at full size.
const FULL_HEADER_PANELS = PANELS;

test.describe("global screens lay out without overflow (F3a S3, ADR 0108: standalone routes, not cockpit panels)", () => {
  test("each global screen renders without overflow", async ({ page }) => {
    await openApp(page);

    for (const { tab } of PANELS) {
      await openScreen(page, tab);
      await expectNoPageOverflow(page);
    }
  });

  // Real-browser a11y (#158): the jsdom guard covers these screens for
  // role/label/structure; only a real browser computes colors, so contrast in
  // their standalone form is checked here — across the viewport matrix and
  // both theme projects.
  for (const { tab } of PANELS) {
    test(`the ${tab} screen has no WCAG A/AA violations`, async ({ page }) => {
      await openApp(page);
      await openScreen(page, tab);

      await expectNoA11yViolations(page, `${tab} screen`);
    });
  }

  for (const { tab, heading, level } of FULL_HEADER_PANELS) {
    test(`renders its full (non-compacted) heading for the ${tab} screen`, async ({ page }) => {
      await openApp(page);
      await openScreen(page, tab);

      const pane = page.locator(".workspace");
      const h1 = pane.getByRole("heading", { level, name: heading, exact: true });

      await expect(h1).toBeVisible();
      const box = await h1.boundingBox();
      expect(box, "heading must lay out").not.toBeNull();
      // The pre-freeze compact-header CSS clips a duplicated title to ~1px; a
      // full, non-compacted heading is comfortably bigger than that.
      expect(box!.width).toBeGreaterThan(2);
      expect(box!.height).toBeGreaterThan(2);
    });
  }
});
