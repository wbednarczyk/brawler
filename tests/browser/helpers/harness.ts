import { test as base, expect, type Locator, type Page } from "@playwright/test";

// Shared browser-test harness. Import `test`/`expect` from here (not directly
// from @playwright/test) to get:
//   - a console-error gate: any console.error / uncaught page error fails the
//     test, catching React warnings and silent runtime breakage.
//   - reusable layout invariants used across journeys and the smoke-walk.

// Console messages that are environmental noise rather than app defects. Keep
// this list tight and justified — every entry is a hole in the gate.
const IGNORED_CONSOLE = [
  /Failed to load resource/i, // favicon / asset 404s under the dev server
  /\[vite\]/i, // vite HMR chatter
];

type Fixtures = {
  /** Collected console errors; asserted empty at teardown. */
  consoleErrors: string[];
};

export const test = base.extend<Fixtures>({
  consoleErrors: [
    async ({ page }, use) => {
      const errors: string[] = [];
      page.on("console", (message) => {
        if (message.type() === "error") errors.push(message.text());
      });
      page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));

      await use(errors);

      const significant = errors.filter((entry) => !IGNORED_CONSOLE.some((re) => re.test(entry)));
      expect(significant, `Unexpected console errors:\n${significant.join("\n")}`).toEqual([]);
    },
    { auto: true },
  ],
});

export { expect };

export async function openApp(page: Page, path = "/") {
  await page.goto(path);
  await expect(page.getByLabel(/Primary navigation|Nawigacja główna/)).toBeVisible();
}

// Open the app and land on the Inbox feed. The default landing is now Today
// (ADR 0054), so feed-focused tests must navigate to the Inbox explicitly rather
// than assume it is the entry screen.
export async function openInbox(page: Page) {
  await openApp(page);
  await page.getByLabel(/Primary navigation|Nawigacja główna/).getByRole("button", { name: "Inbox" }).click();
  await expect(page.getByRole("heading", { name: "Inbox", exact: true })).toBeVisible();
}

// Open a cockpit-hosted global screen (ADR 0054): Research / Notebook / Events /
// Report Season / Watchlists no longer have their own sidebar button — they live
// as cockpit panels. There is also no standalone blank "Cockpit" nav entry
// anymore (ADR 0057 decision 5): the entry point is the "+ New view" creator, so
// this helper creates a throwaway named view, then fills its first grid cell
// from the panel palette. `label` is the panel name as it appears in the palette
// ("Notebook", "Research", "Events", "Report Season").
export async function openCockpitPanel(page: Page, label: string) {
  const nav = page.getByLabel(/Primary navigation|Nawigacja główna/);
  await nav.getByRole("button", { name: "New view" }).click();
  const createModal = page.getByRole("dialog", { name: "New view" });
  await createModal.getByLabel("View name").fill(`${label} test view`);
  await createModal.getByRole("button", { name: "Create view" }).click();
  await expect(page.getByLabel("Research cockpit")).toBeVisible();
  // Wait for the fresh grid to render (the layout-apply effect fires just after
  // mount) before targeting a cell, so "Add panel" doesn't race an empty grid.
  await expect(page.getByText("Pick a panel").first()).toBeVisible();
  await page.getByRole("button", { name: "Add panel" }).click();
  const palette = page.getByRole("dialog", { name: "Command palette" });
  await palette.getByLabel("Search commands").fill(`Open panel: ${label}`);
  await palette.getByRole("button", { name: `Open panel: ${label}`, exact: true }).first().click();
}

// Auto-accept native confirm/alert dialogs for the remainder of the test. Use in
// flows with a confirm-before-destroy step (delete watchlist / notebook entry)
// instead of hand-writing `page.once("dialog", …)` at each call site.
export function acceptDialogs(page: Page) {
  page.on("dialog", (dialog) => {
    void dialog.accept();
  });
}

// No element whose content escapes its box (overflow-x visible) is wider than its
// client box. Containers that scroll/clip (overflow auto/hidden) are excluded —
// they are handling their own overflow on purpose. This is the precise detector
// for the recurring "a wide descendant blows out / clips the pane" class: it
// finds the offending element even when an ancestor hides the symptom.
export async function expectNoHorizontalOverflow(scope: Locator, selector = "*") {
  const offenders = await scope.evaluate((host, sel) => {
    // Replaced/interactive leaf controls can report scrollWidth > clientWidth for
    // sub-pixel/intrinsic reasons that don't represent a layout blowout; a real
    // overflow inside one still inflates its container, which we do flag.
    const SKIP = new Set(["BUTTON", "INPUT", "TEXTAREA", "SELECT", "SVG", "IMG", "PATH"]);
    const elements = [host, ...Array.from(host.querySelectorAll(sel as string))] as HTMLElement[];
    const out: string[] = [];
    for (const el of elements) {
      if (SKIP.has(el.tagName.toUpperCase())) continue;
      const style = getComputedStyle(el);
      if (style.overflowX !== "visible") continue;
      // clientWidth is 0/meaningless for inline (non-replaced) and display:contents
      // elements — comparing scrollWidth against it yields false positives.
      if (style.display === "inline" || style.display === "contents") continue;
      if (el.clientWidth === 0) continue;
      if (el.scrollWidth > el.clientWidth + 1) {
        const id = (typeof el.className === "string" && el.className) || el.tagName.toLowerCase();
        out.push(`${id} (scrollWidth ${el.scrollWidth} > clientWidth ${el.clientWidth})`);
      }
    }
    return out;
  }, selector);

  expect(offenders, `Horizontally overflowing elements:\n${offenders.join("\n")}`).toEqual([]);
}

// The whole document must not require horizontal scrolling at the test viewport.
export async function expectNoPageOverflow(page: Page) {
  const overflow = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    clientWidth: document.documentElement.clientWidth,
  }));
  expect(
    overflow.scrollWidth,
    `Document overflows horizontally (${overflow.scrollWidth} > ${overflow.clientWidth})`,
  ).toBeLessThanOrEqual(overflow.clientWidth + 1);
}

// A region is configured as a bounded internal scroll container (so it scrolls
// rather than pushing the page); it need not currently overflow.
export async function expectInternalScroll(locator: Locator) {
  const overflowY = await locator.evaluate((el) => getComputedStyle(el).overflowY);
  expect(["auto", "scroll"]).toContain(overflowY);
}
