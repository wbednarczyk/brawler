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
