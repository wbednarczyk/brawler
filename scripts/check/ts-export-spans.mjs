#!/usr/bin/env node
// ts-export span extractor (S1, hard-gates closing wave): text-level scan for
// every Rust item carrying `#[ts(export)]` (possibly among other attributes,
// e.g. `#[derive(...)]` `#[ts(export, rename_all = "camelCase")]`). Powers the
// ts-export-reminder PostToolUse hook — a changed/added/removed span means the
// generated TS bindings (`make types`) may be stale.
//
// ponytail ceilings (text-level, no Rust parser): a string literal containing
// `{`/`}`/`[`/`]`/`;` inside an item body or attribute will miscount depth.
// An item counts as exported when the literal substring `ts(export` appears
// ANYWHERE inside a bracket-balanced attribute run starting at that item —
// this covers both the bare `#[ts(export)]` form and this codebase's
// dominant `#[cfg_attr(feature = "ts-export", ts(export, ...))]` form, incl.
// split across lines. A differently-spelled or aliased ts-rs export
// mechanism is invisible to this scan. Raise if a real file needs more.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const OUT_PATH = path.join(REPO_ROOT, ".artifacts/ts-export-spans.json");

function normalize(text) {
  return text
    .split("\n")
    .map((l) => l.replace(/[ \t]+$/, ""))
    .join("\n");
}

function hashSpan(text) {
  return createHash("sha256").update(normalize(text)).digest("hex");
}

function extractName(declText) {
  let m = declText.match(/\b(?:struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)/);
  if (m) return m[1];
  m = declText.match(/\btype\s+([A-Za-z_][A-Za-z0-9_]*)/);
  return m ? m[1] : null;
}

/** From the item-declaration start line, find where the item ends: brace-matched `}` for a
 * struct/enum body, or the first `;` for a tuple/unit struct or type alias. Returns the 0-based
 * end line index and the item's name (null if no struct/enum/type name precedes the end). */
function findItemEnd(lines, startIdx) {
  let bodyStarted = false;
  let depth = 0;
  let declText = "";
  for (let li = startIdx; li < lines.length; li++) {
    const line = lines[li];
    for (let ci = 0; ci < line.length; ci++) {
      const ch = line[ci];
      if (!bodyStarted) {
        if (ch === "{") {
          bodyStarted = true;
          depth = 1;
        } else if (ch === ";") {
          return { endLine: li, name: extractName(declText) };
        } else {
          declText += ch;
        }
      } else if (ch === "{") {
        depth++;
      } else if (ch === "}") {
        depth--;
        if (depth === 0) return { endLine: li, name: extractName(declText) };
      }
    }
    if (!bodyStarted) declText += "\n";
  }
  return { endLine: lines.length - 1, name: extractName(declText) };
}

/** Every `#[ts(export…)]`-carrying item's {name, hash} in `rustSource`, in declaration order. */
export function extractExportSpans(rustSource) {
  const lines = rustSource.split("\n");
  const spans = [];
  let i = 0;
  while (i < lines.length) {
    if (/^\s*#\[/.test(lines[i])) {
      const attrStart = i;
      let hasTsExport = false;
      let j = i;
      // Consume one or more attributes making up this run; each attribute is
      // bracket-balanced ([...]) and may itself span several lines (a split
      // `#[cfg_attr(\n  feature = "ts-export",\n  ts(export, ...)\n)]`).
      while (j < lines.length && /^\s*#\[/.test(lines[j])) {
        let depth = 0;
        do {
          const line = lines[j];
          if (line.includes("ts(export")) hasTsExport = true;
          for (const ch of line) {
            if (ch === "[") depth++;
            else if (ch === "]") depth--;
          }
          j++;
        } while (depth > 0 && j < lines.length);
      }
      if (hasTsExport && j < lines.length) {
        const { endLine, name } = findItemEnd(lines, j);
        if (name) {
          spans.push({ name, hash: hashSpan(lines.slice(attrStart, endLine + 1).join("\n")) });
        }
        i = endLine + 1;
        continue;
      }
      i = j;
      continue;
    }
    i++;
  }
  return spans;
}

/** repo-relative-path -> {ItemName: hash} for every `.rs` file under src-tauri/src carrying at
 * least one export span. */
export function scanRepo(repoRoot = REPO_ROOT) {
  const scanRoot = path.join(repoRoot, "src-tauri/src");
  const out = {};
  if (!existsSync(scanRoot)) return out;
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const abs = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(abs);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        const spans = extractExportSpans(readFileSync(abs, "utf8"));
        if (spans.length > 0) {
          const rel = path.relative(repoRoot, abs).split(path.sep).join("/");
          out[rel] = Object.fromEntries(spans.map((s) => [s.name, s.hash]));
        }
      }
    }
  };
  walk(scanRoot);
  return out;
}

function atomicWriteJson(outPath, data) {
  mkdirSync(path.dirname(outPath), { recursive: true });
  const tmp = `${outPath}.tmp-${process.pid}`;
  writeFileSync(tmp, `${JSON.stringify(data, null, 2)}\n`);
  renameSync(tmp, outPath);
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  if (process.argv.includes("--write")) {
    const map = scanRepo();
    atomicWriteJson(OUT_PATH, map);
    console.log(`ts-export-spans: wrote ${OUT_PATH} (${Object.keys(map).length} file(s) with exported items).`);
  } else {
    console.error("Usage: node scripts/check/ts-export-spans.mjs --write");
    process.exit(64);
  }
}
