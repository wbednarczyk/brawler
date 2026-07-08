import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";

// J6 — Buy / pass decision (docs/ux-journeys.md, ADR 0074). Trigger: research
// maturity or a price condition. Current portion: assemble the synthesis in the
// company workspace — the fundamentals matrix and the quality scorecard — the
// evidence the decision rests on.
//
// Future step (dated): the recording flow itself — writing the decision into the
// decision journal with rationale and evidence links — is v0.51 judgment capture
// (ADR 0071, docs/plans/v0.51-judgment-capture.md). It is not built yet, so this
// journey covers only reaching and reading the synthesis; the ≤15 recording
// budget in ux-journeys.md is the ceiling for when the journal ships. The
// budgets.json floor here tracks the current (short) synthesis-review path.

test.describe("J6 — buy / pass decision", { tag: "@journey" }, () => {
  test("review the synthesis a buy/pass decision rests on", async ({ page }) => {
    const j = journey(page, "J6");
    await openApp(page);

    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await expectNoA11yViolations(page, "Companies list (buy/pass decision)");
    await j.click(page.getByRole("button", { name: "Open GPW:CDR dashboard" }));
    const cockpit = page.getByLabel("Research cockpit");
    await expect(cockpit).toBeVisible();

    // Synthesis part 1: the fundamentals matrix (facts) renders directly.
    await expect(page.getByLabel("Financial facts matrix")).toBeVisible();
    await expectNoPageOverflow(page);

    // Synthesis part 2: the quality scorecard (quality score + criteria).
    await j.click(cockpit.getByRole("button", { name: /Quality/ }).first());
    const qualityPane = page.locator(".cockpit-pane", { has: page.locator(".quality-panel") });
    await expect(qualityPane).toBeVisible();
    await expect(qualityPane.locator(".quality-scorecard-summary")).toBeVisible();
    await expectNoPageOverflow(page);

    j.assertBudget();
  });
});
