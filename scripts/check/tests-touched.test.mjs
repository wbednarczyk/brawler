// Tests-touched gate (G14, hard gates wave 2): a PR's code diff must carry a
// test change (or, for Rust, an inline #[cfg(test)]/#[test] hunk) or the
// `tests:not-needed` label. Scenarios spawn the script against throwaway git
// repos (spawnSync git with `-c user.name`/`-c user.email` so the run never
// depends on the machine's global git config).
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const SCRIPT_PATH = path.join(REPO_ROOT, "scripts/check/tests-touched.mjs");
const GIT_IDENTITY = ["-c", "user.name=Test", "-c", "user.email=test@example.com"];

function git(dir, args) {
  const result = spawnSync("git", [...GIT_IDENTITY, ...args], { cwd: dir, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${result.stderr}`);
  }
  return result.stdout;
}

function makeRepo() {
  const dir = mkdtempSync(path.join(tmpdir(), "tests-touched-"));
  git(dir, ["init", "-q"]);
  return dir;
}

function writeFile(dir, rel, content) {
  const full = path.join(dir, rel);
  mkdirSync(path.dirname(full), { recursive: true });
  writeFileSync(full, content);
}

function commit(dir, message) {
  git(dir, ["add", "-A"]);
  git(dir, ["commit", "-q", "-m", message]);
  return git(dir, ["rev-parse", "HEAD"]).trim();
}

function runScript(dir, base, head, labels = "") {
  return spawnSync("node", [SCRIPT_PATH, "--base", base, "--head", head], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, PR_LABELS: labels },
  });
}

function cleanup(dir) {
  rmSync(dir, { recursive: true, force: true });
}

test("a production TS file changed with no test evidence fails", () => {
  const dir = makeRepo();
  try {
    writeFile(dir, "src/app/Foo.ts", "export const x = 1;\n");
    const base = commit(dir, "init");
    writeFile(dir, "src/app/Foo.ts", "export const x = 2;\n");
    const head = commit(dir, "change");

    const r = runScript(dir, base, head);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /tests:not-needed/);
    assert.match(r.stderr, /src\/app\/Foo\.ts/);
  } finally {
    cleanup(dir);
  }
});

const RUST_BASE = `pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds() {
        assert_eq!(add(1, 1), 2);
    }
}
`;

test("a Rust production edit accompanied by an inline #[cfg(test)] hunk passes", () => {
  const dir = makeRepo();
  try {
    writeFile(dir, "src-tauri/src/math.rs", RUST_BASE);
    const base = commit(dir, "init");
    const head_content = RUST_BASE.replace("a + b\n", "a + b + 0\n").replace(
      "assert_eq!(add(1, 1), 2);\n",
      "assert_eq!(add(1, 1), 2);\n        assert_eq!(add(2, 2), 4);\n",
    );
    writeFile(dir, "src-tauri/src/math.rs", head_content);
    const head = commit(dir, "change both");

    const r = runScript(dir, base, head);
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /test change \(or an inline test-span hunk\)/);
  } finally {
    cleanup(dir);
  }
});

test("a production edit AFTER a #[cfg(test)] mod block (outside its span) fails", () => {
  const dir = makeRepo();
  try {
    const base_content = `#[cfg(test)]
mod tests {
    #[test]
    fn ok() {
        assert!(true);
    }
}

pub fn helper() -> i32 {
    1
}
`;
    writeFile(dir, "src-tauri/src/helper.rs", base_content);
    const base = commit(dir, "init");
    const head_content = base_content.replace("    1\n", "    2\n");
    writeFile(dir, "src-tauri/src/helper.rs", head_content);
    const head = commit(dir, "change helper only");

    const r = runScript(dir, base, head);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /src-tauri\/src\/helper\.rs/);
  } finally {
    cleanup(dir);
  }
});

test("deleting a test file with no production change passes (nothing requires evidence)", () => {
  const dir = makeRepo();
  try {
    writeFile(dir, "src/app/Foo.ts", "export const x = 1;\n");
    writeFile(dir, "src/app/Foo.test.ts", "test('x', () => {});\n");
    const base = commit(dir, "init");
    git(dir, ["rm", "-q", "src/app/Foo.test.ts"]);
    const head = commit(dir, "drop stale test");

    const r = runScript(dir, base, head);
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /no code files changed/);
  } finally {
    cleanup(dir);
  }
});

test("the tests:not-needed label exempts a code-only change", () => {
  const dir = makeRepo();
  try {
    writeFile(dir, "src/app/Foo.ts", "export const x = 1;\n");
    const base = commit(dir, "init");
    writeFile(dir, "src/app/Foo.ts", "export const x = 2;\n");
    const head = commit(dir, "change");

    const r = runScript(dir, base, head, "release:patch, tests:not-needed");
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /tests:not-needed/);
  } finally {
    cleanup(dir);
  }
});

test("docs-only, locale-only, generated-only and pure-rename changes all pass", () => {
  const dir = makeRepo();
  try {
    writeFile(dir, "docs/foo.md", "# Foo\n");
    writeFile(dir, "src/shared/locale/resources/pl.ts", "export const pl = {};\n");
    writeFile(dir, "src/api/generated/Foo.ts", "export type Foo = {};\n");
    writeFile(dir, "src/app/Renameable.ts", "export const same = 1;\n");
    const base = commit(dir, "init");

    writeFile(dir, "docs/foo.md", "# Foo\n\nMore.\n");
    writeFile(dir, "src/shared/locale/resources/pl.ts", "export const pl = { a: 1 };\n");
    writeFile(dir, "src/api/generated/Foo.ts", "export type Foo = { a: number };\n");
    git(dir, ["mv", "src/app/Renameable.ts", "src/app/Renamed.ts"]);
    const head = commit(dir, "docs/locale/generated/rename");

    const r = runScript(dir, base, head);
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /no code files changed/);
  } finally {
    cleanup(dir);
  }
});
