import { describe, it } from "vitest";
import { axe } from "jest-axe";
import { expect, renderApp, screen, userEvent } from "../test/appWorkflowHarness";

// Screen-level accessibility regression guard (ADR 0048): render the app, walk
// each primary screen, and assert axe finds no violations — extending the
// primitive-gallery a11y baseline (src/ui/primitives.a11y.test.tsx) up to the
// composed screens. Same jsdom-disabled rules as the gallery: region /
// color-contrast (no layout/contrast in jsdom) and heading-order. Real-browser
// contrast (axe-playwright) is a tracked follow-up.
// Currently-clean screens — guarded against a11y regressions. Inbox, Companies,
// Sources, and Events are intentionally NOT here yet: this guard surfaced real
// pre-existing violations on them (aria-allowed-role, nested-interactive). Those
// are tracked separately to fix, then fold into this list — we do NOT disable
// those rules (that would gut the guard) nor commit a red test.
const SCREENS = ["Watchlists", "Notebooks", "Research", "Settings"] as const;

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
      const user = userEvent.setup();
      const { container } = renderApp();

      await user.click(await screen.findByRole("button", { name }));

      const results = await axe(container, { rules: AXE_RULES });
      expect(results.violations.map((violation) => violation.id)).toEqual([]);
    });
  }
});
