import {
  test,
  expect,
  openApp,
  journey,
  setPaneSize,
  expectNoHorizontalOverflow,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";

// J2 — A company published a report (docs/ux-journeys.md, ADR 0074). The full
// cross-screen path a user walks when a periodic report lands: open the report →
// extract & review its KPIs → confirm a fact → open the company workspace → see
// the confirmed fact in the fundamentals matrix → resolve a due management claim
// → capture the judgment as a note. This spec absorbs the three legacy
// journeys.spec.ts tests (extract→review→confirm, open-company-without-moving,
// confirmed-KPI-in-matrix) into one measured journey; the old file is deleted.
//
// Future step (dated): v0.51 expectation-vs-actual review (ADR 0071 judgment
// capture) inserts between "review KPIs" and "resolve claims"; it joins this
// journey and its budget when it ships.

test.describe("J2 — a company published a report", { tag: "@journey" }, () => {
  test("open report → confirm KPI → workspace → resolve claim → note", async ({ page }) => {
    const j = journey(page, "J2");
    await openApp(page);

    // Open the Inbox and select the CD PROJEKT report; its detail rail offers the
    // KPI extractor.
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Inbox" }));
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await j.click(page.getByLabel(/Select feed item: CD PROJEKT/).first());
    const launcher = page.getByLabel("AI KPI extraction");
    await expect(launcher).toBeVisible();
    await expectNoA11yViolations(page, "Inbox with report selected");

    // The heavy extraction flow opens in a modal.
    await j.click(launcher.getByRole("button", { name: "Extract KPIs" }));
    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await expect(dialog).toBeVisible();
    await expectNoHorizontalOverflow(dialog);
    await expectNoA11yViolations(page, "KPI extraction dialog");

    // Run the extraction; status and proposals appear inside the modal.
    await j.click(dialog.getByRole("button", { name: /Extract from attachment/ }));
    await expect(dialog.getByText("succeeded")).toBeVisible();
    await expect(dialog.getByText("Revenue")).toBeVisible();

    // Confirm the first known KPI; it transitions to confirmed in place.
    await j.click(dialog.getByRole("button", { name: "Confirm" }).first());
    await expect(dialog.getByText("confirmed").first()).toBeVisible();
    await j.click(dialog.getByRole("button", { name: "Close dialog" }));

    // Open the company — the cockpit dashboard lands scoped to it (ADR 0057),
    // visible and in view without a manual scroll, and the app shell must not have
    // scrolled (the "moves the whole app" regression).
    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await expectNoA11yViolations(page, "Companies list");
    await j.click(page.locator('[data-company-id="company_gpw_cdr"] .company-row-main'));

    const workspace = page.getByRole("region", { name: "Research cockpit", exact: true });
    await expect(workspace).toBeVisible();
    await expect(workspace).toBeInViewport();
    const pageScroll = await page.evaluate(() => ({ y: window.scrollY, top: document.documentElement.scrollTop }));
    expect(pageScroll.y).toBe(0);
    expect(pageScroll.top).toBe(0);
    await expectNoPageOverflow(page);

    // The confirmed fact is wired through the read model: the fundamentals matrix
    // renders directly on the dashboard (no tab).
    const fundamentals = page.getByLabel("Company fundamentals");
    await expect(fundamentals).toBeVisible();
    await expect(page.getByLabel("Financial facts matrix")).toBeVisible();
    await expectNoHorizontalOverflow(fundamentals);

    // Open a fact; its danger "Remove" button renders as a full text button (the
    // icon-only clipping regression).
    await j.click(page.getByRole("button", { name: /^Revenue, / }).first());
    const factDetail = page.getByLabel("Financial fact detail");
    await expect(factDetail).toBeVisible();
    const remove = await factDetail
      .getByRole("button", { name: "Remove" })
      .evaluate((el) => ({ clipped: el.scrollWidth > el.clientWidth + 1, width: el.clientWidth }));
    expect(remove.clipped, "Remove button label is clipped").toBe(false);
    expect(remove.width, "Remove button is squeezed to icon width").toBeGreaterThan(56);

    // Resolve a due management claim from the workspace Claims tab. Force the pane
    // to the L tier so the review queue is shown regardless of the project's
    // viewport (the tier switch is CSS-only container queries).
    await j.click(page.getByLabel("Research cockpit").getByRole("button", { name: "Claims", exact: true }).first());
    const claimsPane = page.locator(".cockpit-pane", { has: page.locator(".company-claims-panel") });
    await expect(claimsPane).toBeVisible();
    await setPaneSize(page, { width: 900, height: 700, pane: claimsPane });
    const reviewQueue = claimsPane.getByLabel("Claims to verify");
    await expect(reviewQueue).toBeVisible();
    await expect(reviewQueue.getByText(/Reported value/).first()).toBeVisible();
    await j.click(reviewQueue.getByRole("button", { name: "Delivered" }).first());
    // The verdict is recorded: the claim's row verdict now reads delivered.
    await expect(claimsPane.getByLabel("Claim verdict").first()).toHaveValue("delivered");
    await expectNoPageOverflow(page);

    // Capture the judgment as a note in the company Notebook.
    await j.click(page.getByLabel("Research cockpit").getByRole("button", { name: "Notebook", exact: true }).first());
    const notebookPane = page.locator(".cockpit-pane", { has: page.locator(".notebook-panel") });
    await expect(notebookPane).toBeVisible();
    await setPaneSize(page, { width: 900, height: 700, pane: notebookPane });
    const notebook = notebookPane.getByLabel("Company notebook");
    await j.click(notebook.getByRole("button", { name: "New note" }));
    await j.fill(notebook.getByLabel("Notebook note title"), "Q3 confirmed, guidance claim delivered");
    await j.fill(
      notebook.getByLabel("Notebook note body"),
      "Revenue confirmed from the new report; the revenue claim came in delivered.",
    );
    await j.click(notebook.getByRole("button", { name: "Save" }));
    await expect(
      notebook.getByLabel("Select notebook entry: Q3 confirmed, guidance claim delivered"),
    ).toHaveCount(1);
    await expectNoPageOverflow(page);

    j.assertBudget();
  });
});
