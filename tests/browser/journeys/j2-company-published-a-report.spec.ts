import {
  test,
  expect,
  openApp,
  journey,
  expectNoHorizontalOverflow,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";
import {
  expectPrimaryActionCount,
  expectActionBeforeScroll,
  expectNextStepVisible,
} from "../helpers/interactionContracts";
import { holdInvocation, primeMockScenario, releaseInvocation } from "../helpers/mockRuntime";

// `recentFeedItems`/`companies` are fetched exactly once at app bootstrap (not
// re-fetched on in-app navigation), so overlay content must be seeded BEFORE
// boot via `primeMockScenario` (helpers/mockRuntime) — a post-`openApp` reset
// races with the bootstrap fetch.

// J2 — A company published a report (docs/ux-journeys.md, ADR 0074). The full
// cross-screen path a user walks when a periodic report lands: open the report →
// open the company workspace → read the deterministically-extracted facts in the
// fundamentals matrix → resolve a due management claim → capture the judgment as
// a note.
//
// ADR 0084 (clean cut) reshaped this journey. The AI KPI-extraction launcher and
// its review modal — J2's former opening act AND its former contracted primary
// action — are removed with the in-app AI layer, together with the staging
// proposals they wrote. What remains is the journey's actual substance: the
// facts (now written by the deterministic extraction tiers, ADR 0061), the
// manual management-claim verdict (explicitly kept by ADR 0084 decision 2), and
// the note that records the judgment.
//
// ADR 0081 Q4 primary action: re-pointed to the company Notebook panel's
// "New note". It is the surviving surface on this journey with an unambiguous
// SINGLE primary action, and it is the step that produces the journey's durable
// artifact. The claims review queue is this journey's decision moment, but its
// Delivered/Missed pair is a deliberate binary — two peer primaries — which
// would need an owner-approved multi-primary exemption reason in the surface's
// experience contract (ui-authoring § interaction-hierarchy contracts). That is
// a design decision, not an implementation one, so it is escalated rather than
// invented here.
//
// Q3 note (ADR 0081): the Claims/Notebook panes no longer force a 900×700 pane
// size — a user journey takes the real disclosure path at the current project
// viewport (density tests may still force a pane; this journey must not). If a
// project's real path genuinely needs different steps, that project's measured
// count is recorded in budgets.json's byProject, not hidden by forcing.

test.describe("J2 — a company published a report", { tag: "@journey" }, () => {
  test("open report → workspace → read facts → resolve claim → note", async ({ page }) => {
    const j = journey(page, "J2");
    await openApp(page);
    await j.markScreen("Today");

    // Open the Inbox and select the CD PROJEKT report.
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Inbox" }));
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await j.markScreen("Inbox");
    await j.click(page.getByLabel(/Select feed item: CD PROJEKT/).first());
    await expectNoA11yViolations(page, "Inbox with report selected");

    // ADR 0084 clean cut: no AI surface is reachable from the report's detail
    // rail — neither a generation affordance nor a read-only viewer.
    await expect(page.getByLabel("AI KPI extraction")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Extract KPIs" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "View analysis" })).toHaveCount(0);
    await expectNoPageOverflow(page);

    // Open the company — the Spółka screen lands scoped to it directly (F3a
    // S1, ADR 0107; the freeform cockpit dashboard is retired outright, ADR
    // 0108), visible and in view without a manual scroll, and the app shell
    // must not have scrolled (the "moves the whole app" regression).
    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await j.markScreen("Companies");
    await expectNoA11yViolations(page, "Companies list");
    await j.click(page.locator('[data-company-id="company_gpw_cdr"] .company-row-main'));

    const workspace = page.getByRole("region", { name: "Company view", exact: true });
    await expect(workspace).toBeVisible();
    await expect(workspace).toBeInViewport();
    await j.markScreen("Company workspace");
    // The Spółka root's semantic company marker (ADR 0081 Q3 observation point)
    // is the context this journey must not silently lose across the Claims/
    // Notebook tool switches below — it is the company the facts, the resolved
    // claim, and the captured note must all end up attached to.
    await j.preserveContext(await workspace.getAttribute("data-company-id"));
    const pageScroll = await page.evaluate(() => ({ y: window.scrollY, top: document.documentElement.scrollTop }));
    expect(pageScroll.y).toBe(0);
    expect(pageScroll.top).toBe(0);
    await expectNoPageOverflow(page);

    // The deterministically-extracted facts are wired through the read model:
    // the KPI core card's own "Fundamentals" button raises the facts matrix
    // in the fundamentals tool (owner dogfooding v0.74: destination buttons
    // are nouns, ADR 0104 dec. 3 amendment). Scoped to the KPI card — the
    // workshop bar now carries an IDENTICALLY-labelled entry to the same
    // tool (wave 2, item 1), a second real entry point this journey doesn't
    // need to exercise here.
    await j.click(workspace.getByLabel("Annual KPI table").getByRole("button", { name: "Fundamentals" }));
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

    // The fact detail is a Modal since card #307 — close it before moving on,
    // or its overlay intercepts every later click in the journey. A real user
    // presses Escape here — count it (sol R1 finding 5: this bypassed `j.press`,
    // undercounting the journey's true interaction total).
    await j.press(page, "Escape");
    await expect(factDetail).toBeHidden();

    // Resolve a due management claim: the workshop bar's "Claims" button stays
    // visible whether or not a tool is open (unlike the glance bar's own
    // "Claims counter" drill target, hidden behind the open Fundamentals
    // tool) and raises the claims tool — at the real pane size the current
    // project viewport gives it, no forced 900×700 shortcut (Q3, ADR 0081).
    // The manual claims path survives ADR 0084.
    await j.click(page.getByRole("group", { name: "Workshop" }).getByRole("button", { name: "Claims", exact: true }));
    const claimsPane = page.getByRole("group", { name: "Workshop tool" });
    await expect(claimsPane).toBeVisible();
    await expect(claimsPane).toHaveAttribute("data-tool", "tezy");
    await j.preserveContext(await workspace.getAttribute("data-company-id"));
    const reviewQueue = claimsPane.getByLabel("Claims to verify");
    // At the short-height density tier (ADR 0076 D6, pane < 480px tall) the
    // full panel folds behind "Show all claims"; a real user at that viewport
    // takes that disclosure step instead of the review queue being pre-forced
    // open. Only present/clicked when the tier actually hides it.
    const shortToggle = claimsPane.getByRole("button", { name: "Show all claims" });
    if (await shortToggle.isVisible()) {
      await j.click(shortToggle);
    }
    await expect(reviewQueue).toBeVisible();
    await expect(reviewQueue.getByText(/Reported value/).first()).toBeVisible();
    await j.click(reviewQueue.getByRole("button", { name: "Delivered" }).first());
    // The verdict is recorded: the claim's row verdict now reads delivered, and
    // the contracted next step stays visible rather than being hidden by success.
    await expectNextStepVisible(claimsPane.getByLabel("Claim verdict").first());
    await expect(claimsPane.getByLabel("Claim verdict").first()).toHaveValue("delivered");
    await expectNoPageOverflow(page);

    // Capture the judgment as a note in the company Notebook: the workshop
    // bar's "Notebook" button raises the notebook tool — again at the real
    // pane size (no forced 900×700 shortcut).
    await j.click(page.getByRole("group", { name: "Workshop" }).getByRole("button", { name: "Notebook", exact: true }));
    const notebookPane = page.getByRole("group", { name: "Workshop tool" });
    await expect(notebookPane).toBeVisible();
    await expect(notebookPane).toHaveAttribute("data-tool", "notatnik");
    await j.preserveContext(await workspace.getAttribute("data-company-id"));
    const notebook = notebookPane.getByLabel("Company notebook");

    // ADR 0081 Q4: this journey's explicit, single primary action — capture the
    // note — must be marked and reachable before any scroll.
    const captureAction = notebook.getByRole("button", { name: "New note" });
    await expectPrimaryActionCount(notebook, { max: 1 });
    await expectActionBeforeScroll(captureAction, notebook);

    await j.click(captureAction);
    await j.fill(notebook.getByLabel("Notebook note title"), "Q3 reviewed, guidance claim delivered");
    await j.fill(
      notebook.getByLabel("Notebook note body"),
      "Revenue read from the new report; the revenue claim came in delivered.",
    );
    await j.click(notebook.getByRole("button", { name: "Save" }));
    await expect(
      notebook.getByLabel("Select notebook entry: Q3 reviewed, guidance claim delivered"),
    ).toHaveCount(1);
    await expectNoPageOverflow(page);

    await j.assertBudget();
  });

  // F3a (ADR 0107, docs/plans/frontend-v2-f3a.md § contract 1): opening a
  // company lands on the engine-free Spółka screen — glance bar + co-visible
  // core — and the claims tool is one click away, opened with the claim to
  // verify highlighted. Red before S1 (screen) and S2 (tool host) by design.
  test("opening the company lands on Spółka: glance bar + co-visible core, Tezy raises the claims tool with the highlighted claim", async ({ page }) => {
    const j = journey(page, "J2");
    await openApp(page);
    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await j.click(page.locator('[data-company-id="company_gpw_cdr"] .company-row-main'));

    const spolka = page.getByRole("region", { name: "Company view", exact: true });
    await expect(spolka).toBeVisible();
    await expect(spolka).toHaveAttribute("data-company-id", "company_gpw_cdr");
    await j.markScreen("Spółka");

    // Glance bar: identity + the four attention counters.
    const glance = spolka.getByLabel("Company glance bar");
    await expect(glance).toBeVisible();
    await expect(spolka.getByText("CDR").first()).toBeVisible();
    for (const counter of ["Signals counter", "Claims counter", "Shorts counter", "Events counter"]) {
      await expect(glance.getByLabel(counter)).toBeVisible();
    }

    // Core: the five surfaces are co-visible at rest, no primary action at rest.
    const core = spolka.getByLabel("Company core");
    for (const section of ["Annual KPI table", "Company feed", "Price chart", "Report coverage", "Recommendations"]) {
      await expect(core.getByLabel(section)).toBeVisible();
    }
    await expectPrimaryActionCount(spolka, { max: 0 });
    await expect(spolka.locator('[data-ui-button-variant="primary"]')).toHaveCount(0);
    await expectNoPageOverflow(page);
    await expectNoA11yViolations(page, "Spółka at rest");

    // One click: the claims tool opens INTO the core zone with the pending
    // claim highlighted (the J5 highlight seam); the tool owns the primary.
    await j.click(spolka.getByRole("button", { name: "Claims", exact: true }));
    const tool = spolka.getByLabel("Workshop tool");
    await expect(tool).toBeVisible();
    await expect(tool.locator(".company-claims-panel")).toBeVisible();
    // The highlighted claim renders in both the claims list and the review
    // queue — one claim identity, however many rows carry it.
    await expect(tool.locator(".claim-row-highlighted").first()).toBeVisible();
    const highlightedIds = await tool
      .locator(".claim-row-highlighted")
      .evaluateAll((rows) => new Set(rows.map((row) => row.getAttribute("data-claim-id"))).size);
    expect(highlightedIds).toBe(1);
    await expectPrimaryActionCount(tool, { max: 1 });
    await expectNoPageOverflow(page);
  });

  // ADR 0081 Q9: hostile filenames/labels stay contained on the surfaces this
  // journey still walks. The KPI-extraction modal that used to carry this check
  // is gone (ADR 0084), so the hostile content is stressed where it now lands —
  // the Inbox stream and the detail rail it opens.
  test("hostile filenames/labels stay contained", async ({ page }) => {
    // Two full-page axe scans + whole-DOM overflow sweeps over the dense hostile
    // scenario. It historically ran close to the default 30s budget and tipped
    // over when WSL I/O slowed (sparse vhdx, 2026-07-14) — give it headroom.
    test.setTimeout(90_000);
    await primeMockScenario(page, { base: "rich", overlays: ["hostile-content", "dense-history"] });
    await openApp(page);
    await page.getByLabel(/Primary navigation/).getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await expectNoPageOverflow(page);

    // The hostile company's feed item — the fixed overlay entity, matched on
    // its stable (non-repeated) title fragment rather than the full string.
    const hostileItem = page.getByLabel(/Select feed item: .*Zażółć gęślą jaźń, wyniki Q3 2026/);
    await hostileItem.first().click();

    // The detail rail renders the hostile title, attachment labels and body
    // without blowing out the layout — the containment guarantee that used to
    // be asserted on the extraction modal's source-name flex child.
    await expectNoA11yViolations(page, "Inbox detail rail (hostile content)");
    await expectNoPageOverflow(page);
  });

  // ADR 0081 Q9: a doubled verdict on the same claim cannot duplicate or
  // overwrite — the resolved claim ends in exactly one state and leaves the
  // review queue rather than staying re-resolvable.
  //
  // NOTE (flagged gap, not weakened here): unlike the retired KPI-confirm button,
  // `CompanyClaimsPanel.resolveVerdict` has NO busy-disabled guard, so a second
  // click during an in-flight `set_claim_verdict` does reach the backend. The
  // write is idempotent so the OUTCOME is safe, which is what this test asserts.
  // Adding the busy guard is a behavior change to a surviving feature and is
  // escalated rather than made silently inside the AI-retirement slice.
  test("double verdict cannot duplicate or overwrite newer state", async ({ page }) => {
    await openApp(page);
    await page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }).click();
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();

    // ADR 0108: the frozen legacy dashboard is retired with the docking
    // engine — this regression case now exercises the Spółka `tezy`
    // (Claims) workshop tool, the surviving linked workflow.
    const spolka = page.getByRole("region", { name: "Company view", exact: true });
    await expect(spolka).toBeVisible();
    await spolka.getByRole("group", { name: "Workshop" }).getByRole("button", { name: "Claims", exact: true }).click();
    const claimsPane = page.getByRole("group", { name: "Workshop tool" });
    await expect(claimsPane).toBeVisible();
    await expect(claimsPane).toHaveAttribute("data-tool", "tezy");
    const shortToggle = claimsPane.getByRole("button", { name: "Show all claims" });
    if (await shortToggle.isVisible()) {
      await shortToggle.click();
    }
    const reviewQueue = claimsPane.getByLabel("Claims to verify");
    await expect(reviewQueue).toBeVisible();

    const deliver = reviewQueue.getByRole("button", { name: "Delivered" }).first();
    // One verdict control per tracked claim — the count that must not grow.
    const claimsBefore = await claimsPane.getByLabel("Claim verdict").count();

    // Hold the verdict command in flight, click a SECOND time while it is still
    // pending, then release — the doubled submission must not produce a doubled
    // or conflicting result.
    const verdictId = await holdInvocation(page, { command: "set_claim_verdict", phase: "before-handler" });
    await deliver.click();
    await deliver.click({ force: true });
    await releaseInvocation(page, verdictId);

    // Exactly one verdict lands: the claim reads delivered, and the doubled
    // submission created no extra claim — no duplicate, no conflicting row.
    await expect(claimsPane.getByLabel("Claim verdict").first()).toHaveValue("delivered");
    await expect(claimsPane.getByLabel("Claim verdict")).toHaveCount(claimsBefore);
  });
});
