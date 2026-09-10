#!/usr/bin/env node
// ts-export-reminder PostToolUse hook (S1, hard-gates closing wave): after an
// Edit/Write/MultiEdit touches a src-tauri/**/*.rs file, diff its
// `#[ts(export)]` item spans (scripts/check/ts-export-spans.mjs) against the
// last-known cache (.artifacts/ts-export-spans.json — the same cache `make
// types` refreshes as its last step) and remind — once per changed state —
// that the generated TS bindings may now be stale. A reminder, not proof:
// `check-local` skips the ts-rs drift guard (only `types-check`/`make check`
// run it).
//
// Seam: BRAWLER_REPO_ROOT overrides the repo root (tests only) — defaults to
// this hook's own repo. Fail-open: malformed JSON, an unreadable/nonexistent
// file, or anything outside src-tauri/**/*.rs exits silently (0, no output).
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { extractExportSpans } from "../../scripts/check/ts-export-spans.mjs";

const DEFAULT_REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

function repoRoot() {
  return process.env.BRAWLER_REPO_ROOT || DEFAULT_REPO_ROOT;
}

function cachePath(root) {
  return path.join(root, ".artifacts/ts-export-spans.json");
}

function readCache(p) {
  try {
    return JSON.parse(readFileSync(p, "utf8"));
  } catch {
    return {};
  }
}

function writeCache(p, cache) {
  mkdirSync(path.dirname(p), { recursive: true });
  const tmp = `${p}.tmp-${process.pid}`;
  writeFileSync(tmp, `${JSON.stringify(cache, null, 2)}\n`);
  renameSync(tmp, p);
}

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (c) => (input += c));
process.stdin.on("end", () => {
  try {
    run();
  } catch {
    process.exit(0); // fail-open
  }
});

function run() {
  let payload;
  try {
    payload = JSON.parse(input);
  } catch {
    process.exit(0);
  }
  if (!["Edit", "Write", "MultiEdit"].includes(payload?.tool_name)) process.exit(0);
  const filePath = payload?.tool_input?.file_path;
  if (typeof filePath !== "string" || filePath === "") process.exit(0);

  const root = repoRoot();
  const absFile = path.isAbsolute(filePath) ? filePath : path.resolve(root, filePath);
  const srcTauriRoot = `${path.join(root, "src-tauri")}${path.sep}`;
  if (!absFile.startsWith(srcTauriRoot) || !absFile.endsWith(".rs")) process.exit(0);
  if (!existsSync(absFile)) process.exit(0);

  let source;
  try {
    source = readFileSync(absFile, "utf8");
  } catch {
    process.exit(0);
  }

  const relFile = path.relative(root, absFile).split(path.sep).join("/");
  const cache = readCache(cachePath(root));
  const hadEntry = Object.prototype.hasOwnProperty.call(cache, relFile);
  const prevMap = hadEntry ? cache[relFile] : undefined;

  const spans = extractExportSpans(source);
  const currMap = Object.fromEntries(spans.map((s) => [s.name, s.hash]));

  const changed = new Set();
  for (const [name, hash] of Object.entries(currMap)) {
    if (!prevMap || !(name in prevMap) || prevMap[name] !== hash) changed.add(name);
  }
  if (prevMap) {
    for (const name of Object.keys(prevMap)) {
      if (!(name in currMap)) changed.add(name);
    }
  }

  const fire = hadEntry ? changed.size > 0 : spans.length > 0;

  if (spans.length > 0) cache[relFile] = currMap;
  else delete cache[relFile];
  writeCache(cachePath(root), cache);

  if (!fire) process.exit(0);

  const names = (hadEntry ? [...changed] : spans.map((s) => s.name)).sort().join(", ");
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PostToolUse",
        additionalContext:
          `${relFile}: exported ts-rs item(s) ${names} changed since the last \`make types\` — ` +
          "regenerate before handover (DoD §C; check-local skips the ts-rs drift guard). " +
          "This is a reminder, not proof of binding freshness.",
      },
    }),
  );
  process.exit(0);
}
