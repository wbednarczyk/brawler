#!/usr/bin/env node
// File-size ratchet (ADR 0103) — the architectural fitness function for the
// CLAUDE.md rule "very large source files are architecture debt". Production
// source files at or over the threshold must be pinned in
// file-size-baseline.json at their EXACT current line count; the gate fails
// when a pinned file grows (extract as part of the change, or hand-raise the
// pin — a reviewer-visible diff line) AND when it shrinks without re-running
// `--write` (so the baseline never carries silent regrow headroom). `--write`
// only moves pins down and removes entries that fell under the threshold;
// it refuses raises and additions by design.
//
// Scope: src-tauri/src/**/*.rs + src/**/*.{ts,tsx}, production code only —
// dedicated test files/dirs, generated bindings and locale resource tables are
// excluded; colocated #[cfg(test)] modules deliberately COUNT toward a file's
// total (extracting tests to a tests.rs is a legitimate ratchet move).

import { readFileSync, writeFileSync, readdirSync, existsSync } from "node:fs";
import path from "node:path";

const BASELINE_PATH = "file-size-baseline.json";
const DEFAULT_THRESHOLD = 1000;
const SCAN_ROOTS = ["src-tauri/src", "src"];
const EXTENSIONS = new Set([".rs", ".ts", ".tsx"]);
const EXCLUDED_PREFIXES = [
  "src/test/",
  "src/api/generated/",
  "src/shared/locale/resources/",
  "src-tauri/src/storage/tests/",
];

export function countLines(content) {
  return content.split("\n").length - 1;
}

export function isExcluded(rel) {
  if (!EXTENSIONS.has(path.extname(rel))) return true;
  if (!SCAN_ROOTS.some((root) => rel.startsWith(`${root}/`))) return true;
  if (EXCLUDED_PREFIXES.some((prefix) => rel.startsWith(prefix))) return true;
  if (path.basename(rel) === "tests.rs") return true;
  if (/\.(test|spec)\.(ts|tsx)$/.test(rel)) return true;
  return false;
}

/** Map of repo-relative path -> line count for every in-scope file, sorted keys. */
export function scanAll(repoRoot) {
  const counts = {};
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const abs = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(abs);
      } else if (entry.isFile()) {
        const rel = path.relative(repoRoot, abs).split(path.sep).join("/");
        if (!isExcluded(rel)) counts[rel] = countLines(readFileSync(abs, "utf8"));
      }
    }
  };
  for (const root of SCAN_ROOTS) {
    const abs = path.join(repoRoot, root);
    if (existsSync(abs)) walk(abs);
  }
  return sortObject(counts);
}

/** Check mode: list of failure messages (empty = gate holds). */
export function evaluate(baseline, scan, threshold) {
  const failures = [];
  const pinned = baseline.files ?? {};
  for (const [rel, lines] of Object.entries(scan)) {
    if (lines >= threshold && !(rel in pinned)) {
      failures.push(
        `${rel} has ${lines} lines (threshold ${threshold}) and is not baselined — split it below the threshold as part of this change (a hand-added baseline entry is the deliberate, reviewed escape hatch).`,
      );
    }
  }
  for (const [rel, pin] of Object.entries(pinned)) {
    const now = scan[rel];
    if (now === undefined) {
      failures.push(`${rel} is baselined but no longer exists — ratchet down: node scripts/check/file-size-ratchet.mjs --write`);
    } else if (now > pin) {
      failures.push(
        `${rel} grew ${pin} -> ${now} — extract as part of this change, or consciously raise its pin in ${BASELINE_PATH} (that diff line is the review gate).`,
      );
    } else if (now < pin) {
      failures.push(`${rel} shrank ${pin} -> ${now} — ratchet down: node scripts/check/file-size-ratchet.mjs --write, commit the result.`);
    }
  }
  return failures;
}

/** --write mode: pins only move down; raises and additions are refused. */
export function ratchetDown(baseline, scan, threshold) {
  const refusals = [];
  const files = {};
  for (const [rel, pin] of Object.entries(baseline.files ?? {})) {
    const now = scan[rel];
    if (now === undefined || now < threshold) continue; // extracted below threshold or deleted: entry drops
    if (now > pin) {
      refusals.push(`--write refuses to raise ${rel} ${pin} -> ${now} — growth is a hand edit to ${BASELINE_PATH}, never automated.`);
      files[rel] = pin;
    } else {
      files[rel] = now;
    }
  }
  for (const [rel, lines] of Object.entries(scan)) {
    if (lines >= threshold && !(rel in (baseline.files ?? {}))) {
      refusals.push(`--write refuses to add ${rel} (${lines} lines) — split it below the threshold, or hand-add the entry as a reviewed decision.`);
    }
  }
  return { baseline: { ...baseline, files: sortObject(files) }, refusals };
}

/** First-run bootstrap: pin the as-is offenders so the gate starts green. */
export function bootstrap(scan, threshold) {
  const files = Object.fromEntries(Object.entries(scan).filter(([, lines]) => lines >= threshold));
  return {
    _comment:
      "File-size ratchet (ADR 0103). Production source files at/over thresholdLines are pinned at their exact line count; `make check` fails when a pinned file grows or shrinks without `node scripts/check/file-size-ratchet.mjs --write`. --write only ratchets down; raising a pin (or adding one) is a deliberate hand edit reviewed in the PR diff.",
    thresholdLines: threshold,
    files: sortObject(files),
  };
}

function sortObject(obj) {
  return Object.fromEntries(Object.entries(obj).sort(([a], [b]) => (a < b ? -1 : 1)));
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === new URL(import.meta.url).pathname;
if (isMain) {
  const write = process.argv.includes("--write");
  const scan = scanAll(process.cwd());

  if (!existsSync(BASELINE_PATH)) {
    if (!write) {
      console.error(`file-size-ratchet: ${BASELINE_PATH} missing — bootstrap it: node scripts/check/file-size-ratchet.mjs --write`);
      process.exit(2);
    }
    const fresh = bootstrap(scan, DEFAULT_THRESHOLD);
    writeFileSync(BASELINE_PATH, `${JSON.stringify(fresh, null, 2)}\n`);
    console.log(`file-size-ratchet: bootstrapped ${BASELINE_PATH} with ${Object.keys(fresh.files).length} pinned files (threshold ${DEFAULT_THRESHOLD}).`);
    process.exit(0);
  }

  const baseline = JSON.parse(readFileSync(BASELINE_PATH, "utf8"));
  const threshold = baseline.thresholdLines ?? DEFAULT_THRESHOLD;

  if (write) {
    const { baseline: next, refusals } = ratchetDown(baseline, scan, threshold);
    if (refusals.length > 0) {
      for (const r of refusals) console.error(`  ✖ ${r}`);
      process.exit(1);
    }
    writeFileSync(BASELINE_PATH, `${JSON.stringify(next, null, 2)}\n`);
    const dropped = Object.keys(baseline.files ?? {}).length - Object.keys(next.files).length;
    console.log(`file-size-ratchet: baseline ratcheted down (${Object.keys(next.files).length} pinned, ${dropped} dropped).`);
    process.exit(0);
  }

  const failures = evaluate(baseline, scan, threshold);
  if (failures.length > 0) {
    for (const f of failures) console.error(`  FAIL ${f}`);
    console.error(`\n✖ file-size ratchet: ${failures.length} violation(s) — oversized-file debt only moves down (ADR 0103).`);
    process.exit(1);
  }
  console.log(`✓ file-size ratchet holds (${Object.keys(baseline.files ?? {}).length} pinned files, threshold ${threshold}).`);
}
