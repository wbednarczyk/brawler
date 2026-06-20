#!/usr/bin/env node
// Benchmark ratchet (ADR 0049, T5). Reads the criterion median estimates from
// the last `make bench` run and flags any kernel that regressed beyond a
// tolerance against the committed floor in bench-baseline.json. This mirrors the
// coverage ratchet, but for performance: it is PERIODIC and machine-dependent —
// it is NOT part of `make check` and absolute wall-clock is never a hard gate.
// Run it on the reference machine; update the baseline deliberately when the
// hardware or the kernels legitimately change.
//
// Input (produced by `make bench`):
//   src-tauri/target/criterion/<bench>/new/estimates.json  (.median.point_estimate, ns)

import { readFileSync } from "node:fs";

// Allow a kernel to be up to this factor slower than baseline before failing —
// generous, because criterion medians vary with machine load. A real regression
// (algorithmic, not noise) blows past this.
const TOLERANCE = 1.6;
// Suggest lowering the floor when a kernel is at least this much faster.
const IMPROVE = 0.8;

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    console.error(`bench-ratchet: cannot read ${path}: ${err.message}`);
    console.error("Run `make bench` first (it produces the criterion estimates this reads).");
    process.exit(2);
  }
}

const baseline = readJson("bench-baseline.json");
const benches = baseline.benches;

let failed = false;
const improvements = [];
for (const [name, floorNs] of Object.entries(benches)) {
  const estimate = readJson(
    `src-tauri/target/criterion/${name}/new/estimates.json`,
  );
  const nowNs = estimate.median.point_estimate;
  const ratio = nowNs / floorNs;
  const status = ratio > TOLERANCE ? "FAIL" : "ok";
  if (status === "FAIL") failed = true;
  if (ratio <= IMPROVE) improvements.push(`${name}: ${Math.round(floorNs)} -> ${Math.round(nowNs)} ns`);
  console.log(
    `  ${status} ${name} ${Math.round(nowNs)} ns (floor ${Math.round(floorNs)} ns, ${ratio.toFixed(2)}x)`,
  );
}

if (improvements.length > 0) {
  console.log(`\nKernels got faster — lower the floor in bench-baseline.json:\n  ${improvements.join("\n  ")}`);
}

if (failed) {
  console.error(
    `\n✖ A kernel regressed beyond ${TOLERANCE}x — investigate (algorithmic regression?) or justify raising the baseline.`,
  );
  process.exit(1);
}
console.log("\n✓ All benched kernels hold within tolerance of the committed floor.");
