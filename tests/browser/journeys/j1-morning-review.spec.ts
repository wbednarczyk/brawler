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

// J1 — Morning review (docs/ux-journeys.md, ADR 0074/0087). Trigger: opening the
// app at the start of the day with ~10 overnight items. The redesigned Today
// (ADR 0087) leads with a grouped, severity-ranked stream: `urgent` rows first,
// same-company repeats collapse into ×N groups, and >3 same-category routine rows
// fold into one cross-company aggregate. The full review — scan the briefing,
// open both urgent items into their workspace, review one member of a group — must
// fit the ≤15-interaction J1 budget (experience contract §8, budgets.json ratchet).
//
// The morning-briefing read (v0.54, ADR 0068 T5) is a PASSIVE scan at the top of
// Today — the strip renders unconditionally, so it is asserted as a screen fixture
// (visible, above the attention stream) rather than a counted interaction.
//
// The seed is the `morning-review` overlay (src/test/scenarios/overlays.ts): 10
// new items — 2 urgent (an insider transaction + a missed-report reconciliation),
// a 2-member notable group (same company, repeats that collapse), and 6 routine
// autopilot runs across distinct companies (the aggregate).

test.describe("J1 — morning review", { tag: "@journey" }, () => {
  test("urgent leads, repeats group, and the full review fits the budget", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["morning-review"] });

    const j = journey(page, "J1");
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");

    // Key screen: Today is the day's landing — it must be axe-clean and not
    // overflow before any interaction.
    await expectNoA11yViolations(page, "Today (morning review)");
    await expectNoPageOverflow(page);

    // Scan the morning-briefing strip at the TOP of Today, before triaging. It is
    // the entry summary of "what changed" — asserted as a fixture (renders
    // unconditionally as a labelled region, above the stream), never a counted
    // interaction (ADR 0087 dec. 5 mockup amendment: the strip replaced the card).
    const briefingStrip = page.getByRole("region", { name: "Morning briefing" });
    const stream = page.getByLabel("Attention stream");
    await expect(briefingStrip).toBeVisible();
    const briefingBox = await briefingStrip.boundingBox();
    const streamBox = await stream.boundingBox();
    expect(briefingBox).not.toBeNull();
    expect(streamBox).not.toBeNull();
    expect(briefingBox!.y).toBeLessThan(streamBox!.y);

    // (a) Urgent rows lead: the two overnight urgent alerts (insider + missed-
    // report reconciliation) sit at the very top of the stream, above every
    // notable/routine row. Wait for both to land first.
    const insiderRow = stream.locator('li[data-category="attention"]').filter({ hasText: "GPW:PKN" });
    const reconRow = stream.locator('li[data-category="attention"]').filter({ hasText: "GPW:KGH" });
    await expect(insiderRow).toBeVisible();
    await expect(reconRow).toBeVisible();
    const severities = await stream.locator("li[data-category]").evaluateAll((rows) =>
      rows.map((row) => (row as HTMLElement).dataset.severity ?? "routine"),
    );
    const rank = { urgent: 0, notable: 1, routine: 2 } as const;
    const ranks = severities.map((s) => rank[s as keyof typeof rank]);
    // Severity is monotonic non-decreasing down the stream, and the top rows are urgent.
    expect(ranks).toEqual([...ranks].sort((a, b) => a - b));
    expect(ranks[0]).toBe(0);
    expect(severities.filter((s) => s === "urgent").length).toBeGreaterThanOrEqual(2);

    // (b) Grouped + aggregate rows render with ×N: a same-company notable group
    // (×2) and a cross-company routine aggregate ("×N companies").
    const groupChips = stream.locator(".today-group-chip");
    const groupChipTexts = await groupChips.allInnerTexts();
    // The 2-member group chip leads with "×2" (D7 appends the pluralized unit,
    // "×2 events" in en); the cross-company aggregate chip is "×N companies".
    expect(groupChipTexts.some((t) => t.trim().startsWith("×2"))).toBe(true);
    expect(groupChipTexts.some((t) => /^×\d+\s+\S+/.test(t.trim()))).toBe(true);

    // (c) Open the first urgent item (insider transaction) into its workspace —
    // the experience contract's single primary action per row, reachable before
    // any scroll (ADR 0081 Q4).
    const insiderAction = insiderRow.getByRole("button", { name: "Review" });
    await expectPrimaryActionCount(insiderRow, { max: 1 });
    await expectActionBeforeScroll(insiderAction, stream);
    await j.clickPrimary(stream, insiderAction);
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await j.markScreen("Company workspace");
    await expectNoPageOverflow(page);

    // Back to Today.
    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");

    // Open the second urgent item (the missed-report reconciliation), then return.
    const reconAction = reconRow.getByRole("button", { name: "Review" });
    await expectPrimaryActionCount(reconRow, { max: 1 });
    await expectActionBeforeScroll(reconAction, stream);
    await j.clickPrimary(stream, reconAction);
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await j.markScreen("Company workspace");
    await expectNoPageOverflow(page);

    await j.click(page.getByLabel(/Primary navigation/).getByRole("button", { name: "Today" }));
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
    await j.markScreen("Today");

    // Review one member of the grouped (×2) row: expand it in place, then open a
    // member into its workspace. The group is PZU's 2 same-category notable rows.
    const groupRow = stream.locator('li[data-category="attention"]').filter({ hasText: "GPW:PZU" });
    await expect(groupRow).toBeVisible();
    await j.click(groupRow.getByRole("button", { name: "Details" }));
    const memberAction = groupRow.locator('[data-member-category="attention"]').first().getByRole("button", { name: "Review" });
    await expect(memberAction).toBeVisible();
    await j.clickPrimary(stream, memberAction);
    await expect(page.getByLabel("Research cockpit")).toBeVisible();
    await j.markScreen("Company workspace");
    await expectNoPageOverflow(page);

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

    // The hostile company (ZZZH) surfaces as a "what changed" row (its feed item
    // is an "Official report" kind). Under the redesign (ADR 0087) the routine
    // "changed" rows across the rich base collapse into a cross-company
    // aggregate, and ZZZH — sharing the sample timestamp, so it sorts last by
    // company id — lands as a COLLAPSED MEMBER. Expand the aggregate to reveal it
    // (or fall back to a top-level changed row when nothing aggregated).
    const stream = page.getByLabel("Attention stream");
    const changedAggregate = stream
      .locator('li[data-category="changed"]')
      .filter({ hasText: /×\d+\s+compan/ });
    const aggregated = (await changedAggregate.count()) > 0;
    if (aggregated) {
      await changedAggregate.first().getByRole("button", { name: "Details" }).first().click();
    }
    // The hostile row is the fixed overlay entity (ZZZH), never a brittle match on
    // the mixed-script title — a compact aggregate member when aggregated, else a
    // top-level changed row.
    const hostileRow = aggregated
      ? stream.locator('[data-member-category="changed"]').filter({ hasText: "ZZZH" })
      : stream.locator('li[data-category="changed"]').filter({ hasText: "ZZZH" });
    await expect(hostileRow.first()).toBeVisible();
    // The long unbreakable title/URL never blows out the layout, even in the
    // tall-narrow band.
    await expectNoPageOverflow(page);

    // ADR 0081 Q4: the hostile row still carries exactly one explicit primary
    // action (a group/aggregate member keeps its own single Review, so `j`/`k`
    // can traverse it). The "reachable before scroll" contract applies to the
    // aggregate's own top-level Review, not to a member the user expanded to.
    await expectPrimaryActionCount(hostileRow.first(), { max: 1 });
    await expect(hostileRow.first().getByRole("button", { name: "Review" })).toBeVisible();
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
    // renders a typed, translated failed-category error strip — NEVER the raw
    // backend `.message` (ADR 0087 consequence: raw error strings on Today are
    // replaced by translated copy) — and the false-quiet state is suppressed
    // while any category errored, so a failure can never masquerade as "nothing
    // needs attention".
    await expect(page.getByText("Couldn't load upcoming reports.")).toBeVisible();
    await expect(page.getByText(seasonFailure.message)).toHaveCount(0);
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
