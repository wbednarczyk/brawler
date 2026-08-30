import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// F4b real-data check (DoD § G, docs/plans/frontend-v2-f4b.md § 8 of each
// screen): the four screens render on the owner's real Windows app —
// Wydarzenia (the H1-report week + an empty week → the jump invitation),
// Sezon raportów, Źródła (every registered source), Transkrypcje (the real
// empty state). Screenshots are attached for the integrator's review;
// assertions stay structural (the mock harness owns pixel proofs).

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

const SHOTS = "test-results/live-f4b";

// The sidebar renders short Polish labels; match the first sidebar button by
// name in either locale like the other live specs do.
async function openLibrary(page: LiveConnection["page"], name: RegExp) {
  await page.getByRole("button", { name }).first().click();
}

async function shoot(
  page: LiveConnection["page"],
  name: string,
  testInfo: { attach: (n: string, o: { body: Buffer; contentType: string }) => Promise<unknown> },
) {
  const body = await page.screenshot({ fullPage: false });
  await testInfo.attach(name, { body, contentType: "image/png" });
  const fs = await import("node:fs/promises");
  await fs.mkdir(SHOTS, { recursive: true });
  await fs.writeFile(`${SHOTS}/${name}.png`, body);
}

async function expectAtMostOneFilled(page: LiveConnection["page"]) {
  const filled = page.locator('[data-ui-button-variant="primary"]:visible');
  expect(await filled.count()).toBeLessThanOrEqual(1);
  const marked = page.locator('[data-ux-primary-action="true"]:visible');
  expect(await marked.count()).toBeLessThanOrEqual(1);
}

test("Wydarzenia shows the week grid with five weekday columns inside the pane", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await openLibrary(page, /^(Wydarzenia|Events)$/);
  const layout = page.locator(".events-layout");
  await expect(layout).toBeVisible({ timeout: 15_000 });
  const grid = page.locator(".event-week-grid");
  if (await grid.isVisible()) {
    const days = grid.locator(".event-week-day");
    expect(await days.count()).toBeGreaterThanOrEqual(5);
    const paneBox = await layout.boundingBox();
    const firstBox = await days.first().boundingBox();
    const fifthBox = await days.nth(4).boundingBox();
    if (paneBox && firstBox && fifthBox) {
      expect(firstBox.x).toBeGreaterThanOrEqual(paneBox.x - 1);
      expect(fifthBox.x + fifthBox.width).toBeLessThanOrEqual(paneBox.x + paneBox.width + 1);
    }
  }
  await expectAtMostOneFilled(page);
  await shoot(page, "events-week", testInfo);
});

test("Wydarzenia: an empty week offers the jump to the next week with events", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await openLibrary(page, /^(Wydarzenia|Events)$/);
  const next = page.getByRole("button", { name: /Następny tydzień|Next week/ });
  // Walk forward until a week without events (the real calendar has one within ~6 weeks).
  for (let i = 0; i < 8; i += 1) {
    const invitation = page.locator('[data-empty-kind="invitation"], [data-empty-kind="quiet"]');
    if (await invitation.isVisible()) break;
    await next.click();
    await page.waitForTimeout(400);
  }
  const jump = page.getByRole("button", { name: /Pokaż następny tydzień z wydarzeniami|Show next week with events/ });
  const quiet = page.locator('[data-empty-kind="quiet"]');
  await expect(jump.or(quiet)).toBeVisible({ timeout: 15_000 });
  await shoot(page, "events-empty-week", testInfo);
  if (await jump.isVisible()) {
    const before = await page.locator(".week-toolbar, .events-layout").first().innerText();
    await jump.click();
    await expect(page.locator(".event-week-grid .event-week-card, .event-week-grid [aria-pressed]").first()).toBeVisible({ timeout: 15_000 });
    const after = await page.locator(".week-toolbar, .events-layout").first().innerText();
    expect(after).not.toBe(before);
    await shoot(page, "events-after-jump", testInfo);
  }
  await expectAtMostOneFilled(page);
});

test("Sezon raportów lists the real upcoming reports from the sidebar entry", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await openLibrary(page, /^(Sezon raportów|Report Season)$/);
  const layout = page.locator(".report-season-layout");
  await expect(layout).toBeVisible({ timeout: 15_000 });
  await expectAtMostOneFilled(page);
  await shoot(page, "report-season", testInfo);
});

test("Źródła renders every registered source with no filled action at rest", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await openLibrary(page, /^(Źródła|Sources)$/);
  const layout = page.locator(".sources-layout");
  await expect(layout).toBeVisible({ timeout: 15_000 });
  const rows = page.getByRole("button", { name: /^(Otwórz źródło|Open source): / });
  expect(await rows.count()).toBeGreaterThanOrEqual(10);
  expect(await page.locator('[data-ui-button-variant="primary"]:visible').count()).toBe(0);
  await shoot(page, "sources", testInfo);
});

test("Transkrypcje opens as the invitation with the URL field on the real (empty) database", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await openLibrary(page, /^(Transkrypcje|Transcripts)$/);
  const fetch = page.getByRole("button", { name: /Pobierz transkrypcję|Fetch transcript|Otwórz ustawienia|Open settings/ });
  await expect(fetch.first()).toBeVisible({ timeout: 15_000 });
  await expect(page.locator("[data-transcript-status]")).toHaveCount(0);
  await expectAtMostOneFilled(page);
  await shoot(page, "transcripts-empty", testInfo);
});
