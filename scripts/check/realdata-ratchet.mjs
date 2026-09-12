#!/usr/bin/env node
// Real-data ratchet (epic #40 S4; ADR 0091 decisions 4-5; #331 PR-A, ADR 0112
// adds the `esef` profile). Sibling of `coverage-ratchet.mjs`, same mechanics:
// a committed baseline, a tolerance for measurement noise, and raises that are
// PRINTED, never written — the owner commits an improvement deliberately.
//
// `--profile honesty` (default) judges the aggregate metrics emitted by the
// `#[ignore]` harness `src-tauri/src/storage/tests/real_data_honesty.rs` on
// the maintainer's real database. **The real database never enters the repo
// or CI** (ADR 0091 dec. 4): the only committed artifact is
// `realdata-honesty-baseline.json` — counts and percentages, never a title,
// ticker, or id. Never add a metric that carries row content.
//
// `--profile esef` judges the aggregate metrics emitted by
// `storage::tests::real_data_esef_v2` (ADR 0112) against
// `realdata-esef-baseline.json`: equality fields that make two runs
// comparable at all, a `matched` floor, and `previously_correct_slots_lost`/
// `false_positives`/`zero_output_events` ceilings — every other numeric field
// is informational (finite, non-negative, not ratcheted). Both profiles keep
// separate baseline/metrics files and separate default paths; an unknown
// profile is a hard failure, never a silent default.
//
// Inputs (both overridable: `--baseline <path>` / `--metrics <path>`, used by
// the self-test `scripts/check/check-realdata-ratchet.sh`):
//   honesty: realdata-honesty-baseline.json / src-tauri/target/realdata-honesty-metrics.json
//   esef:    realdata-esef-baseline.json    / src-tauri/target/realdata-esef-metrics.json
//
// Exit codes:
//   0  every metric holds at or beyond its committed bound
//   1  REGRESSION — a metric moved the wrong way beyond tolerance
//   2  the check could not conclude: inputs unreadable/malformed, an unknown
//      profile, an esef baseline/metrics not yet `"status": "measured"`, an
//      esef equality-field mismatch ("incomparable — rebaseline required"),
//      OR the baseline is stale because the metric improved and was never
//      committed (a silent raise makes the ratchet toothless — commit the
//      new bound / promote deliberately)

import { readFileSync } from "node:fs";

// Each honesty metric declares which direction is BETTER, how much
// measurement noise to forgive, and how big an improvement must be before the
// baseline counts as stale. `raiseBy: null` = a hard bound that can never
// move (filename-as-statement is zero, forever — the harness asserts it too).
const HONESTY_METRICS = [
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

// ESEF measurement v2 (ADR 0112 decisions 6/9). Equality fields make two runs
// comparable at all — any mismatch means the corpus/versions moved and the
// baseline no longer describes the same population, never a pass/fail on
// their own. `matched` is the one floor; the rest are ceilings.
const ESEF_EQUALITY_FIELDS = [
  "profile",
  "measurement_version",
  "gt_version",
  "key_map_version",
  "normalization_version",
  "registry_hash",
  "events",
  "floor_events",
  "issuers",
  "gt_slots",
  "unverified",
];

const ESEF_BOUNDS = [
  {
    key: "matched",
    bound: "floor",
    label: "ground-truth slots matched",
    unit: " slots",
    tolerance: 0,
    raiseBy: 1,
  },
  {
    key: "previously_correct_slots_lost",
    bound: "ceiling",
    label:
      "previously-matched slots that regressed (individual-match zero loss)",
    unit: " slots",
    tolerance: 0,
    raiseBy: null,
  },
  {
    key: "false_positives",
    bound: "ceiling",
    label: "predictions outside any GT slot",
    unit: "",
    tolerance: 0,
    raiseBy: 1,
  },
  {
    key: "zero_output_events",
    bound: "ceiling",
    label: "events producing no extraction output",
    unit: " events",
    tolerance: 0,
    raiseBy: 1,
  },
];

function argValue(flag, fallback) {
  const index = process.argv.indexOf(flag);
  return index !== -1 && process.argv[index + 1]
    ? process.argv[index + 1]
    : fallback;
}

const profile = argValue("--profile", "honesty");
if (profile !== "honesty" && profile !== "esef") {
  console.error(
    `realdata-ratchet: unknown profile "${profile}" (expected "honesty" or "esef").`,
  );
  process.exit(2);
}

const defaultBaseline =
  profile === "esef"
    ? "realdata-esef-baseline.json"
    : "realdata-honesty-baseline.json";
const defaultMetrics =
  profile === "esef"
    ? "src-tauri/target/realdata-esef-metrics.json"
    : "src-tauri/target/realdata-honesty-metrics.json";

const baselinePath = argValue("--baseline", defaultBaseline);
const metricsPath = argValue("--metrics", defaultMetrics);

function readJson(path, hint) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    console.error(`realdata-ratchet: cannot read ${path}: ${err.message}`);
    console.error(hint);
    process.exit(2);
  }
}

const baseline = readJson(
  baselinePath,
  "The baseline is committed — restore it from git.",
);
const metrics = readJson(
  metricsPath,
  profile === "esef"
    ? "Run `make realdata-esef-check` (the harness produces the metrics before this check)."
    : "Run `make realdata-honesty-check` (the harness produces the metrics before this check).",
);

function runHonestyProfile() {
  const regressions = [];
  const raises = [];
  let stale = false;

  for (const metric of HONESTY_METRICS) {
    const { key, bound, label, unit, tolerance, raiseBy } = metric;
    const now = metrics[key];
    const committed = baseline[key];
    if (typeof now !== "number" || typeof committed !== "number") {
      console.error(
        `realdata-ratchet: metric "${key}" is missing from ${typeof now !== "number" ? metricsPath : baselinePath}.`,
      );
      console.error(
        "Harness and baseline must declare the same metric set — do not drop a metric to make the gate pass.",
      );
      process.exit(2);
    }

    const regressed =
      bound === "floor"
        ? now + tolerance < committed
        : now - tolerance > committed;
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
    console.error(
      "An uncommitted improvement leaves the ratchet judging an old, looser app.",
    );
  }

  if (regressions.length > 0) {
    console.error(
      `\n✖ Honesty regressed on the real database:\n  ${regressions.join("\n  ")}\n` +
        "Fix the regression. Do not loosen the baseline to make this pass (ADR 0038).",
    );
    process.exit(1);
  }
  if (stale) process.exit(2);

  console.log(
    "\n✓ Real-data honesty holds at or beyond every committed bound.",
  );
}

function isFiniteNonNegative(value) {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

// Every informational (non-equality, non-bound) numeric leaf, however deeply
// nested (e.g. `availability_all_periods.available`), must be finite and
// non-negative — it is never ratcheted, but a NaN/negative/Infinity there
// means the harness is broken, not that the metric is merely uninteresting.
function collectInformationalErrors(value, path, errors) {
  if (value === null || value === undefined) return;
  if (typeof value === "number") {
    if (!isFiniteNonNegative(value))
      errors.push(
        `${path}: ${JSON.stringify(value)} is not a finite, non-negative number`,
      );
    return;
  }
  if (typeof value === "object" && !Array.isArray(value)) {
    for (const [key, nested] of Object.entries(value))
      collectInformationalErrors(nested, `${path}.${key}`, errors);
  }
}

function runEsefProfile() {
  for (const [label, doc, path] of [
    ["baseline", baseline, baselinePath],
    ["metrics", metrics, metricsPath],
  ]) {
    if (doc.status !== "measured") {
      console.error(
        `realdata-ratchet: ${label} (${path}) has status ${JSON.stringify(doc.status)}, not "measured".`,
      );
      console.error(
        label === "baseline"
          ? "No promoted baseline yet — run the harness, review the report, then `make realdata-esef-promote RUN=<nonce>`."
          : "Run `make realdata-esef-check` (the harness produces measured metrics) before judging this run.",
      );
      process.exit(2);
    }
  }

  for (const field of ESEF_EQUALITY_FIELDS) {
    const committed = baseline[field];
    const now = metrics[field];
    if (committed === undefined || now === undefined) {
      console.error(
        `realdata-ratchet: equality field "${field}" missing from ${committed === undefined ? baselinePath : metricsPath}.`,
      );
      process.exit(2);
    }
    if (committed !== now) {
      console.error(
        `realdata-ratchet: incomparable — rebaseline required. "${field}" differs: baseline ${JSON.stringify(committed)} vs run ${JSON.stringify(now)}.`,
      );
      process.exit(2);
    }
  }

  const regressions = [];
  const raises = [];
  let stale = false;

  for (const bound of ESEF_BOUNDS) {
    const { key, label, unit, tolerance, raiseBy } = bound;
    const now = metrics[key];
    const committed = baseline[key];
    if (!isFiniteNonNegative(now) || !isFiniteNonNegative(committed)) {
      console.error(
        `realdata-ratchet: metric "${key}" must be a finite, non-negative number in both ${baselinePath} and ${metricsPath}.`,
      );
      process.exit(2);
    }

    const regressed =
      bound.bound === "floor"
        ? now + tolerance < committed
        : now - tolerance > committed;
    const improvement =
      bound.bound === "floor" ? now - committed : committed - now;
    const status = regressed ? "FAIL" : "ok";
    console.log(
      `  ${status} ${key} ${now}${unit} (${bound.bound} ${committed}${unit}) — ${label}`,
    );
    if (regressed) {
      regressions.push(
        `${key}: ${now}${unit} vs ${bound.bound} ${committed}${unit}`,
      );
    } else if (raiseBy !== null && improvement >= raiseBy) {
      stale = true;
      raises.push(`${key}: ${committed} -> ${now}`);
    }
  }

  const boundKeys = new Set(ESEF_BOUNDS.map((b) => b.key));
  const infoErrors = [];
  for (const [key, value] of Object.entries(metrics)) {
    if (
      ESEF_EQUALITY_FIELDS.includes(key) ||
      boundKeys.has(key) ||
      key === "status"
    )
      continue;
    collectInformationalErrors(value, key, infoErrors);
  }
  if (infoErrors.length > 0) {
    console.error(
      `realdata-ratchet: non-finite or negative informational metric(s):\n  ${infoErrors.join("\n  ")}`,
    );
    process.exit(2);
  }

  if (raises.length > 0) {
    console.error(
      `\nESEF measurement improved — tighten the committed bound in ${baselinePath}:\n  ${raises.join("\n  ")}`,
    );
    console.error(
      "An uncommitted improvement leaves the ratchet judging a looser app — promote deliberately: `make realdata-esef-promote RUN=<nonce>`.",
    );
  }

  if (regressions.length > 0) {
    console.error(
      `\n✖ ESEF measurement regressed:\n  ${regressions.join("\n  ")}\n` +
        "Fix the regression. Do not loosen the baseline to make this pass (ADR 0038).",
    );
    process.exit(1);
  }
  if (stale) process.exit(2);

  console.log("\n✓ ESEF measurement holds at or beyond every committed bound.");
}

if (profile === "esef") {
  runEsefProfile();
} else {
  runHonestyProfile();
}
