import { test, expect, expectNoA11yViolations, openApp } from "./helpers/harness";
import type { Page } from "@playwright/test";

// Clickable Sources journey against the stateful browser mock runtime
// (ADR 0048): toggling an adapter's enable switch persists through the runtime
// and the control reflects the new state on the next read. The switch is a
// visually-hidden, CSS-animated <input>, so we force-click it (the standard
// pattern for styled checkboxes) and assert on the aria-label that flips.

function navTo(page: Page, name: string) {
  return page.getByLabel("Primary navigation").getByRole("button", { name });
}

test.describe("sources", { tag: "@clickable" }, () => {
  test("toggle a source adapter off and back on", async ({ page }) => {
    await openApp(page);
    await navTo(page, "Sources").click();

    // "Bankier Company Komunikaty" is an enabled optional adapter in the mock.
    const offName = "Turn off Bankier Company Komunikaty";
    const onName = "Turn on Bankier Company Komunikaty";

    await page.getByRole("switch", { name: offName }).click({ force: true });

    // Disabling flips the control to "turn on" (stateful set_source_adapter_enabled
    // reflected into list_source_adapters).
    await expect(page.getByRole("switch", { name: onName })).toHaveCount(1);

    // ...and re-enabling flips it back.
    await page.getByRole("switch", { name: onName }).click({ force: true });
    await expect(page.getByRole("switch", { name: offName })).toHaveCount(1);
  });

  // Real-browser a11y (#158): the jsdom guard (src/app/screens.a11y.test.tsx)
  // covers Sources for role/label/structure, but only a real browser computes
  // colors — so contrast on the adapter switches and status chips is checked
  // here, across the viewport matrix and both themes.
  test("Sources has no WCAG A/AA violations", async ({ page }) => {
    await openApp(page);
    await navTo(page, "Sources").click();
    await expect(page.getByRole("switch").first()).toBeVisible();

    await expectNoA11yViolations(page, "Sources");
  });
});
