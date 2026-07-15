import {
  test,
  expect,
  openApp,
  journey,
  expectNoPageOverflow,
  expectNoA11yViolations,
} from "../helpers/harness";
import { expectPrimaryActionCount, expectActionBeforeScroll } from "../helpers/interactionContracts";
import {
  holdInvocation,
  releaseInvocation,
  rejectInvocation,
  type ScenarioSpec,
} from "../helpers/mockRuntime";
import type { Page } from "@playwright/test";

// `recentFeedItems`/`companies` are fetched exactly once at app bootstrap
// (not re-fetched on in-app navigation), so seeding overlay content AFTER
// `openApp` is too late for it to appear in those reads —
// the app's own bootstrap fetch has already resolved with the pre-overlay
// seed by the time a post-navigation `page.evaluate` reset lands (this raced
// and was observed flaky under worker load). `page.addInitScript` runs
// synchronously as the FIRST script on the page, before the app bundle: it
// traps the `window.__brawlerMock` assignment `installBrowserSmokeRuntime`
// performs and applies the reset in that same synchronous tick, strictly
// before React ever mounts — no timing race.
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

// J1 — Morning review (docs/ux-journeys.md, ADR 0074). Trigger: opening the app
// at the start of the day. The cross-screen path: land on Today/Pulse → triage
// the attention stream (filter by a counter tile) → open the 1–2 items that
// matter into their company workspace → back to Today. Interactions are driven
// through the journey() wrapper so the step count is measured against the budget.
//
// Future step (dated): v0.54 morning briefing — read at the top of Today before
// triage — is not built yet; when it ships it joins this journey and its budget.

test.describe("J1 — morning review", { tag: "@journey" }, () => {
  test("triage the attention stream and open what matters", async ({ page }) => {
    const j = journey(page, "J1");
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");

    // Key screen: Today is the day's landing — it must be axe-clean and not
    // overflow before any interaction.
    await expectNoA11yViolations(page, "Today (morning review)");
    await expectNoPageOverflow(page);

    const autopilotRows = page.locator('li[data-category="autopilot"]');
    const changedRows = page.locator('li[data-category="changed"]');
    await expect(autopilotRows.first()).toBeVisible();
    expect(await changedRows.count()).toBeGreaterThan(0);

    // Triage: the Autopilot counter filters the stream to just those items.
    const autopilotTile = page
      .getByRole("group", { name: "Filter the stream" })
      .getByRole("button", { name: /Autopilot/ });
    await j.click(autopilotTile);
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "true");
    await expect(changedRows).toHaveCount(0);

    // Restore the full stream to scan everything that arrived. The filter is
    // reset (not preserved) here on purpose — the interesting context to
    // protect is that it stays unfiltered across the round trips below.
    await j.click(autopilotTile);
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "false");
    expect(await changedRows.count()).toBeGreaterThan(0);
    await j.preserveContext("today-filter:none");

    // Open the first item that matters (an autopilot run) into its workspace —
    // the experience contract's single primary action for this decision
    // surface, driven through clickPrimary so a future regression that
    // requires scrolling before it's reachable shows up in the review.
    // ADR 0081 Q4: each row is its own scoped decision surface — it must
    // carry exactly one explicit primary action, reachable before any scroll.
    const stream = page.getByLabel("Attention stream");
    const firstAutopilotAction = autopilotRows.first().getByRole("button", { name: "Review" });
    await expectPrimaryActionCount(autopilotRows.first(), { max: 1 });
    await expectActionBeforeScroll(firstAutopilotAction, stream);
    await j.clickPrimary(stream, firstAutopilotAction);
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await j.markScreen("Company workspace");
    await expectNoPageOverflow(page);

    // Back to Today — the filter context (still unfiltered) must have
    // survived the round trip, not silently reset.
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "false");
    await j.preserveContext("today-filter:none");

    // Open the second item that matters (a "what changed" report) then return.
    const changedAction = changedRows.first().getByRole("button", { name: "Review" });
    await expectPrimaryActionCount(changedRows.first(), { max: 1 });
    await expectActionBeforeScroll(changedAction, stream);
    await j.clickPrimary(stream, changedAction);
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await j.markScreen("Company workspace");
    await expectNoPageOverflow(page);

    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "false");
    await j.preserveContext("today-filter:none");

    await j.assertBudget();
  });

  // ADR 0081 Q9: hostile/adversarial content must render legibly and stay
  // navigable — a long mixed-script title/attachment URL is real content
  // shape (GPW ESPI communiqués are not English-only, ASCII-only, or short).
  test("hostile stream stays legible and Review reachable", async ({ page }) => {
    // The tall-narrow band (CLAUDE.md Testing Expectations) is where a long
    // unbreakable title/URL is most likely to blow out the layout.
    await page.setViewportSize({ width: 1366, height: 768 });
    await primeMockScenario(page, { base: "rich", overlays: ["hostile-content", "mixed-locale"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await expectNoPageOverflow(page);

    // The hostile company (ZZZH) surfaces as a "what changed" row (its feed
    // item is an "Official report" kind) — the fixed overlay entity, never a
    // brittle text match on the full mixed-script title.
    const hostileRow = page
      .locator('li[data-category="changed"]')
      .filter({ hasText: "ZZZH" });
    await expect(hostileRow.first()).toBeVisible();
    await expectNoPageOverflow(page);

    // ADR 0081 Q4: still exactly one explicit primary action, reachable
    // before scroll, even under hostile content.
    await expectPrimaryActionCount(hostileRow.first(), { max: 1 });
    const reviewAction = hostileRow.first().getByRole("button", { name: "Review" });
    const stream = page.getByLabel("Attention stream");
    await expectActionBeforeScroll(reviewAction, stream);
    await expectNoA11yViolations(page, "Today (hostile stream)");
  });

  // ADR 0081 Q9: a category read that fails must be explicit, never silently
  // folded into the quiet state (docs/plans/ux-quality-loop-v2.md § J1
  // Recovery). No scenario overlay injects a Today-category read failure —
  // this exercises it directly via the controlled-async bridge.
  test("a failed attention category is explicit, never false quiet", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // `window.__brawlerMock` only exists once the app has booted, so the hold
    // is registered after the first mount, then Today is remounted (via a
    // navigate-away/back round trip) so the hold captures THAT mount's own
    // `list_report_season` read (Today's "upcoming reports" category; unlike
    // claims, it runs unconditionally — no pinned-company prerequisite).
    // Registered TWICE: React StrictMode (dev) double-invokes mount effects,
    // so the remount fires `list_report_season` twice — a discarded "phantom"
    // call, then the real one the rendered component keeps.
    const nav = page.getByLabel(/Primary navigation/);
    await nav.getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    const phantomSeasonId = await holdInvocation(page, { command: "list_report_season", phase: "before-handler" });
    const seasonReadId = await holdInvocation(page, { command: "list_report_season", phase: "before-handler" });
    await nav.getByRole("button", { name: "Today" }).click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const seasonFailure = {
      code: "internal" as const,
      message: "Sample report-season fetch failure (Q9 controlled-async case)",
    };
    // `useReportSeason`'s mount effect has no cancelled-guard, so whichever of
    // the two calls settles LAST is what the UI reflects — reject both so the
    // failure lands regardless of StrictMode's internal ordering.
    await rejectInvocation(page, phantomSeasonId, seasonFailure);
    await rejectInvocation(page, seasonReadId, seasonFailure);

    // The other categories (autopilot, changed) still have content, so the
    // stream is genuinely non-empty — the quiet state must not appear.
    const autopilotRows = page.locator('li[data-category="autopilot"]');
    const changedRows = page.locator('li[data-category="changed"]');
    await expect(autopilotRows).not.toHaveCount(0);
    await expect(changedRows).not.toHaveCount(0);
    await expect(page.locator(".today-stream-quiet")).toHaveCount(0);

    // The failed "Upcoming reports" read is EXPLICIT (ADR 0081 Q9 fix): Today
    // renders `season.error` as a visible failed-category message, and the
    // false-quiet state is suppressed while any category errored — a failure
    // can never masquerade as "nothing needs attention".
    await expect(page.getByText(seasonFailure.message)).toBeVisible();
    await expect(page.locator(".today-stream-quiet")).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await expectNoPageOverflow(page);
  });

  // ADR 0081 Q9: proves the mount/unmount request-cancellation guard on
  // Today's autopilot read — Today has no in-place refresh, so the only way
  // to produce two overlapping `list_autopilot_runs` reads is across a
  // navigate-away/navigate-back remount. A reply belonging to an orphaned
  // (unmounted) Today instance, released AFTER the current instance's own
  // read, must never resurrect stale content into the current instance.
  test("an out-of-order attention read cannot restore stale content", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const nav = page.getByLabel(/Primary navigation/);
    await nav.getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();

    const autopilotRows = page.locator('li[data-category="autopilot"]');

    // Registered before the remount so it captures THAT instance's read.
    // Registered TWICE: React StrictMode (dev) double-invokes mount effects,
    // so each remount fires `list_autopilot_runs` twice — a discarded
    // "phantom" call (its own instance is cancelled almost immediately by
    // React's effect-cleanup), then the real one the rendered component
    // keeps. Only the real one is the "OLDER" read under test.
    const olderPhantomId = await holdInvocation(page, { command: "list_autopilot_runs", phase: "after-handler" });
    const olderId = await holdInvocation(page, { command: "list_autopilot_runs", phase: "after-handler" });
    await nav.getByRole("button", { name: "Today" }).click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    // Held: this fresh instance's own autopilot rows have not arrived yet
    // (the "changed" category already has synchronously-available content,
    // so the screen is not in its global quiet/loading state — only the
    // autopilot category itself is still empty).
    await expect(autopilotRows).toHaveCount(0);
    // The phantom's own instance is already orphaned — releasing it must not
    // surface anything either.
    await releaseInvocation(page, olderPhantomId);
    await expect(autopilotRows).toHaveCount(0);

    // Navigate away again BEFORE releasing — this Today instance (and its
    // pending read) is now orphaned; React's own effect-cleanup `cancelled`
    // flag is the guard under test.
    await nav.getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();

    const newerPhantomId = await holdInvocation(page, { command: "list_autopilot_runs", phase: "after-handler" });
    const newerId = await holdInvocation(page, { command: "list_autopilot_runs", phase: "after-handler" });
    await nav.getByRole("button", { name: "Today" }).click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await expect(autopilotRows).toHaveCount(0);
    await releaseInvocation(page, newerPhantomId);
    await expect(autopilotRows).toHaveCount(0);

    // Release the NEWER (current instance's own) read first.
    await releaseInvocation(page, newerId);
    await expect(autopilotRows.first()).toBeVisible();
    const settledCount = await autopilotRows.count();

    // Then the OLDER, orphaned read — it must not overwrite the current
    // instance's already-settled stream.
    await releaseInvocation(page, olderId);
    await expect(autopilotRows).toHaveCount(settledCount);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await expectNoPageOverflow(page);
  });
});
