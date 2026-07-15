#!/usr/bin/env node
// UX contact-sheet orchestrator (ADR 0081 plan Q5, Radicle `81313f0`).
//
// Generates ONE compact local HTML artifact from the EXISTING Playwright
// visual scenarios (`tests/browser/visual/*.spec.ts`) so a human can review a
// batch of screens cheaply. Committed Playwright baselines remain the
// regression mechanism — this script never compares pixels, it only
// assembles evidence the `chromium-visual`/`chromium-visual-light` runs
// already produce (via BRAWLER_CONTACT_SHEET_DIR sidecars written by
// `tests/browser/visual/helpers.ts`). No Sharp/ImageMagick/native image
// dependency — PNGs are inlined as base64 data URIs.
//
// If the underlying Playwright run fails, the contact sheet is still built
// from whatever sidecars got written before the failing assertion, and this
// script exits with Playwright's ORIGINAL non-zero exit code — the failure
// must not be swallowed.

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = resolve(fileURLToPath(new URL("../..", import.meta.url)));
export const VISUAL_DIR = join(ROOT, "tests", "browser", "visual");
export const ARTIFACTS_DIR = join(ROOT, ".artifacts", "ux-contact-sheets");

/**
 * Loads the plain-JS catalog core directly (never the `.ts` re-export) so
 * this script runs unmodified under every Node this repo runs on, including
 * the nix devshell's pinned version, which does not enable TypeScript
 * type-stripping by default.
 */
export async function loadCatalog() {
  return import(join(VISUAL_DIR, "catalog.core.mjs"));
}

// ---- CLI args ----------------------------------------------------------

export function parseArgs(argv) {
  const opts = { screens: null, changed: false, state: "default", theme: null };
  for (const arg of argv) {
    if (arg === "--changed") {
      opts.changed = true;
      continue;
    }
    if (!arg.startsWith("--")) continue;
    const eq = arg.indexOf("=");
    if (eq === -1) continue;
    const key = arg.slice(2, eq);
    const value = arg.slice(eq + 1);
    if (key === "screens") {
      opts.screens = value
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
    } else if (key === "state") {
      opts.state = value;
    } else if (key === "theme") {
      opts.theme = value;
    }
  }
  return opts;
}

// ---- Changed-file resolution (read-only git diff) ----------------------

/** Read-only: never stages, commits, or mutates repo state. */
export function gitChangedFiles(cwd = ROOT) {
  const tracked = execFileSync("git", ["diff", "--name-only", "HEAD"], { cwd, encoding: "utf8" });
  const staged = execFileSync("git", ["diff", "--name-only", "--cached"], { cwd, encoding: "utf8" });
  const untracked = execFileSync("git", ["ls-files", "--others", "--exclude-standard"], {
    cwd,
    encoding: "utf8",
  });
  const files = new Set();
  for (const line of [...tracked.split("\n"), ...staged.split("\n"), ...untracked.split("\n")]) {
    const trimmed = line.trim();
    if (trimmed) files.add(trimmed);
  }
  return [...files];
}

/**
 * Resolves the screens to shoot. An unmapped/unknown changed file is a hard
 * error (never a silent empty selection) unless `--screens` was also passed
 * explicitly, in which case it wins outright.
 */
export function resolveScreens({ screensArg, changed, changedFiles, resolveChangedFiles }) {
  if (screensArg && screensArg.length > 0) return screensArg;
  if (!changed) {
    throw new Error("contact-sheet: pass --screens=<a,b,c> or --changed");
  }
  const { screens, unknown } = resolveChangedFiles(changedFiles);
  if (unknown.length > 0) {
    throw new Error(
      `contact-sheet: --changed touched file(s) with no catalog mapping (${unknown.join(", ")}) — ` +
        "pass --screens explicitly instead of silently selecting nothing.",
    );
  }
  if (screens.length === 0) {
    throw new Error("contact-sheet: --changed found no mapped screens; pass --screens explicitly.");
  }
  return screens;
}

export function specsForScreens(catalog, screens) {
  const specs = new Set();
  for (const screen of screens) {
    const entry = catalog.findCatalogEntry(screen);
    if (!entry) throw new Error(`contact-sheet: unknown screen "${screen}" (not in catalog.ts)`);
    specs.add(entry.spec);
  }
  return [...specs];
}

// ---- Playwright run -----------------------------------------------------

export function projectsForTheme(theme) {
  if (theme === "dark") return ["chromium-visual"];
  if (theme === "light") return ["chromium-visual-light"];
  return ["chromium-visual", "chromium-visual-light"];
}

export function runPlaywright({ specs, theme, sidecarDir, cwd = ROOT, env = process.env }) {
  // `--project=<name>` (not the space-separated form): Playwright's CLI
  // option is variadic and would otherwise swallow the following spec-path
  // positional args into the previous --project's value list.
  const projectArgs = projectsForTheme(theme).map((p) => `--project=${p}`);
  const specArgs = specs.map((s) => join("tests", "browser", "visual", s));
  const result = spawnSync("npx", ["playwright", "test", ...projectArgs, ...specArgs], {
    cwd,
    stdio: "inherit",
    env: { ...env, BRAWLER_CONTACT_SHEET_DIR: sidecarDir },
  });
  return result.status ?? 1;
}

// ---- Sidecar merge + missing-cell detection -----------------------------

/** Reads every per-worker JSON sidecar in `dir`; unique filenames mean no overwrite races. */
export function mergeSidecars(dir) {
  if (!existsSync(dir)) return [];
  const entries = [];
  for (const name of readdirSync(dir)) {
    if (!name.endsWith(".json")) continue;
    const raw = JSON.parse(readFileSync(join(dir, name), "utf8"));
    entries.push(raw);
  }
  return entries;
}

function cellKey(c) {
  return `${c.screen}::${c.state}::${c.tier}::${c.theme}`;
}

/** Cells the catalog expects but no sidecar exists for — a wiring bug, reported as failure. */
export function findMissingCells(expected, sidecars) {
  const have = new Set(sidecars.map(cellKey));
  return expected.filter((c) => !have.has(cellKey(c)));
}

// ---- HTML rendering -------------------------------------------------------

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

/** Inlines each sidecar's PNG (resolved relative to `dir`) as a base64 data URI. */
export function withImages(sidecars, dir) {
  return sidecars.map((s) => ({
    ...s,
    dataUri: `data:image/png;base64,${readFileSync(join(dir, s.image)).toString("base64")}`,
  }));
}

export function renderHtml({ buildStamp, cells, missing }) {
  const rows = cells
    .map(
      (c) => `      <figure class="cell" data-screen="${escapeHtml(c.screen)}" data-state="${escapeHtml(c.state)}" data-tier="${escapeHtml(c.tier)}" data-theme="${escapeHtml(c.theme)}">
        <img src="${c.dataUri}" alt="${escapeHtml(c.screen)} ${escapeHtml(c.state)} ${escapeHtml(c.tier)} ${escapeHtml(c.theme)}" loading="lazy" />
        <figcaption>${escapeHtml(c.screen)} · ${escapeHtml(c.state)} · ${escapeHtml(c.tier)} · ${escapeHtml(c.theme)}</figcaption>
      </figure>`,
    )
    .join("\n");

  const missingHtml =
    missing.length > 0
      ? `    <section class="missing">
      <h2>Missing cells (${missing.length})</h2>
      <ul>
${missing.map((m) => `        <li>${escapeHtml(m.screen)} · ${escapeHtml(m.state)} · ${escapeHtml(m.tier)} · ${escapeHtml(m.theme)}</li>`).join("\n")}
      </ul>
    </section>`
      : "";

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<title>Brawler UX contact sheet — build ${escapeHtml(buildStamp)}</title>
<style>
  body { font-family: system-ui, sans-serif; background: #111; color: #eee; margin: 0; padding: 1.5rem; }
  h1 { font-size: 1.05rem; font-weight: 600; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 1rem; margin-top: 1rem; }
  .cell { margin: 0; border: 1px solid #333; padding: 0.5rem; background: #1a1a1a; }
  .cell img { max-width: 100%; display: block; background: #fff; }
  figcaption { font-size: 0.75rem; margin-top: 0.35rem; color: #aaa; }
  .missing { margin-top: 1.5rem; color: #f66; }
</style>
</head>
<body>
  <h1>Brawler UX contact sheet — build ${escapeHtml(buildStamp)} (${cells.length} cell${cells.length === 1 ? "" : "s"})</h1>
  <div class="grid">
${rows}
  </div>
${missingHtml}
</body>
</html>
`;
}

// ---- Orchestration --------------------------------------------------------

export function buildStamp() {
  return process.env.BRAWLER_EXPECTED_BUILD_STAMP ?? String(Date.now());
}

/** Merges sidecars, finds missing cells against the catalog's expectation, renders + writes index.html. */
export function assemble({ sidecarDir, buildDir, stamp, screens, state, theme, catalog }) {
  const sidecars = mergeSidecars(sidecarDir);
  const expected = catalog.expectedCells(screens, state, theme ?? undefined);
  const missing = findMissingCells(expected, sidecars);
  const cells = withImages(sidecars, sidecarDir).sort((a, b) => cellKey(a).localeCompare(cellKey(b)));
  const html = renderHtml({ buildStamp: stamp, cells, missing });
  mkdirSync(buildDir, { recursive: true });
  const outFile = join(buildDir, "index.html");
  writeFileSync(outFile, html, "utf8");
  return { outFile, missing, cells };
}

export async function main(argv) {
  const args = parseArgs(argv);
  const catalog = await loadCatalog();

  const screens = resolveScreens({
    screensArg: args.screens,
    changed: args.changed,
    changedFiles: args.changed ? gitChangedFiles() : [],
    resolveChangedFiles: catalog.resolveChangedFiles,
  });
  const specs = specsForScreens(catalog, screens);

  const stamp = buildStamp();
  const buildDir = join(ARTIFACTS_DIR, stamp);
  const sidecarDir = join(buildDir, "sidecars");
  mkdirSync(sidecarDir, { recursive: true });

  const playwrightExit = runPlaywright({ specs, theme: args.theme, sidecarDir });

  const { outFile, missing } = assemble({
    sidecarDir,
    buildDir,
    stamp,
    screens,
    state: args.state,
    theme: args.theme,
    catalog,
  });

  console.log(`contact sheet: ${outFile}`);
  if (missing.length > 0) {
    console.error(`contact sheet: ${missing.length} expected cell(s) missing:`);
    for (const m of missing) console.error(`  ✖ ${m.screen} · ${m.state} · ${m.tier} · ${m.theme}`);
  }

  if (playwrightExit !== 0) return playwrightExit;
  if (missing.length > 0) return 1;
  return 0;
}

const isMainModule = process.argv[1] && import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  main(process.argv.slice(2)).then(
    (code) => process.exit(code),
    (err) => {
      console.error(err.message ?? err);
      process.exit(1);
    },
  );
}
