#!/usr/bin/env node
// Coverage ratchet (ADR 0048, scoped per-layer PR check per ADR 0096 decision
// 4, extended with per-directory floors by the decision-4 amendment 2026-09-08
// — G15, hard gates wave 2). Reads the frontend (Vitest v8) and/or Rust
// (cargo-llvm-cov) line-coverage summary and fails if:
//   - the layer's GLOBAL line coverage drops below its floor in
//     coverage-baseline.json (unchanged from the original ratchet), or
//   - any PER-DIRECTORY floor in coverage-baseline.json's `dirs` block
//     regresses, or a directory with measured coverage has no pin at all.
// Trend enforcement (never regress), not a brittle absolute target: when
// coverage rises meaningfully it prints the new floor to commit.
//
// --layer=frontend|rust scopes the check to one layer's input/floor only
// (`make coverage-frontend` / `make coverage-rust`, each its own PR required
// check) and does not require the OTHER layer's result file to exist. No
// --layer enforces both (needs both summaries present).
//
// --seed prints the `dirs` block for the requested layer(s), computed from
// the summaries already on disk, and exits 0 WITHOUT touching the baseline
// file — paste the block into coverage-baseline.json by hand (a reviewed
// diff line, the same idiom as file-size-ratchet's baseline entries).
// --write raises EXISTING per-directory pins to match improved measured
// coverage; it never lowers a pin (a drop stays red until fixed) and never
// adds a pin for a directory that isn't already pinned — adding one is a
// deliberate, reviewed --seed paste, never something this flag does silently.
//
// Per-directory keys:
//   frontend — src/screens/<name>, src/ui, src/shared, src/app, src/api, or
//     src/(root) for every OTHER measured file under src/ (e.g. src/main.tsx)
//     — a catch-all bucket, mirroring the Rust `(root)` key below, so no
//     measured production file is ever silently unbucketed.
//   rust     — src-tauri/src/<top-level-dir>, or src-tauri/src/(root) for a
//     file directly under src-tauri/src/ with no subdirectory
// A directory with zero executable lines in the measured summary is skipped
// entirely (no pin required, nothing to enforce). A directory absent from
// `dirs` is admitted only when its measured coverage is >= 70% — the PR
// still fails (a baseline pin is a deliberate, reviewed addition) but the
// message names the exact line to add; below 70% it is rejected outright.
// A directory PINNED in `dirs` but producing NO measured coverage at all
// fails outright unless it no longer exists on disk (source deleted — then
// it's a note to drop the stale pin, not a failure).
//
// Base-baseline comparison (G15 amendment 2026-09-08, closes the admission
// hole where a new key could enter at pin 0): with COVERAGE_BASE_REF set (the
// PR's base sha), a `dirs` key NEW relative to that revision's
// coverage-baseline.json must measure AND pin >= 70%; an EXISTING key's pin
// must never read lower than it did at the base revision. Unset (non-PR run,
// or the base baseline can't be read) skips this comparison with a note.
//
// Known blind spot: masking inside a large flat module (e.g. storage, before
// any further submodule split) — coverage debt inside one directory bucket
// can hide behind an aggregate that still clears the floor. Revisit if it
// bites (split the bucket, or lean on the module's own file-size-ratchet pin).
//
// Inputs:
//   coverage/frontend/coverage-summary.json  (per-file .lines + .total.lines.pct)
//   coverage/rust-summary.json               (.data[0].files[] + .data[0].totals.lines.percent)

import { execFileSync } from "node:child_process";
import { readFileSync, statSync, writeFileSync } from "node:fs";

const TOLERANCE = 0.5; // percentage points of slack for measurement noise
const RAISE_BY = 1.0; // suggest raising the floor when current exceeds it by this
const ADMISSION_FLOOR = 70; // a directory absent from the baseline is admitted only at/above this

const args = process.argv.slice(2);
const layerArg = args.find((a) => a.startsWith("--layer="));
const requestedLayer = layerArg ? layerArg.slice("--layer=".length) : null;
if (requestedLayer !== null && !["frontend", "rust"].includes(requestedLayer)) {
  console.error(`coverage-ratchet: --layer must be "frontend" or "rust" (got: ${requestedLayer})`);
  process.exit(2);
}
const layers = requestedLayer ? [requestedLayer] : ["frontend", "rust"];
const seed = args.includes("--seed");
const write = args.includes("--write");

const SUMMARY_PATH = { frontend: "coverage/frontend/coverage-summary.json", rust: "coverage/rust-summary.json" };
const MAKE_TARGET = { frontend: "coverage-frontend", rust: "coverage-rust" };

function readJson(filePath, makeTarget) {
  try {
    return JSON.parse(readFileSync(filePath, "utf8"));
  } catch (err) {
    console.error(`coverage-ratchet: cannot read ${filePath}: ${err.message}`);
    console.error(`Run \`make ${makeTarget}\` (it produces the summary before this check).`);
    process.exit(2);
  }
}

function sortObj(obj) {
  return Object.fromEntries(Object.entries(obj).sort(([a], [b]) => (a < b ? -1 : 1)));
}

function normalizeSep(p) {
  return p.replace(/\\/g, "/");
}

/** The suffix of `rawPath` starting at `marker`, whether rawPath is already
 * relative (starts with marker) or absolute (contains "/" + marker).
 * Boundary-safe: a path merely CONTAINING the marker text (e.g. "mysrc/x")
 * does not match — the marker must sit at a path-segment boundary. */
function afterMarker(rawPath, marker) {
  const p = normalizeSep(rawPath);
  if (p.startsWith(marker)) return p;
  const idx = p.indexOf(`/${marker}`);
  return idx === -1 ? null : p.slice(idx + 1);
}

function frontendDirKey(rawPath) {
  const rel = afterMarker(rawPath, "src/");
  if (rel === null) return null;
  const parts = rel.split("/");
  if (parts[1] === "screens" && parts.length > 2) return `src/screens/${parts[2]}`;
  if (["ui", "shared", "app", "api"].includes(parts[1])) return `src/${parts[1]}`;
  // Catch-all bucket (mirrors the Rust `(root)` key): every measured
  // production frontend file under src/ lands SOMEWHERE, never silently
  // unbucketed (G15 fix #7).
  return "src/(root)";
}

/** Map a `dirs` key back to the filesystem directory it represents — a
 * `(root)` key names the PARENT (loose files directly under it), everything
 * else maps 1:1 to its own path. */
function dirKeyToFsPath(key) {
  return key.endsWith("/(root)") ? key.slice(0, -"/(root)".length) : key;
}

function validPct(n) {
  return typeof n === "number" && Number.isFinite(n) && n >= 0 && n <= 100;
}

function rustDirKey(rawPath) {
  const marker = "src-tauri/src/";
  const rel = afterMarker(rawPath, marker);
  if (rel === null) return null;
  const rest = rel.slice(marker.length);
  const slash = rest.indexOf("/");
  return slash === -1 ? "src-tauri/src/(root)" : `src-tauri/src/${rest.slice(0, slash)}`;
}

function addTo(map, key, covered, total) {
  if (key === null) return;
  if (!map[key]) map[key] = { covered: 0, total: 0 };
  map[key].covered += covered;
  map[key].total += total;
}

function aggregateFrontend(json) {
  const out = {};
  for (const [key, entry] of Object.entries(json)) {
    if (key === "total") continue;
    const lines = entry.lines ?? {};
    addTo(out, frontendDirKey(key), lines.covered ?? 0, lines.total ?? 0);
  }
  return out;
}

function aggregateRust(json) {
  const out = {};
  const files = json?.data?.[0]?.files ?? [];
  for (const f of files) {
    const lines = f.summary?.lines ?? {};
    addTo(out, rustDirKey(f.filename ?? ""), lines.covered ?? 0, lines.count ?? 0);
  }
  return out;
}

const AGGREGATORS = { frontend: aggregateFrontend, rust: aggregateRust };

function globalPct(layer, json) {
  return layer === "frontend" ? json.total.lines.pct : json.data[0].totals.lines.percent;
}

function pctOf(entry) {
  return (entry.covered / entry.total) * 100;
}

function loadAggregated(layer) {
  const json = readJson(SUMMARY_PATH[layer], MAKE_TARGET[layer]);
  return { json, aggregated: AGGREGATORS[layer](json) };
}

function baselineReadTarget() {
  return requestedLayer ? `coverage-${requestedLayer}` : "coverage";
}

// --- --seed: print the `dirs` block for each requested layer, touch nothing --
if (seed) {
  for (const layer of layers) {
    const { aggregated } = loadAggregated(layer);
    const dirs = {};
    for (const [key, entry] of Object.entries(aggregated)) {
      if (entry.total === 0) continue;
      dirs[key] = Math.round(pctOf(entry) * 10) / 10;
    }
    console.log(`\n"${layer}": { "dirs": ${JSON.stringify(sortObj(dirs), null, 2)} }`);
  }
  process.exit(0);
}

// --- --write: raise existing pins only — never lower, never add --------------
if (write) {
  const baseline = readJson("coverage-baseline.json", baselineReadTarget());
  for (const layer of layers) {
    const { aggregated } = loadAggregated(layer);
    const existingDirs = baseline[layer]?.dirs ?? {};
    const nextDirs = { ...existingDirs };
    for (const [key, pin] of Object.entries(existingDirs)) {
      const entry = aggregated[key];
      if (!entry || entry.total === 0) continue; // vanished or hollowed out — leave the pin as-is
      const pct = Math.round(pctOf(entry) * 10) / 10;
      if (pct > pin) nextDirs[key] = pct; // raise only, never lower
    }
    if (baseline[layer]) baseline[layer].dirs = sortObj(nextDirs);
  }
  writeFileSync("coverage-baseline.json", `${JSON.stringify(baseline, null, 2)}\n`);
  console.log(`coverage-ratchet: --write raised eligible per-directory pins (${layers.join(", ")}).`);
  process.exit(0);
}

// --- normal check: global floor (unchanged) + per-directory floors -----------
const baseline = readJson("coverage-baseline.json", baselineReadTarget());

// Base-baseline comparison (G15 fix #5, admission-hole close): a NEW dirs key
// (absent from the BASE revision's baseline) may only enter at/above the 70%
// admission floor for BOTH measured coverage and its pin — a pin of 0 written
// straight into the head baseline no longer buys a free pass. An EXISTING
// key's pin must never read lower than it did at base. COVERAGE_BASE_REF
// (set by full-check.yml from the PR's base sha) drives the comparison; unset
// (a non-PR run) skips it with a printed note, never a hard failure.
const COVERAGE_BASE_REF = process.env.COVERAGE_BASE_REF;
let baseBaseline = null;
if (!COVERAGE_BASE_REF) {
  console.log("coverage-ratchet: COVERAGE_BASE_REF not set (non-PR run) — skipping base-baseline pin comparison.");
} else {
  try {
    const raw = execFileSync("git", ["show", `${COVERAGE_BASE_REF}:coverage-baseline.json`], { encoding: "utf8" });
    baseBaseline = JSON.parse(raw);
  } catch (err) {
    // Fail-closed (hard gates wave 2): COVERAGE_BASE_REF being SET means this
    // is a PR run that expects the comparison to happen. A read failure here
    // (bad ref, shallow checkout, corrupt JSON) must never silently degrade
    // into "skip the check" — that is exactly the admission hole the
    // base-baseline comparison exists to close, just moved one step earlier.
    console.error(
      `coverage-ratchet: COVERAGE_BASE_REF=${COVERAGE_BASE_REF} is set but coverage-baseline.json could not be read at that ref (${err.message}).`,
    );
    console.error("coverage-ratchet: failing closed rather than silently skipping the base-baseline pin comparison.");
    process.exit(1);
  }
}

let failed = false;
const raises = [];

for (const layer of layers) {
  const { json, aggregated } = loadAggregated(layer);

  const floor = baseline[layer].lines;
  if (!validPct(floor)) {
    console.error(`coverage-ratchet: ${layer}.lines in coverage-baseline.json is not a valid percentage (${floor}).`);
    failed = true;
    continue;
  }
  const now = globalPct(layer, json);
  if (!validPct(now)) {
    console.error(`coverage-ratchet: measured ${layer} coverage is not a valid percentage (${now}) — check the coverage summary input.`);
    failed = true;
    continue;
  }
  const status = now + TOLERANCE < floor ? "FAIL" : "ok";
  if (status === "FAIL") failed = true;
  if (now - floor >= RAISE_BY) raises.push(`${layer}: ${floor} -> ${now.toFixed(1)}`);
  console.log(`  ${status} ${layer} lines ${now.toFixed(2)}% (floor ${floor}%)`);

  const dirsBaseline = baseline[layer]?.dirs;
  if (dirsBaseline === undefined) {
    console.error(
      `\ncoverage-ratchet: coverage-baseline.json is missing "${layer}.dirs" — per-directory floors are required (G15, ADR 0096 dec. 4 amendment).`,
    );
    console.error(
      `  Bootstrap it: node scripts/check/coverage-ratchet.mjs --seed --layer=${layer}   (uses the ${layer} summary already on disk, needs no coverage run)`,
    );
    failed = true;
    continue;
  }

  const baseDirs = baseBaseline?.[layer]?.dirs ?? null;

  for (const [key, entry] of Object.entries(aggregated)) {
    if (entry.total === 0) continue; // zero-executable-line directory — nothing to enforce
    const pct = pctOf(entry);
    if (!validPct(pct)) {
      failed = true;
      console.error(`  FAIL ${layer} ${key} measured coverage is not a valid percentage (${pct}) — check the coverage summary input.`);
      continue;
    }
    const pin = dirsBaseline[key];
    if (pin === undefined) {
      failed = true;
      if (pct >= ADMISSION_FLOOR) {
        const suggestedPin = (Math.round(pct * 10) / 10).toFixed(1);
        console.error(
          `  FAIL ${layer} ${key} lines ${pct.toFixed(2)}% (${entry.covered}/${entry.total}) — not pinned; add \`"${key}": ${suggestedPin}\` to coverage-baseline.json dirs.`,
        );
      } else {
        console.error(
          `  FAIL ${layer} ${key} lines ${pct.toFixed(2)}% (${entry.covered}/${entry.total}) — below the ${ADMISSION_FLOOR}% admission floor for an unpinned directory.`,
        );
      }
      continue;
    }
    if (!validPct(pin)) {
      failed = true;
      console.error(`  FAIL ${layer} ${key} pin in coverage-baseline.json is not a valid percentage (${pin}).`);
      continue;
    }

    if (baseDirs !== null) {
      const basePin = baseDirs[key];
      const isNewKey = basePin === undefined || !validPct(basePin);
      if (isNewKey) {
        if (pct < ADMISSION_FLOOR || pin < ADMISSION_FLOOR) {
          failed = true;
          console.error(
            `  FAIL ${layer} ${key} lines ${pct.toFixed(2)}% pin ${pin}% — new directory relative to the base baseline; both measured coverage and the pin must be >= ${ADMISSION_FLOOR}% (G15 admission-hole close).`,
          );
          continue;
        }
      } else if (pin < basePin) {
        failed = true;
        console.error(
          `  FAIL ${layer} ${key} pin lowered from ${basePin}% (base) to ${pin}% (head) — an existing directory's pin must never drop.`,
        );
        continue;
      }
    }

    const dirStatus = pct + TOLERANCE < pin ? "FAIL" : "ok";
    if (dirStatus === "FAIL") failed = true;
    if (pct - pin >= RAISE_BY) raises.push(`${layer} ${key}: ${pin} -> ${pct.toFixed(1)}`);
    console.log(`  ${dirStatus} ${layer} ${key} lines ${pct.toFixed(2)}% (${entry.covered}/${entry.total}) (floor ${pin}%)`);
  }

  // Pinned-but-unmeasured (G15 fix #6): a key in `dirs` that never showed up
  // in `aggregated` at all (not even a zero-total entry) got NO measured
  // coverage this run. That's a hard failure UNLESS the directory is
  // genuinely gone from disk (source deleted) — then it's a drop-the-pin note.
  for (const key of Object.keys(dirsBaseline)) {
    if (aggregated[key]) continue; // already handled above
    const fsPath = dirKeyToFsPath(key);
    let existsOnDisk = false;
    try {
      existsOnDisk = statSync(fsPath).isDirectory();
    } catch {
      existsOnDisk = false;
    }
    if (!existsOnDisk) {
      console.log(`  note ${layer} ${key} — no measured coverage and the directory no longer exists on disk; drop its pin from coverage-baseline.json.`);
      continue;
    }
    failed = true;
    console.error(
      `  FAIL ${layer} ${key} is pinned at ${dirsBaseline[key]}% but produced no measured coverage, and the directory still exists on disk — fix coverage collection for it, or delete the dead code (not just the pin).`,
    );
  }

  // Union-of-keys extension (hard gates wave 2): the checks above cover every
  // key in `aggregated` (measured) and every key in `dirsBaseline` (head
  // pins) — but a key pinned in the BASE baseline that vanished from BOTH the
  // head baseline and this run's measured coverage falls through both loops
  // untouched, silently dropping its pin instead of failing. Close that gap
  // by walking the union's third member: base-only keys.
  if (baseDirs !== null) {
    for (const key of Object.keys(baseDirs)) {
      if (dirsBaseline[key] !== undefined) continue; // still pinned in head — handled above
      if (aggregated[key]?.total > 0) continue; // still measured — handled by the "not pinned" branch above
      const fsPath = dirKeyToFsPath(key);
      let existsOnDisk = false;
      try {
        existsOnDisk = statSync(fsPath).isDirectory();
      } catch {
        existsOnDisk = false;
      }
      if (!existsOnDisk) {
        console.log(
          `  note ${layer} ${key} — pinned at ${baseDirs[key]}% in the base baseline but no longer present in the head baseline; the directory no longer exists on disk, so the dropped pin is expected.`,
        );
        continue;
      }
      failed = true;
      console.error(
        `  FAIL ${layer} ${key} was pinned at ${baseDirs[key]}% in the base baseline but its pin is missing from the head baseline, and the directory still exists on disk — a pin cannot silently disappear (restore it, or delete the directory).`,
      );
    }
  }
}

if (raises.length > 0) {
  console.log(`\nCoverage rose — raise the floor in coverage-baseline.json (or run --write for the per-directory pins):\n  ${raises.join("\n  ")}`);
}

if (failed) {
  console.error(
    "\n✖ Coverage dropped below a committed floor, or a measured directory has no pin — add tests, justify lowering a floor, or add the new directory's pin (see messages above).",
  );
  process.exit(1);
}
console.log("\n✓ Coverage holds at or above every committed floor (global and per-directory).");
