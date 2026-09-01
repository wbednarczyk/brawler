import {
  test,
  expect,
  openApp,
  setPaneSize,
  resetPaneSize,
  expectNoPageOverflow,
  expectNoHorizontalOverflow,
  expectNoOverlap,
} from "./helpers/harness";
import { expectFilledAtRest } from "./helpers/interactionContracts";
import type { Locator, Page } from "@playwright/test";

// Panel density matrix — U7 cluster E1 (ADR 0076 D6): Inbox + Sources. These are
// full-screen sidebar screens hosted directly in `.workspace`, the named `pane`
// size container (shell.css), so `setPaneSize(page, { …, pane })` forces the
// workspace's own inline size and the `@container pane (…)` rules resolve against
// it regardless of the real window/dock cell. Runner pattern mirrors
// density-matrix.spec.ts; jsdom has no container queries, so the tier switch
// itself is only assertable in a real browser (the affordance DOM + toggle
// semantics are unit-tested in InboxScreen.test.tsx / SourcesScreen.test.tsx).

const TIER_SIZE = {
  S: { width: 380, height: 700 },
  M: { width: 600, height: 700 },
  L: { width: 900, height: 700 },
  short: { width: 900, height: 440 },
} as const;

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

function workspace(page: Page): Locator {
  return page.locator(".workspace");
}

type Box = NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>;

async function box(locator: Locator): Promise<Box> {
  const value = await locator.boundingBox();
  expect(value, "element has a layout box").not.toBeNull();
  return value!;
}

async function openInboxScreen(page: Page): Promise<Locator> {
  await nav(page).getByRole("button", { name: "Inbox" }).click();
  await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
  return workspace(page);
}

async function openSourcesScreen(page: Page): Promise<Locator> {
  await nav(page).getByRole("button", { name: "Sources" }).click();
  await expect(page.getByLabel("Source list")).toBeVisible();
  return workspace(page);
}

test.describe("Inbox density contract", { tag: "@clickable" }, () => {
  test("reshapes list/detail across pane tiers with an S overlay", async ({ page }) => {
    await openApp(page);
    const pane = await openInboxScreen(page);
    const list = page.locator(".feed-panel");
    const detail = page.locator(".detail-pane");
    const firstRow = page.locator(".feed-row").first();

    // L (>760): list ∥ detail — detail sits to the RIGHT of the list on one row.
    await setPaneSize(page, { ...TIER_SIZE.L, pane });
    {
      const listBox = await box(list);
      const detailBox = await box(detail);
      expect(detailBox.x, "detail beside the list at L").toBeGreaterThan(listBox.x + listBox.width / 2);
      expect(Math.abs(detailBox.y - listBox.y), "list and detail share a row at L").toBeLessThan(40);
    }
    await expectNoPageOverflow(page);

    // M (420–760): list + detail stacked — detail shares the list's left edge and
    // sits below it.
    await setPaneSize(page, { ...TIER_SIZE.M, pane });
    {
      const listBox = await box(list);
      const detailBox = await box(detail);
      expect(Math.abs(detailBox.x - listBox.x), "detail shares the list column at M").toBeLessThan(6);
      expect(detailBox.y, "detail stacked below the list at M").toBeGreaterThan(listBox.y + 10);
    }
    await expectNoPageOverflow(page);

    // S (<420): list only. With no selection the detail is hidden and the list
    // fills the pane; selecting an item raises the detail as a full-pane overlay
    // with a visible back control that returns to the list.
    await setPaneSize(page, { ...TIER_SIZE.S, pane });
    await expect(detail).toBeHidden();
    await expect(list).toBeVisible();
    await expectNoPageOverflow(page);

    await firstRow.click();
    await expect(detail).toBeVisible();
    {
      // The overlay is a full-pane overlay covering the list: it starts at or
      // before the list's left edge and spans past its right edge (not a narrow
      // side rail).
      const listBox = await box(list);
      const detailBox = await box(detail);
      expect(detailBox.x, "overlay starts at/left of the list at S").toBeLessThanOrEqual(listBox.x + 1);
      expect(
        detailBox.x + detailBox.width,
        "overlay spans past the list's right edge at S",
      ).toBeGreaterThanOrEqual(listBox.x + listBox.width - 1);
    }
    const back = page.getByRole("button", { name: "Back to list" });
    await expect(back).toBeVisible();
    await expectNoPageOverflow(page);

    await back.click();
    await expect(detail).toBeHidden();
    await expect(list).toBeVisible();
    await expectNoPageOverflow(page);

    await resetPaneSize(page, pane);
  });

  // #417 class: a REPORT-kind item's attachment filenames are real-world
  // hostile — long, mixed underscores/spaces, Polish diacritics, no natural
  // break points (`feed_results_report` in browserSmokeRuntime.ts, already
  // shaped for this). At M/L the detail pane's own scroll container
  // (`.detail-pane`, F1 S4/S5) and the document-list container it hosts
  // (`.feed-doc-list`, FeedDetailReport) must contain it — never a horizontal
  // scrollbar/blowout — and the report body must not paint over the company-
  // context block stacked below it (#416 class).
  test("contains a hostile report attachment filename at M/L, S overlay unaffected", async ({ page }) => {
    await openApp(page);
    const pane = await openInboxScreen(page);
    const hostileRow = page.locator('[data-feed-item-id="feed_results_report"]');
    const detail = page.locator(".detail-pane");
    const docList = page.locator(".feed-doc-list");
    const contextSection = page.locator(".feed-context-section");

    // The row sits past the visible fold of a long list — search narrows the
    // list to just this item so it's the (stable, unscrolled) only row,
    // rather than fighting the sticky filter-toolbar's scroll-position dance.
    // The row's own click handler TOGGLES selection (`toggleFeedItem`) — never
    // click it a second time while already selected, or it deselects.
    await page.getByRole("textbox", { name: /Search|Szukaj/ }).first().fill("wyniki za I półrocze");
    await expect(hostileRow).toBeVisible();

    // Click at the default pane size: Inbox auto-selects the first item, and at
    // a forced M size that open detail overlays the list and intercepts the
    // pointer. Selection survives the tier changes below.
    await hostileRow.click();

    for (const tier of ["M", "L"] as const) {
      await setPaneSize(page, { ...TIER_SIZE[tier], pane });
      await expect(detail).toBeVisible();
      await expect(docList).toBeVisible();
      await expectNoHorizontalOverflow(detail);
      await expectNoHorizontalOverflow(docList);
      await expectNoOverlap(docList, contextSection, `report body vs. company context at ${tier}`);
      await expectNoPageOverflow(page);
    }

    // S: the pre-existing full-pane overlay contract (this slice touched no
    // overlay CSS) — verified here rather than assumed. Selection survives a
    // tier change (list-only state), so close the still-open overlay from the
    // L-tier selection above before asserting the "no selection ⇒ hidden"
    // baseline the S contract test also relies on.
    await setPaneSize(page, { ...TIER_SIZE.S, pane });
    await page.getByRole("button", { name: "Back to list" }).click();
    await expect(detail).toBeHidden();
    await expect(page.locator(".feed-panel")).toBeVisible();
    await hostileRow.click();
    await expect(detail).toBeVisible();
    await expect(docList).toBeVisible();
    await expectNoHorizontalOverflow(detail);
    await expectNoHorizontalOverflow(docList);
    await expectNoPageOverflow(page);

    await resetPaneSize(page, pane);
  });

  test("reduces the visible filters at the short height tier", async ({ page }) => {
    await openApp(page);
    const pane = await openInboxScreen(page);
    const search = page.getByRole("textbox", { name: /Search|Szukaj/ }).first();
    const watchlistFilter = page.getByLabel("Inbox watchlist");
    const disclosure = page.locator(".feed-panel .filter-toolbar-disclosure");

    // Tall: all filter controls are visible inline, no disclosure needed.
    await setPaneSize(page, { ...TIER_SIZE.L, pane });
    await expect(watchlistFilter).toBeVisible();
    await expect(disclosure).toBeHidden();
    await expectNoPageOverflow(page);

    // Short: the secondary selects fold behind the "Filters" disclosure while the
    // search field stays visible.
    await setPaneSize(page, { ...TIER_SIZE.short, pane });
    await expect(search).toBeVisible();
    await expect(disclosure).toBeVisible();
    await expect(watchlistFilter).toBeHidden();
    await expectNoPageOverflow(page);

    await resetPaneSize(page, pane);
  });
});

test.describe("Sources density contract", { tag: "@clickable" }, () => {
  test("folds schedule and diagnostics summaries across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openSourcesScreen(page);
    const schedule = page.locator(".source-row-schedule").first();
    const diagnostics = page.locator(".source-row-diagnostics").first();

    // S (<420): source rows + status chip only — schedule + diagnostics fold.
    await setPaneSize(page, { ...TIER_SIZE.S, pane });
    await expect(schedule).toBeHidden();
    await expect(diagnostics).toBeHidden();
    await expectNoPageOverflow(page);

    // M (420–760): schedule/settings inline; diagnostics still folded.
    await setPaneSize(page, { ...TIER_SIZE.M, pane });
    await expect(schedule).toBeVisible();
    await expect(diagnostics).toBeHidden();
    await expectNoPageOverflow(page);

    // L (>760): + diagnostics column.
    await setPaneSize(page, { ...TIER_SIZE.L, pane });
    await expect(schedule).toBeVisible();
    await expect(diagnostics).toBeVisible();
    await expectNoPageOverflow(page);

    // short (<480h): rows only — both summaries fold regardless of width.
    await setPaneSize(page, { ...TIER_SIZE.short, pane });
    await expect(schedule).toBeHidden();
    await expect(diagnostics).toBeHidden();
    await expectNoPageOverflow(page);

    await resetPaneSize(page, pane);
  });

  // F4b S4 (contract § Sources, shared guardrail table): the language pass
  // adds no new fold tiers — this proves the existing group layout holds
  // together (no sibling-group overlap, no page overflow) and that the
  // screen's `expectSinglePrimary(root, 0)` invariant (Sources never has a
  // primary action) holds in the real browser too, across S/M/L.
  test("adjacent source groups never overlap, and no primary renders, at S/M/L", async ({ page }) => {
    await openApp(page);
    const pane = await openSourcesScreen(page);
    const groups = page.locator(".source-group");

    for (const tier of ["S", "M", "L"] as const) {
      await setPaneSize(page, { ...TIER_SIZE[tier], pane });
      await expect(groups.first()).toBeVisible();
      const groupCount = await groups.count();
      if (groupCount > 1) {
        await expectNoOverlap(groups.nth(0), groups.nth(1), `adjacent source groups at ${tier}`);
      }
      await expectNoPageOverflow(page);
      await expectFilledAtRest(pane, { max: 0 });
      await resetPaneSize(page, pane);
    }
  });
});
