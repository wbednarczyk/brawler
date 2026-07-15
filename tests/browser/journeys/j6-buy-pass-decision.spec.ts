import {
  test,
  expect,
  openApp,
  journey,
  setPaneSize,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";

// J6 — Buy / pass decision (docs/ux-journeys.md, ADR 0074). Trigger: research
// maturity or a price condition. Two parts, both exercised here:
//  1. Assemble the synthesis in the company workspace — the fundamentals matrix
//     and the quality scorecard — the evidence the decision rests on.
//  2. Record the decision (v0.52 judgment capture, ADR 0071): open the company's
//     decision-journal panel, compose an entry (kind + rationale), save it, then
//     link one piece of supporting evidence (provenance). The entry becomes
//     visible in the immutable journal list.
//
// The ≤15 recording-flow ceiling in ux-journeys.md has room; the budgets.json
// floor tracks the measured count of this real recording path.

test.describe("J6 — buy / pass decision", { tag: "@journey" }, () => {
  test("review the synthesis then record the decision in the journal", async ({ page }) => {
    const j = journey(page, "J6");
    await openApp(page);
    await j.markScreen("Today");

    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await j.markScreen("Companies");
    await expectNoA11yViolations(page, "Companies list (buy/pass decision)");
    await j.click(page.getByRole("button", { name: "Open GPW:CDR dashboard" }));
    const cockpit = page.getByLabel("Research cockpit");
    await expect(cockpit).toBeVisible();
    await j.markScreen("Company workspace");

    // Synthesis part 1: the fundamentals matrix (facts) renders directly.
    await expect(page.getByLabel("Financial facts matrix")).toBeVisible();
    await expectNoPageOverflow(page);

    // Synthesis part 2: the quality scorecard (quality score + criteria).
    await j.click(cockpit.getByRole("button", { name: /Quality/ }).first());
    const qualityPane = page.locator(".cockpit-pane", { has: page.locator(".quality-panel") });
    await expect(qualityPane).toBeVisible();
    await expect(qualityPane.locator(".quality-scorecard-summary")).toBeVisible();
    await expectNoPageOverflow(page);
    // The dashboard tabs several panels into shared groups, so an INACTIVE tab is
    // on screen — assert it, so the a11y probe below genuinely gates the Dockview
    // tab-button case (card 02e991e #4), not just the quality rows.
    await expect(page.locator('.cockpit-tab-activate[aria-pressed="false"]').first()).toBeVisible();
    // Gate the scorecard + multi-tab cockpit a11y (ADR 0045): keeps the quality
    // criterion rows and the inactive Dockview tab buttons from regressing.
    await expectNoA11yViolations(page, "Quality scorecard + cockpit tabs");

    // Record the decision (ADR 0071): the decision-journal panel isn't in the
    // curated dashboard, so add it via the same "Add panel" → palette affordance
    // the cockpit exposes everywhere.
    await j.click(cockpit.getByRole("button", { name: "Add panel" }));
    const palette = page.getByRole("dialog", { name: "Command palette" });
    await j.markModal("Command palette");
    await j.fill(palette.getByLabel("Search commands"), "Open panel: Decision journal");
    await j.click(
      palette.getByRole("button", { name: "Open panel: Decision journal", exact: true }).first(),
    );
    const journalPane = page.locator(".cockpit-pane", { has: page.locator(".decision-journal-panel") });
    await expect(journalPane).toBeVisible();
    // Force a wide/tall tier so the composer + list + evidence detail all mount
    // regardless of the default cockpit cell and the project viewport.
    await setPaneSize(page, { width: 900, height: 700, pane: journalPane });
    const journal = journalPane.getByLabel("Decision journal");

    // Compose the entry: a kind (the judgment) + a Markdown rationale. The
    // decided-on date defaults to today, so it is not a separate interaction.
    await j.click(journal.getByRole("button", { name: "New entry" }));
    await j.selectOption(journal.getByLabel("Decision kind"), "pass");
    await j.fill(
      journal.getByLabel("Decision rationale"),
      "Valuation is stretched for the current growth; passing for now, will revisit after H1.",
    );
    await expectNoA11yViolations(page, "Decision journal composer");
    await j.click(journal.getByRole("button", { name: "Save" }));

    // Recorded: the entry is now in the immutable journal list and auto-selected,
    // so its evidence picker is open.
    const entryRow = journal.getByLabel(/Select decision entry: Pass/);
    await expect(entryRow).toHaveCount(1);

    // Provenance: link one piece of supporting evidence from the company timeline.
    const linkButtons = journal.getByRole("button", { name: "Link evidence" });
    await expect(linkButtons.first()).toBeVisible();
    const linkCountBefore = await linkButtons.count();
    await j.click(linkButtons.first());
    // The linked item drops its "Link" affordance, so the candidate count falls.
    await expect(linkButtons).toHaveCount(linkCountBefore - 1);
    await expectNoPageOverflow(page);

    await j.assertBudget();
  });
});
