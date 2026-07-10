import { chromium } from "@playwright/test";

// Build-freshness guard (bug 2059fd8). Locally `reuseExistingServer` is on, so a
// long-lived dev server from another worktree/branch — or from before your latest
// edit — can keep serving pre-rework code. The browser suite then byte-matches
// STALE visual/density baselines and reports green (observed during Panel B QG).
//
// This runs once, AFTER the webServer is up and BEFORE any spec, and refuses to
// proceed when the server actually answering on the port was built from a
// different source snapshot than this run expects. It reads the stamp the running
// server baked in (`window.__BRAWLER_BUILD_STAMP__`, set only under the
// browser-smoke build) and compares it to the stamp the config computed from the
// current `src/` tree (`BRAWLER_EXPECTED_BUILD_STAMP`). A freshly-started server
// (the normal local flow) matches and passes; a reused stale one does not, and we
// fail loudly instead of false-greening.
async function globalSetup(): Promise<void> {
  const expected = process.env.BRAWLER_EXPECTED_BUILD_STAMP;
  const url = process.env.BRAWLER_SMOKE_URL ?? "http://127.0.0.1:4321";
  // No expected stamp means the config did not compute one (unexpected) — do not
  // block the run on a guard that has nothing to compare against.
  if (!expected) return;

  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.goto(url, { waitUntil: "domcontentloaded" });
    const served = await page.evaluate(
      () => (window as unknown as { __BRAWLER_BUILD_STAMP__?: string }).__BRAWLER_BUILD_STAMP__,
    );
    if (served !== expected) {
      throw new Error(
        `Browser suite aborted — the dev server on ${url} is serving a STALE build ` +
          `(served stamp ${served ?? "<none>"}, expected ${expected}). A reused dev server ` +
          `from another worktree/branch or from before your latest edit would false-green ` +
          `visual/density baselines. Stop that server so a fresh one starts (or run the ` +
          `browser suite without a pre-existing dev server on this port).`,
      );
    }
  } finally {
    await browser.close();
  }
}

export default globalSetup;
