import {
  test,
  expect,
  openApp,
  journey,
  setPaneSize,
  expectNoPageOverflow,
  expectNoA11yViolations,
  type Journey,
} from "../helpers/harness";
import type { Locator, Page } from "@playwright/test";

// J4 — Report-season preparation (docs/ux-journeys.md, ADR 0074). Trigger:
// upcoming report dates across the watchlist. Path: open the Report-season
// cockpit → open a company's pre-report card (open questions, unresolved claims,
// last KPIs, evidence) → write expectations (v0.52, ADR 0071) → mark it prepared.
// Budget is ≤13 per company; this spec prepares one company, so the whole flow —
// including reaching the panel through the cockpit "New view" creator — is
// measured against that ceiling.
//
// Ceiling note (v0.52 re-baseline, owner-approved): the J4 ceiling was raised
// 10→13 when the "write expectations" step (ADR 0071) shipped. 6 of the 9
// pre-existing interactions are cockpit-panel-opening overhead (New view creator
// + palette); the expectations step is new recorded value, not a UX regression.
//
// Mock note: the browser seed pre-marks upcoming entries `prepared`, so the
// mark-prepared click cannot be observed flipping "Upcoming" → "Prepared"; the
// journey asserts the reviewed card and the prepared end-state instead. The seed
// has no recorded expectation for the occurrence, so the composer opens in the
// unfrozen "Write expectations" state.

// Journey-aware version of the shared openCockpitPanel helper: every click/fill
// runs through the wrapper so opening the panel counts toward the budget.
async function openCockpitPanelViaJourney(j: Journey, page: Page, label: string): Promise<void> {
  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await j.click(nav.getByRole("button", { name: "New view" }));
  const createModal = page.getByRole("dialog", { name: "New view" });
  await j.markModal("New view");
  await j.fill(createModal.getByLabel("View name"), `${label} test view`);
  await j.click(createModal.getByRole("button", { name: "Create view" }));
  await expect(page.getByLabel("Research cockpit")).toBeVisible();
  await expect(page.getByText("Pick a panel").first()).toBeVisible();
  await j.click(page.getByRole("button", { name: "Add panel" }));
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await j.markModal("Command palette");
  await j.fill(palette.getByLabel("Search commands"), `Open panel: ${label}`);
  await j.click(palette.getByRole("button", { name: `Open panel: ${label}`, exact: true }).first());
}

test.describe("J4 — report-season preparation", { tag: "@journey" }, () => {
  test("review a pre-report card, write expectations, and mark it prepared", async ({ page }) => {
    const j = journey(page, "J4");
    await openApp(page);
    await j.markScreen("Today");

    await openCockpitPanelViaJourney(j, page, "Report Season");
    const pane: Locator = page.locator(".cockpit-pane", { has: page.locator(".report-season-layout") });
    await expect(pane).toBeVisible();
    await j.markScreen("Report season");

    // Force the L tier so the full pre-report card mounts regardless of the
    // default (small) cockpit cell and the project viewport.
    await setPaneSize(page, { width: 900, height: 700, pane });

    // Open the first company's pre-report card and review it.
    const firstRow = pane.locator(".report-season-row").first();
    await expect(firstRow).toBeVisible();
    await j.click(firstRow);
    const card = pane.locator(".report-season-card").first();
    await expect(card).toBeVisible();
    await expect(card.locator(".report-season-card-prep")).toBeVisible();
    await expect(card.getByText(/Unresolved claims/)).toBeVisible();
    await expectNoPageOverflow(page);
    // Gate the pre-report card's a11y (ADR 0045): keeps the ExpandableRow/
    // ListRow list-semantics class from regressing on this cockpit panel.
    await expectNoA11yViolations(page, "Report-season pre-report card");

    // Write expectations (v0.52, ADR 0071): open the composer, record a stance
    // and the period the upcoming report covers, and save. This is the recorded
    // "what I expect before results land" — the done-well end state from v0.52.
    await j.click(card.getByRole("button", { name: "Write expectations" }));
    await expect(card.getByLabel("Your stance")).toBeVisible();
    await expectNoA11yViolations(page, "Report-season expectations composer");
    await j.fill(card.getByLabel("Your stance"), "Expecting revenue to keep its double-digit growth.");
    await j.selectOption(card.getByLabel("Period type"), "Q4");
    await j.click(card.getByRole("button", { name: "Save expectations" }));
    // Saved: the composer collapses to the recorded-stance summary with an edit
    // affordance (the expectation is now editable until the report lands).
    await expect(card.getByRole("button", { name: "Edit expectations" })).toBeVisible();
    await expectNoPageOverflow(page);

    // Mark prepared — the done-well end state for a near-report company.
    await j.click(card.getByRole("button", { name: "Mark prepared" }));
    await expect(pane.getByText("Prepared", { exact: true }).first()).toBeVisible();
    await expectNoPageOverflow(page);

    await j.assertBudget();
  });
});
