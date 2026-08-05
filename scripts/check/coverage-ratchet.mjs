#!/usr/bin/env node
// Coverage ratchet (ADR 0048, scoped per-layer PR check per ADR 0096 decision
// 4). Reads the frontend (Vitest v8) and/or Rust (cargo-llvm-cov)
// line-coverage summary and fails if it drops below the committed floor in
// coverage-baseline.json. Trend enforcement (never regress), not a brittle
// absolute target: when coverage rises meaningfully it prints the new floor.
//
// --layer=frontend|rust scopes the check to one layer's input/floor only
// (`make coverage-frontend` / `make coverage-rust`, each its own PR required
// check) and does not require the OTHER layer's result file to exist. No
// --layer enforces both (needs both summaries present).
//
// Inputs:
//   coverage/frontend/coverage-summary.json  (.total.lines.pct)
//   coverage/rust-summary.json               (.data[0].totals.lines.percent)

import { readFileSync } from "node:fs";

const TOLERANCE = 0.5; // percentage points of slack for measurement noise
const RAISE_BY = 1.0; // suggest raising the floor when current exceeds it by this

const layerArg = process.argv.find((a) => a.startsWith("--layer="));
const requestedLayer = layerArg ? layerArg.slice("--layer=".length) : null;
if (requestedLayer !== null && !["frontend", "rust"].includes(requestedLayer)) {
  console.error(`coverage-ratchet: --layer must be "frontend" or "rust" (got: ${requestedLayer})`);
  process.exit(2);
}
const layers = requestedLayer ? [requestedLayer] : ["frontend", "rust"];

function readJson(path, makeTarget) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    console.error(`coverage-ratchet: cannot read ${path}: ${err.message}`);
    console.error(`Run \`make ${makeTarget}\` (it produces the summary before this check).`);
    process.exit(2);
  }
}

const baseline = readJson("coverage-baseline.json", requestedLayer ? `coverage-${requestedLayer}` : "coverage");

const current = {};
if (layers.includes("frontend")) {
  current.frontend = readJson("coverage/frontend/coverage-summary.json", "coverage-frontend").total.lines.pct;
}
if (layers.includes("rust")) {
  current.rust = readJson("coverage/rust-summary.json", "coverage-rust").data[0].totals.lines.percent;
}

let failed = false;
const raises = [];
for (const layer of layers) {
  const floor = baseline[layer].lines;
  const now = current[layer];
  const status = now + TOLERANCE < floor ? "FAIL" : "ok";
  if (status === "FAIL") failed = true;
  if (now - floor >= RAISE_BY) raises.push(`${layer}: ${floor} -> ${now.toFixed(1)}`);
  console.log(`  ${status} ${layer} lines ${now.toFixed(2)}% (floor ${floor}%)`);
}

if (raises.length > 0) {
  console.log(`\nCoverage rose — raise the floor in coverage-baseline.json:\n  ${raises.join("\n  ")}`);
}

if (failed) {
  console.error("\n✖ Coverage dropped below the committed floor — add tests or justify lowering the baseline.");
  process.exit(1);
}
console.log("\n✓ Coverage holds at or above the committed floor.");
