import {
  test,
  expect,
  openApp,
  setPaneSize,
  resetPaneSize,
  expectNoPageOverflow,
  expectNoHorizontalOverflow,
} from "./helpers/harness";
import { primeMockScenario, type ScenarioSpec } from "./helpers/mockRuntime";
import type { Locator, Page } from "@playwright/test";

// Panel density matrix (ADR 0076 D6 / U7). Every panel/screen defines
// behavior at pane-width tiers (S <420 · M 420–760 · L >760) and height tiers
// (short <480 · tall ≥480), driven by `@container pane (…)` rules on the named
// `pane` size container (.spolka-layout / .workspace, added in U7). This spec is
// the table-driven runner the per-panel CLUSTER tasks extend: each appends a
// PANEL_CONTRACTS entry with its opener and per-tier assertions. `setPaneSize`
// forces the hosting pane's inline size so the container queries fire regardless
// of the window/dock-cell size (jsdom has no container queries, so the tier
// behavior can only be asserted here in a real browser — the disclosure/toggle
// semantics are unit-tested in src/ui/primitives.test.tsx).

type PaneLocator = Locator;
// Each check runs with the pane already forced to its tier size.
type TierCheck = (page: Page, pane: PaneLocator) => Promise<void>;

type PanelContract = {
  panel: string;
  // Opens the panel and returns the pane Locator to size (a panel may live in a
  // multi-pane dashboard, so the pane is not always the first `.spolka-layout`).
  open: (page: Page) => Promise<PaneLocator>;
  tiers: { S?: TierCheck; M?: TierCheck; L?: TierCheck; short?: TierCheck };
  // Seeded BEFORE `openApp` (mock reads happen once at bootstrap) — only the
  // contracts that need non-default data set this (Today's S-tier row cap +
  // hostile-title wrap needs a dense, hostile-content day).
  scenario?: ScenarioSpec;
};

const TIER_SIZE = {
  S: { width: 380, height: 700 },
  M: { width: 600, height: 700 },
  L: { width: 900, height: 700 },
  short: { width: 900, height: 440 },
} as const;

const TIER_ORDER = ["S", "M", "L", "short"] as const;

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

// Reference opener: the company Notebook tool owns the "existing container
// query" the Notebook contract row references — the `.notebook-workspace`
// list∥detail grid in CompanyNotebookSection. The tier behavior is exercised
// through the Spółka screen's "notatnik" workshop tool (F3a S2/S3, ADR 0107;
// the Notebooks-global screen it once shared this contract with retired in
// F4c S2, ADR 0108 amendment — every deep link now lands here too).
async function openCompanyNotebook(page: Page): Promise<PaneLocator> {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.getByRole("button", { name: "Open GPW:CDR" }).click();
  await expect(page.getByRole("region", { name: "Company view" })).toBeVisible();
  await page.getByRole("button", { name: "Notebook", exact: true }).click();
  const pane = page.locator(".spolka-layout");
  await expect(pane).toBeVisible();
  await expect(pane.locator(".notebook-workspace")).toBeVisible();
  return pane;
}

async function notebookColumns(page: Page): Promise<{ list: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>>; detail: NonNullable<Awaited<ReturnType<Locator["boundingBox"]>>> }> {
  const list = await page.locator(".notebook-list").first().boundingBox();
  const detail = await page.locator(".notebook-detail").first().boundingBox();
  expect(list, "notebook list rendered").not.toBeNull();
  expect(detail, "notebook detail rendered").not.toBeNull();
  return { list: list!, detail: detail! };
}

async function expectStacked(page: Page) {
  const { list, detail } = await notebookColumns(page);
  // One column: list and detail share the left edge, detail sits BELOW the list.
  expect(Math.abs(detail.x - list.x), "detail shares the list's column").toBeLessThan(6);
  expect(detail.y, "detail stacked below the list").toBeGreaterThan(list.y + 10);
}

async function expectSideBySide(page: Page) {
  const { list, detail } = await notebookColumns(page);
  // Two columns: detail sits to the RIGHT of the list on the same row.
  expect(detail.x, "detail beside the list").toBeGreaterThan(list.x + list.width / 2);
  expect(Math.abs(detail.y - list.y), "list and detail on one row").toBeLessThan(40);
}

// Today is the default landing screen (ADR 0054) — no navigation needed, just
// the heading. Seeded via the contract's `scenario` (below): `today-dense`
// stamps its 29 autopilot runs onto the scenario's own newest day (not a
// fixed date — the mock's browser seed, `browserSmokeRuntime.ts`, already
// carries a fixed CD Projekt run further out than any base-scenario date),
// so the dense wall always lands in a VISIBLE slot, never rolled into
// "Wcześniej". `hostile-content`'s long, unbroken ESPI title stays pinned to
// SAMPLE_NOW — an older, only-3-row day that itself never gets capped, but
// still falls behind the mock's OTHER fixed seed dates (per-company
// `periodic_report` calendar events) into "Wcześniej" — expand it once so
// the hostile row is actually in the DOM to assert against.
async function openToday(page: Page): Promise<PaneLocator> {
  await expect(page.getByRole("heading", { name: "Today", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Open days" }).click();
  return page.locator(".workspace");
}

function todayDenseSection(page: Page): Locator {
  return page.locator(".dayq-section").first();
}

async function expectTodayNoInnerOverflow(page: Page, pane: PaneLocator) {
  // The pane-level check (`expectNoPageOverflow`, run by the loop below) is
  // document-wide; this additionally scopes to the day-section containers
  // themselves (contract §"no horizontal scroll on the pane AND on the inner
  // day-section containers").
  await expectNoHorizontalOverflow(pane, ".dayq-section, .dayq-day-rows, .dayq-row");
}

const PANEL_CONTRACTS: PanelContract[] = [
  {
    panel: "Notebook",
    open: openCompanyNotebook,
    tiers: {
      // S/M stack below the L tier (pane ≤ 759px); L shows list ∥ detail. The
      // S-tier single-column TOGGLE (list OR detail) is a follow-up cluster task
      // (cluster C) — only the stacking boundary is asserted here.
      S: (page) => expectStacked(page),
      M: (page) => expectStacked(page),
      L: (page) => expectSideBySide(page),
    },
  },
  {
    panel: "Today",
    open: openToday,
    scenario: { base: "rich", overlays: ["hostile-content", "today-dense"] },
    tiers: {
      // S top-3+zwiń (plan Dense row): the dense day caps to 3 rows + a
      // "+N · Otwórz dzień" recovery line, zero horizontal scroll.
      S: async (page, pane) => {
        const dense = todayDenseSection(page);
        await expect(dense.locator(".dayq-row")).toHaveCount(3);
        await expect(dense.locator(".dayq-day-cap")).toBeVisible();
        // The hostile-content overlay's own (uncapped, 3-row) day is always
        // fully visible — the Dense row's "hostile long ESPI titles wrap
        // inside cards" case.
        await expect(page.getByText("Zażółć gęślą jaźń", { exact: false })).toBeVisible();
        await expectTodayNoInnerOverflow(page, pane);
      },
      // M/L: no cap, every row renders (Dense row "M/L wszystkie").
      M: async (page, pane) => {
        const dense = todayDenseSection(page);
        await expect(dense.locator(".dayq-day-cap")).toHaveCount(0);
        await expect(dense.locator(".dayq-row").first()).toBeVisible();
        await expectTodayNoInnerOverflow(page, pane);
      },
      L: async (page, pane) => {
        const dense = todayDenseSection(page);
        await expect(dense.locator(".dayq-day-cap")).toHaveCount(0);
        await expectTodayNoInnerOverflow(page, pane);
      },
    },
  },
];

test.describe("panel density matrix", { tag: "@clickable" }, () => {
  for (const contract of PANEL_CONTRACTS) {
    test(`${contract.panel} honors its pane-density tiers`, async ({ page }) => {
      // Priming must happen BEFORE `openApp`'s navigation (bootstrap reads
      // happen once at mount) — contracts that need non-default data set
      // `scenario`; the rest keep today's default (no priming).
      if (contract.scenario) await primeMockScenario(page, contract.scenario);
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
});
