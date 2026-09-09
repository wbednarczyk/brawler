// Guard (S1, hard-gates closing wave): ts-export-reminder must fire exactly
// once per changed export-span state (added/changed/removed item) and stay
// silent otherwise, including on the very first sighting of a file with no
// exported items. Drives the hook exactly as Claude Code's PostToolUse
// machinery does: JSON on stdin, read hookSpecificOutput.additionalContext.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { extractExportSpans } from "./ts-export-spans.mjs";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const HOOK_PATH = path.join(REPO_ROOT, ".claude/hooks/ts-export-reminder.sh");

function makeFakeRepo() {
  const root = mkdtempSync(path.join(tmpdir(), "ts-export-reminder-"));
  mkdirSync(path.join(root, "src-tauri/src"), { recursive: true });
  return root;
}

function runHook(root, toolName, relFilePath) {
  const filePath = path.join(root, relFilePath);
  const input = JSON.stringify({ tool_name: toolName, tool_input: { file_path: filePath } });
  const result = spawnSync("bash", [HOOK_PATH], {
    input,
    encoding: "utf8",
    env: { ...process.env, BRAWLER_REPO_ROOT: root },
  });
  const stdout = result.stdout.trim();
  if (!stdout) return null;
  return JSON.parse(stdout).hookSpecificOutput?.additionalContext ?? null;
}

const EXPORTING_RS = `use ts_rs::TS;

#[derive(TS)]
#[ts(export)]
pub struct Foo {
    pub a: i32,
}
`;

test("non-Rust path stays silent", () => {
  const root = makeFakeRepo();
  writeFileSync(path.join(root, "src-tauri/src/main.rs.txt"), EXPORTING_RS);
  assert.equal(runHook(root, "Edit", "src-tauri/src/main.rs.txt"), null);
  rmSync(root, { recursive: true, force: true });
});

test("path outside src-tauri stays silent", () => {
  const root = makeFakeRepo();
  mkdirSync(path.join(root, "src"), { recursive: true });
  writeFileSync(path.join(root, "src/main.rs"), EXPORTING_RS);
  assert.equal(runHook(root, "Edit", "src/main.rs"), null);
  rmSync(root, { recursive: true, force: true });
});

test("Rust file with no ts(export stays silent", () => {
  const root = makeFakeRepo();
  writeFileSync(path.join(root, "src-tauri/src/plain.rs"), "pub struct Plain { pub a: i32 }\n");
  assert.equal(runHook(root, "Edit", "src-tauri/src/plain.rs"), null);
  rmSync(root, { recursive: true, force: true });
});

test("exporting file: first sighting fires once and seeds; identical second run stays silent", () => {
  const root = makeFakeRepo();
  const rel = "src-tauri/src/foo.rs";
  writeFileSync(path.join(root, rel), EXPORTING_RS);
  const first = runHook(root, "Edit", rel);
  assert.match(first, /foo\.rs.*Foo/s);
  const second = runHook(root, "Edit", rel);
  assert.equal(second, null);
  rmSync(root, { recursive: true, force: true });
});

test("edit outside any span (private fn changed) stays silent after seeding", () => {
  const root = makeFakeRepo();
  const rel = "src-tauri/src/foo2.rs";
  writeFileSync(path.join(root, rel), EXPORTING_RS);
  runHook(root, "Edit", rel); // seed
  writeFileSync(path.join(root, rel), `${EXPORTING_RS}\nfn helper() -> i32 {\n    1\n}\n`);
  assert.equal(runHook(root, "Edit", rel), null);
  rmSync(root, { recursive: true, force: true });
});

test("enum variant added fires naming the enum", () => {
  const root = makeFakeRepo();
  const rel = "src-tauri/src/bar.rs";
  const before = "#[derive(TS)]\n#[ts(export)]\npub enum Bar {\n    A,\n}\n";
  const after = "#[derive(TS)]\n#[ts(export)]\npub enum Bar {\n    A,\n    B,\n}\n";
  writeFileSync(path.join(root, rel), before);
  runHook(root, "Edit", rel); // seed
  writeFileSync(path.join(root, rel), after);
  const msg = runHook(root, "Edit", rel);
  assert.match(msg, /Bar/);
  rmSync(root, { recursive: true, force: true });
});

test("item removed fires", () => {
  const root = makeFakeRepo();
  const rel = "src-tauri/src/baz.rs";
  writeFileSync(path.join(root, rel), EXPORTING_RS);
  runHook(root, "Edit", rel); // seed
  writeFileSync(path.join(root, rel), "pub struct Plain { pub a: i32 }\n");
  const msg = runHook(root, "Edit", rel);
  assert.match(msg, /Foo/);
  rmSync(root, { recursive: true, force: true });
});

test("Write tool_name fires the same way", () => {
  const root = makeFakeRepo();
  const rel = "src-tauri/src/w.rs";
  writeFileSync(path.join(root, rel), EXPORTING_RS);
  assert.match(runHook(root, "Write", rel), /Foo/);
  rmSync(root, { recursive: true, force: true });
});

test("MultiEdit tool_name fires the same way", () => {
  const root = makeFakeRepo();
  const rel = "src-tauri/src/m.rs";
  writeFileSync(path.join(root, rel), EXPORTING_RS);
  assert.match(runHook(root, "MultiEdit", rel), /Foo/);
  rmSync(root, { recursive: true, force: true });
});

test("missing cache with spans fires once, then silent", () => {
  const root = makeFakeRepo();
  const rel = "src-tauri/src/n.rs";
  writeFileSync(path.join(root, rel), EXPORTING_RS);
  const first = runHook(root, "Edit", rel);
  assert.notEqual(first, null);
  const second = runHook(root, "Edit", rel);
  assert.equal(second, null);
  rmSync(root, { recursive: true, force: true });
});

test("extractExportSpans: two exported structs + one exported enum + one non-exported struct + rename_all form", () => {
  const src = `
#[derive(Debug, Serialize)]
#[ts(export)]
pub struct Alpha {
    pub a: i32,
}

pub struct NotExported {
    pub b: i32,
}

#[derive(TS)]
#[ts(export, rename_all = "camelCase")]
pub struct Beta {
    pub my_field: String,
}

#[derive(TS)]
#[ts(export)]
pub enum Gamma {
    One,
    Two(i32),
}
`;
  const spans = extractExportSpans(src);
  const names = spans.map((s) => s.name).sort();
  assert.deepEqual(names, ["Alpha", "Beta", "Gamma"]);
  for (const s of spans) {
    assert.equal(typeof s.hash, "string");
    assert.equal(s.hash.length, 64);
  }
});

test("extractExportSpans: the codebase's real cfg_attr(feature = \"ts-export\", ts(export, ...)) form, split across lines", () => {
  const src = `use super::*;

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub applied_migrations: i64,
}

pub struct NotExported {
    pub a: i64,
}
`;
  const spans = extractExportSpans(src);
  assert.deepEqual(
    spans.map((s) => s.name),
    ["DatabaseStatus"],
  );
});
