import { defineConfig, devices } from "@playwright/test";

const port = 4321;

export default defineConfig({
  testDir: "./tests/browser",
  timeout: 30_000,
  expect: {
    timeout: 5_000,
  },
  // Per-test isolation (the browser mock runtime is re-seeded fresh on each
  // page load, one context per test) makes full parallelism safe — ADR 0048.
  // Workers are capped at half the cores so the browser fleet does not
  // oversubscribe the CPU and cause false-timeout flakiness.
  fullyParallel: true,
  workers: process.env.CI ? 2 : "50%",
  retries: 0,
  reporter: [["list"]],
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "off",
  },
  projects: [
    {
      name: "chromium-compact",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1366, height: 768 },
      },
    },
    {
      name: "chromium-wide",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1920, height: 1080 },
      },
    },
    {
      // A quarter of a 49" 5120x1440 ultrawide (vertical 4-way split) at 100% OS
      // scaling: a tall, narrow window the app is commonly run in. Per AGENTS.md.
      name: "chromium-quarter-uw",
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
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1024, height: 1152 },
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
