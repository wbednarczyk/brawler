#!/usr/bin/env node
// Tests-touched gate (G14, hard gates wave 2, ADR 0045 harvest 2026-09-08).
// An ACKNOWLEDGEMENT gate, not proof of behavioral coverage: a PR that
// changes production code must carry a changed test file, or (Rust) an
// inline #[cfg(test)]/#[test] hunk in the same diff, or the `tests:not-needed`
// label — a pure refactor legitimately keeps its existing tests untouched.
//
// Usage: node scripts/check/tests-touched.mjs --base <sha> --head <sha>
// Labels come from the PR_LABELS env var as a JSON array (never a comma list —
// a label whose own name contains a comma, e.g. "review,tests:not-needed",
// must not fragment into a false match), never a CLI arg — the caller
// (Makefile target / workflow) passes them straight through via `env:`, never
// interpolated into a shell command line.

import { execFileSync } from "node:child_process";
import path from "node:path";

function usage(message) {
  if (message) console.error(`tests-touched: ${message}`);
  console.error("Usage: node scripts/check/tests-touched.mjs --base <sha> --head <sha>  (PR_LABELS env, comma-separated)");
  process.exit(64);
}

function parseArgs(argv) {
  let base = null;
  let head = null;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--base") base = argv[++i];
    else if (argv[i] === "--head") head = argv[++i];
  }
  return { base, head };
}

function git(args) {
  return execFileSync("git", args, { encoding: "utf8", maxBuffer: 1024 * 1024 * 64 });
}

// --- path classification -----------------------------------------------------

function isFrontendCodeFile(p) {
  if (!p.startsWith("src/")) return false;
  if (!(p.endsWith(".ts") || p.endsWith(".tsx"))) return false;
  if (/\.test\./.test(p)) return false;
  if (p.startsWith("src/test/")) return false;
  if (p.endsWith(".d.ts")) return false;
  if (p.startsWith("src/api/generated/")) return false;
  if (p.startsWith("src/shared/locale/resources/")) return false;
  if (p === "src/main.tsx" || p === "src/gallery.tsx") return false;
  return true;
}

function segments(p) {
  return p.split("/");
}

function isRustCodeFile(p) {
  if (!p.startsWith("src-tauri/src/")) return false;
  if (!p.endsWith(".rs")) return false;
  const segs = segments(p);
  if (segs.includes("tests")) return false; // **/tests/**
  const base = segs[segs.length - 1];
  if (base === "tests.rs") return false;
  if (base.endsWith("_tests.rs")) return false;
  if (p.startsWith("src-tauri/src/bin/")) return false;
  return true;
}

function isCodeFile(p) {
  return isFrontendCodeFile(p) || isRustCodeFile(p);
}

function isTestEvidenceFile(p) {
  if (p.startsWith("src/") && /\.test\./.test(p)) return true;
  if (p.startsWith("src/test/")) return true;
  if (p.startsWith("tests/")) return true;
  if (p.startsWith("src-tauri/tests/")) return true;
  if (p.startsWith("src-tauri/src/") && segments(p).includes("tests")) return true;
  const base = segments(p)[segments(p).length - 1];
  if (base === "tests.rs") return true;
  if (base.endsWith("_tests.rs")) return true;
  if (p.endsWith(".snap")) return true;
  if (segments(p).includes("snapshots")) return true;
  return false;
}

// --- diff parsing --------------------------------------------------------------

/** Parse `git diff --name-status -M` output into {status, path} or
 * {status:"R", score, oldPath, newPath} entries. */
export function parseNameStatus(output) {
  const entries = [];
  for (const line of output.split("\n")) {
    if (!line.trim()) continue;
    const fields = line.split("\t");
    const statusField = fields[0];
    if (statusField[0] === "R" || statusField[0] === "C") {
      entries.push({
        status: statusField[0],
        score: parseInt(statusField.slice(1), 10),
        oldPath: fields[1],
        newPath: fields[2],
      });
    } else {
      entries.push({ status: statusField[0], path: fields[1] });
    }
  }
  return entries;
}

/** Parse a `git diff -U0 -M` unified diff into a map of newPath -> {oldPath, hunks}. */
export function parseHunksByNewPath(diffText) {
  const map = new Map();
  let currentOld = null;
  let currentNew = null;
  for (const line of diffText.split("\n")) {
    const gitHeader = line.match(/^diff --git a\/(.+) b\/(.+)$/);
    if (gitHeader) {
      currentOld = gitHeader[1];
      currentNew = gitHeader[2];
      continue;
    }
    const hunkMatch = line.match(/^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/);
    if (hunkMatch && currentNew) {
      const oldStart = parseInt(hunkMatch[1], 10);
      const oldLines = hunkMatch[2] !== undefined ? parseInt(hunkMatch[2], 10) : 1;
      const newStart = parseInt(hunkMatch[3], 10);
      const newLines = hunkMatch[4] !== undefined ? parseInt(hunkMatch[4], 10) : 1;
      if (!map.has(currentNew)) map.set(currentNew, { oldPath: currentOld, hunks: [] });
      map.get(currentNew).hunks.push({ oldStart, oldLines, newStart, newLines });
    }
  }
  return map;
}

// --- inline #[cfg(test)] / #[test] span detection -----------------------------

// Replace string/char literals and comments with spaces (newlines preserved)
// so brace-matching and keyword search never trip on braces or the word
// "test" inside a string or comment. ponytail: block comments are treated as
// non-nested (Rust technically allows nesting) and raw strings support only
// the common 0-3 `#` forms — upgrade if a real file trips either ceiling.
export function maskStringsAndComments(src) {
  const out = src.split("");
  const n = out.length;
  let i = 0;
  while (i < n) {
    const c = src[i];
    if (c === "/" && src[i + 1] === "/") {
      while (i < n && src[i] !== "\n") {
        out[i] = " ";
        i++;
      }
      continue;
    }
    if (c === "/" && src[i + 1] === "*") {
      out[i] = " ";
      out[i + 1] = " ";
      i += 2;
      while (i < n && !(src[i] === "*" && src[i + 1] === "/")) {
        if (src[i] !== "\n") out[i] = " ";
        i++;
      }
      if (i < n) {
        out[i] = " ";
        out[i + 1] = " ";
        i += 2;
      }
      continue;
    }
    if (c === "r" && (src[i + 1] === '"' || src[i + 1] === "#")) {
      let j = i + 1;
      let hashes = 0;
      while (src[j] === "#" && hashes < 3) {
        hashes++;
        j++;
      }
      if (src[j] === '"') {
        const closer = `"${"#".repeat(hashes)}`;
        for (let k = i; k <= j; k++) if (src[k] !== "\n") out[k] = " ";
        const end = src.indexOf(closer, j + 1);
        const stop = end === -1 ? n : end + closer.length;
        for (let k = j + 1; k < stop; k++) if (src[k] !== "\n") out[k] = " ";
        i = stop;
        continue;
      }
    }
    if (c === '"') {
      out[i] = " ";
      i++;
      while (i < n && src[i] !== '"') {
        if (src[i] === "\\") {
          if (src[i] !== "\n") out[i] = " ";
          i++;
          if (i < n && src[i] !== "\n") out[i] = " ";
          i++;
          continue;
        }
        if (src[i] !== "\n") out[i] = " ";
        i++;
      }
      if (i < n) {
        out[i] = " ";
        i++;
      }
      continue;
    }
    if (c === "'") {
      // char literal 'x' / '\n' vs a lifetime 'a — only mask the char-literal shape.
      let j = i + 1;
      if (src[j] === "\\") j += 2;
      else j += 1;
      if (src[j] === "'") {
        for (let k = i; k <= j; k++) if (src[k] !== "\n") out[k] = " ";
        i = j + 1;
        continue;
      }
    }
    i++;
  }
  return out.join("");
}

function lineOf(text, idx) {
  let line = 1;
  for (let i = 0; i < idx; i++) if (text[i] === "\n") line++;
  return line;
}

function matchBrace(text, openIdx) {
  let depth = 0;
  for (let i = openIdx; i < text.length; i++) {
    if (text[i] === "{") depth++;
    else if (text[i] === "}") {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function headerBetween(masked, fromIdx, braceIdx) {
  return masked
    .slice(fromIdx, braceIdx)
    .replace(/#\[[^\]]*\]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

const MOD_HEADER_RE = /^(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*$/;
const FN_HEADER_RE = /^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+[A-Za-z_][A-Za-z0-9_]*\s*\([\s\S]*\)(?:\s*->\s*[\s\S]*)?$/;

/** Line-range spans (1-indexed, inclusive) of every #[cfg(test)] mod block and
 * #[test]/#[tokio::test] fn body in a Rust source file. */
export function computeTestSpans(content) {
  const masked = maskStringsAndComments(content);
  const spans = [];

  const cfgTestRe = /#\[\s*cfg\(\s*test\s*\)\s*\]/g;
  let m;
  while ((m = cfgTestRe.exec(masked))) {
    const braceIdx = masked.indexOf("{", cfgTestRe.lastIndex);
    if (braceIdx === -1) continue;
    const header = headerBetween(masked, cfgTestRe.lastIndex, braceIdx);
    if (!MOD_HEADER_RE.test(header)) continue;
    const endIdx = matchBrace(masked, braceIdx);
    if (endIdx === -1) continue;
    spans.push([lineOf(masked, m.index), lineOf(masked, endIdx)]);
  }

  const testFnRe = /#\[\s*(?:tokio::)?test\b[^\]]*\]/g;
  while ((m = testFnRe.exec(masked))) {
    const braceIdx = masked.indexOf("{", testFnRe.lastIndex);
    if (braceIdx === -1) continue;
    const header = headerBetween(masked, testFnRe.lastIndex, braceIdx);
    if (!FN_HEADER_RE.test(header)) continue;
    const endIdx = matchBrace(masked, braceIdx);
    if (endIdx === -1) continue;
    spans.push([lineOf(masked, m.index), lineOf(masked, endIdx)]);
  }

  return spans;
}

function overlaps(a, b) {
  return a[0] <= b[1] && b[0] <= a[1];
}

// --- main ------------------------------------------------------------------

const isMain = process.argv[1] && path.resolve(process.argv[1]) === new URL(import.meta.url).pathname;
if (isMain) {
  const { base, head } = parseArgs(process.argv.slice(2));
  if (!base || !head) usage("both --base and --head are required");

  // Resolve the merge-base ONCE and use it for both diffs and old-content
  // lookups — the PR diff (base...head) is always mb-vs-head, but if `base`
  // has since moved (an advanced base branch), `git show base:path` reads the
  // WRONG revision unless it's pinned to the same mb (G14 fix #1).
  let mergeBase;
  try {
    mergeBase = git(["merge-base", base, head]).trim();
  } catch (err) {
    console.error(`tests-touched: git merge-base failed for ${base} ${head}: ${err.message}`);
    process.exit(2);
  }

  let nameStatusOut;
  try {
    nameStatusOut = git(["diff", "--name-status", "-M", `${mergeBase}..${head}`]);
  } catch (err) {
    console.error(`tests-touched: git diff failed for ${mergeBase}..${head}: ${err.message}`);
    process.exit(2);
  }

  const entries = parseNameStatus(nameStatusOut);

  let labels = [];
  const rawLabels = process.env.PR_LABELS ?? "[]";
  try {
    const parsed = JSON.parse(rawLabels);
    if (!Array.isArray(parsed)) throw new Error("PR_LABELS JSON value is not an array");
    labels = parsed.filter((l) => typeof l === "string");
  } catch (err) {
    console.error(`tests-touched: PR_LABELS is not a valid JSON array (${err.message}); treating labels as empty.`);
  }
  const exempt = labels.includes("tests:not-needed");

  const codeFiles = [];
  const rustCandidates = [];
  let hasEvidence = false;

  for (const e of entries) {
    const filePath = e.status === "R" || e.status === "C" ? e.newPath : e.path;
    const oldPath = e.status === "R" ? e.oldPath : filePath;

    // Test-evidence classification happens BEFORE the deletion/rename skip
    // below: a DELETED or renamed test file is still evidence a test changed
    // (G14 fix #2). Deletions of CODE files still never require evidence.
    if (isTestEvidenceFile(filePath)) {
      hasEvidence = true;
      continue;
    }

    if (e.status === "D") continue; // deletions of code files never require evidence
    if (e.status === "R" && e.score === 100) continue; // pure rename, no content change

    if (isRustCodeFile(filePath)) {
      codeFiles.push(filePath);
      rustCandidates.push({ path: filePath, oldPath });
      continue;
    }
    if (isFrontendCodeFile(filePath)) {
      codeFiles.push(filePath);
    }
  }

  if (!hasEvidence && rustCandidates.length > 0) {
    let hunkDiff;
    try {
      hunkDiff = git(["diff", "--no-color", "-U0", "-M", `${mergeBase}..${head}`]);
    } catch (err) {
      console.error(`tests-touched: git diff -U0 failed for ${mergeBase}..${head}: ${err.message}`);
      process.exit(2);
    }
    const hunkMap = parseHunksByNewPath(hunkDiff);
    for (const cand of rustCandidates) {
      const entry = hunkMap.get(cand.path);
      if (!entry) continue;
      let baseContent = null;
      let headContent = null;
      try {
        baseContent = git(["show", `${mergeBase}:${entry.oldPath}`]);
      } catch {
        baseContent = null;
      }
      try {
        headContent = git(["show", `${head}:${cand.path}`]);
      } catch {
        headContent = null;
      }
      const baseSpans = baseContent !== null ? computeTestSpans(baseContent) : [];
      const headSpans = headContent !== null ? computeTestSpans(headContent) : [];
      for (const h of entry.hunks) {
        if (h.oldLines > 0 && baseSpans.length > 0) {
          const delRange = [h.oldStart, h.oldStart + h.oldLines - 1];
          if (baseSpans.some((s) => overlaps(s, delRange))) {
            hasEvidence = true;
            break;
          }
        }
        if (h.newLines > 0 && headSpans.length > 0) {
          const addRange = [h.newStart, h.newStart + h.newLines - 1];
          if (headSpans.some((s) => overlaps(s, addRange))) {
            hasEvidence = true;
            break;
          }
        }
      }
      if (hasEvidence) break;
    }
  }

  if (codeFiles.length === 0) {
    console.log("✓ tests-touched: no code files changed — nothing requires test evidence.");
    process.exit(0);
  }
  if (hasEvidence) {
    console.log("✓ tests-touched: test change (or an inline test-span hunk) found alongside the code change.");
    process.exit(0);
  }
  if (exempt) {
    console.log("✓ tests-touched: no test evidence, but exempted by the `tests:not-needed` label.");
    console.log("  Code files changed:");
    for (const f of codeFiles) console.log(`    ${f}`);
    process.exit(0);
  }

  console.error("✖ tests-touched: code changed with no test change (or acknowledged exemption).");
  console.error("  Code files changed:");
  for (const f of codeFiles) console.error(`    ${f}`);
  console.error("  Add or modify a test (or, for Rust, edit inside a #[cfg(test)]/#[test] span in the same diff),");
  console.error("  or add the `tests:not-needed` label if this change genuinely needs none.");
  process.exit(1);
}
