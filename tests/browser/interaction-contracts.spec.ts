import { test, expect } from "./helpers/harness";
import {
  expectPrimaryActionCount,
  expectActionBeforeScroll,
  expectFocusOrder,
  expectNamedIconActions,
  expectNextStepVisible,
} from "./helpers/interactionContracts";

// Focused contract tests for the ADR 0081 Q4 discoverability/interaction-
// hierarchy helpers, exercised against small self-contained fixtures
// (page.setContent) rather than a real app screen, so the helper contract is
// verified in isolation from any one journey. J1/J2 adopt the same helpers
// against the real decision surfaces (tests/browser/journeys/).

test.describe("interactionContracts helpers", { tag: "@journey" }, () => {
  test("expectPrimaryActionCount fails a surface with two marked primary actions", async ({ page }) => {
    await page.setContent(`
      <section aria-label="surface">
        <button data-ux-primary-action="true">Save</button>
        <button data-ux-primary-action="true">Also save</button>
      </section>
    `);
    const surface = page.getByLabel("surface");
    await expect(expectPrimaryActionCount(surface, { max: 1 })).rejects.toThrow();
  });

  test("expectPrimaryActionCount fails a surface whose primary-action marker is missing (metadata dropped)", async ({
    page,
  }) => {
    // Mirrors J1/J2: before data-ux-primary-action is added to the real
    // decision surface's button, the contract must redden, not silently pass.
    await page.setContent(`
      <section aria-label="surface">
        <button>Review</button>
      </section>
    `);
    await expect(expectPrimaryActionCount(page.getByLabel("surface"), { max: 1 })).rejects.toThrow();
  });

  test("expectPrimaryActionCount passes a surface with exactly one marked primary action", async ({ page }) => {
    await page.setContent(`
      <section aria-label="surface">
        <button data-ux-primary-action="true">Save</button>
        <button>Cancel</button>
      </section>
    `);
    await expectPrimaryActionCount(page.getByLabel("surface"), { max: 1 });
  });

  test("expectPrimaryActionCount rejects a multi-primary exemption with no reason", async ({ page }) => {
    await page.setContent(`
      <section aria-label="surface">
        <button data-ux-primary-action="true">A</button>
        <button data-ux-primary-action="true">B</button>
      </section>
    `);
    await expect(expectPrimaryActionCount(page.getByLabel("surface"), { max: 2 })).rejects.toThrow(
      /non-empty `reason`/,
    );
  });

  test("expectActionBeforeScroll fails when the primary action starts below its scrollport", async ({ page }) => {
    await page.setContent(`
      <div aria-label="scrollport" style="position:relative; width:200px; height:100px; overflow:auto;">
        <div style="height:400px;"></div>
        <button aria-label="primary" style="position:relative;">Continue</button>
      </div>
    `);
    const scrollOwner = page.getByLabel("scrollport");
    const action = page.getByLabel("primary");
    await expect(expectActionBeforeScroll(action, scrollOwner)).rejects.toThrow();
  });

  test("expectActionBeforeScroll passes when the primary action is already visible", async ({ page }) => {
    await page.setContent(`
      <div aria-label="scrollport" style="position:relative; width:200px; height:200px; overflow:auto;">
        <button aria-label="primary">Continue</button>
      </div>
    `);
    await expectActionBeforeScroll(page.getByLabel("primary"), page.getByLabel("scrollport"));
  });

  test("expectNamedIconActions fails when a primitive icon button has no accessible name", async ({ page }) => {
    await page.setContent(`
      <section aria-label="surface">
        <button data-ui-button-variant="icon"><svg></svg></button>
      </section>
    `);
    await expect(expectNamedIconActions(page.getByLabel("surface"))).rejects.toThrow();
  });

  test("expectNamedIconActions passes when the icon button carries an aria-label", async ({ page }) => {
    await page.setContent(`
      <section aria-label="surface">
        <button data-ui-button-variant="icon" aria-label="Delete row"><svg></svg></button>
      </section>
    `);
    await expectNamedIconActions(page.getByLabel("surface"));
  });

  test("expectFocusOrder fails when the declared Tab sequence is reversed", async ({ page }) => {
    await page.setContent(`
      <button aria-label="first">First</button>
      <button aria-label="second">Second</button>
    `);
    const first = page.getByLabel("first");
    const second = page.getByLabel("second");
    // Declare the sequence backwards relative to DOM/Tab order.
    await expect(expectFocusOrder(page, [second, first])).rejects.toThrow();
  });

  test("expectFocusOrder passes when the declared sequence matches Tab order", async ({ page }) => {
    await page.setContent(`
      <button aria-label="first">First</button>
      <button aria-label="second">Second</button>
    `);
    await expectFocusOrder(page, [page.getByLabel("first"), page.getByLabel("second")]);
  });

  test("expectNextStepVisible fails when success hides the contracted next step", async ({ page }) => {
    await page.setContent(`<p aria-label="next step" style="display:none;">Now review the fact.</p>`);
    await expect(expectNextStepVisible(page.getByLabel("next step"))).rejects.toThrow();
  });

  test("expectNextStepVisible passes when the next step stays visible", async ({ page }) => {
    await page.setContent(`<p aria-label="next step">Now review the fact.</p>`);
    await expectNextStepVisible(page.getByLabel("next step"));
  });
});
