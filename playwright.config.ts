import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";

import { defineConfig, devices } from "@playwright/test";

import { pinnedRenderer } from "./scripts/ux/pinned-renderer.mjs";

const port = 4321;
// Inside the pinned Playwright docker image (#448) — the only place pixel
// baselines are compared; everywhere else the visual specs execute without comparing.
const pinned = pinnedRenderer();
// Deterministic rasterization for the pixel compare (#448): partial raster
// reuses already-rasterized tiles, so the anti-aliasing of curves (rounded
// borders) depended on frame history and jittered by ±2/255 between identical
// runs; measured — `--disable-gpu` alone leaves 1–2 cells unstable, the pair
// gives 3 byte-identical full regenerations.
const VISUAL_CHROMIUM_ARGS = ["--disable-gpu", "--disable-partial-raster"];

// Build-freshness stamp (bug 2059fd8). `reuseExistingServer` locally can hand the
// browser suite a dev server started from ANOTHER worktree/branch (or from before
// your latest edit) that serves pre-rework code — the suite then byte-matches
// STALE visual/density baselines and reports green. The newest source mtime is a
// cheap identity for "the code this run expects to be served": it is baked into
// the server the webServer command starts (→ `window.__BRAWLER_BUILD_STAMP__`) and
// recorded as the expected value; `global-setup` refuses to run against a reused
// server whose baked stamp differs. A freshly-started server (the normal local
// flow) bakes this same value, so nothing changes there.
function newestSourceMtime(dir: string): number {
  let newest = 0;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    newest = entry.isDirectory()
      ? Math.max(newest, newestSourceMtime(full))
      : Math.max(newest, statSync(full).mtimeMs);
  }
  return newest;
}

const buildStamp = String(Math.round(newestSourceMtime(join(process.cwd(), "src"))));
process.env.BRAWLER_EXPECTED_BUILD_STAMP = buildStamp;
process.env.BRAWLER_SMOKE_URL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./tests/browser",
  timeout: 30_000,
  expect: {
    timeout: 5_000,
    // All-panels visual baseline (ADR 0076 D7). Only the `tests/browser/visual/**`
    // specs (chromium-visual / -light projects) call toHaveScreenshot. Tolerance
    // is zero (#448): the compare runs only in the pinned renderer, where there
    // is no font/antialiasing drift to tolerate (pixelmatch's own anti-aliasing
    // exemption is closed by the byte-equality check in tests/browser/visual/
    // helpers.ts). Animations freeze at their end state so the Skeleton pulse /
    // transitions cannot flake the compare.
    toHaveScreenshot: {
      maxDiffPixels: 0,
      threshold: 0,
      animations: "disabled",
    },
  },
  // Per-test isolation (the browser mock runtime is re-seeded fresh on each
  // page load, one context per test) makes full parallelism safe — ADR 0048.
  // Workers are capped at half the cores so the browser fleet does not
  // oversubscribe the CPU and cause false-timeout flakiness.
  // Build-freshness guard (bug 2059fd8): abort before the suite runs if a reused
  // dev server is serving a stale build (see `newestSourceMtime` above).
  globalSetup: "./tests/browser/global-setup.ts",
  fullyParallel: true,
  workers: process.env.CI ? 2 : "50%",
  // Local runs never retry (a flake should be seen and fixed); CI retries once
  // to absorb the occasional environmental flake without masking real failures.
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"]],
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: process.env.CI ? "on-first-retry" : "retain-on-failure",
    screenshot: "only-on-failure",
    video: "off",
  },
  projects: [
    {
      name: "chromium-compact",
      testIgnore: /visual\//,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1366, height: 768 },
      },
    },
    {
      name: "chromium-wide",
      testIgnore: /visual\//,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1920, height: 1080 },
      },
    },
    {
      // A quarter of a 49" 5120x1440 ultrawide (vertical 4-way split) at 100% OS
      // scaling: a tall, narrow window the app is commonly run in. Per CLAUDE.md.
      name: "chromium-quarter-uw",
      testIgnore: /visual\//,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1280, height: 1440 },
      },
    },
    {
      // The same quarter-ultrawide window at 125% OS scaling (effective CSS px).
      // This lands in the band where sidebar + two-column content must stack to
      // avoid clipping the detail pane.
      name: "chromium-quarter-uw-125",
      testIgnore: /visual\//,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1024, height: 1152 },
      },
    },
    {
      // The full suite under the LIGHT theme (ADR 0076 D3). Running the whole
      // Playwright matrix × both themes is a combinatorial explosion for little
      // marginal signal, so light joins the matrix as this single compact
      // project — enough to catch semantic-token regressions that only surface
      // in the light palette. Mechanism: `storageState` seeds a localStorage key
      // (`brawler:smoke:theme=light`) that the browser-smoke runtime reads at
      // install time to force the persisted theme before the app renders. This
      // survives in-context navigations (a query param would not), so every
      // spec in the suite runs light without touching the shared helpers.
      name: "chromium-compact-light",
      testIgnore: /visual\//,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1366, height: 768 },
        storageState: "./tests/browser/light-theme.storage.json",
      },
    },
    // ---- All-panels visual regression baseline (ADR 0076 Decision 7 / U11) ----
    // Screenshots compare on exactly ONE environment per theme so a baseline is
    // never captured 5× across the viewport matrix. Both projects run ONLY the
    // `tests/browser/visual/**` specs (every other project excludes that dir via
    // testIgnore above), on the compact viewport, with motion reduced. Pixel
    // comparison happens only inside the pinned docker renderer (`make
    // check-visual`, a required CI check; #448); host runs and the CI shards
    // still EXECUTE the specs (layout + console gates hold) with
    // `ignoreSnapshots`. `retries: 0`: a retry could mask nondeterminism the
    // zero-tolerance compare exists to catch. `metadata.pinnedRenderer` lets the
    // shoot helpers gate their byte-equality assertion without a second import.
    {
      name: "chromium-visual",
      testMatch: /visual\/.*\.spec\.ts$/,
      ignoreSnapshots: !pinned,
      retries: 0,
      metadata: { pinnedRenderer: pinned },
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1366, height: 768 },
        reducedMotion: "reduce",
        launchOptions: { args: VISUAL_CHROMIUM_ARGS },
      },
    },
    {
      // The light-theme pass covers the M tier only (ADR 0076 D7: "one light pass
      // at M"); the shoot helpers detect this project by name and skip S/L.
      name: "chromium-visual-light",
      testMatch: /visual\/.*\.spec\.ts$/,
      ignoreSnapshots: !pinned,
      retries: 0,
      metadata: { pinnedRenderer: pinned },
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1366, height: 768 },
        reducedMotion: "reduce",
        storageState: "./tests/browser/light-theme.storage.json",
        launchOptions: { args: VISUAL_CHROMIUM_ARGS },
      },
    },
  ],
  webServer: {
    command: `VITE_BRAWLER_BROWSER_SMOKE=1 VITE_BRAWLER_BUILD_STAMP=${buildStamp} npx vite --host 127.0.0.1 --port ${port}`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
