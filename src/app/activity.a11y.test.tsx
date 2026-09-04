import { describe, it } from "vitest";
import { axe } from "jest-axe";
import { expect, renderApp, screen, userEvent } from "../test/appWorkflowHarness";

// F3d S2 (#133, plan § D4 item 8): a11y regression guard for the Activity
// panel, separate from the screen matrix (`screens.a11y.test.tsx`) — this is
// a topbar dialog, not a screen. Same allowlist (#177): jsdom computes no
// colors (contrast is real-browser-only, `expectNoA11yViolations`).
const AXE_RULES = {
  "color-contrast": { enabled: false },
};

describe("Activity panel accessibility (#133)", () => {
  it("renders with no axe violations, seeded rows expanded and collapsed", async () => {
    const user = userEvent.setup();
    const { container } = renderApp();

    await screen.findByRole("button", { name: "Today" });
    await user.click(await screen.findByRole("button", { name: "Open activity" }));
    const dialog = await screen.findByRole("dialog", { name: "Activity" });
    await expect.poll(() => dialog.querySelectorAll(".activity-item, [data-empty-kind]").length).toBeGreaterThan(0);

    const restResults = await axe(container, { rules: AXE_RULES });
    expect(restResults.violations.map((violation) => violation.id)).toEqual([]);

    const toggle = dialog.querySelector<HTMLElement>(".expandable-row");
    if (toggle) {
      await user.click(toggle);
      const expandedResults = await axe(container, { rules: AXE_RULES });
      expect(expandedResults.violations.map((violation) => violation.id)).toEqual([]);
    }
  });
});
