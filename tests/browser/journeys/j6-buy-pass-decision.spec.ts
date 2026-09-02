import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";

// J6 — Buy / pass decision (docs/ux-journeys.md, ADR 0074, ADR 0107). Trigger:
// research maturity or a price condition. F3a redefinition (plan § Lista
// zgód 5): the decision journal is reached as `Spółka → Dziennik decyzji`
// (destination noun, ADR 0104 dec. 3 amendment) — the old cockpit "Add panel"
// path is retired (ADR 0108). The Spółka core is
// always co-visible before any tool opens (KPI table, feed, price, coverage,
// recommendations — the synthesis), so this journey exercises the quality
// scorecard (an explicit workshop tool a real decision leans on) before
// recording the decision:
//  1. Open the quality tool — the scorecard evidence the decision rests on.
//  2. Record the decision (ADR 0071): switch to the decision-journal tool
//     (the workshop bar stays visible with a tool open — one click switches
//     tools), compose an entry (kind + rationale), save it, then link one
//     piece of supporting evidence (provenance). The entry becomes visible in
//     the immutable journal list.
//
// Budget: re-baselined at this redefinition's first honest measurement + 1
// (consent 5, ADR 0107) — see budgets.json.

test.describe("J6 — buy / pass decision", { tag: "@journey" }, () => {
  test("decision entry via Spółka → Dziennik decyzji", async ({ page }) => {
    const j = journey(page, "J6");
    await openApp(page);
    await j.markScreen("Today");

    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await j.markScreen("Companies");
    await expectNoA11yViolations(page, "Companies list (buy/pass decision)");
    await j.click(page.locator('[data-company-id="company_gpw_cdr"] .company-row-main'));

    const spolka = page.getByRole("region", { name: "Company view", exact: true });
    await expect(spolka).toBeVisible();
    await expect(spolka).toHaveAttribute("data-company-id", "company_gpw_cdr");
    await j.markScreen("Spółka");

    // Synthesis: the quality scorecard.
    await j.click(spolka.getByRole("group", { name: "Workshop" }).getByRole("button", { name: "Quality", exact: true }));
    const qualityTool = spolka.getByLabel("Workshop tool");
    await expect(qualityTool).toBeVisible();
    await expect(qualityTool).toHaveAttribute("data-tool", "jakosc");
    await expect(qualityTool.locator(".quality-scorecard-summary")).toBeVisible();
    await expectNoPageOverflow(page);
    // Gate the scorecard a11y (ADR 0045): keeps the quality criterion rows
    // from regressing on the Spółka workshop tool host.
    await expectNoA11yViolations(page, "Quality scorecard (Spółka workshop)");

    // Record the decision (ADR 0071): switch tools via the workshop bar —
    // stays visible whether or not a tool is open, so it's a single click.
    await j.click(
      spolka.getByRole("group", { name: "Workshop" }).getByRole("button", { name: "Decision journal", exact: true }),
    );
    const journalTool = spolka.getByLabel("Workshop tool");
    await expect(journalTool).toBeVisible();
    await expect(journalTool).toHaveAttribute("data-tool", "dziennik");
    const journal = journalTool.getByLabel("Decision journal");
    await expect(journal).toBeVisible();

    // Compose the entry: a kind (the judgment) + a Markdown rationale. The
    // decided-on date defaults to today, so it is not a separate interaction.
    await j.click(journal.getByRole("button", { name: "New entry" }));
    await j.selectOption(journal.getByLabel("Decision kind"), "pass");
    await j.fill(
      journal.getByLabel("Decision rationale"),
      "Valuation is stretched for the current growth; passing for now, will revisit after H1.",
    );
    await expectNoA11yViolations(page, "Decision journal composer (Spółka)");
    await j.click(journal.getByRole("button", { name: "Save" }));

    // Recorded: the entry is now in the immutable journal list and auto-
    // selected, so its evidence picker is open.
    const entryRow = journal.getByLabel(/Select decision entry: Pass/);
    await expect(entryRow).toHaveCount(1);

    // Provenance: link one piece of supporting evidence from the company
    // timeline.
    const linkButtons = journal.getByRole("button", { name: /^(Link to decision|Powiąż z decyzją):/ });
    await expect(linkButtons.first()).toBeVisible();
    const linkCountBefore = await linkButtons.count();
    await j.click(linkButtons.first());
    // The linked item drops its "Link" affordance, so the candidate count falls.
    await expect(linkButtons).toHaveCount(linkCountBefore - 1);
    await expectNoPageOverflow(page);

    await j.assertBudget();
  });
});
