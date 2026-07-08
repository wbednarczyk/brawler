import {
  test,
  expect,
  openApp,
  journey,
  setPaneSize,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";

// J5 — Claim verification (docs/ux-journeys.md, ADR 0074). Trigger: the "claims
// to verify" queue resurfaces a due claim. Path: open the company's Claims
// workspace → read the claim beside its evidence (the matching reported fact for
// a quantitative claim) → set the verdict. Done well: a verdict is recorded
// against the evidence, nothing left silently overdue.

test.describe("J5 — claim verification", { tag: "@journey" }, () => {
  test("resolve a due claim against its evidence", async ({ page }) => {
    const j = journey(page, "J5");
    await openApp(page);

    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await expectNoA11yViolations(page, "Companies list (claim verification)");
    await j.click(page.getByRole("button", { name: "Open GPW:CDR dashboard" }));
    await expect(page.getByLabel("Research cockpit")).toBeVisible();

    // Open the Claims tab; force the pane to L so the review queue (with the
    // Delivered/Missed actions) renders regardless of the project's viewport.
    await j.click(page.getByLabel("Research cockpit").getByRole("button", { name: "Claims", exact: true }).first());
    const claimsPane = page.locator(".cockpit-pane", { has: page.locator(".company-claims-panel") });
    await expect(claimsPane).toBeVisible();
    await setPaneSize(page, { width: 900, height: 700, pane: claimsPane });

    const reviewQueue = claimsPane.getByLabel("Claims to verify");
    await expect(reviewQueue).toBeVisible();
    // The claim sits beside its evidence — the matching reported value.
    await expect(reviewQueue.getByText(/Reported value/).first()).toBeVisible();

    // Set the verdict from the queue; it is recorded against the claim.
    await j.click(reviewQueue.getByRole("button", { name: "Delivered" }).first());
    await expect(claimsPane.getByLabel("Claim verdict").first()).toHaveValue("delivered");
    await expectNoPageOverflow(page);

    j.assertBudget();
  });
});
