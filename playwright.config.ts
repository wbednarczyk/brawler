import { defineConfig, devices } from "@playwright/test";

const port = 4321;

export default defineConfig({
  testDir: "./tests/browser",
  timeout: 30_000,
  expect: {
    timeout: 5_000,
    // All-panels visual baseline (ADR 0076 D7). Only the `tests/browser/visual/**`
    // specs (chromium-visual / -light projects) call toHaveScreenshot; these
    // defaults apply the ADR's tolerance and freeze animations to their end state
    // so the Skeleton pulse / transitions can't flake the pixel compare.
    toHaveScreenshot: {
      maxDiffPixelRatio: 0.01,
      animations: "disabled",
    },
  },
  // Per-test isolation (the browser mock runtime is re-seeded fresh on each
  // page load, one context per test) makes full parallelism safe — ADR 0048.
  // Workers are capped at half the cores so the browser fleet does not
  // oversubscribe the CPU and cause false-timeout flakiness.
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
    // comparison is skipped under CI (`ignoreSnapshots`) — font antialiasing
    // differs across machines and the repo is local-first — so CI still EXECUTES
    // the specs (layout + console gates hold) without a machine-dependent diff.
    {
      name: "chromium-visual",
      testMatch: /visual\/.*\.spec\.ts$/,
      ignoreSnapshots: !!process.env.CI,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1366, height: 768 },
        reducedMotion: "reduce",
      },
    },
    {
      // The light-theme pass covers the M tier only (ADR 0076 D7: "one light pass
      // at M"); the shoot helpers detect this project by name and skip S/L.
      name: "chromium-visual-light",
      testMatch: /visual\/.*\.spec\.ts$/,
      ignoreSnapshots: !!process.env.CI,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1366, height: 768 },
        reducedMotion: "reduce",
        storageState: "./tests/browser/light-theme.storage.json",
      },
    },
  ],
  webServer: {
    command: `VITE_BRAWLER_BROWSER_SMOKE=1 npx vite --host 127.0.0.1 --port ${port}`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
