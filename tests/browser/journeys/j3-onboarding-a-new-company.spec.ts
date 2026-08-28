import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";

// J3 — Onboarding a new company (docs/ux-journeys.md, ADR 0074). Trigger:
// deciding to track a company. Current cross-screen path: Companies → add the
// company (registry lookup autofills a match; a genuinely new ticker is typed) →
// history backfill kicks off automatically → open the company → record the first
// note, "why I'm watching this". "Fueled" = tracked, with feed/reports/
// fundamentals surfaces present and a recorded reason.
//
// Scope notes:
//  - Add-to-watchlist is a J3 step too, but it is a separate current-portion flow
//    already covered end-to-end by watchlists.spec.ts; folding its 5-interaction
//    picker in here would push J3 past its ≤12 ceiling. It stays out of this
//    measured path by design.
//  - v0.56 ownership joined below (zero-interaction: the Basic Info ownership
//    section empty-states/populates automatically once the company is tracked).
//  - Future steps (dated): v0.53+ sector & ratios, v0.57 health
//    scores arrive automatically once tracked; they join this journey when built.

test.describe("J3 — onboarding a new company", { tag: "@journey" }, () => {
  test("add a company and record why I'm watching it", async ({ page }) => {
    const j = journey(page, "J3");
    await openApp(page);
    await j.markScreen("Today");

    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await j.markScreen("Companies");
    await expectNoA11yViolations(page, "Companies list (onboarding)");

    // Add a genuinely new company (exchange defaults to GPW). The registry lookup
    // runs on blur; a new ticker simply has no match and is added as typed.
    await j.fill(page.getByLabel("Ticker", { exact: true }), "TST");
    await j.fill(page.getByLabel("Name", { exact: true }), "Test Co S.A.");
    await j.click(page.getByRole("button", { name: "Add", exact: true }));

    // Tracked: the new company is listed (backfill kicks off automatically — not a
    // user interaction).
    const list = page.getByLabel("Companies list");
    await expect(list.getByLabel("Open GPW:TST")).toBeVisible();

    // Open the new company's workspace and record the first note. F3a S3 (ADR
    // 0107 decision 5): opening a company now lands the Spółka screen directly
    // (the "Research cockpit" it used to open is retired outright, ADR 0108).
    await j.click(list.getByLabel("Open GPW:TST"));
    await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
    await j.markScreen("Company workspace");
    await expectNoPageOverflow(page);

    // v0.56 ownership (ADR 0072): the Basic Info ownership section shows up for
    // a freshly tracked company — the empty state invites the report backfill;
    // population is fully automatic afterwards. F3a S3 (ADR 0107): ownership
    // now lives behind the Spółka screen's "Ownership" workshop tool (the
    // pre-freeze curated dashboard showed Basic Info without an extra click —
    // one added interaction, folded into the budget below).
    await j.click(page.getByRole("button", { name: "Ownership", exact: true }));
    const ownershipSection = page.locator(".ownership-section");
    await expect(ownershipSection).toBeVisible();
    await expect(
      ownershipSection.getByText("No ownership disclosures yet", { exact: false }),
    ).toBeVisible();

    // "Notebook" is the Spółka workshop bar's own destination button (F3a
    // S2/S3, ADR 0107; noun label per ADR 0104 dec. 3 amendment) — no pane
    // forcing (never force pane sizes on a journey): the notebook tool's "New
    // note" affordance is reachable at every density tier.
    await j.click(page.getByRole("button", { name: "Notebook", exact: true }));
    const notebookPane = page.locator(".spolka-layout");
    await expect(notebookPane.locator(".notebook-panel")).toBeVisible();
    const notebook = notebookPane.getByLabel("Company notebook");
    await j.click(notebook.getByRole("button", { name: "New note" }));
    await j.fill(notebook.getByLabel("Notebook note title"), "Why I'm watching");
    await j.fill(
      notebook.getByLabel("Notebook note body"),
      "Tracking for its growth runway; revisit after the next report.",
    );
    await j.click(notebook.getByRole("button", { name: "Save" }));
    await expect(notebook.getByLabel("Select notebook entry: Why I'm watching")).toHaveCount(1);
    await expectNoPageOverflow(page);

    await j.assertBudget();
  });
});
