import { test, expect, openApp, expectNoPageOverflow } from "./helpers/harness";
import {
  holdInvocation,
  primeMockScenario,
  rejectInvocation,
} from "./helpers/mockRuntime";
import {
  PRUNED_CLEAN_SNAPSHOT,
  PRUNED_GLUED_FILENAME,
  PRUNED_GLUED_HUMAN,
} from "../../src/test/scenarios/overlays";

// Today/Pulse redesigned to journey J1 (ADR 0076 U-Rb): a single prioritized
// stream with roving j/k navigation plus a counters column that filters the
// stream. Clickable coverage for the two interactions the shell smoke test does
// not exercise. The browser runtime locale is en.
test.describe("Today stream — filter + keyboard", { tag: "@clickable" }, () => {
  test("a counter tile filters the stream to its category and restores it", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // The seed carries an autopilot run + report ("what changed") feed items.
    const autopilotRows = page.locator('li[data-category="autopilot"]');
    const changedRows = page.locator('li[data-category="changed"]');
    await expect(autopilotRows.first()).toBeVisible();
    expect(await changedRows.count()).toBeGreaterThan(0);

    const autopilotTile = page
      .getByRole("group", { name: "Filter the stream" })
      .getByRole("button", { name: /Autopilot/ });

    await autopilotTile.click();
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "true");
    // Only autopilot rows remain; the "what changed" rows are filtered out.
    await expect(changedRows).toHaveCount(0);
    await expect(autopilotRows.first()).toBeVisible();

    await autopilotTile.click();
    await expect(autopilotTile).toHaveAttribute("aria-pressed", "false");
    expect(await changedRows.count()).toBeGreaterThan(0);

    await expectNoPageOverflow(page);
  });

  test("j/k moves roving focus across the stream's action buttons", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const rowButtons = page.locator('[data-today-row="true"]');
    expect(await rowButtons.count()).toBeGreaterThanOrEqual(2);

    await rowButtons.nth(0).focus();
    await expect(rowButtons.nth(0)).toBeFocused();

    await page.keyboard.press("j");
    await expect(rowButtons.nth(1)).toBeFocused();

    await page.keyboard.press("k");
    await expect(rowButtons.nth(0)).toBeFocused();

    await page.keyboard.press("ArrowDown");
    await expect(rowButtons.nth(1)).toBeFocused();
  });
});

// Redesign coverage (ADR 0087): the "Pilne" (urgent) counter tile, roving that
// crosses a group's header into its expanded members, and an aggregate that
// expands/collapses in place. Seeded with the `morning-review` overlay — 2 urgent
// rows, a ×2 notable group (GPW:PZU), and a routine autopilot aggregate.
test.describe("Today stream — urgent tile, group roving, aggregate expand", { tag: "@clickable" }, () => {
  test("the Pilne (Urgent) tile filters the stream to urgent rows and restores it", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["morning-review"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const stream = page.getByLabel("Attention stream");
    // The two overnight urgent rows have landed.
    await expect(stream.locator('li[data-severity="urgent"]').first()).toBeVisible();
    // A non-urgent (routine) row is also present before filtering.
    await expect(stream.locator('li[data-severity="routine"]').first()).toBeVisible();

    const urgentTile = page
      .getByRole("group", { name: "Filter the stream" })
      .getByRole("button", { name: /Urgent/ });
    await urgentTile.click();
    await expect(urgentTile).toHaveAttribute("aria-pressed", "true");

    // Only urgent rows remain — every visible top-level row is urgent.
    const severities = await stream
      .locator("li[data-category]")
      .evaluateAll((rows) => rows.map((r) => (r as HTMLElement).dataset.severity ?? "routine"));
    expect(severities.length).toBeGreaterThanOrEqual(2);
    expect(severities.every((s) => s === "urgent")).toBe(true);

    // Toggling off restores the mixed-severity stream.
    await urgentTile.click();
    await expect(urgentTile).toHaveAttribute("aria-pressed", "false");
    await expect(stream.locator('li[data-severity="routine"]').first()).toBeVisible();
    await expectNoPageOverflow(page);
  });

  test("j/k roving crosses a group header into its expanded members and on past the group", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["morning-review"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const stream = page.getByLabel("Attention stream");
    const groupRow = stream.locator('li[data-category="attention"]').filter({ hasText: "GPW:PZU" });
    await expect(groupRow).toBeVisible();
    // The ×2 count chip marks this as a collapsed group (D7 adds the pluralized
    // unit — "×2 events" in en — so match the count marker, not an exact string).
    await expect(groupRow.locator(".today-group-chip")).toContainText("×2");

    // Expand the group in place so its members join the roving order.
    await groupRow.getByRole("button", { name: "Details" }).click();
    await expect(groupRow.locator('[data-member-category="attention"]').first()).toBeVisible();

    // Focus the group header's Review, then `j` walks INTO the members (DOM order:
    // header action, then each member's action) and finally out to the next row.
    const headerReview = groupRow.getByRole("button", { name: "Review" }).first();
    await headerReview.focus();
    await expect(headerReview).toBeFocused();

    const firstMember = groupRow
      .locator('[data-member-category="attention"]')
      .nth(0)
      .getByRole("button", { name: "Review" });
    const secondMember = groupRow
      .locator('[data-member-category="attention"]')
      .nth(1)
      .getByRole("button", { name: "Review" });

    await page.keyboard.press("j");
    await expect(firstMember).toBeFocused();
    await page.keyboard.press("j");
    await expect(secondMember).toBeFocused();
    // One more `j` leaves the group entirely (onto the next stream row's action).
    await page.keyboard.press("j");
    await expect(secondMember).not.toBeFocused();
    // `k` walks back into the group's last member.
    await page.keyboard.press("k");
    await expect(secondMember).toBeFocused();
  });

  test("an aggregate row expands its collapsed companies in place and collapses again", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["morning-review"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const stream = page.getByLabel("Attention stream");
    // The routine autopilot rows folded into one "×N companies" aggregate row.
    // Scope to the autopilot category — the base scenario also aggregates other
    // routine categories (upcoming, changed), each its own "×N companies" row.
    const aggregateRow = stream
      .locator('li[data-category="autopilot"]')
      .filter({ hasText: /×\d+\s+compan/ });
    await expect(aggregateRow).toBeVisible();
    // Collapsed: no member rows rendered yet.
    await expect(aggregateRow.locator("[data-member-category]")).toHaveCount(0);

    // The HEADER disclosure is the first Details button in the row (each expanded
    // autopilot member carries its own Details, so scope to `.first()`).
    const headerDisclosure = aggregateRow.getByRole("button", { name: "Details" }).first();
    await headerDisclosure.click();
    await expect(aggregateRow.locator("[data-member-category]").first()).toBeVisible();
    const expandedCount = await aggregateRow.locator("[data-member-category]").count();
    expect(expandedCount).toBeGreaterThan(3);

    await headerDisclosure.click();
    await expect(aggregateRow.locator("[data-member-category]")).toHaveCount(0);
    await expectNoPageOverflow(page);
  });
});

// Adversarial Today states (ADR 0081 locked boundary — base scenarios + overlays,
// no new Playwright projects): the Dense and Partial rows of the state matrix
// (docs/plans/v0.60-today-redesign.md §11).
test.describe("Today adversarial states — dense + partial", { tag: "@clickable" }, () => {
  test("dense: a wall of routine runs folds into one aggregate; the stream stays a screenful", async ({ page }) => {
    // One routine autopilot run per company (28 in the browser store) → far over
    // the aggregate threshold. Seeded before boot.
    await primeMockScenario(page, { base: "rich", overlays: ["today-dense"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const stream = page.getByLabel("Attention stream");
    // The whole autopilot wall collapsed into a single "×N companies" row.
    const aggregateRow = stream
      .locator('li[data-category="autopilot"]')
      .filter({ hasText: /×\d+\s+compan/ });
    await expect(aggregateRow).toBeVisible();
    // The count reflects the dense wall (≥ 10 companies collapsed).
    const chipText = await aggregateRow.locator(".today-group-chip").innerText();
    const collapsed = Number(chipText.replace(/[^\d]/g, ""));
    expect(collapsed).toBeGreaterThanOrEqual(10);

    // Aggregation keeps the stream short — a handful of top-level rows, never a
    // 28-row wall — and the page never overflows horizontally.
    const topLevelRows = await stream.locator("li[data-category]").count();
    expect(topLevelRows).toBeLessThan(15);
    await expectNoPageOverflow(page);

    // Also holds in the tall-narrow quarter-ultrawide band.
    await page.setViewportSize({ width: 1008, height: 1152 });
    await expectNoPageOverflow(page);
  });

  test("partial: one category errors → an inline error strip, the rest alive, quiet state blocked", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // `list_report_season` (upcoming reports) fetches unconditionally on mount, so
    // erroring it — across a navigate-away/back remount that the held calls
    // capture (StrictMode double-invokes the mount effect) — reproduces exactly
    // one failed Today category without touching the others.
    const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
    await nav.getByRole("button", { name: "Inbox" }).click();
    await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
    const phantomId = await holdInvocation(page, { command: "list_report_season", phase: "before-handler" });
    const seasonId = await holdInvocation(page, { command: "list_report_season", phase: "before-handler" });
    await nav.getByRole("button", { name: "Today" }).click();
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const failure = {
      code: "internal" as const,
      message: "Sample upcoming-reports fetch failure (Today partial-state case)",
    };
    await rejectInvocation(page, phantomId, failure);
    await rejectInvocation(page, seasonId, failure);

    // The failed category is EXPLICIT — a typed, translated inline error strip
    // (never the raw backend `.message`, ADR 0087 consequence) — never folded
    // into the quiet state.
    await expect(page.getByText("Couldn't load upcoming reports.")).toBeVisible();
    await expect(page.getByText(failure.message)).toHaveCount(0);
    await expect(page.locator(".today-stream-quiet")).toHaveCount(0);
    // The rest of the stream is still alive (autopilot + changed rows loaded).
    await expect(page.locator('li[data-category="autopilot"]').first()).toBeVisible();
    await expect(page.locator('li[data-category="changed"]').first()).toBeVisible();
    // The error strip offers a retry affordance.
    await expect(
      page.locator(".today-error-strip").getByRole("button", { name: "Try again" }),
    ).toBeVisible();
    await expectNoPageOverflow(page);
  });
});

// UI dogfooding finding ⇒ overlay (docs/testing.md standing rule; owner dogfooding
// 2026-07-23): the two data states the owner's real database exposed on Today,
// reproduced via ADR 0081 overlays so the tolerant render is pinned in CI forever.
test.describe("Today dogfooding states — orphaned evidence + pruned feed", { tag: "@clickable" }, () => {
  test("orphaned evidence (null title + gone rule) renders a category fallback, never blank or crashed", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["orphaned-evidence"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    // No generic error fallback — the missing rule/signal must not crash the stream.
    await expect(page.locator(".app-error-recovery")).toHaveCount(0);

    const stream = page.getByLabel("Attention stream");
    // Each orphan company's row still renders, with a non-empty statement and its
    // category badge — the fallback, not a blank line. The statement must be the
    // localized generic copy, never a raw trigger enum token like
    // "signal_category" (issue #119).
    for (const ticker of ["ZZO1", "ZZO2", "ZZO3"]) {
      const row = stream.locator("li[data-category='attention']").filter({ hasText: ticker });
      await expect(row).toBeVisible();
      const title = await row.locator(".today-row-title").innerText();
      expect(title.trim().length).toBeGreaterThan(0);
      expect(title.trim()).not.toMatch(/^[a-z0-9]+(?:_[a-z0-9]+)+$/);
      await expect(row.locator(".ui-status-chip").first()).toBeVisible();
    }
    await expectNoPageOverflow(page);
  });

  test("pruned feed renders the surviving snapshot; a glued filename splits off the statement", async ({ page }) => {
    await primeMockScenario(page, { base: "rich", overlays: ["pruned-feed"] });
    await openApp(page);
    await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();

    const stream = page.getByLabel("Attention stream");
    // The clean snapshot title survives the pruned feed row and renders verbatim.
    await expect(stream.getByText(PRUNED_CLEAN_SNAPSHOT)).toBeVisible();

    // The glued snapshot splits: the human part is the statement, the filename drops
    // to a quiet document link — the extension never lands in the statement.
    const gluedRow = stream.locator("li[data-category='attention']").filter({ hasText: "ZZP2" });
    await expect(gluedRow.locator(".today-row-title")).toHaveText(PRUNED_GLUED_HUMAN);
    await expect(gluedRow.locator(".today-row-doc-link")).toHaveText(PRUNED_GLUED_FILENAME);
    await expectNoPageOverflow(page);
  });
});
