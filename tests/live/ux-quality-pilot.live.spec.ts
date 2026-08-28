import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";
import {
  isCheckpointRun,
  recordCheckpoint,
  requireCheckpointMeta,
} from "./helpers/checkpointEvidence";

// Q9 pilot — read-safe mechanical evidence for J1 (morning review) and J2 (report
// published) against the REAL Windows app (ADR 0081). This drives the MECHANICAL
// path and records evidence a human charter then judges for clarity / usefulness /
// trustworthiness — automation NEVER emits a quality verdict. Deliberately
// NON-MUTATING: navigation + reads only. Any J2 step that would mutate the owner
// database (running an extraction, confirming a KPI) is intentionally NOT taken
// here — that requires an explicit opt-in pilot spec and an owner-selected record.
//
// Run scoped, with charter metadata, NOT part of make check:
//   BRAWLER_UX_JOURNEY="J1+J2 pilot" BRAWLER_UX_CARD=31a0fd5 BRAWLER_UX_STAGE=mid \
//   make live-cycle LIVE_SPEC=tests/live/ux-quality-pilot.live.spec.ts

let connection: LiveConnection;

test.skip(!isCheckpointRun(), "not a UX pilot run (set BRAWLER_UX_STAGE/JOURNEY/CARD)");

test.beforeAll(async () => {
  connection = await connectToLiveApp();
});

test.afterAll(async () => {
  await connection?.browser.close();
});

test("J1 + J2 read-safe mechanical evidence on the real app", async () => {
  const meta = requireCheckpointMeta();
  const { page } = connection;
  const observations: string[] = [];
  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  const errorFallback = page.locator(".app-error-recovery");

  // --- J1: morning review ---
  await nav.getByRole("button", { name: /Today|Dziś/ }).click();
  await expect(errorFallback).toHaveCount(0);
  const attention = page.locator(".today-stream-row");
  const quiet = page.locator(".today-stream-quiet");
  const attentionCount = await attention.count();
  const isQuiet = (await quiet.count()) > 0;
  expect(attentionCount > 0 || isQuiet).toBeTruthy();
  observations.push(
    isQuiet
      ? "J1: explicit quiet state, not a blank pane."
      : `J1: attention stream rendered ${attentionCount} row(s), no error fallback.`,
  );

  const review = page.locator(".today-row-review").first();
  if (await review.count()) {
    await review.click();
    const spolka = page.getByRole("region", { name: /Widok spółki|Company view/ });
    await expect(spolka).toBeVisible();
    await expect(spolka).not.toHaveAttribute("data-company-id", "");
    observations.push("J1: Review opened a company-scoped Spółka screen; return next.");
    await nav.getByRole("button", { name: /Today|Dziś/ }).click();
    await expect(errorFallback).toHaveCount(0);
  } else {
    observations.push("J1: quiet morning — no Review action to open.");
  }

  // --- J2: report published (read-safe — surface discovery only, no extraction) ---
  await nav.getByRole("button", { name: /Inbox/ }).click();
  await expect(errorFallback).toHaveCount(0);
  const feedItems = page.locator("[data-feed-item-id], .feed-item");
  const feedCount = await feedItems.count();
  if (feedCount > 0) {
    await feedItems.first().click();
    // The KPI-extraction entry point is the J2 primary decision surface. Confirm it
    // is DISCOVERABLE without running it (running mutates the owner DB).
    const launcher = page.getByLabel(/AI KPI extraction/);
    const launcherVisible = (await launcher.count()) > 0 && (await launcher.first().isVisible());
    observations.push(
      launcherVisible
        ? "J2: report selected; KPI-extraction entry point is discoverable (not run — read-safe)."
        : "J2: report selected; KPI-extraction entry point NOT visible for this item (finding for the charter).",
    );
  } else {
    observations.push("J2: no feed items in the real inbox right now — nothing to select.");
  }
  await expect(errorFallback).toHaveCount(0);

  const manifest = await recordCheckpoint(page, meta, {
    datasetLabel: process.env.BRAWLER_UX_DATASET ?? "owner real database (label only)",
    nowIso: new Date().toISOString(),
    observations,
  });
  await page.screenshot({ path: `${manifest.screenshotDir}/j1-j2-pilot.png`, fullPage: true });

  // Evidence recorded; the human charter (exploratory question, P1/P2/P3 findings,
  // verdict proceed|revise|block, which judgments stayed human) is answered OUTSIDE
  // this automation.
  expect(manifest.windowsNative, "pilot must run against the native Windows app").toBeTruthy();
});
