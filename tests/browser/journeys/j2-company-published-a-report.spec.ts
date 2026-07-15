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
  expectFocusOrder,
  expectNextStepVisible,
} from "../helpers/interactionContracts";
import {
  holdInvocation,
  releaseInvocation,
  rejectInvocation,
  type ScenarioSpec,
} from "../helpers/mockRuntime";
import type { Page } from "@playwright/test";

// `recentFeedItems`/`companies` are fetched exactly once at app bootstrap
// (not re-fetched on in-app navigation), so seeding overlay content AFTER
// `openApp` is too late for it to appear in those reads — the app's own
// bootstrap fetch has already resolved with the pre-overlay seed by then
// (this was observed flaky when raced with a post-navigation reset).
// `page.addInitScript` runs synchronously as the FIRST script on the page,
// before the app bundle: it traps the `window.__brawlerMock` assignment
// `installBrowserSmokeRuntime` performs and applies the reset in that same
// synchronous tick, strictly before React ever mounts — no timing race.
async function primeMockScenario(page: Page, spec: ScenarioSpec): Promise<void> {
  await page.addInitScript((s) => {
    let installed: unknown;
    Object.defineProperty(window, "__brawlerMock", {
      configurable: true,
      get() {
        return installed;
      },
      set(bridge) {
        (bridge as { reset: (spec: unknown) => void }).reset(s);
        installed = bridge;
      },
    });
  }, spec);
}

// J2 — A company published a report (docs/ux-journeys.md, ADR 0074). The full
// cross-screen path a user walks when a periodic report lands: open the report →
// extract & review its KPIs → confirm a fact → open the company workspace → see
// the confirmed fact in the fundamentals matrix → resolve a due management claim
// → capture the judgment as a note. This spec absorbs the three legacy
// journeys.spec.ts tests (extract→review→confirm, open-company-without-moving,
// confirmed-KPI-in-matrix) into one measured journey; the old file is deleted.
//
// Future step (dated): v0.52 expectation-vs-actual review (ADR 0071 judgment
// capture) inserts between "review KPIs" and "resolve claims"; it joins this
// journey and its budget when it ships.
//
// Q3 note (ADR 0081): the Claims/Notebook panes no longer force a 900×700 pane
// size — a user journey takes the real disclosure path at the current project
// viewport (density tests may still force a pane; this journey must not). If a
// project's real path genuinely needs different steps, that project's measured
// count is recorded in budgets.json's byProject, not hidden by forcing.

test.describe("J2 — a company published a report", { tag: "@journey" }, () => {
  test("open report → confirm KPI → workspace → resolve claim → note", async ({ page }) => {
    const j = journey(page, "J2");
    await openApp(page);
    await j.markScreen("Today");

    // Open the Inbox and select the CD PROJEKT report; its detail rail offers the
    // KPI extractor.
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Inbox" }));
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await j.markScreen("Inbox");
    await j.click(page.getByLabel(/Select feed item: CD PROJEKT/).first());
    const launcher = page.getByLabel("AI KPI extraction");
    await expect(launcher).toBeVisible();
    await expectNoA11yViolations(page, "Inbox with report selected");

    // ADR 0081 Q4: the launcher surface's explicit, single primary action —
    // open the extractor — must be marked and reachable before any scroll.
    const extractAction = launcher.getByRole("button", { name: "Extract KPIs" });
    await expectPrimaryActionCount(launcher, { max: 1 });
    await expectActionBeforeScroll(extractAction, launcher);

    // The heavy extraction flow opens in a modal.
    await j.click(extractAction);
    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await expect(dialog).toBeVisible();
    await j.markModal("KPI extraction");
    await expectNoHorizontalOverflow(dialog);
    await expectNoA11yViolations(page, "KPI extraction dialog");

    // Run the extraction; status and proposals appear inside the modal.
    await j.click(dialog.getByRole("button", { name: /Extract from attachment/ }));
    await expect(dialog.getByText("succeeded")).toBeVisible();
    await expect(dialog.getByText("Revenue")).toBeVisible();
    await j.expectFeedback(dialog.getByText("succeeded"));

    // Declared Tab sequence across the review footer's bulk actions (ADR 0081
    // Q4): known-confirm, then accept-suggestions, then refresh.
    await expectFocusOrder(page, [
      dialog.getByRole("button", { name: "Confirm all known" }),
      dialog.getByRole("button", { name: "Accept all suggestions" }),
      dialog.getByRole("button", { name: "Refresh" }),
    ]);

    // Confirm the first known KPI; it transitions to confirmed in place — the
    // contracted next step must stay visible, not get hidden by success.
    await j.click(dialog.getByRole("button", { name: "Confirm" }).first());
    await expectNextStepVisible(dialog.getByText("confirmed").first());
    await j.click(dialog.getByRole("button", { name: "Close dialog" }));

    // Open the company — the cockpit dashboard lands scoped to it (ADR 0057),
    // visible and in view without a manual scroll, and the app shell must not have
    // scrolled (the "moves the whole app" regression).
    await j.click(page.getByLabel("Primary navigation").getByRole("button", { name: "Companies" }));
    await expect(page.getByLabel("Companies list")).toBeVisible();
    await j.markScreen("Companies");
    await expectNoA11yViolations(page, "Companies list");
    await j.click(page.locator('[data-company-id="company_gpw_cdr"] .company-row-main'));

    const workspace = page.getByRole("region", { name: "Research cockpit", exact: true });
    await expect(workspace).toBeVisible();
    await expect(workspace).toBeInViewport();
    await j.markScreen("Company workspace");
    // The cockpit root's semantic company marker (ADR 0081 Q3 observation point)
    // is the context this journey must not silently lose across the Claims/
    // Notebook tab switches below — it is the company the confirmed fact, the
    // resolved claim, and the captured note must all end up attached to.
    await j.preserveContext(await workspace.getAttribute("data-company-id"));
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

    // Resolve a due management claim from the workspace Claims tab, at the real
    // pane size the current project viewport gives it — no forced 900×700
    // shortcut (Q3, ADR 0081).
    await j.click(page.getByLabel("Research cockpit").getByRole("button", { name: "Claims", exact: true }).first());
    const claimsPane = page.locator(".cockpit-pane", { has: page.locator(".company-claims-panel") });
    await expect(claimsPane).toBeVisible();
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
    // The verdict is recorded: the claim's row verdict now reads delivered.
    await expect(claimsPane.getByLabel("Claim verdict").first()).toHaveValue("delivered");
    await expectNoPageOverflow(page);

    // Capture the judgment as a note in the company Notebook, again at the real
    // pane size (no forced 900×700 shortcut).
    await j.click(page.getByLabel("Research cockpit").getByRole("button", { name: "Notebook", exact: true }).first());
    const notebookPane = page.locator(".cockpit-pane", { has: page.locator(".notebook-panel") });
    await expect(notebookPane).toBeVisible();
    await j.preserveContext(await workspace.getAttribute("data-company-id"));
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

    await j.assertBudget();
  });

  // ADR 0081 Q9: hostile filenames/labels stay contained inside the extraction
  // modal (no overlay wires directly into this modal's data, so `dense-history`
  // only stresses the surrounding Inbox stream; the modal check targets the
  // hostile company's long unbreakable attachment URL and its proposals).
  test("hostile filenames/labels/citations stay contained", async ({ page }) => {
    // The heaviest journey test: two full-page axe scans + whole-DOM overflow
    // sweeps over the dense hostile scenario. It historically ran at ~22s of
    // the default 30s budget and tipped over when WSL I/O slowed (sparse vhdx,
    // 2026-07-14) — give it headroom; every assertion stays as-is.
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
    const launcher = page.getByLabel("AI KPI extraction");
    await expect(launcher).toBeVisible();
    await launcher.getByRole("button", { name: "Extract KPIs" }).click();

    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await expect(dialog).toBeVisible();
    // The DOCUMENT itself does not overflow (the modal correctly clips at the
    // viewport level) — a lighter, direct check than `expectNoPageOverflow`
    // here, which also scans for ANY horizontally-scrollable container and
    // would trip on the known internal-modal bug documented below before the
    // rest of this test's coverage (extraction, a11y) gets to run.
    await expect
      .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1))
      .toBe(true);
    await expectNoA11yViolations(page, "KPI extraction dialog (hostile content)");

    // Run extraction so dense proposals/citations are also on-screen.
    const hostileSource = dialog.getByRole("button", { name: /Extract from attachment/ });
    await expect(hostileSource).toBeVisible();
    await hostileSource.click();
    await expect(dialog.getByText("succeeded")).toBeVisible();
    await expectNoA11yViolations(page, "KPI extraction dialog (hostile content, proposals loaded)");

    // Regression guard (ADR 0081 Q9): the hostile attachment URL — a single
    // unbreakable token with no spaces — is the extraction source button's
    // label (`.kpi-extraction-source-name`). Before the fix it blew out the
    // modal's internal width (scrollWidth 1620 > clientWidth 526 at 1366×768)
    // and made `.ui-modal-body` horizontally scrollable. The fix (`min-width:0`
    // on that flex child so its `text-overflow: ellipsis` engages) truncates
    // the URL instead; this assertion stays to keep the regression caught.
    await expectNoHorizontalOverflow(dialog);
    await expectNoPageOverflow(page);
  });

  // ADR 0081 Q9: an extraction failure must be explicit, with a working retry
  // — never a silent "succeeded" with zero proposals.
  test("explicit extraction failure + retry, never success with zero effect", async ({ page }) => {
    await openApp(page);
    await page.getByLabel(/Primary navigation/).getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await page.getByLabel(/Select feed item: CD PROJEKT/).first().click();
    const launcher = page.getByLabel("AI KPI extraction");
    await launcher.getByRole("button", { name: "Extract KPIs" }).click();
    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await expect(dialog).toBeVisible();

    const extractionId = await holdInvocation(page, { command: "start_kpi_extraction", phase: "before-handler" });
    const extractAction = dialog.getByRole("button", { name: /Extract from attachment/ });
    await extractAction.click();

    await rejectInvocation(page, extractionId, {
      code: "provider",
      message: "Sample extraction provider failure (Q9 controlled-async case)",
    });

    // Explicit, visible failure — never a silent success.
    await expect(dialog.getByText(/Sample extraction provider failure/)).toBeVisible();
    await expect(dialog.getByText("succeeded")).toHaveCount(0);
    await expectNoA11yViolations(page, "KPI extraction dialog (extraction failed)");

    // Retry: the same source action is still available and can now succeed —
    // never a dead end.
    await extractAction.click();
    await expect(dialog.getByText("succeeded")).toBeVisible();
    await expect(dialog.getByText("Revenue")).toBeVisible();
  });

  // ADR 0081 Q9: a doubled confirm on the same proposal cannot create a
  // duplicate/overwrite — the primitive's own busy-disabled state prevents a
  // second submission, then the confirmed proposal's action row disappears
  // entirely (never re-confirmable).
  test("double action / modal reopen / reversed completion cannot duplicate or overwrite newer state", async ({
    page,
  }) => {
    await openApp(page);
    await page.getByLabel(/Primary navigation/).getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    await page.getByLabel(/Select feed item: CD PROJEKT/).first().click();
    const launcher = page.getByLabel("AI KPI extraction");
    await launcher.getByRole("button", { name: "Extract KPIs" }).click();
    const dialog = page.getByRole("dialog", { name: /KPI extraction/ });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: /Extract from attachment/ }).click();
    await expect(dialog.getByText("succeeded")).toBeVisible();

    const firstProposal = dialog.locator(".kpi-extraction-proposal").first();
    const confirmButton = firstProposal.getByRole("button", { name: "Confirm" });

    // Hold the confirm command in flight and prove the button is disabled —
    // a doubled click cannot submit a second confirm for the same proposal.
    const confirmId = await holdInvocation(page, { command: "confirm_kpi_proposal", phase: "before-handler" });
    await confirmButton.click();
    await expect(confirmButton).toBeDisabled();
    await releaseInvocation(page, confirmId);

    // Exactly one confirmation lands: the proposal is confirmed once, and its
    // action row (including Confirm) is gone — never re-confirmable.
    await expect(firstProposal.getByText("confirmed")).toBeVisible();
    await expect(firstProposal.getByRole("button", { name: "Confirm" })).toHaveCount(0);

    // Closing and reopening the modal does not unmount the extraction state
    // (the launcher's `job` lives above the modal) — the confirmed state
    // survives the round trip rather than reverting or duplicating.
    await dialog.getByRole("button", { name: "Close dialog" }).click();
    await expect(dialog).toBeHidden();
    await launcher.getByRole("button", { name: "Review extracted KPIs" }).click();
    await expect(dialog).toBeVisible();
    await expect(dialog.locator(".kpi-extraction-proposal").first().getByText("confirmed")).toBeVisible();
    await expect(dialog.locator(".kpi-extraction-proposal").first().getByRole("button", { name: "Confirm" })).toHaveCount(
      0,
    );
  });
});
