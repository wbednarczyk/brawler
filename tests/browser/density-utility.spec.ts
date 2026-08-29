import {
  test,
  expect,
  openApp,
  openScreen,
  setPaneSize,
  resetPaneSize,
  expectNoPageOverflow,
} from "./helpers/harness";
import type { Locator, Page } from "@playwright/test";

// U7-E2 density contracts (ADR 0076 D6): Watchlists · Transcripts · Settings ·
// Diagnostics. jsdom has no container queries, so the *tier switch itself* can
// only be asserted in a real browser — this spec forces the hosting `pane` size
// container (.workspace for sidebar screens / .spolka-layout for company tools)
// to each tier via `setPaneSize` and asserts the contract's visibility rules
// plus the no-page-overflow gate. The disclosure/select *semantics* are
// unit-tested in each screen's *.test.tsx (jsdom-visible). Runner shape mirrors
// tests/browser/density-matrix.spec.ts.
//
// Diagnostics is intentionally absent: it is developer-gated and the browser
// smoke runtime ships developerMode:false with no mocked unlock path (the real
// unlock is a hidden Ctrl+Alt+Shift+D chord → passphrase → backend command), so
// it is unreachable here. Its tiers are covered by the reachable-fold unit tests
// in src/screens/Diagnostics/DiagnosticsScreen.test.tsx.

const TIER = {
  S: { width: 380, height: 700 },
  M: { width: 600, height: 700 },
  L: { width: 900, height: 700 },
  short: { width: 900, height: 440 },
} as const;

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

async function sizeTo(page: Page, tier: keyof typeof TIER, pane: Locator) {
  await resetPaneSize(page, pane);
  await setPaneSize(page, { ...TIER[tier], pane });
}

test.describe("U7-E2 density contracts", { tag: "@clickable" }, () => {
  test("Watchlists (sidebar) folds detail below M and folds the ISIN column below L", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Watchlists" }).click();

    // Seed a watchlist with one member so the company table has an ISIN cell to
    // fold at the M tier.
    await page.getByLabel("Watchlist name").fill("Density check");
    await page.getByRole("button", { name: "Create" }).click();
    const row = page
      .getByLabel("Watchlists", { exact: true })
      .getByRole("button")
      .filter({ hasText: "Density check" });
    await row.click();
    const detail = page.getByLabel("Selected watchlist");
    await detail.getByRole("button", { name: "Add companies" }).click();
    const picker = page.getByLabel("Add companies", { exact: true });
    await picker.locator(".watchlist-picker-row").first().click();
    await picker.getByRole("button", { name: "Add selected" }).click();
    await expect(page.locator(".watchlist-member-row")).toHaveCount(1);

    const pane = page.locator(".workspace");
    // The ISIN renders as a meta span nested inside the name cell, not its own
    // grid column (F4a S3 redesign: ticker | name (+ ISIN) | actions).
    const isinCell = page.locator(".watchlist-member-row .watchlist-member-isin").first();

    // L (>760): detail visible + full company table (ISIN column shown).
    await sizeTo(page, "L", pane);
    await expect(detail).toBeVisible();
    await expect(isinCell).toBeVisible();
    await expectNoPageOverflow(page);

    // M (420–760): detail (membership editor) visible; the decorative ISIN
    // column folds while the ticker identity stays.
    await sizeTo(page, "M", pane);
    await expect(detail).toBeVisible();
    await expect(isinCell).toBeHidden();
    await expectNoPageOverflow(page);

    // S (<420): names + counts only — the detail folds.
    await sizeTo(page, "S", pane);
    await expect(page.locator(".watchlist-list")).toBeVisible();
    await expect(detail).toBeHidden();
    await expectNoPageOverflow(page);

    // short (<480h): names + counts only.
    await sizeTo(page, "short", pane);
    await expect(page.locator(".watchlist-list")).toBeVisible();
    await expect(detail).toBeHidden();
    await expectNoPageOverflow(page);
  });

  // The cockpit-hosted "Watchlists" global panel (opened via the retired
  // "Add panel" surface) no longer exists (F3a S3, ADR 0107 decision 5; ADR 0108) —
  // Watchlists has always had its own sidebar nav button too (unlike
  // Research/Events/Notebooks/Report Season, it never gained an "Open
  // screen: …" palette entry, since it already had one). This exercises the
  // same `.watchlists-workspace` component through its one surviving entry
  // point, sized via the screen's own `.workspace` pane container.
  test("Watchlists (sidebar screen, forced to a narrow pane) folds detail below M", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Watchlists" }).click();
    const pane = page.locator(".workspace");
    await expect(pane.locator(".watchlists-workspace")).toBeVisible();
    const detail = pane.getByLabel("Selected watchlist");
    const list = pane.locator(".watchlist-list");

    await sizeTo(page, "L", pane);
    await expect(detail).toBeVisible();
    await expectNoPageOverflow(page);

    await sizeTo(page, "M", pane);
    await expect(detail).toBeVisible();
    await expectNoPageOverflow(page);

    await sizeTo(page, "S", pane);
    await expect(list).toBeVisible();
    await expect(detail).toBeHidden();
    await expectNoPageOverflow(page);

    await sizeTo(page, "short", pane);
    await expect(list).toBeVisible();
    await expect(detail).toBeHidden();
    await expectNoPageOverflow(page);
  });

  test("Transcripts folds the segment review behind a disclosure at S/short", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Transcripts" }).click();
    await expect(page.getByLabel("Transcript jobs")).toBeVisible();

    // The smoke runtime seeds no transcript jobs, so create one (the stateful
    // create reflects into the list), then expand it to reveal the detail — the
    // segments disclosure lives there regardless of whether segments loaded.
    await page.getByLabel("Transcript URL").fill("https://www.youtube.com/watch?v=densitycheck");
    await page.getByRole("button", { name: "Create job" }).click();
    await expect(page.locator(".transcript-row")).toHaveCount(1);
    await page.locator(".transcript-row").first().click();
    const segmentsToggle = page.getByRole("button", { name: "Segments" });
    const pane = page.locator(".workspace");

    // M/L: segments render inline; the disclosure toggle is hidden.
    await sizeTo(page, "L", pane);
    await expect(segmentsToggle).toBeHidden();
    await expectNoPageOverflow(page);

    await sizeTo(page, "M", pane);
    await expect(segmentsToggle).toBeHidden();
    await expectNoPageOverflow(page);

    // S: the segment review folds behind the "Segments" disclosure.
    await sizeTo(page, "S", pane);
    await expect(segmentsToggle).toBeVisible();
    await expectNoPageOverflow(page);

    // short: list only — the disclosure gates the segments here too.
    await sizeTo(page, "short", pane);
    await expect(segmentsToggle).toBeVisible();
    await expectNoPageOverflow(page);
  });

  test("Settings collapses its section tab list to a select at S", async ({ page }) => {
    await openApp(page);
    await nav(page).getByRole("button", { name: "Settings" }).click();
    await expect(page.getByLabel("Application settings")).toBeVisible();

    const pane = page.locator(".workspace");
    const subnav = page.locator(".settings-subnav");
    const sectionSelect = page.getByLabel("Settings section", { exact: true });

    // M/L: the tab list (Subnav) drives navigation; the select is hidden.
    await sizeTo(page, "L", pane);
    await expect(subnav).toBeVisible();
    await expect(sectionSelect).toBeHidden();
    await expectNoPageOverflow(page);

    await sizeTo(page, "M", pane);
    await expect(subnav).toBeVisible();
    await expect(sectionSelect).toBeHidden();
    await expectNoPageOverflow(page);

    // S: the tab list collapses to the section select, which drives the switch.
    await sizeTo(page, "S", pane);
    await expect(subnav).toBeHidden();
    await expect(sectionSelect).toBeVisible();
    await sectionSelect.selectOption("transcripts");
    await expect(page.getByRole("heading", { name: "Transcripts", exact: true })).toBeVisible();
    await expectNoPageOverflow(page);
  });
});
