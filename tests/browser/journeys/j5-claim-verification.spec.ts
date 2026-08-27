import {
  test,
  expect,
  openApp,
  journey,
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
    await j.markScreen("Today");

    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await j.markScreen("Companies");
    await expectNoA11yViolations(page, "Companies list (claim verification)");
    // F3a S3 (ADR 0107 decision 5): opening a company now lands the Spółka
    // screen directly.
    await j.click(page.getByRole("button", { name: "Open GPW:CDR" }));
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
    await j.markScreen("Company workspace");

    // "Claims" is the Spółka workshop bar's own destination button (F3a S2/S3,
    // ADR 0107; noun label per ADR 0104 dec. 3 amendment) — no pane forcing
    // (never force pane sizes on a journey): the review queue (with the
    // Delivered/Missed actions) renders at the project's own viewport,
    // comfortably above the L-tier threshold.
    await j.click(page.getByRole("button", { name: "Claims", exact: true }));
    const claimsPane = page.locator(".spolka-layout");
    await expect(claimsPane.locator(".company-claims-panel")).toBeVisible();

    const reviewQueue = claimsPane.getByLabel("Claims to verify");
    await expect(reviewQueue).toBeVisible();
    // The claim sits beside its evidence — the matching reported value.
    await expect(reviewQueue.getByText(/Reported value/).first()).toBeVisible();

    // Set the verdict from the queue; it is recorded against the claim.
    await j.click(reviewQueue.getByRole("button", { name: "Delivered" }).first());
    await expect(claimsPane.getByLabel("Claim verdict").first()).toHaveValue("delivered");
    await expectNoPageOverflow(page);

    await j.assertBudget();
  });
});
