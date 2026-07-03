import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// Minimal live-drive proof (ADR 0066): connect to the real running Brawler app
// over CDP, assert the app shell actually rendered, capture a screenshot for
// human review, and log the app version. Deliberately non-destructive — this
// suite never creates, edits, or deletes data in the real local database.
//
// Requires a live app with remote debugging enabled: run
// scripts/windows/dev-live.ps1 on Windows first, then `make live-drive` here.

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  // For a CDP-attached Browser, .close() disconnects Playwright and clears any
  // contexts *it* created — it does not terminate the real app process, since
  // we never launched it and only attached to its existing context.
  await connection.browser.close();
});

test("live app shell renders and reports its version", async () => {
  const { page } = connection;

  const sidebarNav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await expect(sidebarNav).toBeVisible();

  const versionBadge = page.locator(".brand-version");
  await expect(versionBadge).toBeVisible();
  const versionText = await versionBadge.textContent();
  console.log(`Live Brawler app version: ${versionText}`);
  expect(versionText).toMatch(/^v\d+\.\d+\.\d+/);

  await page.screenshot({ path: "test-results/live/app-shell.png", fullPage: true });
});
