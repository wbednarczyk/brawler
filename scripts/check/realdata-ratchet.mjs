#!/usr/bin/env node
// Real-data honesty ratchet (epic #40 S4; ADR 0091 decisions 4-5). Sibling of
// `coverage-ratchet.mjs`, same mechanics: a committed baseline, a tolerance for
// measurement noise, and raises that are PRINTED, never written — the owner
// commits an improvement deliberately.
//
// It judges the aggregate metrics emitted by the `#[ignore]` harness
// `src-tauri/src/storage/tests/real_data_honesty.rs` on the maintainer's real
// database. **The real database never enters the repo or CI** (ADR 0091 dec. 4):
// the only committed artifact is `realdata-honesty-baseline.json` — counts and
// percentages, never a title, ticker, or id. Never add a metric that carries
// row content.
//
// Inputs:
//   realdata-honesty-baseline.json                    (committed floors/ceilings)
//   src-tauri/target/realdata-honesty-metrics.json    (this run, gitignored)
// Both overridable: `--baseline <path>` / `--metrics <path>` (used by the
// self-test `scripts/check/check-realdata-ratchet.sh`).
//
// Exit codes:
//   0  every metric holds at or beyond its committed bound
//   1  HONESTY REGRESSION — a metric moved the wrong way beyond tolerance
//   2  the check could not conclude: inputs unreadable/malformed, OR the
//      baseline is stale because honesty IMPROVED and was never committed
//      (a silent raise makes the ratchet toothless — commit the new bound)

import { readFileSync } from "node:fs";

// Each metric declares which direction is BETTER, how much measurement noise to
// forgive, and how big an improvement must be before the baseline counts as
// stale. `raiseBy: null` = a hard bound that can never move (filename-as-
// statement is zero, forever — the harness asserts it too).
const METRICS = [
  {
    key: "specificity_pct",
    bound: "floor",
    label: "rows stating something concrete",
    unit: "%",
    tolerance: 0.5,
    raiseBy: 1.0,
  },
  {
    key: "orphaned_evidence",
    bound: "ceiling",
    label: "events whose evidence resolves to nothing",
    unit: " rows",
    tolerance: 0,
    raiseBy: 1,
  },
  {
    key: "filename_as_statement",
    bound: "ceiling",
    label: "row statements that are a raw filename",
    unit: " rows",
    tolerance: 0,
    raiseBy: null,
  },
  // Epic #40 S5. ADR 0091 specified a HARD zero here; the first real
  // measurement (2026-07-29) found 82 stored rows already in the dishonest
  // state, written by a defect S5 fixes forward (an outcome row re-upserted by a
  // re-run overwrote its fact count with 0 while keeping `reason_code =
  // "emitted"`). A hard bound seeded above zero is not a bound, so this lands as
  // a ratcheted CEILING that decays as the owner re-extracts, and becomes the
  // hard zero the ADR asks for once it reaches 0. Same instrument, same
  // precedent as `orphaned_evidence` (seeded at a known defect class, #119).
  {
    key: "zero_effect_successes",
    bound: "ceiling",
    label: "successes recording no fact while claiming an emission",
    unit: " outcomes",
    tolerance: 0,
    raiseBy: 1,
  },
  {
    key: "silent_missing_metrics",
    bound: "ceiling",
    label: "health read-model outputs missing without naming what is missing",
    unit: " outputs",
    tolerance: 0,
    raiseBy: 1,
  },
];

function argValue(flag, fallback) {
  const index = process.argv.indexOf(flag);
  return index !== -1 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
}

const baselinePath = argValue("--baseline", "realdata-honesty-baseline.json");
const metricsPath = argValue("--metrics", "src-tauri/target/realdata-honesty-metrics.json");

function readJson(path, hint) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    console.error(`realdata-ratchet: cannot read ${path}: ${err.message}`);
    console.error(hint);
    process.exit(2);
  }
}

const baseline = readJson(baselinePath, "The baseline is committed — restore it from git.");
const metrics = readJson(
  metricsPath,
  "Run `make realdata-honesty-check` (the harness produces the metrics before this check).",
);

const regressions = [];
const raises = [];
let stale = false;

for (const metric of METRICS) {
  const { key, bound, label, unit, tolerance, raiseBy } = metric;
  const now = metrics[key];
  const committed = baseline[key];
  if (typeof now !== "number" || typeof committed !== "number") {
    console.error(
      `realdata-ratchet: metric "${key}" is missing from ${typeof now !== "number" ? metricsPath : baselinePath}.`,
    );
    console.error("Harness and baseline must declare the same metric set — do not drop a metric to make the gate pass.");
    process.exit(2);
  }

  const regressed =
    bound === "floor" ? now + tolerance < committed : now - tolerance > committed;
  const improvement = bound === "floor" ? now - committed : committed - now;
  const status = regressed ? "FAIL" : "ok";
  console.log(
    `  ${status} ${key} ${now}${unit} (${bound} ${committed}${unit}) — ${label}`,
  );
  if (regressed) {
    regressions.push(`${key}: ${now}${unit} vs ${bound} ${committed}${unit}`);
  } else if (raiseBy !== null && improvement >= raiseBy) {
    stale = true;
    raises.push(`${key}: ${committed} -> ${now}`);
  }
}

if (raises.length > 0) {
  console.error(
    `\nHonesty improved — tighten the committed bound in ${baselinePath}:\n  ${raises.join("\n  ")}`,
  );
  console.error("An uncommitted improvement leaves the ratchet judging an old, looser app.");
}

if (regressions.length > 0) {
  console.error(
    `\n✖ Honesty regressed on the real database:\n  ${regressions.join("\n  ")}\n` +
      "Fix the regression. Do not loosen the baseline to make this pass (ADR 0038).",
  );
  process.exit(1);
}
if (stale) process.exit(2);

console.log("\n✓ Real-data honesty holds at or beyond every committed bound.");
