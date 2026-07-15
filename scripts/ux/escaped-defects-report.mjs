#!/usr/bin/env node
// Escaped-defect taxonomy trend report (ADR 0081, plan Q7).
//
// Scans docs/retros/*.md for an explicitly marked escaped-defect table
// (`<!-- escaped-defects:start -->` … `<!-- escaped-defects:end -->`,
// shape documented in docs/retros/TEMPLATE.md), validates it against the
// canonical origin-class / detection-stage / disposition enums, and prints
// counts by origin/stage plus repeated classes (count >= 2).
//
// Advisory only: this NEVER fails because counts increased — counts inform
// guardrail harvest, they are not a performance target. It fails only on a
// malformed opted-in row (unknown enum value, missing cell, duplicate ref).
// A retro with no marked table is silently skipped, never flagged — most
// historical retros predate this taxonomy and stay valid as-is.
//
// Stays off `make check` during the Q7 pilot (ADR 0081); Q9 decides whether
// malformed rows join a deterministic docs check.

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const MARKER_START = "<!-- escaped-defects:start -->";
const MARKER_END = "<!-- escaped-defects:end -->";

export const ORIGIN_CLASSES = new Set([
  "spec-gap",
  "ux-decision",
  "missing-state",
  "mock-realism",
  "integration-seam",
  "responsive-layout",
  "visual-hierarchy",
  "async-race",
  "native-runtime",
  "real-data-shape",
  "test-flake",
]);

export const DETECTION_STAGES = new Set([
  "implementation",
  "targeted-test",
  "full-gate",
  "vertical-slice",
  "mid-milestone",
  "release-dogfood",
  "post-release/user",
  "unknown/historical-evidence-insufficient",
]);

const FIXED_DISPOSITIONS = new Set([
  "automated-guardrail",
  "human-checklist",
  "fixed-instance-only",
  "accepted-limitation",
]);
const TRACKED_DISPOSITION = /^tracked:[0-9a-f]{7}$/;

export const STATUSES = new Set(["open", "closed"]);

export function isValidDisposition(value) {
  return FIXED_DISPOSITIONS.has(value) || TRACKED_DISPOSITION.test(value);
}

const DEFAULT_DIR = join(fileURLToPath(new URL("../..", import.meta.url)), "docs", "retros");

/** Slice the content between the escaped-defects markers, or null if absent. */
export function extractMarkedTable(content) {
  const startIdx = content.indexOf(MARKER_START);
  const endIdx = content.indexOf(MARKER_END);
  if (startIdx === -1 || endIdx === -1 || endIdx < startIdx) return null;
  return content.slice(startIdx + MARKER_START.length, endIdx);
}

function splitRow(line) {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());
}

function isSeparatorRow(cells) {
  return cells.length > 0 && cells.every((cell) => /^:?-{3,}:?$/.test(cell));
}

/** Parse markdown table data rows (header + separator dropped) into raw cell arrays. */
export function parseTableRows(tableBlock) {
  const lines = tableBlock
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.startsWith("|"));
  if (lines.length === 0) return [];
  const rows = lines.map(splitRow);
  const dataRows = [];
  let sawSeparator = false;
  rows.forEach((cells, i) => {
    if (i === 0) return; // header row
    if (!sawSeparator && isSeparatorRow(cells)) {
      sawSeparator = true;
      return;
    }
    dataRows.push(cells);
  });
  return dataRows;
}

/**
 * Parse one retro file's content. Returns `{ file, hasTable, defects, errors }`.
 * A file with no marker pair yields `hasTable: false` and is treated as ignored,
 * never an error.
 */
export function parseFileContent(fileLabel, content) {
  const table = extractMarkedTable(content);
  if (table === null) {
    return { file: fileLabel, hasTable: false, defects: [], errors: [] };
  }

  const rows = parseTableRows(table);
  const defects = [];
  const errors = [];

  rows.forEach((cells, i) => {
    const rowNo = i + 1;
    if (cells.length !== 6 || cells.some((cell) => cell.length === 0)) {
      errors.push(
        `${fileLabel}: escaped-defect row ${rowNo} malformed (expected 6 non-empty cells ` +
          `"Ref | Origin class | Detected at | Earliest prevention point | Disposition | Status", got [${cells.join(" | ")}])`,
      );
      return;
    }

    const [ref, originClass, detectedAt, earliestPrevention, disposition, status] = cells;
    const rowErrors = [];
    if (!ORIGIN_CLASSES.has(originClass)) rowErrors.push(`unknown origin class "${originClass}"`);
    if (!DETECTION_STAGES.has(detectedAt)) rowErrors.push(`unknown detection stage "${detectedAt}"`);
    if (!isValidDisposition(disposition)) rowErrors.push(`unknown disposition "${disposition}"`);
    if (!STATUSES.has(status)) rowErrors.push(`unknown status "${status}"`);

    if (rowErrors.length > 0) {
      errors.push(`${fileLabel}: row ${rowNo} (${ref}) ${rowErrors.join("; ")}`);
      return;
    }

    defects.push({ file: fileLabel, ref, originClass, detectedAt, earliestPrevention, disposition, status });
  });

  return { file: fileLabel, hasTable: true, defects, errors };
}

export function listMarkdownFiles(dir) {
  return readdirSync(dir)
    .filter((name) => name.endsWith(".md"))
    .sort()
    .map((name) => join(dir, name));
}

function parseFile(path) {
  const content = readFileSync(path, "utf8");
  return parseFileContent(path, content);
}

/** Merge per-file results, flag duplicate refs across the whole scanned set. */
export function aggregate(fileResults) {
  const defects = [];
  const errors = [];
  const seenRefs = new Map();

  for (const result of fileResults) {
    errors.push(...result.errors);
    for (const defect of result.defects) {
      const firstSeenIn = seenRefs.get(defect.ref);
      if (firstSeenIn) {
        errors.push(`duplicate escaped-defect ref "${defect.ref}" in ${defect.file} (first seen in ${firstSeenIn})`);
        continue;
      }
      seenRefs.set(defect.ref, defect.file);
      defects.push(defect);
    }
  }

  return { defects, errors };
}

export function summarize(defects, fileResults) {
  const byOrigin = new Map();
  const byStage = new Map();
  for (const defect of defects) {
    byOrigin.set(defect.originClass, (byOrigin.get(defect.originClass) ?? 0) + 1);
    byStage.set(defect.detectedAt, (byStage.get(defect.detectedAt) ?? 0) + 1);
  }
  return {
    filesScanned: fileResults.length,
    filesWithTable: fileResults.filter((f) => f.hasTable).length,
    total: defects.length,
    byOrigin,
    byStage,
  };
}

export function formatReport(summary, errors) {
  if (errors.length > 0) {
    return [
      "Escaped-defect report (advisory, ADR 0081 Q7) — validation FAILED",
      ...errors.map((e) => `  ✖ ${e}`),
    ].join("\n");
  }

  const lines = [];
  lines.push("Escaped-defect report (advisory, ADR 0081 Q7)");
  lines.push(`Scanned ${summary.filesScanned} retro file(s), ${summary.filesWithTable} with a marked table.`);
  lines.push(`Defects recorded: ${summary.total}`);

  if (summary.total === 0) {
    lines.push("");
    lines.push("No marked escaped-defect rows yet.");
    return lines.join("\n");
  }

  lines.push("");
  lines.push("By origin class:");
  for (const [k, v] of summary.byOrigin) lines.push(`  ${k}: ${v}`);

  lines.push("");
  lines.push("By detection stage:");
  for (const [k, v] of summary.byStage) lines.push(`  ${k}: ${v}`);

  const repeated = [...summary.byOrigin].filter(([, count]) => count >= 2);
  lines.push("");
  if (repeated.length > 0) {
    lines.push("Repeated origin classes (count >= 2) — guardrail-harvest candidates:");
    for (const [k, v] of repeated) lines.push(`  ${k} (${v})`);
  } else {
    lines.push("No origin class repeated yet (count >= 2).");
  }

  lines.push("");
  lines.push(
    "Advisory only: counts inform guardrail harvest, never a target or gate. Not every " +
      "repeated class needs a new automated test — a human-checklist disposition is valid.",
  );
  return lines.join("\n");
}

export function run(dir = DEFAULT_DIR) {
  const files = listMarkdownFiles(dir);
  const fileResults = files.map(parseFile);
  const { defects, errors } = aggregate(fileResults);
  const summary = summarize(defects, fileResults);
  const output = formatReport(summary, errors);
  return { exitCode: errors.length > 0 ? 1 : 0, output, errors, defects, summary };
}

const isMainModule = process.argv[1] && import.meta.url === `file://${process.argv[1]}`;
if (isMainModule) {
  const { exitCode, output } = run();
  console.log(output);
  process.exit(exitCode);
}
