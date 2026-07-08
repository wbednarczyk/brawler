import AxeBuilder from "@axe-core/playwright";
import { test as base, expect, type Locator, type Page } from "@playwright/test";
import budgets from "../journeys/budgets.json" with { type: "json" };

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

  // Panel-internal horizontal scrollBARS (harvested guardrail, ADR 0045).
  // `expectNoHorizontalOverflow` above deliberately skips overflow auto/scroll
  // containers ("handling their own overflow on purpose") and the document
  // check only sees page-level overflow — so a dockview panel quietly growing
  // a horizontal scrollbar (long unwrapped prose in a row slot) escaped every
  // gate. An element that actually scrolls horizontally is a layout bug unless
  // it, or an ancestor, is explicitly marked as intentional wide content with
  // `data-hscroll`.
  const scrollers = await page.evaluate(() => {
    const offenders: string[] = [];
    for (const el of Array.from(document.querySelectorAll<HTMLElement>("*"))) {
      if (el.scrollWidth <= el.clientWidth + 1) continue;
      if (el.clientWidth === 0) continue;
      const overflowX = getComputedStyle(el).overflowX;
      if (overflowX !== "auto" && overflowX !== "scroll") continue;
      if (el.closest("[data-hscroll]")) continue;
      const classes =
        typeof el.className === "string" && el.className.trim()
          ? `.${el.className.trim().split(/\s+/).join(".")}`
          : "";
      offenders.push(`${el.tagName.toLowerCase()}${classes} (${el.scrollWidth} > ${el.clientWidth})`);
    }
    return offenders;
  });
  expect(
    scrollers,
    `Horizontally scrollable containers — wrap the content, or mark deliberate wide content with data-hscroll:\n${scrollers.join("\n")}`,
  ).toEqual([]);
}

// Real-browser accessibility gate (U9): run axe-core against the live rendered
// page and fail on any WCAG 2.0/2.1 A/AA violation. Unlike the jsdom unit guard
// (src/app/screens.a11y.test.tsx), this exercises the actual layout and computed
// colors, so it is the ONLY layer that catches real color-contrast — and it runs
// across the full viewport matrix incl. the light theme project, covering both
// palettes × both modes. Scoped to the WCAG A/AA tags on purpose: best-practice
// rules (region / heading-order / landmark-*) are structural design choices for a
// multi-pane desktop workspace, not conformance failures, and stay out of the gate.
const A11Y_TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"];

export async function expectNoA11yViolations(page: Page, context: string) {
  const results = await new AxeBuilder({ page }).withTags(A11Y_TAGS).analyze();
  const summary = results.violations
    .map(
      (violation) =>
        `[${violation.id}] ${violation.help}\n` +
        violation.nodes
          .map((node) => `    ${node.target.join(" ")}\n      ${node.failureSummary?.replace(/\n/g, "\n      ")}`)
          .join("\n"),
    )
    .join("\n");
  expect(
    results.violations.map((violation) => violation.id),
    `axe WCAG A/AA violations on ${context}:\n${summary}`,
  ).toEqual([]);
}

// A region is configured as a bounded internal scroll container (so it scrolls
// rather than pushing the page); it need not currently overflow.
export async function expectInternalScroll(locator: Locator) {
  const overflowY = await locator.evaluate((el) => getComputedStyle(el).overflowY);
  expect(["auto", "scroll"]).toContain(overflowY);
}

// Panel density contracts (ADR 0076 D6) are driven by `@container pane (…)`
// rules on the named `pane` size container (.cockpit-pane / .workspace). To
// exercise a width/height tier in a real browser we force the pane's own inline
// size — direct style is the sanctioned approach (plan U7: "bezpośredni styl")
// because the container query then resolves against the forced size regardless
// of the window/dock cell. By default targets the FIRST visible `.cockpit-pane`;
// pass an explicit locator to size a different pane.
export async function setPaneSize(
  page: Page,
  { width, height, pane }: { width?: number; height?: number; pane?: Locator } = {},
) {
  const target = pane ?? page.locator(".cockpit-pane").first();
  await target.evaluate((el, size) => {
    const node = el as HTMLElement;
    if (size.width != null) node.style.width = `${size.width}px`;
    if (size.height != null) node.style.height = `${size.height}px`;
    // Definite own size: neutralise a flex/grid track that would otherwise
    // stretch the pane back to the cell, so the forced size actually holds.
    node.style.flex = "0 0 auto";
  }, { width, height });
}

export async function resetPaneSize(page: Page, pane?: Locator) {
  const target = pane ?? page.locator(".cockpit-pane").first();
  await target.evaluate((el) => {
    const node = el as HTMLElement;
    node.style.removeProperty("width");
    node.style.removeProperty("height");
    node.style.removeProperty("flex");
  });
}

// ---------------------------------------------------------------------------
// Journey interaction counter (ADR 0074 pt 3) — the explicit, no-magic wrapper.
//
// Counting convention: ONE wrapper call = ONE user interaction. `click`, `fill`,
// `press`, and `selectOption` each perform the Playwright action and increment
// the counter by exactly 1 — nothing counts implicitly, so the count is the
// number of discrete things the user did to complete the journey. Navigation
// performed inside a journey (opening a cockpit panel, drilling into a company)
// is driven through the SAME wrapper so its clicks are counted too; never mix a
// raw `locator.click()` into a measured journey.
//
// Budgets are calibrated by FIRST measurement (per journey), recorded in
// `tests/browser/journeys/budgets.json` as the ratchet FLOOR, and must never
// exceed the documented ceilings in `docs/ux-journeys.md`. `assertBudget()`
// reddens when the journey got LONGER than its floor (a UX regression, like a
// byte-budget regression); when it got measurably SHORTER (> 2 under floor) it
// logs a tighten suggestion so the floor is deliberately re-ratcheted down.
export type Journey = {
  click(locator: Locator): Promise<void>;
  fill(locator: Locator, value: string): Promise<void>;
  press(target: Locator | Page, key: string): Promise<void>;
  selectOption(locator: Locator, value: string): Promise<void>;
  /** Interactions performed so far. */
  readonly count: number;
  /** Assert count ≤ budget; log a tighten suggestion when well under. */
  assertBudget(): void;
};

const journeyBudgets = budgets as Record<string, number>;

// A Page has `keyboard`; a Locator does not — the discriminator for `press`.
function isPage(target: Locator | Page): target is Page {
  return "keyboard" in target;
}

export function journey(page: Page, id: string): Journey {
  let count = 0;
  const budget = journeyBudgets[id];

  return {
    async click(locator) {
      await locator.click();
      count += 1;
    },
    async fill(locator, value) {
      await locator.fill(value);
      count += 1;
    },
    async press(target, key) {
      if (isPage(target)) await target.keyboard.press(key);
      else await target.press(key);
      count += 1;
    },
    async selectOption(locator, value) {
      await locator.selectOption(value);
      count += 1;
    },
    get count() {
      return count;
    },
    assertBudget() {
      expect(
        budget,
        `No budget entry for journey "${id}" in tests/browser/journeys/budgets.json`,
      ).toBeGreaterThan(0);
      const viewport = JSON.stringify(page.viewportSize());
      expect(
        count,
        `Journey "${id}" exceeded its interaction budget: ${count} > ${budget} ` +
          `(viewport ${viewport}). A journey got longer — this is a UX regression. ` +
          `Fix the flow, or (if the extra step is genuinely required) raise the budget ` +
          `in budgets.json AND ux-journeys.md deliberately.`,
      ).toBeLessThanOrEqual(budget);
      if (count < budget - 2) {
        // eslint-disable-next-line no-console
        console.log(
          `[journey ${id}] measured ${count} interactions, budget ${budget} ` +
            `(viewport ${viewport}). The journey is measurably shorter than its floor — ` +
            `tighten budgets.json to ${count + 1} to ratchet the improvement in.`,
        );
      }
    },
  };
}
