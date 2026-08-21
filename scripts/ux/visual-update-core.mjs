// Pure resolution/naming/diff logic for `visual-update-guard.mjs` (F0.5 plan
// decision 5). Split out so it is unit-testable without spawning Playwright:
// the guard runner (I/O — hashing files, spawning `npx playwright`) stays a
// thin shell around these functions.
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { CATALOG, findCatalogEntry, expectedCells } from "../../tests/browser/visual/catalog.core.mjs";

const VISUAL_DIR = join(fileURLToPath(import.meta.url), "..", "..", "..", "tests", "browser", "visual");

// Playwright's default snapshot naming: `{arg}-{projectName}-{platform}.png`.
// Our specs always pass `${screen}-${tier}.png` as the arg (helpers.ts).
const PROJECT_BY_THEME = { dark: "chromium-visual", light: "chromium-visual-light" };

/** Resolves a catalog screen id to its owning spec and the cells it expects. Throws on an unknown id. */
export function resolveScreen(screenId) {
  const entry = findCatalogEntry(screenId);
  if (!entry) {
    const known = CATALOG.map((e) => e.screen).join(", ");
    throw new Error(`Unknown SCREEN "${screenId}" — valid catalog ids: ${known}`);
  }
  return { spec: entry.spec, cells: expectedCells([screenId]) };
}

/** Snapshot directory Playwright writes/reads for a given spec file. */
export function specSnapshotDir(spec) {
  return join(VISUAL_DIR, `${spec}-snapshots`);
}

/** Maps an expected cell ({screen,state,tier,theme}) to its baseline PNG's absolute path. */
export function cellFileName(cell) {
  const entry = findCatalogEntry(cell.screen);
  if (!entry) throw new Error(`cellFileName: unknown screen "${cell.screen}"`);
  const project = PROJECT_BY_THEME[cell.theme];
  if (!project) throw new Error(`cellFileName: unknown theme "${cell.theme}"`);
  return join(specSnapshotDir(entry.spec), `${cell.screen}-${cell.tier}-${project}-${process.platform}.png`);
}

/** Every expected cell across the whole catalog (76 today) — the ALL-mode assertion set. */
export function allExpectedCells() {
  return expectedCells(CATALOG.map((entry) => entry.screen));
}

/**
 * Pure comparator over {path -> sha256} maps. `targetFiles` are the cells the
 * run was allowed to change; anything else that changed is sibling drift.
 */
export function diffSnapshots(preMap, postMap, targetFiles) {
  const targets = new Set(targetFiles);
  const preKeys = new Set(Object.keys(preMap));
  const postKeys = new Set(Object.keys(postMap));

  // Filename-set equality applies to SIBLINGS only: a target cell may
  // legitimately (re)appear — the rm-PNGs-first workflow deletes targets
  // before the run — and a target that FAILED to regenerate is reported via
  // `missingTarget`, not as a removal. (Gap found on first real per-screen
  // use: a freshly re-shot target counted as "added" drift.)
  const added = [...postKeys].filter((key) => !preKeys.has(key) && !targets.has(key));
  const removed = [...preKeys].filter((key) => !postKeys.has(key) && !targets.has(key));
  const changedSiblings = [...postKeys].filter(
    (key) => !targets.has(key) && preKeys.has(key) && preMap[key] !== postMap[key],
  );
  const missingTarget = targetFiles.filter((file) => !postKeys.has(file));

  return { added, removed, changedSiblings, missingTarget };
}
