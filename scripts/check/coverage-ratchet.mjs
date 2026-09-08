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
//   frontend — src/screens/<name>, src/ui, src/shared, src/app, src/api
//     (aggregated from the per-file entries; a file outside these buckets,
//     e.g. src/main.tsx, is not attributed to any directory floor)
//   rust     — src-tauri/src/<top-level-dir>, or src-tauri/src/(root) for a
//     file directly under src-tauri/src/ with no subdirectory
// A directory with zero executable lines in the measured summary is skipped
// entirely (no pin required, nothing to enforce). A directory absent from
// `dirs` is admitted only when its measured coverage is >= 70% — the PR
// still fails (a baseline pin is a deliberate, reviewed addition) but the
// message names the exact line to add; below 70% it is rejected outright.
//
// Known blind spot: masking inside a large flat module (e.g. storage, before
// any further submodule split) — coverage debt inside one directory bucket
// can hide behind an aggregate that still clears the floor. Revisit if it
// bites (split the bucket, or lean on the module's own file-size-ratchet pin).
//
// Inputs:
//   coverage/frontend/coverage-summary.json  (per-file .lines + .total.lines.pct)
//   coverage/rust-summary.json               (.data[0].files[] + .data[0].totals.lines.percent)

import { readFileSync, writeFileSync } from "node:fs";

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
  return null;
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

let failed = false;
const raises = [];

for (const layer of layers) {
  const { json, aggregated } = loadAggregated(layer);

  const floor = baseline[layer].lines;
  const now = globalPct(layer, json);
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

  for (const [key, entry] of Object.entries(aggregated)) {
    if (entry.total === 0) continue; // zero-executable-line directory — nothing to enforce
    const pct = pctOf(entry);
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
    const dirStatus = pct + TOLERANCE < pin ? "FAIL" : "ok";
    if (dirStatus === "FAIL") failed = true;
    if (pct - pin >= RAISE_BY) raises.push(`${layer} ${key}: ${pin} -> ${pct.toFixed(1)}`);
    console.log(`  ${dirStatus} ${layer} ${key} lines ${pct.toFixed(2)}% (${entry.covered}/${entry.total}) (floor ${pin}%)`);
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
