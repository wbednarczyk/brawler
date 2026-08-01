import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import { driveSourcesRefresh } from "./helpers/sourcesRefresh";

// v0.56 pivot live verification (ADR 0072 as amended 2026-07-16): drive a MANUAL
// source refresh in the real Windows app so the fixed BiznesRadar akcjonariat
// parser rewrites the aggregator bases (migration 0088 cleared the garbage ones),
// then confirm the Ownership section renders sane percentages. The refresh spans
// every adapter (BiznesRadar walks ~50 company pages with a politeness delay),
// so the wait is generously long.
//
// Contention (issue #308): the app is the OWNER'S — a sweep may already be in
// flight when this starts, and demanding an idle refresh button made this fail
// as a product defect. `driveSourcesRefresh` joins an in-flight sweep instead,
// and asserts the sweep really entered its in-flight state (the previous
// hard-coded in-flight label had drifted, making the wait a silent no-op).

/**
 * Budget for one full real sweep across every adapter. Measured on the owner's
 * app 2026-08-01: **244s** on an otherwise idle app. The old 540s budget still
 * blew up (#308) because a contended sweep — one already running, or racing the
 * BiznesRadar fundamentals pull for the same per-adapter lock — runs several
 * times longer, so this leaves ~3.7x headroom over the idle measurement. A
 * genuine hang still fails, just later.
 */
const SWEEP_SETTLE_MS = 900_000;

let connection: LiveConnection;

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("manual refresh rewrites aggregator ownership with sane values", async ({}) => {
  const { page } = connection;
  // The sweep budget plus a tail for navigation and the ownership assertions.
  test.setTimeout(SWEEP_SETTLE_MS + 120_000);

  // Sources screen → manual refresh of all adapters (or join one already
  // running), then wait for it to settle.
  await page
    .getByRole("button", { name: /^(Sources|Źródła)/ })
    .first()
    .click();
  const startedAt = Date.now();
  const mode = await driveSourcesRefresh(page, { settleTimeout: SWEEP_SETTLE_MS });
  const elapsedSeconds = Math.round((Date.now() - startedAt) / 1000);
  console.log(
    `live sources sweep ${mode === "joined" ? "joined (already running)" : "clicked"}, ` +
      `settled in ${elapsedSeconds}s of the ${SWEEP_SETTLE_MS / 1000}s budget`,
  );

  // Ownership section on the active company's Basic info: every legend/row
  // percentage must be plausible (≤ 100) — the defect rendered share counts.
  await page
    .getByRole("button", { name: /^(Dashboard|Pulpit)/ })
    .first()
    .click();
  await expect(page.getByLabel(/Research cockpit|Kokpit/)).toBeVisible({ timeout: 15_000 });
  const tab = page.getByRole("button", { name: /^(Podstawowe informacje|Basic info)$/ }).first();
  await tab.click();
  const section = page.locator(".basic-info-panel .ownership-section");
  await expect(section).toBeVisible({ timeout: 15_000 });

  const percentTexts = await section.locator("text=/%/").allTextContents();
  const values = percentTexts
    .flatMap((chunk) => chunk.match(/(\d[\d\s ]*[.,]?\d*)\s*%/g) ?? [])
    .map((token) =>
      Number(token.replace(/[%\s ]/g, "").replace(",", ".")),
    )
    .filter((value) => Number.isFinite(value));
  expect(values.length).toBeGreaterThan(0);
  for (const value of values) {
    expect(value).toBeLessThanOrEqual(100);
  }

  await page.screenshot({ path: "test-results/live/ownership-after-brfix.png", fullPage: true });
  console.log(`live ownership after refresh: ${values.length} rendered percentages, all ≤ 100`);
});
