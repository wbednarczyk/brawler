import { describe, it } from "vitest";
import { axe } from "jest-axe";
import { expect, renderApp, screen } from "../test/appWorkflowHarness";

// Screen-level accessibility regression guard (ADR 0048): render the app, walk
// each primary screen, and assert axe finds no violations — extending the
// primitive-gallery a11y baseline (src/ui/primitives.a11y.test.tsx) up to the
// composed screens. Same jsdom-disabled rules as the gallery: region /
// color-contrast (no layout/contrast in jsdom) and heading-order. Real-browser
// contrast (axe-playwright) is a tracked follow-up.
// Every primary screen is guarded against a11y regressions (U9: the previously
// excluded Inbox/Companies/Sources/Events were remediated — aria-allowed-role and
// nested-interactive fixed at the DOM/ARIA level, not by disabling rules). Zero
// exclusions: a new screen with violations reddens this guard.
const SCREENS = [
  "Watchlists",
  "Notebooks",
  "Research",
  "Settings",
  "Cockpit",
  "Inbox",
  "Companies",
  "Sources",
  "Events",
] as const;

const AXE_RULES = {
  region: { enabled: false },
  "color-contrast": { enabled: false },
  "heading-order": { enabled: false },
  // Notebooks/Research render their detail pane as an in-workspace complementary
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
      // (ADR 0053 phase 6) no longer exposes some of these as buttons (they are
      // Cockpit panels now, and Cockpit itself has no standalone nav button per
      // ADR 0057 decision 5), but they remain valid sections we still guard.
      // "Today" is always present in the spine and stands in as the render-ready
      // signal regardless of which section is active.
      const { container } = renderApp({ section: name });

      await screen.findByRole("button", { name: "Today" });

      const results = await axe(container, { rules: AXE_RULES });
      expect(results.violations.map((violation) => violation.id)).toEqual([]);
    });
  }
});
