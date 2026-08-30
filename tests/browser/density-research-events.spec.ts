import { test, expect, openApp, openScreen, setPaneSize, resetPaneSize, expectNoPageOverflow, expectNoOverlap, expectTextFits } from "./helpers/harness";
import { expectFilledAtRest } from "./helpers/interactionContracts";
import type { Locator, Page } from "@playwright/test";

// U7-D panel density contracts (ADR 0076 D6): Research, Events, Report Season.
// Same table-driven runner as density-matrix.spec.ts — each panel appends a
// PANEL_CONTRACTS entry whose `open` returns the pane Locator to size and whose
// `tiers` assert the contract's per-tier visibility after `setPaneSize` forces
// the hosting `pane` size container (.workspace, F3a S3 — these screens are
// standalone routes) to that tier. jsdom has no container queries, so the
// tier switch is browser-only here; the fold/expand and list-override STATE
// is unit-tested in the screens' *.test.tsx files.
//
// The ADR names two audit regressions this cluster must fix — "Research rows
// overlapping the reminder button" and "the Events week calendar reduced to day
// headers" in a short 2×2 pane — asserted as explicit cases below.

type PaneLocator = Locator;
type TierCheck = (page: Page, pane: PaneLocator) => Promise<void>;

type PanelContract = {
  panel: string;
  open: (page: Page) => Promise<PaneLocator>;
  tiers: { S?: TierCheck; M?: TierCheck; L?: TierCheck; short?: TierCheck };
};

const TIER_SIZE = {
  S: { width: 380, height: 700 },
  M: { width: 600, height: 700 },
  L: { width: 900, height: 700 },
  short: { width: 900, height: 440 },
} as const;

const TIER_ORDER = ["S", "M", "L", "short"] as const;

// F3a S3 (ADR 0107 decision 5): Research/Events/Report Season are standalone
// routes (reached via the ⌘K palette's "Open screen: …" entries) —
// `.workspace` (the `<main>` wrapping whatever screen is active, shell.css)
// is their `pane` size container. Asserts the expected screen root actually
// mounted inside it before handing the pane back.
async function paneWith(page: Page, rootSelector: string): Promise<PaneLocator> {
  const pane = page.locator(".workspace");
  await expect(pane.locator(rootSelector)).toBeVisible();
  return pane;
}

async function openResearch(page: Page): Promise<PaneLocator> {
  await openScreen(page, "Research");
  return paneWith(page, ".research-panel");
}

async function openEvents(page: Page): Promise<PaneLocator> {
  await openScreen(page, "Events");
  return paneWith(page, ".events-layout");
}

async function openReportSeason(page: Page): Promise<PaneLocator> {
  await openScreen(page, "Report Season");
  const pane = await paneWith(page, ".report-season-layout");
  // The default pane is small (short); size the pane to L before expanding
  // so the pre-report card is mounted and visible, then let the tier loop resize
  // it (the expanded state persists across resizes).
  await setPaneSize(page, { ...TIER_SIZE.L, pane });
  const firstRow = pane.locator(".report-season-row").first();
  await expect(firstRow).toBeVisible();
  await firstRow.click();
  await expect(pane.locator(".report-season-card").first()).toBeVisible();
  return pane;
}

const PANEL_CONTRACTS: PanelContract[] = [
  {
    panel: "Research",
    open: openResearch,
    tiers: {
      // S: tabs + timeline only; review-queue/questions fold to count chips.
      // (The AI-brief aside was retired with the in-app AI layer, ADR 0084 — the
      // panel no longer renders a `.research-aside` at any tier.) Summary stays.
      S: async (page, pane) => {
        await expect(pane.getByRole("button", { name: /Review queue/ })).toBeVisible();
        await expect(pane.getByRole("button", { name: /Research questions/ })).toBeVisible();
        await expect(pane.locator(".research-timeline")).toBeVisible();
        // The evidence timeline never collapses: standalone hosts at ~768px tall let
        // reminders + questions swallow the stack and the timeline hit 0px (F3a S4 harvest).
        expect((await pane.locator(".research-timeline-shell").boundingBox())?.height ?? 0).toBeGreaterThanOrEqual(200);
        await expect(pane.locator(".research-summary")).toBeVisible();
        await expect(pane.locator(".research-reminders")).toBeHidden();
        await expect(pane.locator(".research-questions")).toBeHidden();
      },
      // M: + review-queue strip (reminders body reveals); questions stay a chip.
      M: async (page, pane) => {
        await expect(pane.locator(".research-reminders")).toBeVisible();
        await expect(pane.getByRole("button", { name: /Research questions/ })).toBeVisible();
        await expect(pane.locator(".research-questions")).toBeHidden();
        await expect(pane.locator(".research-timeline")).toBeVisible();
        // #416: the revealed reminders body must not paint over the questions
        // fold below it — visibility alone cannot see this.
        await expectNoOverlap(
          pane.locator('.research-fold[data-fold="m"]'),
          pane.locator('.research-fold[data-fold="l"]'),
          "Research review-queue fold vs questions fold (M)",
        );
      },
      // L: + questions/reminders columns; both fold chips gone.
      L: async (page, pane) => {
        await expect(pane.locator(".research-reminders")).toBeVisible();
        await expect(pane.locator(".research-questions")).toBeVisible();
        await expect(pane.getByRole("button", { name: /Review queue/ })).toBeHidden();
        await expect(pane.getByRole("button", { name: /Research questions/ })).toBeHidden();
        await expectNoOverlap(
          pane.locator('.research-fold[data-fold="m"]'),
          pane.locator('.research-fold[data-fold="l"]'),
          "Research review-queue fold vs questions fold (L)",
        );
      },
      // short: summary counts + timeline; everything else re-folds behind chips.
      short: async (page, pane) => {
        await expect(pane.locator(".research-summary")).toBeVisible();
        await expect(pane.locator(".research-timeline")).toBeVisible();
        await expect(pane.locator(".research-reminders")).toBeHidden();
      },
    },
  },
  {
    panel: "Events",
    open: openEvents,
    tiers: {
      // S: list mode forced — no week grid, no "Week" option offered.
      S: async (page, pane) => {
        await expect(pane.locator(".event-week-grid")).toHaveCount(0);
        await expect(pane.getByRole("button", { name: "Week", exact: true })).toHaveCount(0);
        await expect(pane.locator(".events-layout")).toBeVisible();
        await expectFilledAtRest(pane, { max: 1 });
      },
      // M: week grid in its bounded scroller (persisted default mode = week).
      M: async (page, pane) => {
        await expect(pane.locator(".event-week-grid")).toBeVisible();
        await expect(pane.locator(".event-week-scroll[data-hscroll]")).toBeVisible();
        await expectFilledAtRest(pane, { max: 1 });
      },
      // L (#431): the `@container pane (min-width: 900px)` override drops the
      // grid's hard minimum — the first and fifth weekday columns must paint
      // fully inside the pane (no horizontal scroll) and their day headers
      // must never wrap/clip a word (#417).
      L: async (page, pane) => {
        await expect(pane.locator(".event-week-grid")).toBeVisible();
        const scroller = pane.locator(".event-week-scroll");
        const scrollerOverflow = await scroller.evaluate((el) => el.scrollWidth - el.clientWidth);
        expect(scrollerOverflow).toBeLessThanOrEqual(1);

        const paneBox = await pane.boundingBox();
        const days = pane.locator(".event-week-day");
        const first = await days.first().boundingBox();
        const last = await days.nth(4).boundingBox();
        expect(paneBox && first && last).toBeTruthy();
        if (paneBox && first && last) {
          expect(first.x).toBeGreaterThanOrEqual(paneBox.x - 1);
          expect(first.x + first.width).toBeLessThanOrEqual(paneBox.x + paneBox.width + 1);
          expect(last.x).toBeGreaterThanOrEqual(paneBox.x - 1);
          expect(last.x + last.width).toBeLessThanOrEqual(paneBox.x + paneBox.width + 1);
        }
        await expectTextFits(pane.locator(".event-week-day-header"));
        await expectFilledAtRest(pane, { max: 1 });
      },
      // short: list mode forced — no clipped week grid.
      short: async (page, pane) => {
        await expect(pane.locator(".event-week-grid")).toHaveCount(0);
        await expect(pane.locator(".events-layout")).toBeVisible();
        await expectFilledAtRest(pane, { max: 1 });
      },
    },
  },
  {
    panel: "Report Season",
    open: openReportSeason,
    tiers: {
      // S: rows only — name+date+state chip; the pre-report card folds away.
      S: async (page, pane) => {
        await expect(pane.locator(".report-season-row").first()).toBeVisible();
        await expect(pane.locator(".report-season-card")).toBeHidden();
      },
      // M: + prep checklist inline (incl. the expectations block); extended
      // context (KPIs, evidence) still folds.
      M: async (page, pane) => {
        await expect(pane.locator(".report-season-card")).toBeVisible();
        await expect(pane.locator(".report-season-card-prep")).toBeVisible();
        await expect(pane.locator(".report-season-expectations").first()).toBeVisible();
        await expect(pane.locator(".report-season-card-extended")).toBeHidden();
      },
      // L: + full pre-report card (prep + expectations + extended context).
      L: async (page, pane) => {
        await expect(pane.locator(".report-season-card-prep")).toBeVisible();
        await expect(pane.locator(".report-season-expectations").first()).toBeVisible();
        await expect(pane.locator(".report-season-card-extended")).toBeVisible();
      },
      // short: rows only.
      short: async (page, pane) => {
        await expect(pane.locator(".report-season-row").first()).toBeVisible();
        await expect(pane.locator(".report-season-card")).toBeHidden();
      },
    },
  },
];

test.describe("panel density — Research / Events / Report Season", { tag: "@clickable" }, () => {
  for (const contract of PANEL_CONTRACTS) {
    test(`${contract.panel} honors its pane-density tiers`, async ({ page }) => {
      await openApp(page);
      const pane = await contract.open(page);
      for (const tier of TIER_ORDER) {
        const check = contract.tiers[tier];
        if (!check) continue;
        await setPaneSize(page, { ...TIER_SIZE[tier], pane });
        await check(page, pane);
        await expectNoPageOverflow(page);
        await resetPaneSize(page, pane);
      }
    });
  }

  // Audit regression (ADR 0076 D6): in a short 2×2 pane the Research rows
  // overlapped the reminder button. The short tier must show summary counts +
  // timeline with the review queue folded — never the full rows on top of it.
  test("Research short cell shows summary + timeline with the review queue folded", async ({ page }) => {
    await openApp(page);
    const pane = await openResearch(page);
    await setPaneSize(page, { ...TIER_SIZE.short, pane });
    await expect(pane.locator(".research-summary")).toBeVisible();
    await expect(pane.locator(".research-timeline")).toBeVisible();
    await expect(pane.getByRole("button", { name: /Review queue/ })).toBeVisible();
    await expect(pane.locator(".research-reminders")).toBeHidden();
    await expectNoPageOverflow(page);
  });

  // Audit regression (ADR 0076 D6): the Events week calendar collapsed to bare day
  // headers in a short/narrow cell. At S and short the list renders and the week
  // grid is not rendered at all — nothing to clip.
  test("Events short and S cells render the list, never a clipped week grid", async ({ page }) => {
    await openApp(page);
    const pane = await openEvents(page);

    await setPaneSize(page, { ...TIER_SIZE.S, pane });
    await expect(pane.locator(".event-week-grid")).toHaveCount(0);
    await expect(pane.locator(".events-layout")).toBeVisible();
    await expectNoPageOverflow(page);
    await resetPaneSize(page, pane);

    await setPaneSize(page, { ...TIER_SIZE.short, pane });
    await expect(pane.locator(".event-week-grid")).toHaveCount(0);
    await expect(pane.locator(".events-layout")).toBeVisible();
    await expectNoPageOverflow(page);
  });

  // J4 (ADR 0071): the expectations composer's period picker + metric rows are
  // the widest new content in the pre-report card. Opened in a narrow M pane with
  // a metric row added, neither the pane nor an inner scroller overflows.
  test("Report Season expectations composer stays within a narrow pane", async ({ page }) => {
    await openApp(page);
    const pane = await openReportSeason(page);
    await setPaneSize(page, { ...TIER_SIZE.M, pane });

    await pane.getByRole("button", { name: /Write expectations/ }).click();
    await expect(pane.locator(".report-season-expectation-composer")).toBeVisible();
    await pane.getByRole("button", { name: /Add metric expectation/ }).click();
    await expect(pane.locator(".report-season-metric-row")).toBeVisible();

    // The pre-report card scrolls internally; assert its own scroll container plus
    // the metric row do not blow out horizontally (document.scrollWidth reads 0
    // for inner overflow — check the container).
    const card = pane.locator(".report-season-card").first();
    const overflow = await card.evaluate(
      (el) => el.scrollWidth - el.clientWidth,
    );
    expect(overflow).toBeLessThanOrEqual(1);
    await expectNoPageOverflow(page);
  });
});
