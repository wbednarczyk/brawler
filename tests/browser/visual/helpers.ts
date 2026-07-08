import { expect, resetPaneSize, setPaneSize, test, type Locator, type Page } from "../helpers/harness";

// Shared shooting helpers for the all-panels visual baseline (ADR 0076 D7 / U11).
//
// Determinism contract (V4): before every shot we assert the panel's content is
// rendered (the caller does the density-spec content assertion) AND wait for
// `document.fonts.ready` so glyph metrics are stable. Motion is off via the
// project `reducedMotion: "reduce"` + the global `toHaveScreenshot` default
// `animations: "disabled"` (playwright.config.ts), which also freezes the
// Skeleton pulse to its end state. `maxDiffPixelRatio: 0.01` is the ADR tolerance,
// also set once in the global expect config. Mask only genuinely nondeterministic
// regions (spinner / live clock) — SAMPLE_NOW is fixed so timestamps are stable,
// and no masks are currently needed.

export type Tier = "S" | "M" | "L";

const TIER_WIDTH: Record<Tier, number> = { S: 380, M: 600, L: 900 };
const TIER_HEIGHT = 700;

// The light project (`chromium-visual-light`) shoots the M tier only.
function lightPass(): boolean {
  return test.info().project.name.endsWith("light");
}

async function settle(page: Page): Promise<void> {
  await page.evaluate(() => document.fonts.ready);
}

type ShootOptions = { mask?: Locator[] };

function screenshotOptions(opts: ShootOptions): { mask?: Locator[] } {
  return opts.mask ? { mask: opts.mask } : {};
}

// A cockpit panel: force the hosting `.cockpit-pane` to each width tier and
// snapshot the pane element. Dark shoots S/M/L; light shoots M only.
export async function shootPanel(
  page: Page,
  pane: Locator,
  name: string,
  opts: ShootOptions = {},
): Promise<void> {
  const tiers: Tier[] = lightPass() ? ["M"] : ["S", "M", "L"];
  for (const tier of tiers) {
    await setPaneSize(page, { width: TIER_WIDTH[tier], height: TIER_HEIGHT, pane });
    await settle(page);
    await expect(pane).toHaveScreenshot(`${name}-${tier}.png`, screenshotOptions(opts));
    await resetPaneSize(page, pane);
  }
}

// A full-screen sidebar/home screen hosted in `.workspace`: the M-equivalent is
// the workspace at the real project viewport (no forced size, per V3). Dark
// additionally forces the tiers the density spec already forces on `.workspace`
// (pass them in `forced`, e.g. ["S", "L"]); light shoots the M-equivalent only.
export async function shootScreen(
  page: Page,
  name: string,
  opts: ShootOptions & { forced?: Tier[] } = {},
): Promise<void> {
  const workspace = page.locator(".workspace");
  const shot = screenshotOptions(opts);

  await settle(page);
  await expect(workspace).toHaveScreenshot(`${name}-M.png`, shot);
  if (lightPass()) return;

  for (const tier of opts.forced ?? []) {
    await setPaneSize(page, { width: TIER_WIDTH[tier], height: TIER_HEIGHT, pane: workspace });
    await settle(page);
    await expect(workspace).toHaveScreenshot(`${name}-${tier}.png`, shot);
    await resetPaneSize(page, workspace);
  }
}
