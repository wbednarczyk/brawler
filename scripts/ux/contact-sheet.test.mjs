// Node built-in test runner (`node:test`) for the UX contact-sheet orchestrator
// (ADR 0081 plan Q5, Radicle `81313f0`). Exercises the catalog contract and the
// pure merge/render functions against synthetic sidecar data under a throwaway
// temp dir; the final test spawns the real CLI end-to-end (slow — it drives an
// actual Playwright run against two screens).

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import {
  ROOT,
  loadCatalog,
  parseArgs,
  resolveScreens,
  mergeSidecars,
  withImages,
  renderHtml,
  assemble,
} from "./contact-sheet.mjs";

function withTempDir(fn) {
  const dir = mkdtempSync(join(tmpdir(), "contact-sheet-test-"));
  try {
    return fn(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function writeSidecar(dir, name, meta, pngBytes = Buffer.from([137, 80, 78, 71])) {
  writeFileSync(join(dir, `${name}.png`), pngBytes);
  writeFileSync(join(dir, `${name}.json`), JSON.stringify({ ...meta, image: `${name}.png` }));
}

// ---- 1. duplicate/missing catalog screen IDs are rejected ----------------

test("catalog rejects a duplicate screen id", async () => {
  const catalog = await loadCatalog();
  const dup = [...catalog.CATALOG, { screen: "today", spec: "visual-shell-today.spec.ts", states: ["default"], tiers: ["M"] }];
  assert.throws(() => catalog.validateCatalog(dup), /duplicate catalog screen id "today"/);
});

test("catalog rejects an entry with a missing screen id", async () => {
  const catalog = await loadCatalog();
  const missing = [...catalog.CATALOG, { screen: "", spec: "visual-shell-today.spec.ts", states: ["default"], tiers: ["M"] }];
  assert.throws(() => catalog.validateCatalog(missing), /missing a screen id/);
});

// ---- 2. a visual case without catalog metadata is flagged -----------------

test("assertCataloged throws for a screen absent from the catalog", async () => {
  const catalog = await loadCatalog();
  assert.throws(() => catalog.assertCataloged("no-such-screen", "default"), /has no tests\/browser\/visual\/catalog\.ts entry/);
});

test("assertCataloged throws for a known screen with an uncataloged state", async () => {
  const catalog = await loadCatalog();
  assert.throws(() => catalog.assertCataloged("today", "empty-inbox"), /has no catalog state "empty-inbox"/);
});

// ---- 3. parallel sidecars merge without loss -------------------------------

test("mergeSidecars reads every per-worker sidecar without loss", () => {
  withTempDir((dir) => {
    writeSidecar(dir, "today__default__M__dark__chromium-visual--w0-0", {
      screen: "today",
      state: "default",
      tier: "M",
      theme: "dark",
      project: "chromium-visual",
      buildStamp: "123",
    });
    writeSidecar(dir, "fundamentals__default__S__dark__chromium-visual--w1-0", {
      screen: "fundamentals",
      state: "default",
      tier: "S",
      theme: "dark",
      project: "chromium-visual",
      buildStamp: "123",
    });
    writeSidecar(dir, "fundamentals__default__M__light__chromium-visual-light--w1-1", {
      screen: "fundamentals",
      state: "default",
      tier: "M",
      theme: "light",
      project: "chromium-visual-light",
      buildStamp: "123",
    });

    const merged = mergeSidecars(dir);
    assert.equal(merged.length, 3);
    const screens = merged.map((m) => `${m.screen}:${m.tier}:${m.theme}`).sort();
    assert.deepEqual(screens, ["fundamentals:M:light", "fundamentals:S:dark", "today:M:dark"]);
  });
});

test("mergeSidecars returns nothing for a directory that was never created", () => {
  const missingDir = join(tmpdir(), "contact-sheet-never-created-dir");
  assert.deepEqual(mergeSidecars(missingDir), []);
});

// ---- 4. emitted HTML contains each selected screen/state/tier/theme cell + build stamp

test("renderHtml contains a cell per screen/state/tier/theme and the build stamp", () => {
  withTempDir((dir) => {
    writeSidecar(dir, "a", { screen: "today", state: "default", tier: "M", theme: "dark", project: "chromium-visual" });
    writeSidecar(dir, "b", { screen: "today", state: "default", tier: "M", theme: "light", project: "chromium-visual-light" });
    const sidecars = mergeSidecars(dir);
    const cells = withImages(sidecars, dir);
    const html = renderHtml({ buildStamp: "buildstamp-999", cells, missing: [] });

    assert.match(html, /buildstamp-999/);
    assert.match(html, /data-screen="today" data-state="default" data-tier="M" data-theme="dark"/);
    assert.match(html, /data-screen="today" data-state="default" data-tier="M" data-theme="light"/);
    assert.match(html, /data:image\/png;base64,/);
  });
});

test("assemble() reports a missing expected cell as failure", async () => {
  const catalog = await loadCatalog();
  withTempDir((dir) => {
    const sidecarDir = join(dir, "sidecars");
    mkdirSync(sidecarDir);
    // "today" is M-only (dark + light) — only write the dark cell, leaving the
    // light cell unwritten so it must surface as missing.
    writeSidecar(sidecarDir, "a", { screen: "today", state: "default", tier: "M", theme: "dark", project: "chromium-visual" });

    const { missing } = assemble({
      sidecarDir,
      buildDir: dir,
      stamp: "s1",
      screens: ["today"],
      state: "default",
      catalog,
    });

    assert.equal(missing.length, 1);
    assert.equal(missing[0].theme, "light");
  });
});

// ---- 5. --changed maps a representative screen and a shared style file ----

test("resolveScreens maps a spec-owned file to its screens via --changed", async () => {
  const catalog = await loadCatalog();
  const screens = resolveScreens({
    screensArg: null,
    changed: true,
    changedFiles: ["tests/browser/visual/visual-shell-today.spec.ts"],
    resolveChangedFiles: catalog.resolveChangedFiles,
  });
  assert.deepEqual(screens.sort(), ["cockpit-shell", "today"]);
});

test("resolveScreens maps a shared style file to every catalog screen via --changed", async () => {
  const catalog = await loadCatalog();
  const screens = resolveScreens({
    screensArg: null,
    changed: true,
    changedFiles: ["src/ui/Badge.tsx"],
    resolveChangedFiles: catalog.resolveChangedFiles,
  });
  assert.equal(screens.length, catalog.CATALOG.length);
  assert.ok(screens.includes("fundamentals"));
});

test("resolveScreens rejects an unmapped changed file instead of silently selecting nothing", async () => {
  const catalog = await loadCatalog();
  assert.throws(
    () =>
      resolveScreens({
        screensArg: null,
        changed: true,
        changedFiles: ["src/screens/SomeNewScreen.tsx"],
        resolveChangedFiles: catalog.resolveChangedFiles,
      }),
    /no catalog mapping.*pass --screens explicitly/s,
  );
});

test("resolveScreens throws with neither --screens nor --changed", () => {
  assert.throws(
    () => resolveScreens({ screensArg: null, changed: false, changedFiles: [], resolveChangedFiles: () => ({ screens: [], unknown: [] }) }),
    /pass --screens.*or --changed/,
  );
});

// ---- parseArgs ---------------------------------------------------------

test("parseArgs reads --screens, --changed, --state, --theme", () => {
  const opts = parseArgs(["--screens=today,fundamentals", "--changed", "--state=empty", "--theme=light"]);
  assert.deepEqual(opts, { screens: ["today", "fundamentals"], changed: true, state: "empty", theme: "light" });
});

// ---- 6. a baseline update without SCREEN or REASON fails -------------------

test("visual-update-guard exits non-zero and prints usage without SCREEN/REASON", () => {
  const guard = fileURLToPath(new URL("./visual-update-guard.mjs", import.meta.url));
  assert.throws(() => {
    execFileSync(process.execPath, [guard], { env: { ...process.env, SCREEN: "", REASON: "" } });
  }, /Command failed/);
});

test("visual-update-guard exits non-zero when only SCREEN is set", () => {
  const guard = fileURLToPath(new URL("./visual-update-guard.mjs", import.meta.url));
  assert.throws(() => {
    execFileSync(process.execPath, [guard], { env: { ...process.env, SCREEN: "today", REASON: "" } });
  });
});

// ---- 7. a real two-screen contact-sheet smoke produces an openable HTML file

test(
  "real contact-sheet smoke: two screens produce an openable index.html",
  { timeout: 180_000 },
  () => {
    const stamp = `smoketest-${Date.now()}`;
    const result = execFileSync(
      process.execPath,
      [join(ROOT, "scripts", "ux", "contact-sheet.mjs"), "--screens=today,fundamentals", "--theme=dark"],
      {
        cwd: ROOT,
        encoding: "utf8",
        env: { ...process.env, BRAWLER_EXPECTED_BUILD_STAMP: stamp },
      },
    );

    assert.match(result, /contact sheet: .*index\.html/);
    const outFile = join(ROOT, ".artifacts", "ux-contact-sheets", stamp, "index.html");
    assert.ok(existsSync(outFile), `expected ${outFile} to exist`);
    const html = readFileSync(outFile, "utf8");
    assert.match(html, /<!doctype html>/);
    assert.match(html, /data-screen="today"/);
    assert.match(html, /data-screen="fundamentals"/);
    assert.match(html, new RegExp(stamp));

    rmSync(join(ROOT, ".artifacts", "ux-contact-sheets", stamp), { recursive: true, force: true });
  },
);
