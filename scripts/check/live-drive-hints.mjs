#!/usr/bin/env node
// Advisory live-drive hint (card #337, epic #335, ADR 0096 principle 5): prints
// when a scoped live-drive of the owner's real app is worth it before merging a
// PR. A dumb path -> hint list; classification only ever decides what is SAID,
// never what is CHECKED (no gate anywhere depends on this file's output).
// Always exits 0 on the classification path — this is advisory, never a red
// check. Wired into CI as the `live-drive-hint` job in
// .github/workflows/full-check.yml, and runnable locally via
// `make live-drive-hints BASE=<sha> HEAD=<sha> [PR=<n>]`.

import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

// Ordered rules — FIRST match per file wins. Order matters: e.g. a migrations
// file also lives under src-tauri/src/storage/, but the migration hint is more
// specific and must win over the generic storage hint.
const RULES = [
  {
    test: (f) =>
      f.startsWith("src-tauri/migrations/") ||
      f === "src-tauri/src/storage/migrations.rs",
    hint: "Migrations changed — a PR binary applies them to the live database on first launch; drive against the real DB deliberately (backup first).",
  },
  {
    test: (f) =>
      f.startsWith("src-tauri/src/commands/") || f.startsWith("src-tauri/src/mcp"),
    hint: "IPC/MCP surface changed — drive the affected command flow on the real app.",
  },
  {
    test: (f) => f.startsWith("src-tauri/src/jobs/"),
    hint: "Background jobs changed — watch a real sweep/autopilot cycle live.",
  },
  {
    test: (f) => f.startsWith("src-tauri/src/source_adapters/"),
    hint: "Source adapters changed — verify against the real sources live.",
  },
  {
    test: (f) => f.startsWith("src-tauri/src/storage/"),
    hint: "Storage/read models changed — check the affected panels over the real DB.",
  },
  {
    test: (f) =>
      f.startsWith("src/") &&
      /\.(ts|tsx|css)$/.test(f) &&
      !f.startsWith("src/api/generated/") &&
      !f.startsWith("src/test/"),
    hint: "UI changed — a scoped live-drive (make pr-live-cycle PR=n LIVE_SPEC=<spec>) of the affected panel is worth it.",
  },
];

// Pure classifier: files in -> ordered, deduplicated hint strings out. No I/O,
// no process access — the unit tests exercise this directly.
//
// "First match per file wins" means per FILE, not per rule: a migration file
// also lives under src-tauri/src/storage/, so each file is assigned to the
// first rule (in RULES order) whose test matches it, and only that rule's
// hint counts for that file. Output order is RULES order among the rules with
// at least one assigned file (dedup is a side effect of that grouping).
export function classify(files) {
  const matchedRuleIndices = new Set();
  for (const file of files) {
    const idx = RULES.findIndex((rule) => rule.test(file));
    if (idx !== -1) matchedRuleIndices.add(idx);
  }
  return RULES.filter((_, idx) => matchedRuleIndices.has(idx)).map((rule) => rule.hint);
}

function changedFiles(base, head) {
  const result = spawnSync("git", ["diff", "--name-only", "-z", `${base}...${head}`], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    const reason = (result.stderr || `git diff exited ${result.status}`).trim();
    throw new Error(reason);
  }
  return result.stdout.split("\0").filter(Boolean);
}

function writeSummary(lines) {
  const summaryFile = process.env.GITHUB_STEP_SUMMARY;
  if (!summaryFile) return;
  appendFileSync(summaryFile, `${lines.join("\n")}\n`);
}

function warnUnavailable(reason) {
  const message = `::warning::live-drive hints unavailable: ${reason}`;
  console.log(message);
  writeSummary([`**live-drive hints unavailable:** ${reason}`]);
}

function render(hints, pr) {
  const lines = [];
  if (hints.length === 0) {
    lines.push("No runtime-behavior paths touched — no live-drive hint.");
  } else {
    lines.push("This PR touches paths worth a live-drive:");
    for (const hint of hints) lines.push(`- ${hint}`);
    const prArg = pr ? pr : "<n>";
    lines.push(
      `Suggested: make pr-live-cycle PR=${prArg}  (downloads this PR's cross-built .exe and drives the live suite — no WSL rebuild)`,
    );
  }
  return lines;
}

function main() {
  const base = process.env.BASE;
  const head = process.env.HEAD;
  const pr = process.env.PR;

  if (!base || !head) {
    warnUnavailable("BASE and/or HEAD env vars not set");
    return;
  }

  let files;
  try {
    files = changedFiles(base, head);
  } catch (err) {
    warnUnavailable(err.message);
    return;
  }

  const hints = classify(files);
  const lines = render(hints, pr);
  console.log(lines.join("\n"));
  writeSummary(lines);
}

// CLI entry point only — importers (the test file) get `classify` without
// running `main`. A crash here (bad code, not bad input) is deliberately NOT
// caught: this guard is the only thing standing between "advisory" and "silent
// no-op forever" if the script itself breaks.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
  process.exit(0);
}
