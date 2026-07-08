import { test, expect, openApp, openCockpitPanel, setPaneSize } from "../helpers/harness";
import { shootPanel } from "./helpers";
import type { Locator, Page } from "@playwright/test";

// Visual baseline — Research / Events / Report Season panels (ADR 0076 D7 / U11),
// mirroring density-research-events.spec.ts.

function paneWith(page: Page, rootSelector: string): Locator {
  return page.locator(".cockpit-pane", { has: page.locator(rootSelector) });
}

async function openResearch(page: Page): Promise<Locator> {
  await openCockpitPanel(page, "Research");
  const pane = paneWith(page, ".research-panel");
  await expect(pane).toBeVisible();
  return pane;
}

async function openEvents(page: Page): Promise<Locator> {
  await openCockpitPanel(page, "Events");
  const pane = paneWith(page, ".events-layout");
  await expect(pane).toBeVisible();
  return pane;
}

async function openReportSeason(page: Page): Promise<Locator> {
  await openCockpitPanel(page, "Report Season");
  const pane = paneWith(page, ".report-season-layout");
  await expect(pane).toBeVisible();
  // Size to L first so the pre-report card mounts, then expand the first row; the
  // expanded state persists across the tier resizes the shoot helper performs.
  await setPaneSize(page, { width: 900, height: 700, pane });
  const firstRow = pane.locator(".report-season-row").first();
  await expect(firstRow).toBeVisible();
  await firstRow.click();
  await expect(pane.locator(".report-season-card").first()).toBeVisible();
  return pane;
}

test.describe("visual — research / events / report season", () => {
  test("Research across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openResearch(page);
    await expect(pane.locator(".research-timeline")).toBeVisible();
    await shootPanel(page, pane, "research");
  });

  test("Events across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openEvents(page);
    await expect(pane.locator(".events-layout")).toBeVisible();
    await shootPanel(page, pane, "events");
  });

  test("Report Season across pane tiers", async ({ page }) => {
    await openApp(page);
    const pane = await openReportSeason(page);
    await shootPanel(page, pane, "report-season");
  });
});
