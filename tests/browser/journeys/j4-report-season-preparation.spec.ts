import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
  type Journey,
} from "../helpers/harness";
import type { Page } from "@playwright/test";

// J4 — Report-season preparation (docs/ux-journeys.md, ADR 0074, ADR 0107).
// Trigger: upcoming report dates across the watchlist. F3a redefinition
// (plan § Lista zgód 5): the freeform view-creation leg ("+ New view" → "Add
// panel" → palette) is retired (ADR 0108); the journey enters the Report
// Season SCREEN directly through the global palette's `Open screen: …` entry
// (plan "Trasy powierzchni globalnych po F3a") — the screen is a full route,
// not a cockpit-hosted panel. Path: open the screen → open a company's
// pre-report card (open questions, unresolved claims, last KPIs, evidence) →
// add expectations (ADR 0071) → mark it as prepared.
//
// Budget: re-baselined at this redefinition's first honest measurement + 1
// (consent 5, ADR 0107) — see budgets.json.
//
// Mock note: the browser seed pre-marks upcoming entries `prepared`, so the
// mark-as-prepared click cannot be observed flipping "Nadchodzący" →
// "Przygotowany"; the journey asserts the reviewed card and the prepared
// end-state instead. The seed has no recorded expectation for the
// occurrence, so the composer opens in the unfrozen "Add expectations"
// state (F4b S4 label pass).

async function openScreenViaJourney(j: Journey, page: Page, label: string): Promise<void> {
  await j.press(page, "Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await j.markModal("Command palette");
  await j.fill(palette.getByLabel("Search commands"), `Open screen: ${label}`);
  await j.click(palette.getByRole("option", { name: `Open screen: ${label}`, exact: true }).first());
}

test.describe("J4 — report-season preparation", { tag: "@journey" }, () => {
  test("prep starts from Otwórz ekran: Report Season (no view creation)", async ({ page }) => {
    const j = journey(page, "J4");
    await openApp(page);
    await j.markScreen("Today");

    await openScreenViaJourney(j, page, "Report Season");
    const layout = page.locator(".report-season-layout");
    await expect(layout).toBeVisible();
    await j.markScreen("Report season");

    // Open the first company's pre-report card and review it.
    const firstRow = layout.locator(".report-season-row").first();
    await expect(firstRow).toBeVisible();
    await j.click(firstRow);
    const card = layout.locator(".report-season-card").first();
    await expect(card).toBeVisible();
    await expect(card.locator(".report-season-card-prep")).toBeVisible();
    await expect(card.getByText(/Unresolved claims/)).toBeVisible();
    await expectNoPageOverflow(page);
    // Gate the pre-report card's a11y (ADR 0045): keeps the ExpandableRow/
    // ListRow list-semantics class from regressing on this screen.
    await expectNoA11yViolations(page, "Report-season pre-report card");

    // Add expectations (v0.52, ADR 0071; F4b S4 label pass): open the
    // composer, record a stance and the period the upcoming report covers,
    // and save. This is the recorded "what I expect before results land" —
    // the done-well end state.
    await j.click(card.getByRole("button", { name: "Add expectations" }));
    await expect(card.getByLabel("Your stance")).toBeVisible();
    await expectNoA11yViolations(page, "Report-season expectations composer");
    await j.fill(card.getByLabel("Your stance"), "Expecting revenue to keep its double-digit growth.");
    await j.selectOption(card.getByLabel("Period type"), "Q4");
    await j.click(card.getByRole("button", { name: "Save", exact: true }));
    // Saved: the composer collapses to the recorded-stance summary with an
    // edit affordance (the expectation is now editable until the report lands).
    await expect(card.getByRole("button", { name: "Edit expectations" })).toBeVisible();
    await expectNoPageOverflow(page);

    // Mark as prepared — the done-well end state for a near-report company.
    await j.click(card.getByRole("button", { name: "Mark as prepared" }));
    await expect(layout.getByText("Prepared", { exact: true }).first()).toBeVisible();
    await expectNoPageOverflow(page);

    await j.assertBudget();
  });
});
