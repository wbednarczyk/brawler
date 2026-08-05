#!/usr/bin/env node
// Base-vs-head bench audit comparison (ADR 0096, card #336). Both runs come
// from `make audit-bench-ci` on ONE runner (.github/workflows/bench-audit.yml):
// a `--save-baseline audit-base` run of BASE_SHA, then a `--baseline-lenient`
// run of HEAD in the same criterion dir. This reads criterion's own
// change/estimates.json (relative regression, immune to cross-machine
// wall-clock drift) rather than comparing absolute nanoseconds. Advisory —
// see the Makefile audit-bench-ci block: a red run files a card, never blocks
// a merge on its own.

import { existsSync, readFileSync, appendFileSync, readdirSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { pathToFileURL } from "node:url";

// A kernel FAILs when criterion's change-median confidence-interval lower
// bound says it is at least this much slower, i.e. slower with confidence —
// not merely a noisy point estimate.
const FAIL_THRESHOLD = 0.3;

function readEstimateValue(filePath, pick) {
  let raw;
  try {
    raw = readFileSync(filePath, "utf8");
  } catch (err) {
    throw new Error(`bench-compare: cannot read ${filePath}: ${err.message} — criterion JSON layout changed?`);
  }
  let data;
  try {
    data = JSON.parse(raw);
  } catch (err) {
    throw new Error(`bench-compare: malformed JSON in ${filePath}: ${err.message} — criterion JSON layout changed?`);
  }
  const value = pick(data);
  if (!Number.isFinite(value)) {
    throw new Error(
      `bench-compare: expected a finite number in ${filePath}, got ${JSON.stringify(value)} — criterion JSON layout changed?`,
    );
  }
  return value;
}

const medianPointEstimate = (filePath) => readEstimateValue(filePath, (d) => d?.median?.point_estimate);
const changePointEstimate = (filePath) => readEstimateValue(filePath, (d) => d?.median?.point_estimate);
const changeLowerBound = (filePath) =>
  readEstimateValue(filePath, (d) => d?.median?.confidence_interval?.lower_bound);

/**
 * Recursively walk `root` for criterion kernel directories: any directory
 * containing `audit-base/estimates.json` or `new/estimates.json`. Criterion
 * nests group/function directories to arbitrary depth; `report/` directories
 * (HTML output, no estimates) are skipped. Returns a Map keyed by the
 * '/'-joined path relative to root, value `{ dir, hasBase, hasNew }`.
 */
export function collectKernels(root) {
  const kernels = new Map();

  function hasEstimates(dir, sub) {
    return existsSync(join(dir, sub, "estimates.json"));
  }

  function walk(dir) {
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    const hasBase = hasEstimates(dir, "audit-base");
    const hasNew = hasEstimates(dir, "new");
    if (hasBase || hasNew) {
      const id = relative(root, dir).split(sep).join("/");
      kernels.set(id, { dir, hasBase, hasNew });
    }
    for (const entry of entries) {
      if (!entry.isDirectory() || entry.name === "report") continue;
      walk(join(dir, entry.name));
    }
  }

  walk(root);
  return kernels;
}

/**
 * Build the comparison report for a criterion output root: per-kernel base
 * vs head medians and criterion's own relative-change estimate, plus kernels
 * only present on one side (informational, never fail). Throws when there is
 * nothing comparable — an audit that measured nothing must not be green.
 */
export function compareReport(root) {
  const kernels = [...collectKernels(root).entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));

  const comparable = [];
  const added = [];
  const removed = [];

  for (const [id, info] of kernels) {
    if (info.hasBase && info.hasNew) {
      const baseNs = medianPointEstimate(join(info.dir, "audit-base", "estimates.json"));
      const headNs = medianPointEstimate(join(info.dir, "new", "estimates.json"));
      const changeFile = join(info.dir, "change", "estimates.json");
      const changePoint = changePointEstimate(changeFile);
      const changeLower = changeLowerBound(changeFile);
      const verdict = changeLower > FAIL_THRESHOLD ? "FAIL" : "ok";
      comparable.push({ id, baseNs, headNs, changePoint, changeLower, verdict });
    } else if (info.hasNew) {
      added.push(id);
    } else if (info.hasBase) {
      removed.push(id);
    }
  }

  if (comparable.length === 0) {
    throw new Error(
      `bench-compare: zero comparable kernels under ${root} (present in both audit-base/ and new/) — the audit measured nothing.`,
    );
  }

  return { comparable, added, removed, overallFail: comparable.some((k) => k.verdict === "FAIL") };
}

function formatPercent(x) {
  return `${x >= 0 ? "+" : ""}${(x * 100).toFixed(1)}%`;
}

function buildTextReport(report, root) {
  const header = ["kernel", "base ns", "head ns", "Δ% (point)", "Δ% CI low", "verdict"];
  const rows = report.comparable.map((k) => [
    k.id,
    String(Math.round(k.baseNs)),
    String(Math.round(k.headNs)),
    formatPercent(k.changePoint),
    formatPercent(k.changeLower),
    k.verdict,
  ]);
  const widths = header.map((h, i) => Math.max(h.length, ...rows.map((r) => r[i].length)));
  const fmtRow = (cols) => cols.map((c, i) => c.padEnd(widths[i])).join("  ");

  const lines = [`Bench audit comparison (criterion dir: ${root})`, "", fmtRow(header)];
  lines.push(widths.map((w) => "-".repeat(w)).join("  "));
  for (const r of rows) lines.push(fmtRow(r));
  if (report.added.length > 0) {
    lines.push("", `Added kernels (no base measurement, informational): ${report.added.join(", ")}`);
  }
  if (report.removed.length > 0) {
    lines.push("", `Removed kernels (no head measurement, informational): ${report.removed.join(", ")}`);
  }
  return lines.join("\n");
}

function buildMarkdownSummary(report) {
  const lines = [
    "### Bench audit: base vs head",
    "",
    "| kernel | base ns | head ns | Δ% (point) | Δ% CI low | verdict |",
    "| --- | --- | --- | --- | --- | --- |",
  ];
  for (const k of report.comparable) {
    lines.push(
      `| ${k.id} | ${Math.round(k.baseNs)} | ${Math.round(k.headNs)} | ${formatPercent(k.changePoint)} | ${formatPercent(k.changeLower)} | ${k.verdict} |`,
    );
  }
  if (report.added.length > 0) lines.push("", `**Added:** ${report.added.join(", ")}`);
  if (report.removed.length > 0) lines.push("", `**Removed:** ${report.removed.join(", ")}`);
  return `${lines.join("\n")}\n`;
}

function main() {
  const root = process.env.CRITERION_DIR || "src-tauri/target/criterion";

  let report;
  try {
    report = compareReport(root);
  } catch (err) {
    console.error(err.message);
    process.exit(2);
  }

  console.log(buildTextReport(report, root));

  const summaryPath = process.env.GITHUB_STEP_SUMMARY;
  if (summaryPath) {
    try {
      appendFileSync(summaryPath, buildMarkdownSummary(report));
    } catch (err) {
      console.error(`bench-compare: failed to write GITHUB_STEP_SUMMARY: ${err.message}`);
    }
  }

  if (report.overallFail) {
    console.error(
      `\n✖ One or more kernels regressed ≥${Math.round(FAIL_THRESHOLD * 100)}% with confidence — this is an advisory audit: investigate or file a card, it never blocks a merge on its own.`,
    );
    process.exit(1);
  }
  console.log("\n✓ No kernel regressed beyond the advisory threshold.");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
