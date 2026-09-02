import { test, expect, openApp, setPaneSize, expectTextFits } from "../helpers/harness";
import { shootPanel, shootRegion } from "./helpers";
import type { Locator, Page } from "@playwright/test";

// Visual baseline — Quality + Report-documents panels (ADR 0076 D7 / U11),
// mirroring density-quality-docs.spec.ts.

function nav(page: Page) {
  return page.getByLabel(/Primary navigation|Nawigacja główna/);
}

// F3a S3 (ADR 0107): opening a company lands the Spółka screen; each tool
// opens via the ⌘K palette.
async function openCompany(page: Page) {
  await nav(page).getByRole("button", { name: "Companies" }).click();
  await page.locator('[data-company-id="company_gpw_cdr"] .company-row-main').click();
  await page.getByRole("region", { name: "Company view" }).waitFor();
}

async function openTool(page: Page, label: string): Promise<Locator> {
  await openCompany(page);
  await page.keyboard.press("Control+K");
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill(label);
  await palette.getByRole("button", { name: label, exact: true }).first().click();
  // `.spolka-layout`, not the tool group itself, carries the density
  // contracts' `container: pane / size` (spolka.css).
  await expect(page.getByRole("group", { name: "Workshop tool" })).toBeVisible();
  return page.locator(".spolka-layout");
}

async function openQuality(page: Page): Promise<Locator> {
  return openTool(page, "Open quality");
}

async function openDocuments(page: Page): Promise<Locator> {
  return openTool(page, "Open documents");
}

async function openCoverage(page: Page): Promise<Locator> {
  return openTool(page, "Open coverage");
}

test.describe("visual — quality + report documents", () => {
  test("Quality across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openQuality(page);
    await expect(pane.locator(".quality-scorecard-summary")).toBeVisible();
    await shootPanel(page, pane, "quality");
  });

  test("Report documents across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openDocuments(page);
    await expect(pane.getByRole("link").first()).toBeVisible();
    await shootPanel(page, pane, "report-documents");
  });

  test("Coverage across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openCoverage(page);
    await expect(pane.locator("table.coverage-table")).toBeVisible();
    await shootPanel(page, pane, "coverage");
  });

  // The pane shot above is clipped at the tier height, so the actions footer
  // sat outside every baseline: epic #398 added a third action there and not
  // one baseline pixel moved. Shot as its own region so the actions — the most
  // clicked part of the panel — actually have visual coverage.
  test("Coverage actions footer", async ({ page }) => {
    await openApp(page);
    const pane = await openCoverage(page);
    const actions = pane.locator(".coverage-actions");
    await expect(actions).toBeVisible();
    await shootRegion(page, pane, actions, "coverage-actions");
  });

  // Same blind spot, second instance (ADR 0045 guardrail harvest): the
  // unnamed-positions list is below the fold AND behind a disclosure, so it
  // was in no baseline either — and it shipped with the position name clipped
  // to one character at this width. A region shot catches the layout
  // regression in the pinned renderer (`make check-visual`, a required CI
  // check — #448); host runs execute it without comparing.
  test("Unnamed positions list", async ({ page }) => {
    await openApp(page);
    const pane = await openCoverage(page);
    const capture = pane.locator(".coverage-raw-capture");
    await capture.getByRole("button", { name: /Show the unnamed positions/ }).click();
    const list = capture.locator(".coverage-uncrosswalked-concepts");
    await expect(list).toBeVisible();
    await shootRegion(page, pane, list, "coverage-unnamed-positions");

    // #417 class: the title, the positions counter and the `Show in
    // Fundamentals` action must never clip mid-word ("2 posi…", "Show in
    // Fundamen…"). Ellipsis is tolerated only on the title; the counter and
    // the action label must actually fit. 260px is the pane width at which
    // this panel's own content column (not the pane itself) narrows enough
    // to squeeze the title below its natural width — the real trigger for
    // the #417 mid-word-clip class (root cause: `.ui-section-title`, a
    // shared `src/ui` primitive, had no explicit grid track, so its `h3`
    // child ignored the shrunk flex-item box around it).
    await setPaneSize(page, { width: 260, height: 900, pane });
    await expect(list.locator("h3 [data-ux-text-fit]")).toBeVisible();
    await expectTextFits(list.locator("h3 [data-ux-text-fit]"), { allowEllipsis: true });
    await expectTextFits(
      list.locator(".ui-section-header-meta [data-ux-text-fit], .coverage-uncrosswalked-list [data-ux-text-fit]"),
    );
  });
});
