#!/usr/bin/env node
// Guard for `visual-update` (ADR 0081 plan Q5 STOP-AND-ASK; F0.5 plan
// decision 5): a Playwright snapshot update must never run silently, and must
// never silently rewrite screens it wasn't asked to touch.
//
// Two modes, mutually exclusive:
//   SCREEN=<catalog id>  — resolves the id via visual-update-core.mjs to its
//     owning spec, hashes that spec's existing baselines, runs the spec with
//     --update-snapshots=all (NOT bare --update-snapshots: that is "changed"
//     mode and leaves in-tolerance siblings unrewritten, making a hash check
//     vacuous), then hard-fails on any filename-set change, any non-target
//     hash change ("sibling drift"), or a missing target cell.
//   ALL=1 — full repaint: runs both visual projects over every spec with
//     --update-snapshots=all, then asserts every one of the catalog's
//     expected cells (76 today) exists on disk. No sibling check (everything
//     is a legitimate target).
// REASON is mandatory in both modes and is printed into the run log so the
// change description can cite it.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { allExpectedCells, cellFileName, diffSnapshots, resolveScreen, specSnapshotDir } from "./visual-update-core.mjs";

const screen = process.env.SCREEN?.trim();
const allRaw = process.env.ALL?.trim();
const reason = process.env.REASON?.trim();

// ALL must be literally "1" — any other non-empty value (a typo like ALL=0)
// must not trigger a full baseline rewrite.
if (allRaw && allRaw !== "1") {
  console.error(`visual-update: ALL must be exactly "1" (got ALL=${allRaw})`);
  process.exit(1);
}
const all = allRaw === "1";

if (!!screen === all || !reason) {
  console.error(
    'Usage: SCREEN=<catalog-id> REASON="why this baseline changed" npm run visual-update\n' +
      '   or: ALL=1 REASON="why this baseline changed" npm run visual-update',
  );
  process.exit(1);
}

console.log(`visual-update: SCREEN=${screen ?? ""} ALL=${all ? "1" : ""} REASON=${reason}`);

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function hashDir(dir) {
  const map = {};
  let names;
  try {
    names = readdirSync(dir);
  } catch (err) {
    if (err.code === "ENOENT") return map;
    throw err;
  }
  for (const name of names) {
    if (!name.endsWith(".png")) continue;
    const full = join(dir, name);
    map[full] = sha256(full);
  }
  return map;
}

function runPlaywright(args) {
  return spawnSync("npx", ["playwright", "test", ...args], { stdio: "inherit" });
}

if (screen) {
  let resolved;
  try {
    resolved = resolveScreen(screen);
  } catch (err) {
    console.error(err.message);
    process.exit(1);
  }
  const { spec, cells } = resolved;
  const dir = specSnapshotDir(spec);
  const targetFiles = cells.map(cellFileName);

  const before = hashDir(dir);
  const result = runPlaywright([
    `tests/browser/visual/${spec}`,
    "--project=chromium-visual",
    "--project=chromium-visual-light",
    "--update-snapshots=all",
  ]);
  if (result.status !== 0) {
    console.error(`visual-update: playwright exited ${result.status}`);
    process.exit(result.status ?? 1);
  }
  const after = hashDir(dir);

  const { added, removed, changedSiblings, missingTarget } = diffSnapshots(before, after, targetFiles);
  let failed = false;
  if (added.length || removed.length) {
    failed = true;
    console.error("visual-update: baseline filename set changed:");
    if (added.length) console.error(`  added: ${added.join(", ")}`);
    if (removed.length) console.error(`  removed: ${removed.join(", ")}`);
  }
  if (changedSiblings.length) {
    failed = true;
    console.error(`sibling drift — investigate: ${changedSiblings.join(", ")}`);
  }
  if (missingTarget.length) {
    failed = true;
    console.error(`visual-update: expected cell(s) missing after update: ${missingTarget.join(", ")}`);
  }
  if (failed) process.exit(1);
  console.log(`visual-update: SCREEN=${screen} updated cleanly (${targetFiles.length} cell(s), 0 sibling changes).`);
  process.exit(0);
}

// ALL mode.
const result = runPlaywright([
  "tests/browser/visual",
  "--project=chromium-visual",
  "--project=chromium-visual-light",
  "--update-snapshots=all",
]);
if (result.status !== 0) {
  console.error(`visual-update: playwright exited ${result.status}`);
  process.exit(result.status ?? 1);
}

const expected = allExpectedCells();
const missing = expected.map(cellFileName).filter((path) => !existsSync(path));
if (missing.length) {
  console.error(`visual-update: missing expected baseline cell(s) after ALL update:\n  ${missing.join("\n  ")}`);
  process.exit(1);
}
console.log(`visual-update: ALL sweep verified ${expected.length} expected cell(s) on disk.`);
