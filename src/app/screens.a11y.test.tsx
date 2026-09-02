import { describe, it } from "vitest";
import { axe } from "jest-axe";
import { expect, renderApp, screen, userEvent } from "../test/appWorkflowHarness";

// Screen-level accessibility regression guard (ADR 0048): render the app, walk
// each primary screen, and assert axe finds no violations — extending the
// primitive-gallery a11y baseline (src/ui/primitives.a11y.test.tsx) up to the
// composed screens.
// Every primary screen is guarded against a11y regressions (U9). Zero
// exclusions: a new screen with violations reddens this guard.
const SCREENS = [
  "Watchlists",
  "Research",
  "Settings",
  "Inbox",
  "Companies",
  "Sources",
  "Events",
  // F4b S1: joins the Library nav in S4 (decision 1); reachable today via the
  // palette / deep link, so the guard applies now.
  "ReportSeason",
  "Transcripts",
] as const;

// Only two rules stay off, each for a reason that cannot be engineered away
// here (#177).
const AXE_RULES = {
  // jsdom computes no colors, so this rule can only ever be vacuous here. Contrast
  // is checked where it is real: `expectNoA11yViolations` (@axe-core/playwright)
  // runs the WCAG A/AA set in a real browser, across the viewport matrix and the
  // light-theme project — both palettes.
  "color-contrast": { enabled: false },
  // Research renders its detail pane as an in-workspace complementary
  // <aside>; this rule wants complementary landmarks at top level, which does not
  // fit a multi-pane desktop workspace where the rail is intentionally nested in
  // the main content. Disabled deliberately (a design point, not a defect) so the
  // guard still catches the label/role/ARIA/structure regressions that matter.
  "landmark-complementary-is-top-level": { enabled: false },
};

describe("screen accessibility", () => {
  for (const name of SCREENS) {
    it(`${name} renders with no axe violations`, async () => {
      // Land directly on the screen via the initial section: the slimmed top-nav
      // (ADR 0054) does not expose some of these as buttons — they are reached
      // via the palette or deep links — but they remain valid sections we still
      // guard. "Today" is always present in the spine and stands in as the render-ready
      // signal regardless of which section is active.
      const { container } = renderApp({ section: name });

      await screen.findByRole("button", { name: "Today" });

      const results = await axe(container, { rules: AXE_RULES });
      expect(results.violations.map((violation) => violation.id)).toEqual([]);
    });
  }

  // sol R1 finding 10: the Spółka screen (ADR 0107 — F3a's default company
  // deep-dive) was absent from this matrix entirely. Reached via a company
  // row (no standalone nav item), checked both at rest and with one hosted
  // workshop tool open — the exact state a hosted panel's own landmark would
  // surface in (the Spółka ToolHost host).
  it("Spółka renders with no axe violations, at rest and with one tool open", async () => {
    const rules = AXE_RULES;
    const user = userEvent.setup();
    const { container } = renderApp();

    await screen.findByRole("button", { name: "Today" });
    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR" }));
    await screen.findByRole("region", { name: "Company view" });

    const restResults = await axe(container, { rules });
    expect(restResults.violations.map((violation) => violation.id)).toEqual([]);

    await user.click(screen.getByRole("button", { name: "Claims" }));
    await screen.findByRole("group", { name: "Workshop tool" });

    const toolResults = await axe(container, { rules });
    expect(toolResults.violations.map((violation) => violation.id)).toEqual([]);
  });
});
