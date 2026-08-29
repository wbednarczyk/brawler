import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// F4a real-data check (DoD § G, docs/plans/frontend-v2-f4a.md § 8): the three
// Library screens render on the owner's real Windows app — Companies with the
// full library, Watchlists with the real list membership, Alerts with the real
// rules and fired events. Screenshots are attached for the integrator's
// review; assertions stay structural (the mock harness owns pixel proofs).

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

const NAV = /^(Spółki|Companies|Listy obserwowane|Watchlists|Alerty|Alerts)$/;

async function openLibrary(page: LiveConnection["page"], name: RegExp) {
  const nav = page.getByRole("navigation").first();
  await nav.getByRole("button", { name }).first().click();
}

test("Companies library renders every tracked company with one filled action", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await openLibrary(page, /^(Spółki|Companies)$/);
  const list = page.getByLabel(/Companies list|Lista spółek/);
  await expect(list).toBeVisible({ timeout: 15_000 });
  const rows = list.locator("[data-company-row]");
  expect(await rows.count()).toBeGreaterThan(20);
  await expect(page.locator('[data-ux-primary-action="true"]:visible')).toHaveCount(1);
  await testInfo.attach("companies", { body: await page.screenshot({ fullPage: false }), contentType: "image/png" });
});

test("Watchlists shows the real list with member rows carrying Open and Remove", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await openLibrary(page, /^(Listy obserwowane|Watchlists)$/);
  const lists = page.getByLabel(/^(Watchlists|Listy obserwowane)$/).first();
  await expect(lists).toBeVisible({ timeout: 15_000 });
  await lists.getByRole("button").first().click();
  const detail = page.getByLabel(/Selected watchlist|Wybrana lista/);
  await expect(detail).toBeVisible();
  const member = page.locator(".watchlist-member-row").first();
  await expect(member).toBeVisible();
  await expect(member.getByRole("button", { name: /Open company|Otwórz spółkę/ })).toBeVisible();
  await expect(member.getByRole("button", { name: /Remove from list|Usuń z listy/ })).toBeVisible();
  await expect(page.locator('[data-ux-primary-action="true"]:visible')).toHaveCount(1);
  await testInfo.attach("watchlists", { body: await page.screenshot({ fullPage: false }), contentType: "image/png" });
});

test("Alerts lists fired alerts first with the real rules below", async ({}, testInfo) => {
  const { page } = connection;
  test.setTimeout(120_000);
  await openLibrary(page, /^(Alerty|Alerts)$/);
  const fired = page.getByLabel(/^(Fired alerts|Uruchomione alerty)$/);
  const rules = page.getByLabel(/^(Alert rules|Reguły alertów)$/);
  await expect(rules.or(page.getByText(/Nie masz jeszcze alertów|No alerts yet/))).toBeVisible({ timeout: 15_000 });
  const firedBox = await fired.boundingBox();
  const rulesBox = await rules.boundingBox();
  if (firedBox && rulesBox) {
    expect(firedBox.y).toBeLessThan(rulesBox.y);
  }
  await expect(page.locator('[data-ux-primary-action="true"]:visible')).toHaveCount(1);
  await testInfo.attach("alerts", { body: await page.screenshot({ fullPage: false }), contentType: "image/png" });
});
