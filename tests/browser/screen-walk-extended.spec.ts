import {
  test,
  expect,
  openApp,
  openCockpitPanel,
  expectNoA11yViolations,
  expectNoPageOverflow,
} from "./helpers/harness";

// The screens that moved off the sidebar into the cockpit (ADR 0054) —
// Research / Notebook / Events / Report Season — no longer have their own nav
// button; they live as cockpit panels. Open each from the command palette and
// assert it renders without horizontal page overflow. The auto console-error
// gate (harness fixture) additionally fails on a render error / unmocked command,
// so this is the coverage that those screens still mount cleanly in their new home.
//
// `heading` is the panel's leading in-pane heading text (may differ from the dock
// tab label, e.g. tab "Notebook" vs heading "Notebooks"); `tab` is the dock tab
// label; `level` is the heading level the panel renders (PanelHeader → h1;
// SectionHeader-based global panels may lead with a lower level, e.g. the journal).
const PANELS = [
  { tab: "Research", heading: "Research", level: 1 },
  { tab: "Notebook", heading: "Notebooks", level: 1 },
  { tab: "Events", heading: "Events", level: 1 },
  { tab: "Report Season", heading: "Report Season", level: 1 },
] as const;

// The D6 compact-header rule (Decision 6) covers every cockpit-hosted panel whose
// leading heading would otherwise duplicate its dock tab — including cockpit-native
// globals like the cross-company decision journal (ADR 0071), whose tab already
// reads "Journal (all companies)". Kept separate from the former-secondary-screen
// overflow list above so the D6 sweep can grow without lengthening that loop.
const COMPACT_HEADER_PANELS = [
  ...PANELS,
  { tab: "Journal (all companies)", heading: "Decision journal", level: 3 },
] as const;

test.describe("cockpit-hosted screens lay out without overflow", () => {
  test("each former secondary screen renders as a cockpit panel without overflow", async ({ page }) => {
    await openApp(page);

    for (const { tab } of PANELS) {
      await openCockpitPanel(page, tab);
      await expectNoPageOverflow(page);
    }
  });

  // Real-browser a11y (#158): the jsdom guard covers these screens for
  // role/label/structure; only a real browser computes colors, so contrast in
  // their cockpit-hosted form is checked here — across the viewport matrix and
  // both theme projects.
  for (const { tab } of PANELS) {
    test(`the ${tab} panel has no WCAG A/AA violations`, async ({ page }) => {
      await openApp(page);
      await openCockpitPanel(page, tab);

      await expectNoA11yViolations(page, `${tab} cockpit panel`);
    });
  }

  // Compact header (ADR 0076 Decision 6, K3 double panel chrome): a cockpit-hosted
  // panel must NOT repeat the dock tab's title as a visible in-panel H1. The
  // leading header is compacted via `.ui-pane-lead-header` — the H1 stays in the
  // accessible tree (so `aria-labelledby`/heading queries still resolve, and axe
  // stays green) but is clipped to ~1px so a sighted user never reads a duplicate
  // title. The dock TAB carries the name instead. (The same screens rendered
  // full-screen in `.workspace` keep their full headers — the rule is
  // `.cockpit-pane`-scoped — asserted by the sidebar screen-visible suites.)
  for (const { tab, heading, level } of COMPACT_HEADER_PANELS) {
    test(`compacts the duplicate in-pane heading for the ${tab} panel`, async ({ page }) => {
      await openApp(page);
      await openCockpitPanel(page, tab);

      const pane = page.locator(".cockpit-pane", {
        has: page.getByRole("heading", { level, name: heading, exact: true }),
      });
      const h1 = pane.getByRole("heading", { level, name: heading, exact: true });

      // Present in the accessible tree (heading role resolves)…
      await expect(h1).toBeAttached();
      // …but visually removed: clipped to the ~1px visually-hidden box, so it is
      // not a readable duplicate of the tab title.
      const box = await h1.boundingBox();
      expect(box, "heading must still lay out (visually-hidden, not display:none)").not.toBeNull();
      expect(box!.width).toBeLessThanOrEqual(2);
      expect(box!.height).toBeLessThanOrEqual(2);

      // The dock tab is the visible carrier of the panel name.
      await expect(page.locator(".cockpit-tab-activate", { hasText: tab }).first()).toBeVisible();
    });
  }
});
