import { defineConfig, devices } from "@playwright/test";

// Live-drive config (ADR 0066, docs/testing.md § Live drive): connects Playwright
// to the REAL packaged Windows app (real backend, real local SQLite DB) over
// WebView2's Chrome DevTools Protocol, instead of the mocked Vite preview the main
// `playwright.config.ts` drives. Deliberately separate from that config:
//   - no `webServer` — the app is launched independently (scripts/windows/dev-live.ps1)
//     and is not something this config can start or own the lifecycle of;
//   - a distinct `testDir` (tests/live, not tests/browser) so it can NEVER be picked
//     up by `make check` — it needs a live app, credentials-free
///    CI has neither, and unlike the mocked browser suite it is not hermetic;
//   - generous timeouts — a real app on a real machine is slower and less
//     deterministic than the mocked dev server.
// Never add this config's testDir to the main config, and never run this file from
// `make check` — see the Makefile `live-drive` target's exclusion note.
export default defineConfig({
  testDir: "./tests/live",
  timeout: 60_000,
  expect: {
    timeout: 15_000,
  },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["list"]],
  use: {
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "off",
  },
  projects: [
    {
      name: "live-cdp",
      use: {
        ...devices["Desktop Chrome"],
      },
    },
  ],
});
