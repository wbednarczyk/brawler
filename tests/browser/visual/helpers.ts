import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, resetPaneSize, setPaneSize, test, type Locator, type Page } from "../helpers/harness";
import { assertCataloged, findCatalogEntry, type Tier } from "./catalog";

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

export type { Tier };

const TIER_WIDTH: Record<Tier, number> = { S: 380, M: 600, L: 900 };
const TIER_HEIGHT = 700;

// The light project (`chromium-visual-light`) shoots the M tier only.
function lightPass(): boolean {
  return test.info().project.name.endsWith("light");
}

function themeFor(): "dark" | "light" {
  return lightPass() ? "light" : "dark";
}

async function settle(page: Page): Promise<void> {
  await page.evaluate(() => document.fonts.ready);
  // Hide the toast overlay for per-screen baselines (analogous to the disabled
  // CSS animations): a transient action-feedback toast (ADR 0097 — the only
  // kind) raised by a scripted action would float over the shot, coupling the
  // baseline to timing. Toast look/behavior has its own coverage
  // (Toast.test.tsx, primitives a11y); screen baselines assert the screen.
  // Injected per-shot, idempotent. The sidebar Today badge is deterministic in
  // shots: the mock scenario's attention fetch resolves synchronously with the
  // other seeded reads the shot already waits for.
  await page
    .addStyleTag({ content: ".ui-toast-viewport { display: none !important; }" })
    .catch(() => {});
}

type ShootOptions = { mask?: Locator[]; state?: string };

// Visual figure proof (ADR 0104 dec. 2 amendment, F4a S1): when the catalog
// cell declares a `figures` minimum, assert it BEFORE capturing evidence —
// same spirit as the density spec's own content assertion, but for the
// shared `Figure` primitive's `[data-figure]` marker specifically. A no-op
// for the many cells that don't declare one yet.
async function assertFigureMinimum(root: Locator, screen: string, state: string): Promise<void> {
  // F4b S2: an "empty"/invitation state has no data to render as a figure by
  // definition — the minimum is a "default" (content-bearing) state proof.
  if (state !== "default") return;
  const figures = findCatalogEntry(screen)?.figures;
  if (!figures) return;
  const count = await root.locator(figures.selector).count();
  expect(
    count,
    `${screen}: expected at least ${figures.min} "${figures.selector}" figure(s), found ${count}`,
  ).toBeGreaterThanOrEqual(figures.min);
}

function screenshotOptions(opts: ShootOptions): { mask?: Locator[] } {
  return opts.mask ? { mask: opts.mask } : {};
}

// F4b S2: a non-"default" state gets its own baseline filename — otherwise a
// screen with two shot states (e.g. Transcripts default + empty) would
// silently overwrite one baseline with the other (both call sites shared the
// same `${name}-${tier}.png` leaf before this).
function snapshotName(name: string, state: string, tier: string): string {
  return state === "default" ? `${name}-${tier}.png` : `${name}-${state}-${tier}.png`;
}

// Contact-sheet evidence (ADR 0081 plan Q5). When BRAWLER_CONTACT_SHEET_DIR is
// set, capture the same settled locator as a current PNG plus a JSON metadata
// sidecar BEFORE the toHaveScreenshot assertion below runs, so evidence exists
// for the contact-sheet orchestrator to assemble even if that assertion then
// fails (the orchestrator still returns Playwright's original non-zero exit).
// Filenames are unique per worker (workerIndex + a per-worker counter) so
// `fullyParallel` workers never race on the same path.
let sidecarCounter = 0;

async function writeContactSheetEvidence(
  page: Page,
  locator: Locator,
  params: { screen: string; state: string; tier: Tier },
): Promise<void> {
  const dir = process.env.BRAWLER_CONTACT_SHEET_DIR;
  if (!dir) return;
  assertCataloged(params.screen, params.state);
  mkdirSync(dir, { recursive: true });

  const info = test.info();
  const theme = themeFor();
  const buildStamp =
    (await page.evaluate(
      () => (window as unknown as { __BRAWLER_BUILD_STAMP__?: string }).__BRAWLER_BUILD_STAMP__,
    )) ??
    process.env.BRAWLER_EXPECTED_BUILD_STAMP ??
    "unknown";

  const base = `${params.screen}__${params.state}__${params.tier}__${theme}__${info.project.name}--w${info.workerIndex}-${sidecarCounter++}`;
  const png = await locator.screenshot();
  writeFileSync(join(dir, `${base}.png`), png);
  writeFileSync(
    join(dir, `${base}.json`),
    JSON.stringify(
      {
        screen: params.screen,
        state: params.state,
        tier: params.tier,
        theme,
        project: info.project.name,
        buildStamp,
        image: `${base}.png`,
      },
      null,
      2,
    ),
  );
}

// A company workshop-tool or global-screen panel: force the hosting pane
// (`.spolka-layout` / `.workspace`) to each width tier and snapshot the pane
// element. Dark shoots S/M/L; light shoots M only.
export async function shootPanel(
  page: Page,
  pane: Locator,
  name: string,
  opts: ShootOptions = {},
): Promise<void> {
  const state = opts.state ?? "default";
  const tiers: Tier[] = lightPass() ? ["M"] : ["S", "M", "L"];
  for (const tier of tiers) {
    await setPaneSize(page, { width: TIER_WIDTH[tier], height: TIER_HEIGHT, pane });
    await settle(page);
    await assertFigureMinimum(pane, name, state);
    await writeContactSheetEvidence(page, pane, { screen: name, state, tier });
    await expect(pane).toHaveScreenshot(snapshotName(name, state, tier), screenshotOptions(opts));
    await resetPaneSize(page, pane);
  }
}

// A region INSIDE a panel that `shootPanel` cannot reach: the pane shot is
// clipped at `TIER_HEIGHT`, so anything below the fold (a panel's actions
// footer) never enters a baseline and can change without a single pixel of
// diff — the failure this repo already hit once on the coverage actions
// footer (see `shootPanel`'s maximize comment) and again in epic #398, where
// a new footer button left every baseline green. Shot at the M tier only, in
// both themes: a narrow region is what makes the baseline stable when the
// content above it changes.
export async function shootRegion(
  page: Page,
  pane: Locator,
  region: Locator,
  name: string,
  opts: ShootOptions = {},
): Promise<void> {
  const state = opts.state ?? "default";
  await setPaneSize(page, { width: TIER_WIDTH.M, height: TIER_HEIGHT, pane });
  await region.scrollIntoViewIfNeeded();
  await settle(page);
  await assertFigureMinimum(region, name, state);
  await writeContactSheetEvidence(page, region, { screen: name, state, tier: "M" });
  await expect(region).toHaveScreenshot(snapshotName(name, state, "M"), screenshotOptions(opts));
  await resetPaneSize(page, pane);
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
  const state = opts.state ?? "default";

  await settle(page);
  await assertFigureMinimum(workspace, name, state);
  await writeContactSheetEvidence(page, workspace, { screen: name, state, tier: "M" });
  await expect(workspace).toHaveScreenshot(snapshotName(name, state, "M"), shot);
  if (lightPass()) return;

  for (const tier of opts.forced ?? []) {
    await setPaneSize(page, { width: TIER_WIDTH[tier], height: TIER_HEIGHT, pane: workspace });
    await settle(page);
    await writeContactSheetEvidence(page, workspace, { screen: name, state, tier });
    await expect(workspace).toHaveScreenshot(snapshotName(name, state, tier), shot);
    await resetPaneSize(page, workspace);
  }
}
