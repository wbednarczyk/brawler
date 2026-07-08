import { test, expect, openApp, setPaneSize, resetPaneSize, expectNoPageOverflow } from "./helpers/harness";
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
});
