#!/usr/bin/env node
// Coverage ratchet (ADR 0048). Reads the frontend (Vitest v8) and Rust
// (cargo-llvm-cov) line-coverage summaries and fails if either drops below the
// committed floor in coverage-baseline.json. This enforces the full-coverage
// policy as a trend (never regress) without a brittle absolute target: when
// coverage rises meaningfully it prints the new floors to commit.
//
// Inputs (produced by `make coverage`):
//   coverage/frontend/coverage-summary.json  (.total.lines.pct)
//   coverage/rust-summary.json               (.data[0].totals.lines.percent)

import { readFileSync } from "node:fs";

const TOLERANCE = 0.5; // percentage points of slack for measurement noise
const RAISE_BY = 1.0; // suggest raising the floor when current exceeds it by this

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    console.error(`coverage-ratchet: cannot read ${path}: ${err.message}`);
    console.error("Run `make coverage` (it produces the summaries before this check).");
    process.exit(2);
  }
}

const baseline = readJson("coverage-baseline.json");
const frontend = readJson("coverage/frontend/coverage-summary.json");
const rust = readJson("coverage/rust-summary.json");

const current = {
  frontend: frontend.total.lines.pct,
  rust: rust.data[0].totals.lines.percent,
};

let failed = false;
const raises = [];
for (const layer of ["frontend", "rust"]) {
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
