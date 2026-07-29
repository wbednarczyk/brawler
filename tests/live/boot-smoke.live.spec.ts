import { test, expect } from "@playwright/test";
import { connectToLiveApp, type LiveConnection } from "./helpers/liveConnect";

// Windows boot smoke (#206, ADR 0090 §B2): the minimal "the shipped .exe
// actually boots" assertion set, safe on a CLEAN database — unlike the rest of
// tests/live/ it must never assume the owner's populated DB. Driven by
// `make live-smoke EXE=<path>` both on a windows-latest runner (against the
// windows-build artifact) and on the owner's machine. Non-destructive: reads
// the shell, writes nothing.
//
// Console-error honesty: Playwright attaches AFTER the app booted (CDP attach),
// so boot-time console output is already gone — this spec asserts no critical
// console errors during the observed window (shell assertions + one nav
// round-trip), which is the strongest claim an attach-after-boot harness can
// make. A crash-on-boot still fails loudly earlier: the CDP endpoint never
// comes up and `make live-smoke` times out.

let connection: LiveConnection;
const criticalErrors: string[] = [];

test.beforeAll(async () => {
  connection = await connectToLiveApp();
  connection.page.on("console", (message) => {
    if (message.type() !== "error") return;
    const text = message.text();
    // Benign noise on a clean DB: favicon/asset 404s and devtools chatter are
    // not boot failures. Everything else is critical by default — an allowlist,
    // so a new error class fails the smoke instead of being silently ignored.
    if (/favicon|net::ERR_FILE_NOT_FOUND/i.test(text)) return;
    criticalErrors.push(text);
  });
  connection.page.on("pageerror", (error) => {
    criticalErrors.push(`pageerror: ${error.message}`);
  });
});

test.afterAll(async () => {
  await connection.browser.close();
});

test("clean-DB boot: shell renders, version badge present, nav responds, no critical console errors", async () => {
  const { page } = connection;

  // 1. The app shell rendered (window exists, main layout mounted).
  const sidebarNav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await expect(sidebarNav).toBeVisible();

  // 2. The version badge renders a real version — proves the frontend asked
  //    the Rust backend and got an answer (IPC alive), not a white screen.
  const versionBadge = page.locator(".brand-version");
  await expect(versionBadge).toBeVisible();
  await expect(versionBadge).toHaveText(/^v\d+\.\d+\.\d+/);

  // 3. One navigation round-trip: the shell responds to input and a second
  //    screen renders (main-content region swaps). Companies exists on a clean
  //    DB (its empty state is a valid render).
  await page.screenshot({ path: "test-results/live/boot-shell.png", fullPage: true });
  const companiesItem = sidebarNav.getByRole("button", { name: /Companies|Spółki/ }).first();
  await companiesItem.click();
  await expect(companiesItem).toHaveAttribute("aria-current", "page");
  await page.screenshot({ path: "test-results/live/boot-companies.png", fullPage: true });

  expect(
    criticalErrors,
    `critical console errors during the boot smoke:\n${criticalErrors.join("\n")}`,
  ).toEqual([]);
});
