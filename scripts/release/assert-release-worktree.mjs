#!/usr/bin/env node
import { execFileSync } from "node:child_process";

const allowedFiles = new Set([
  "CHANGELOG.md",
  "docs/kanban-archive.md",
  "docs/kanban.md",
  "docs/roadmap.md",
  "package-lock.json",
  "package.json",
  "src-tauri/Cargo.lock",
  "src-tauri/Cargo.toml",
  "src-tauri/src/lib.rs",
  "src-tauri/tauri.conf.json",
]);

const status = execFileSync("git", ["status", "--porcelain"], {
  encoding: "utf8",
});

const unexpected = status
  .split("\n")
  .filter(Boolean)
  .map((line) => {
    const path = line.slice(3);
    const renamedPath = path.includes(" -> ") ? path.split(" -> ").at(-1) : path;
    return { line, path: renamedPath ?? path };
  })
  .filter(({ path }) => !allowedFiles.has(path));

if (unexpected.length > 0) {
  console.error("Release target found non-release working-tree changes:");
  for (const { line } of unexpected) {
    console.error(`  ${line}`);
  }
  console.error("\nCommit, stash, or remove unrelated changes before running make release.");
  process.exit(1);
}
