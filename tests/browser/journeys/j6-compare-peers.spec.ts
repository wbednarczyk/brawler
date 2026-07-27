import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";

// J6 — Buy / pass decision, the peer-comparison entry point (v0.61, ADR 0089).
// The buy/pass synthesis now includes relative position: instead of assembling
// peer context by hand in a spreadsheet, the user opens Compare (Porównaj),
// picks a zestaw spółek, and reads the same canonical KPIs aligned across the
// companies plus the comparative-valuation L1 section — all in-app.
//
// This measures the "compare peers on a canonical KPI" flow named in the
// experience contract (plan §A3, §12). It is a distinct J6 sub-flow with its
// own friction budget (J6-compare) so it does not perturb the recording-flow
// floor in j6-buy-pass-decision.spec.ts. The seeded pair is GPW:CDR (PLN,
// populated valuation) + GPW:CBF (EUR, thin peer set) — the same fixtures the
// A3/A7/B3 layout spec drives, reused, not duplicated.

test.describe("J6 — compare peers on a canonical KPI", { tag: "@journey" }, () => {
  test("open Porównaj, pick a zestaw, read the Profil comparison + valuation", async ({ page }) => {
    const j = journey(page, "J6-compare");
    await openApp(page);
    await j.markScreen("Today");

    // Entry: the restored sidebar Porównaj entry (under Dashboard).
    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Compare" }));
    await expect(page.getByRole("heading", { name: "Compare", exact: true })).toBeVisible();
    await j.markScreen("Compare");

    // The quiet invite before a zestaw exists — no CTA-spam (frame 1).
    await expect(page.getByText(/Pick at least 2 companies with confirmed data/)).toBeVisible();

    // Select the zestaw spółek — two tracked companies. §A7: the comparison
    // computes reactively on selection, there is no submit button to press.
    await j.selectOption(page.getByLabel(/Add company/), "company_gpw_cdr");
    await j.selectOption(page.getByLabel(/Add company/), "company_gpw_cbf");

    // Synthesis: the Profil pivot — canonical KPIs down, companies across, a
    // Różnica (Difference) column for the pair, evidence links, the EUR→PLN FX
    // chip on the EUR company.
    const table = page.getByRole("table");
    await expect(table).toBeVisible();
    await expect(table.getByRole("columnheader", { name: "KPI" })).toBeVisible();
    await expect(table.getByRole("columnheader", { name: "Difference" })).toBeVisible();
    await expect(table.getByRole("columnheader", { name: /GPW:CDR/ })).toBeVisible();
    await expect(table.getByRole("columnheader", { name: /GPW:CBF/ })).toBeVisible();
    await expect(table.getByText("EUR→PLN")).toBeVisible();
    await expect(
      table.getByRole("button", { name: /Open evidence for GPW:CDR/ }).first(),
    ).toBeVisible();
    await expectNoPageOverflow(page);

    // Relative valuation (§B3): the comparative-valuation L1 section — percentile
    // chips with N, football field, confidence — is part of the same synthesis
    // (default scope = first company, seeded populated). The section is always
    // present; a thin peer set names its threshold rather than disappearing.
    const valuation = page.getByRole("region", { name: "Valuation" });
    await expect(valuation).toBeVisible();
    await expect(valuation.getByText("Comparative valuation L1")).toBeVisible();

    // Gate the Compare synthesis a11y (ADR 0045): the pivot table, FX chip, and
    // valuation chips must stay accessible as the screen evolves.
    await expectNoA11yViolations(page, "Compare Profil + valuation (buy/pass synthesis)");

    await j.assertBudget();
  });
});
